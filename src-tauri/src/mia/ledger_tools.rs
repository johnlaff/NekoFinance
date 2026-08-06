//! As três perguntas de recorte próprio: quais lançamentos, quais tags e o que já está
//! comprometido à frente.
//!
//! É a fatia que tira a conversa do resumo e a leva ao lançamento. O agregado sempre cobre o
//! filtro inteiro, nunca a página: uma resposta paginada que somasse só a página mentiria — e
//! mentiria bem, porque a soma pareceria certa.

use super::envelope::{
    Cursor, Delta, ErrorCode, Listing, Page, Period, ToolError, ToolOutput, ToolResult,
};
use super::time_tools::month_period;
use super::{Args, insert};
use crate::calendar;
use crate::commands::{RecentRow, TransactionRow, hydrate_transactions};
use crate::forecast::{self, EventKind};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

// --- O recorte de lançamentos -----------------------------------------------------------

/// Vocabulário do argumento `sort`. A ordenação é do consumidor, então ela precisa ser dita —
/// mas só nestas quatro palavras, e cada uma vira um `ORDER BY` que já existe no domínio.
const SORT: &[&str] = &["date_desc", "date_asc", "amount_desc", "amount_asc"];
const PAYMENT_METHODS: &[&str] = &["debit", "credit", "pix", "cash"];
const NATURES: &[&str] = &["fixed", "variable"];

#[derive(Serialize)]
struct TransactionDto {
    id: String,
    date: String,
    description: String,
    amount_cents: i64,
    /// `income` · `expense` · `transfer` — a forma da linha no banco.
    r#type: String,
    /// O tipo do MÉTODO, classificado pelo motor: entrada · saída · diário · cartão · economia ·
    /// patrimônio. Nulo quando o movimento é net-zero para o método (transferência entre contas
    /// líquidas), que o motor legitimamente ignora.
    movement: Option<&'static str>,
    payment_method: String,
    is_fixed: bool,
    /// `projetado` · `importado` · `manual`.
    provenance: String,
    /// Parcela n de N, quando a linha pertence a uma série com fim.
    #[serde(skip_serializing_if = "Option::is_none")]
    installment: Option<Installment>,
    /// Há dinheiro que volta ligado a esta linha.
    has_refund_link: bool,
    /// Tags com identidade: é por ela que a próxima pergunta filtra.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<NamedDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<ItemDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owners: Option<Vec<OwnerShareDto>>,
}

#[derive(Serialize, Clone, Copy)]
struct Installment {
    index: i64,
    total: i64,
}

#[derive(Serialize)]
struct NamedDto {
    id: String,
    name: String,
}

/// Quem respondeu por quanto de uma linha. O id vem junto porque filtrar por pessoa exige o id,
/// e nenhuma outra ferramenta o entrega.
#[derive(Serialize)]
struct OwnerShareDto {
    person_id: String,
    name: String,
    amount_cents: i64,
}

#[derive(Serialize)]
struct ItemDto {
    description: String,
    amount_cents: i64,
    /// O balde do método a que o item pertence, derivado da seção da nota.
    kind: String,
}

/// A linha como o filtro a vê antes de ser vestida: o suficiente para somar, ordenar e paginar
/// sem carregar tags, itens e titulares de um recorte inteiro.
#[derive(sqlx::FromRow)]
struct KeyRow {
    id: String,
    r#type: String,
    amount: i64,
    is_fixed: i64,
    payment_method: Option<String>,
    /// Classe de liquidez da conta-destino — o que separa Economia de Patrimônio numa
    /// transferência, e o que faz uma transferência entre líquidas não contar em régua nenhuma.
    to_liquidity: Option<String>,
}

pub(crate) async fn search_transactions(
    pool: &SqlitePool,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let (start, end) = match args.range("range")? {
        Some(range) => range,
        None => current_month(today),
    };
    let sort = args.choice("sort", SORT)?.unwrap_or("date_desc");
    let min_cents = args.cents("min_cents")?;
    let max_cents = args.cents("max_cents")?;
    let account_id = args.text("account_id")?;
    let tag_id = args.text("tag_id")?;
    let owner_person_id = args.text("owner_person_id")?;
    let payment_method = args.choice("payment_method", PAYMENT_METHODS)?;
    let nature = args.choice("nature", NATURES)?;

    // Fragmento constante escolhido por vocabulário fechado; nenhum dado da chamada entra por
    // formatação — os filtros viajam por placeholder, como em qualquer outra leitura do repo.
    let order = match sort {
        "date_asc" => "t.date ASC, t.id ASC",
        "amount_desc" => "ABS(t.amount) DESC, t.id ASC",
        "amount_asc" => "ABS(t.amount) ASC, t.id ASC",
        _ => "t.date DESC, t.id ASC",
    };
    let filter = "FROM \"transaction\" t \
                  LEFT JOIN account dest ON dest.id = t.to_account_id \
         WHERE t.scenario_id IS NULL \
           AND t.date >= ?1 AND t.date <= ?2 \
           AND (?3 IS NULL OR ABS(t.amount) >= ?3) \
           AND (?4 IS NULL OR ABS(t.amount) <= ?4) \
           AND (?5 IS NULL OR t.from_account_id = ?5 OR t.to_account_id = ?5 \
                OR EXISTS (SELECT 1 FROM invoice i \
                            WHERE i.id = t.invoice_id AND i.account_id = ?5)) \
           AND (?6 IS NULL OR EXISTS (SELECT 1 FROM transaction_tag tt \
                                       WHERE tt.transaction_id = t.id AND tt.tag_id = ?6)) \
           AND (?7 IS NULL OR EXISTS (SELECT 1 FROM split s \
                                       WHERE s.transaction_id = t.id \
                                         AND s.owner_person_id = ?7)) \
           AND (?8 IS NULL OR t.payment_method = ?8) \
           AND (?9 IS NULL OR t.is_fixed = ?9)";
    let sql = format!(
        "SELECT t.id, t.type, t.amount, t.is_fixed, t.payment_method, \
                dest.liquidity AS to_liquidity {filter} ORDER BY {order}"
    );
    let keys: Vec<KeyRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(start.to_string())
        .bind(end.to_string())
        .bind(min_cents)
        .bind(max_cents)
        .bind(account_id)
        .bind(tag_id)
        .bind(owner_person_id)
        .bind(payment_method)
        .bind(nature.map(|n| i64::from(n == "fixed")))
        .fetch_all(pool)
        .await
        .map_err(|e| ToolError::read_failed(format!("recorte de lançamentos: {e}")))?;

    // Os totais somam o FILTRO inteiro, antes de qualquer corte de página. A classificação de
    // cada linha é feita uma vez e serve às duas leituras: o total por tipo e a linha da página.
    let movements: std::collections::HashMap<String, Option<&'static str>> = keys
        .iter()
        .map(|k| (k.id.clone(), movement_of(k)))
        .collect();
    let totals = totals_of(&keys, &movements);

    // O cursor é ancorado no recorte E na ordem: o mesmo filtro lido de outro jeito é outra
    // lista, e continuar nela pela posição antiga pularia ou repetiria linha.
    let scope = format!(
        "search;{start}..{end};{sort};{};{};{};{};{};{};{}",
        text_of(min_cents),
        text_of(max_cents),
        account_id.unwrap_or(""),
        tag_id.unwrap_or(""),
        owner_person_id.unwrap_or(""),
        payment_method.unwrap_or(""),
        nature.unwrap_or("")
    );
    let ids: Vec<String> = keys.into_iter().map(|k| k.id).collect();
    let offset = match args.text("cursor")? {
        None => 0,
        Some(raw) => {
            let from = Cursor::decode(raw, &scope)?;
            ids.iter()
                .position(|id| *id == from)
                .ok_or_else(Cursor::refused)?
        }
    };
    let page = Page::from(ids, offset, &scope, Clone::clone);

    let rows = read_lines(pool, &page.items, order).await?;
    let mut shares = if args.wants("owners") {
        owner_shares(pool, &page.items).await?
    } else {
        std::collections::HashMap::new()
    };
    let items: Vec<TransactionDto> = rows
        .into_iter()
        .map(|row| {
            let movement = movements.get(&row.id).copied().flatten();
            let owners = shares.remove(&row.id).unwrap_or_default();
            line_dto(row, movement, owners, args)
        })
        .collect();

    Ok(ToolOutput {
        period: Period::between(start, end),
        data: json!({
            "transactions": page.with_items(items),
            "totals": totals,
        }),
    })
}

/// Lê as linhas da página pelos ids escolhidos, na mesma ordem em que o recorte as ordenou.
async fn read_lines(
    pool: &SqlitePool,
    ids: &[String],
    order: &str,
) -> Result<Vec<TransactionRow>, ToolError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let columns = crate::commands::RECENT_ROW_COLUMNS;
    let sql = format!(
        "SELECT {columns} FROM \"transaction\" t WHERE t.id IN ({placeholders}) ORDER BY {order}"
    );
    let mut query = sqlx::query_as::<_, RecentRow>(sqlx::AssertSqlSafe(sql));
    for id in ids {
        query = query.bind(id);
    }
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| ToolError::read_failed(format!("linhas do recorte: {e}")))?;
    // A hidratação é a mesma do Livro-razão: tags, itens da nota, titulares e a posição na série
    // saem de uma leitura em lote só, e a conversa nunca mostra uma linha diferente da da tela.
    hydrate_transactions(pool, rows)
        .await
        .map_err(ToolError::read_failed)
}

/// Quem respondeu por quanto, por lançamento da página — uma leitura em lote, com o id da
/// pessoa que a linha do Livro-razão (só nomes) não carrega.
async fn owner_shares(
    pool: &SqlitePool,
    ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<OwnerShareDto>>, ToolError> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT s.transaction_id, s.owner_person_id, p.name, s.amount \
         FROM split s JOIN person p ON p.id = s.owner_person_id \
         WHERE s.transaction_id IN ({placeholders}) ORDER BY p.name COLLATE NOCASE"
    );
    let mut query = sqlx::query_as::<_, (String, String, String, i64)>(sqlx::AssertSqlSafe(sql));
    for id in ids {
        query = query.bind(id);
    }
    let mut by_transaction: std::collections::HashMap<String, Vec<OwnerShareDto>> =
        std::collections::HashMap::new();
    for (transaction_id, person_id, name, amount_cents) in query
        .fetch_all(pool)
        .await
        .map_err(|e| ToolError::read_failed(format!("titulares do recorte: {e}")))?
    {
        by_transaction
            .entry(transaction_id)
            .or_default()
            .push(OwnerShareDto {
                person_id,
                name,
                amount_cents,
            });
    }
    Ok(by_transaction)
}

fn line_dto(
    row: TransactionRow,
    movement: Option<&'static str>,
    owners: Vec<OwnerShareDto>,
    args: &Args,
) -> TransactionDto {
    TransactionDto {
        installment: match (row.installment_index, row.installment_total) {
            (Some(index), Some(total)) => Some(Installment { index, total }),
            _ => None,
        },
        tags: args.wants("tags").then(|| {
            row.tags
                .iter()
                .map(|t| NamedDto {
                    id: t.id.clone(),
                    name: t.name.clone(),
                })
                .collect()
        }),
        items: args.wants("items").then(|| {
            row.line_items
                .iter()
                .map(|i| ItemDto {
                    description: i.description.clone(),
                    amount_cents: i.amount_cents,
                    kind: i.kind.clone(),
                })
                .collect()
        }),
        owners: args.wants("owners").then_some(owners),
        id: row.id,
        date: row.date,
        description: row.description,
        amount_cents: row.amount,
        r#type: row.r#type,
        movement,
        payment_method: row.payment_method,
        is_fixed: row.is_fixed,
        provenance: row.provenance,
        has_refund_link: row.has_refund_link,
    }
}

/// O que o recorte movimentou, por tipo de linha e por tipo do método, numa passada só.
///
/// `by_movement` é a soma das linhas FILTRADAS — não os baldes do mês, que o motor compõe com as
/// faturas materializadas e as máscaras de tag. Quem pergunta o custo de vida do mês tem
/// get_month_analysis; aqui a soma responde pelo filtro que a pergunta desenhou.
fn totals_of(
    keys: &[KeyRow],
    movements: &std::collections::HashMap<String, Option<&'static str>>,
) -> Value {
    let (mut income, mut expense, mut transfer) = (0i64, 0i64, 0i64);
    let mut by_movement: std::collections::HashMap<&'static str, i64> =
        MOVEMENTS.iter().map(|name| (*name, 0)).collect();
    for key in keys {
        let cents = key.amount.abs();
        match key.r#type.as_str() {
            "income" => income += cents,
            "expense" => expense += cents,
            "transfer" => transfer += cents,
            _ => {}
        }
        if let Some(Some(movement)) = movements.get(&key.id) {
            *by_movement.entry(movement).or_default() += cents;
        }
    }
    let mut totals = json!({
        "count": keys.len(),
        "income_cents": income,
        "expense_cents": expense,
        "transfer_cents": transfer,
    });
    let mut movement_totals = json!({});
    for name in MOVEMENTS {
        insert(
            &mut movement_totals,
            name,
            by_movement.get(name).copied().unwrap_or_default(),
        );
    }
    insert(&mut totals, "by_movement", movement_totals);
    totals
}

pub(super) const MOVEMENTS: &[&str] = &[
    "entrada",
    "saida",
    "diario",
    "cartao",
    "economia",
    "patrimonio",
];

/// O tipo do método de uma linha, pela regra do motor — nunca por uma segunda classificação
/// escrita aqui, que divergiria da que o motor aplica nas telas.
fn movement_of(key: &KeyRow) -> Option<&'static str> {
    forecast::classify(
        &key.r#type,
        key.is_fixed != 0,
        key.payment_method.as_deref(),
        key.to_liquidity.as_deref(),
    )
    .map(movement_name)
}

fn movement_name(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Income => "entrada",
        EventKind::FixedOut => "saida",
        EventKind::Daily => "diario",
        EventKind::Cartao => "cartao",
        EventKind::Economia => "economia",
        EventKind::Patrimonio => "patrimonio",
    }
}

// --- As tags como interruptores de régua -------------------------------------------------

#[derive(Serialize)]
struct TagDto {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    emoji: Option<String>,
    /// Em quais réguas a tag CONTA. É o vocabulário da tag: interruptor de contabilidade, não
    /// envelope de orçamento — o método não orça por categoria.
    counts_in: Value,
    month_total_cents: i64,
    transaction_count: i64,
}

#[derive(Serialize)]
struct TagEffectDto {
    tag_id: String,
    name: String,
    performance_delta_cents: i64,
    cost_delta_cents: i64,
    savings_base_delta_cents: i64,
    savings_amount_delta_cents: i64,
    daily_avg_delta_cents: i64,
}

#[derive(Serialize)]
struct ThirdPartyDto {
    person_id: String,
    name: String,
    out_cents: i64,
    back_cents: i64,
    expected_cents: i64,
    /// `favor` · `open` · `series` · `settled` · `none`.
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_since_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installment: Option<Installment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settled_date: Option<String>,
}

pub(crate) async fn tags(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let (year, month) = args
        .month("month")?
        .unwrap_or((today.year(), today.month()));
    // A tela das tags é a régua: os efeitos por interruptor são reexecuções do motor sobre os
    // eventos do mês, e recomputá-los aqui abriria a divergência que a régua única fecha. O
    // default enxuto vale para o que SAI no fio — a leitura é a mesma que a tela paga.
    let screen = crate::tags_screen::tags_screen_dto(pool, today, year, month)
        .await
        .map_err(ToolError::read_failed)?;

    let tags: Vec<TagDto> = screen
        .tags
        .iter()
        .map(|t| TagDto {
            id: t.id.clone(),
            name: t.name.clone(),
            emoji: t.emoji.clone(),
            counts_in: json!({
                "performance": t.counts_in.performance,
                "cost_of_living": t.counts_in.cost_of_living,
                "savings": t.counts_in.savings,
                "daily_avg": t.counts_in.daily_avg,
            }),
            month_total_cents: t.month_total_cents,
            transaction_count: t.txn_count,
        })
        .collect();

    let mut data = json!({
        "month": screen.month,
        "cost_of_living_cents": screen.verdict.cost_current_cents,
        // O custo se TODAS as tags contassem: o preço das exceções, já subtraído de ninguém.
        "cost_if_every_tag_counted_cents": screen.verdict.cost_all_on_cents,
        "has_exceptions": screen.verdict.has_exceptions,
        "third_party_monthly_avg_cents": screen.verdict.third_party_avg_cents,
        "third_party_people": screen.verdict.third_party_people,
        "tags": Listing::capped(tags),
    });

    if args.wants("effects") {
        let effects: Vec<TagEffectDto> = screen
            .tags
            .iter()
            .map(|t| TagEffectDto {
                tag_id: t.id.clone(),
                name: t.name.clone(),
                performance_delta_cents: t.effects.performance_delta_cents,
                cost_delta_cents: t.effects.cost_delta_cents,
                savings_base_delta_cents: t.effects.savings_base_delta_cents,
                savings_amount_delta_cents: t.effects.savings_amount_delta_cents,
                daily_avg_delta_cents: t.effects.daily_avg_delta_cents,
            })
            .collect();
        insert(&mut data, "effects", Listing::capped(effects));
    }

    if args.wants("third_parties") {
        let lines: Vec<ThirdPartyDto> = screen
            .third_parties
            .iter()
            .map(|l| ThirdPartyDto {
                person_id: l.person_id.clone(),
                name: l.name.clone(),
                out_cents: l.out_cents,
                back_cents: l.back_cents,
                expected_cents: l.expected_cents,
                state: l.state.clone(),
                open_since_days: l.open_since_days,
                installment: match (l.series_done, l.series_total) {
                    (Some(index), Some(total)) => Some(Installment {
                        index: i64::from(index),
                        total: i64::from(total),
                    }),
                    _ => None,
                },
                settled_date: l.settled_date.clone(),
            })
            .collect();
        insert(&mut data, "third_parties", Listing::capped(lines));
    }

    Ok(ToolOutput {
        period: month_period(year, month)?,
        data,
    })
}

// --- O que já está comprometido ----------------------------------------------------------

/// Quantos meses à frente a pergunta alcança quando ninguém pede um recorte. Compromisso olha
/// para a frente: doze meses é o horizonte em que uma parcela ainda muda uma decisão de hoje.
const COMMITMENT_MONTHS: u32 = 12;

#[derive(Serialize)]
struct CardSeriesDto {
    id: String,
    card_name: String,
    description: String,
    /// `installment` (parcelamento, com fim) · `subscription` (assinatura, sem fim).
    kind: &'static str,
    amount_cents: i64,
    /// Total de parcelas. Nulo na assinatura — dizer "5 de 5" mentiria sobre o que vem depois.
    installments_total: Option<i64>,
    occurrences_in_range: usize,
    committed_cents: i64,
    next: NextCycleDto,
    refund: RefundDto,
}

#[derive(Serialize)]
struct NextCycleDto {
    cycle_month: String,
    /// O dia em que o dinheiro sai: a fatura é um lump no vencimento, não um gasto por compra.
    due_date: String,
    installment_index: Option<i64>,
}

/// O dinheiro que volta ligado a um compromisso. Nunca reduz a parcela: o método é bruto, e o
/// vínculo existe para que a leitura líquida seja marcada, não para apagar a saída.
#[derive(Serialize)]
struct RefundDto {
    linked: bool,
    expected_cents: i64,
}

#[derive(Serialize)]
struct RecurringDto {
    recurrence_id: String,
    description: String,
    /// `diaria` · `semanal` · `mensal`.
    frequency: String,
    amount_cents: i64,
    installments_total: Option<i64>,
    occurrences_in_range: usize,
    committed_cents: i64,
    next: NextDateDto,
}

#[derive(Serialize)]
struct NextDateDto {
    date: String,
    installment_index: Option<i64>,
}

#[derive(Serialize)]
struct ObligationDto {
    id: String,
    name: String,
    /// O balde do método a que a obrigação pertence.
    kind: String,
    occurrences_in_range: usize,
    committed_cents: i64,
}

#[derive(Serialize)]
struct OccurrenceDto {
    date: String,
    description: String,
    amount_cents: i64,
    /// De onde a ocorrência vem: `card_series` · `recurrence` · `obligation`.
    source: &'static str,
    source_id: String,
    installment_index: Option<i64>,
}

#[derive(Serialize)]
struct MonthTotalDto {
    month: String,
    total_cents: i64,
    count: i64,
}

fn month_total_dto(month: &crate::obligations::ObligationMonthTotal) -> MonthTotalDto {
    MonthTotalDto {
        month: format!("{:04}-{:02}", month.year, month.month),
        total_cents: month.total_cents,
        count: month.count,
    }
}

pub(crate) async fn commitments(pool: &SqlitePool, args: &Args, today: NaiveDate) -> ToolResult {
    let (start, end) = match args.range("range")? {
        Some(range) => range,
        None => (today, months_ahead(today, COMMITMENT_MONTHS)),
    };

    if let Some(obligation_id) = args.text("obligation_id")? {
        return obligation_answer(pool, obligation_id, start, end, args).await;
    }

    // Cada fonte é lida uma vez; agrupar por série e listar ocorrência a ocorrência são duas
    // leituras das MESMAS linhas, e reconsultar o banco para a segunda abriria a chance de as
    // duas discordarem.
    let card = card_occurrences(pool, start, end).await?;
    let recurring = recurring_occurrences(pool, start, end).await?;
    let series = series_of(&card, &series_refunds(pool).await?);
    let recurring_series = recurring_series_of(&recurring);
    let obligations = obligations_in(pool, start, end).await?;

    let committed_cents: i64 = series.iter().map(|s| s.committed_cents).sum::<i64>()
        + recurring_series
            .iter()
            .map(|r| r.committed_cents)
            .sum::<i64>();

    let mut data = json!({
        // O que as séries de cartão e as séries do Livro-razão comprometem no recorte. As
        // obrigações ficam de fora da soma: elas nomeiam itens dentro de lançamentos que já
        // contam aqui ou no Saldo, e somá-las de novo contaria o mesmo dinheiro duas vezes.
        "committed_cents": committed_cents,
        "card_series": Listing::capped(series),
        "recurring": Listing::capped(recurring_series),
        "obligations": Listing::capped(obligations),
    });

    if args.wants("occurrences") {
        let mut occurrences: Vec<OccurrenceDto> = card
            .iter()
            .map(card_occurrence_dto)
            .chain(recurring.iter().map(recurring_occurrence_dto))
            .collect();
        occurrences.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| a.description.cmp(&b.description))
        });
        insert(&mut data, "occurrences", Listing::capped(occurrences));
    }

    Ok(ToolOutput {
        period: Period::between(start, end),
        data,
    })
}

/// Uma parcela ou mensalidade de série de cartão dentro do recorte, com a fatura que a cobra.
#[derive(sqlx::FromRow)]
struct CardOccurrence {
    series_id: String,
    description: String,
    /// O valor combinado da série; `cents` é o da ocorrência, que uma edição pode ter mudado.
    amount_cents: i64,
    /// Total de parcelas; nulo na assinatura.
    count: Option<i64>,
    start_cycle_month: String,
    card_name: String,
    cycle_month: String,
    due_date: String,
    cents: i64,
}

async fn card_occurrences(
    pool: &SqlitePool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<CardOccurrence>, ToolError> {
    // O compromisso do cartão acontece no VENCIMENTO da fatura, não na data da compra: é ali que
    // o dinheiro sai, e é assim que o motor injeta o lump.
    sqlx::query_as(
        "SELECT cs.id AS series_id, cs.description, cs.amount_cents, cs.count, \
                cs.start_cycle_month, a.name AS card_name, i.cycle_month, i.due_date, \
                t.amount AS cents \
         FROM \"transaction\" t \
         JOIN card_series cs ON cs.id = t.card_series_id \
         JOIN invoice i ON i.id = t.invoice_id \
         JOIN account a ON a.id = cs.account_id \
         WHERE t.scenario_id IS NULL AND i.due_date >= ?1 AND i.due_date <= ?2 \
         ORDER BY i.due_date, cs.id",
    )
    .bind(start.to_string())
    .bind(end.to_string())
    .fetch_all(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("séries de cartão: {e}")))
}

/// A parcela n/N sai do índice do CICLO — a série ancora em faturas consecutivas, e nenhuma
/// linha guarda o contador. Assinatura não tem n/N: dizer "5 de 5" mentiria sobre janeiro.
fn cycle_installment(occurrence: &CardOccurrence) -> Option<i64> {
    occurrence.count.and(crate::cards::cycle_index(
        &occurrence.start_cycle_month,
        &occurrence.cycle_month,
    ))
}

fn series_of(
    occurrences: &[CardOccurrence],
    refunds: &std::collections::HashMap<String, i64>,
) -> Vec<CardSeriesDto> {
    let mut out: Vec<CardSeriesDto> = Vec::new();
    for occurrence in occurrences {
        if let Some(existing) = out.iter_mut().find(|s| s.id == occurrence.series_id) {
            existing.occurrences_in_range += 1;
            existing.committed_cents += occurrence.cents.abs();
            continue;
        }
        let expected_cents = refunds.get(&occurrence.series_id).copied().unwrap_or(0);
        out.push(CardSeriesDto {
            id: occurrence.series_id.clone(),
            card_name: occurrence.card_name.clone(),
            description: occurrence.description.clone(),
            kind: if occurrence.count.is_some() {
                "installment"
            } else {
                "subscription"
            },
            amount_cents: occurrence.amount_cents,
            installments_total: occurrence.count,
            occurrences_in_range: 1,
            committed_cents: occurrence.cents.abs(),
            // A lista vem ordenada por vencimento, então a primeira do recorte é a próxima.
            next: NextCycleDto {
                cycle_month: occurrence.cycle_month.clone(),
                due_date: occurrence.due_date.clone(),
                installment_index: cycle_installment(occurrence),
            },
            refund: RefundDto {
                linked: expected_cents != 0,
                expected_cents,
            },
        });
    }
    out
}

fn card_occurrence_dto(occurrence: &CardOccurrence) -> OccurrenceDto {
    OccurrenceDto {
        date: occurrence.due_date.clone(),
        description: occurrence.description.clone(),
        amount_cents: occurrence.cents.abs(),
        source: "card_series",
        source_id: occurrence.series_id.clone(),
        installment_index: cycle_installment(occurrence),
    }
}

/// Quanto de dinheiro vinculado se espera de volta por série.
async fn series_refunds(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<String, i64>, ToolError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT refund_series_id, SUM(ABS(amount)) FROM \"transaction\" \
         WHERE refund_series_id IS NOT NULL AND scenario_id IS NULL \
         GROUP BY refund_series_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("reembolsos das séries: {e}")))?;
    Ok(rows.into_iter().collect())
}

/// Uma ocorrência de série do Livro-razão dentro do recorte.
#[derive(sqlx::FromRow)]
struct RecurringOccurrence {
    id: String,
    recurrence_id: String,
    description: String,
    date: String,
    cents: i64,
    frequency: String,
    /// Total de ocorrências; nulo na série sem fim declarado.
    repetitions: Option<i64>,
}

async fn recurring_occurrences(
    pool: &SqlitePool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<RecurringOccurrence>, ToolError> {
    sqlx::query_as(
        "SELECT t.id, t.recurrence_id, COALESCE(t.description, '') AS description, t.date, \
                t.amount AS cents, r.frequency, r.repetitions \
         FROM \"transaction\" t JOIN recurrence r ON r.id = t.recurrence_id \
         WHERE t.scenario_id IS NULL AND t.date >= ?1 AND t.date <= ?2 \
         ORDER BY t.date, t.id",
    )
    .bind(start.to_string())
    .bind(end.to_string())
    .fetch_all(pool)
    .await
    .map_err(|e| ToolError::read_failed(format!("séries do livro-razão: {e}")))
}

/// A posição 1-based da ocorrência na série, lida do id `{série}:{i}` — só quando a série tem
/// fim declarado, porque sem N a posição não conta uma história.
fn series_installment(occurrence: &RecurringOccurrence) -> Option<i64> {
    occurrence
        .repetitions
        .and(crate::recurrence::occurrence_index(&occurrence.id))
        .map(|index| index + 1)
}

fn recurring_series_of(occurrences: &[RecurringOccurrence]) -> Vec<RecurringDto> {
    let mut out: Vec<RecurringDto> = Vec::new();
    for occurrence in occurrences {
        if let Some(existing) = out
            .iter_mut()
            .find(|r| r.recurrence_id == occurrence.recurrence_id)
        {
            existing.occurrences_in_range += 1;
            existing.committed_cents += occurrence.cents.abs();
            continue;
        }
        out.push(RecurringDto {
            recurrence_id: occurrence.recurrence_id.clone(),
            description: occurrence.description.clone(),
            frequency: occurrence.frequency.clone(),
            amount_cents: occurrence.cents.abs(),
            installments_total: occurrence.repetitions,
            occurrences_in_range: 1,
            committed_cents: occurrence.cents.abs(),
            next: NextDateDto {
                date: occurrence.date.clone(),
                installment_index: series_installment(occurrence),
            },
        });
    }
    out
}

fn recurring_occurrence_dto(occurrence: &RecurringOccurrence) -> OccurrenceDto {
    OccurrenceDto {
        date: occurrence.date.clone(),
        description: occurrence.description.clone(),
        amount_cents: occurrence.cents.abs(),
        source: "recurrence",
        source_id: occurrence.recurrence_id.clone(),
        installment_index: series_installment(occurrence),
    }
}

async fn obligations_in(
    pool: &SqlitePool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ObligationDto>, ToolError> {
    let (start, end) = (start.to_string(), end.to_string());
    let mut out = Vec::new();
    for obligation in crate::obligations::list_obligations(pool)
        .await
        .map_err(ToolError::read_failed)?
    {
        let items = crate::obligations::obligation_items(pool, &obligation.id)
            .await
            .map_err(ToolError::read_failed)?;
        let inside: Vec<_> = items
            .iter()
            .filter(|i| in_range(&i.date, &start, &end))
            .collect();
        if inside.is_empty() {
            continue;
        }
        out.push(ObligationDto {
            id: obligation.id,
            name: obligation.name,
            kind: obligation.kind,
            occurrences_in_range: inside.len(),
            committed_cents: inside.iter().map(|i| i.amount_cents.abs()).sum(),
        });
    }
    Ok(out)
}

/// A resposta por obrigação: a série que a planilha não guarda, com o mês típico e o último mês
/// já comparados — subir de aluguel é a pergunta, e a conta não pode ficar para quem lê.
async fn obligation_answer(
    pool: &SqlitePool,
    obligation_id: &str,
    start: NaiveDate,
    end: NaiveDate,
    args: &Args,
) -> ToolResult {
    let obligation = crate::obligations::list_obligations(pool)
        .await
        .map_err(ToolError::read_failed)?
        .into_iter()
        .find(|o| o.id == obligation_id)
        .ok_or_else(|| {
            ToolError::new(
                ErrorCode::NotFound,
                format!("Não existe a obrigação \"{obligation_id}\"."),
                "Chame de novo sem obligation_id para ver as obrigações que o recorte alcança."
                    .to_string(),
            )
        })?;

    let history = crate::obligations::obligation_history(pool, obligation_id)
        .await
        .map_err(ToolError::read_failed)?;
    let items = crate::obligations::obligation_items(pool, obligation_id)
        .await
        .map_err(ToolError::read_failed)?;

    // O mês típico é a MEDIANA, o mesmo estimador que as réguas do método usam: um mês atípico
    // de aluguel não deve mover a referência contra a qual o último mês é lido.
    let typical_cents = forecast::median_cents(history.iter().map(|m| m.total_cents).collect());
    let last = history.last();
    let (from, to) = (start.to_string(), end.to_string());
    let months: Vec<MonthTotalDto> = history.iter().map(month_total_dto).collect();

    let mut data = json!({
        "obligation": {
            "id": obligation.id,
            "name": obligation.name,
            "kind": obligation.kind,
        },
        "committed_cents": items
            .iter()
            .filter(|i| in_range(&i.date, &from, &to))
            .map(|i| i.amount_cents.abs())
            .sum::<i64>(),
        "typical_cents": typical_cents,
        "last": last.map(month_total_dto),
        "delta_vs_typical": last.map(|m| Delta::between(m.total_cents, typical_cents)),
        "months": Listing::capped(months),
    });

    if args.wants("occurrences") {
        let occurrences: Vec<OccurrenceDto> = items
            .iter()
            .filter(|i| in_range(&i.date, &from, &to))
            .map(|i| OccurrenceDto {
                date: i.date.clone(),
                description: i.description.clone(),
                amount_cents: i.amount_cents.abs(),
                source: "obligation",
                source_id: obligation_id.to_string(),
                installment_index: None,
            })
            .collect();
        insert(&mut data, "occurrences", Listing::capped(occurrences));
    }

    Ok(ToolOutput {
        period: Period::between(start, end),
        data,
    })
}

// --- Costura ----------------------------------------------------------------------------

/// O valor como ele entra na impressão do recorte: ausente é ausente, e não um número que
/// alguém poderia ter pedido de verdade.
fn text_of(cents: Option<i64>) -> String {
    cents.map(|v| v.to_string()).unwrap_or_default()
}

/// A data cai no recorte? A comparação é textual porque as duas pontas são ISO — o formato do
/// domínio inteiro — e converter para comparar só abriria caminho para um erro de parse.
fn in_range(date: &str, start: &str, end: &str) -> bool {
    date >= start && date <= end
}

fn months_ahead(today: NaiveDate, months: u32) -> NaiveDate {
    today + chrono::Months::new(months)
}

fn current_month(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("dia 1 existe");
    (
        start,
        calendar::last_day_of_month(today.year(), today.month()),
    )
}
