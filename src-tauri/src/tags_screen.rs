//! Tela Tags: a tag como interruptor de contabilidade por régua.
//!
//! O núcleo é puro (efeitos = contribuição marginal ao estado atual, veredito de custo, máquina de
//! estados de terceiros) e testável sem IO; o shell carrega os eventos do mês e os vínculos de
//! pessoa e chama o núcleo. O efeito de cada interruptor é COMPUTADO pelo motor (nunca prosa
//! estimada): Performance mexe pelo líquido, Custo de vida pela saída — repetir o mesmo número
//! seria mentira aritmética.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tauri::State;

use crate::commands::forecast_cmds;
use crate::forecast::{self, CashflowEvent, MetricEvent, MonthMetric, RulerMask};

// --- DTO da tela -----------------------------------------------------------------------------

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TagsScreenVerdict {
    /// Custo de vida com os interruptores atuais (manchete A/C).
    pub cost_current_cents: i64,
    /// Custo de vida se TODAS as tags contassem em custo ("sem as exceções, contariam …").
    pub cost_all_on_cents: i64,
    /// Média mensal do dinheiro de terceiros na janela (manchete B); `null` = nada detectado.
    pub third_party_avg_cents: Option<i64>,
    /// Pessoas distintas com movimento de terceiro na janela.
    pub third_party_people: u32,
    /// Alguma tag tem algum flag de exclusão ligado.
    pub has_exceptions: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ThirdPartyLine {
    pub person_id: String,
    pub name: String,
    /// Saiu na sua conta em nome dela, no mês da tela.
    pub out_cents: i64,
    /// Voltou (realizado) no mês da tela.
    pub back_cents: i64,
    /// Vinculado ainda não realizado (retorno esperado).
    pub expected_cents: i64,
    /// `favor` · `open` · `series` · `settled` · `none`.
    pub state: String,
    /// Idade do "em aberto" em dias (só no estado `open`).
    pub open_since_days: Option<u32>,
    /// Parcela k de N (só no estado `series`).
    pub series_done: Option<u32>,
    pub series_total: Option<u32>,
    /// Data da quitação (só no estado `settled`).
    pub settled_date: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TagCountsIn {
    pub performance: bool,
    pub cost_of_living: bool,
    pub savings: bool,
    pub daily_avg: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TagEffectsDto {
    /// Contribuição marginal à Performance pelo LÍQUIDO (entrou − saiu); pode ser negativa.
    pub performance_delta_cents: i64,
    /// Contribuição ao Custo de vida pela SAÍDA.
    pub cost_delta_cents: i64,
    /// Δ na renda-base do Economizado%.
    pub savings_base_delta_cents: i64,
    /// Δ na economia registrada (numerador do Economizado%).
    pub savings_amount_delta_cents: i64,
    /// Δ no diário médio (D/N).
    pub daily_avg_delta_cents: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
    pub is_special: bool,
    /// `true` = a régua CALCULA a tag (interruptor ligado).
    pub counts_in: TagCountsIn,
    /// O que a tag movimentou no mês (mesma semântica do `tag_totals`).
    pub month_total_cents: i64,
    pub txn_count: i64,
    pub effects: TagEffectsDto,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TagsScreenDto {
    pub month: String,
    pub verdict: TagsScreenVerdict,
    pub third_parties: Vec<ThirdPartyLine>,
    pub tags: Vec<TagRow>,
    /// Última sincronização com a planilha — a IDADE do dado que a manchete F exibe quando a
    /// leitura atual falha. Sozinho não significa falha: a falha é o erro da própria query.
    pub last_sync_at: Option<String>,
}

// --- Núcleo puro: efeitos (contribuição marginal) --------------------------------------------

/// Flags de exclusão por régua de uma tag. `true` = a régua homônima NÃO calcula a tag.
#[derive(Debug, Clone)]
struct TagRuler {
    exclude_from_performance: bool,
    exclude_from_cost_of_living: bool,
    exclude_from_savings: bool,
    exclude_from_daily_avg: bool,
}

impl TagRuler {
    /// Máscara-base da tag: a régua conta quando o flag de exclusão é `false`.
    fn base_mask(&self) -> RulerMask {
        RulerMask {
            performance: !self.exclude_from_performance,
            cost_of_living: !self.exclude_from_cost_of_living,
            savings: !self.exclude_from_savings,
            daily_avg: !self.exclude_from_daily_avg,
        }
    }
}

/// Evento de métrica do mês com os índices das tags do lançamento-pai (em `tags`). Índices vazios
/// = evento sem tag OU sintético (fatura materializada) → conta em todas as réguas.
#[derive(Debug, Clone)]
struct TaggedEvent {
    event: CashflowEvent,
    tag_idx: Vec<usize>,
}

/// Qual régua de uma tag inverter na reexecução.
#[derive(Debug, Clone, Copy)]
enum Ruler {
    Performance,
    CostOfLiving,
    Savings,
    DailyAvg,
}

/// Métricas do mês sob uma tabela de máscaras por tag. A máscara de um evento é a INTERSEÇÃO das
/// máscaras das suas tags (`RulerMask::and`); sem tag herda `ALL`. `month_metrics_for` sempre
/// devolve o mês pedido (mesmo sem evento algum).
fn month_metric_under(
    today: NaiveDate,
    events: &[TaggedEvent],
    tag_masks: &[RulerMask],
    screen: (i32, u32),
    annotation: &HashMap<(i32, u32), i64>,
) -> MonthMetric {
    let metric_events: Vec<MetricEvent> = events
        .iter()
        .map(|te| {
            let mask = te
                .tag_idx
                .iter()
                .fold(RulerMask::ALL, |acc, &ti| acc.and(tag_masks[ti]));
            MetricEvent {
                event: te.event,
                mask,
            }
        })
        .collect();
    forecast::month_metrics_for(today, &metric_events, &[screen], annotation)
        .into_iter()
        .next()
        .expect("month_metrics_for devolve o mês pedido")
}

/// Recomputa as métricas invertendo SÓ a régua `flip` da tag `ti` (demais interruptores como
/// estão) — a peça da contribuição marginal.
fn metric_with_flipped(
    today: NaiveDate,
    events: &[TaggedEvent],
    base_masks: &[RulerMask],
    ti: usize,
    flip: Ruler,
    screen: (i32, u32),
    annotation: &HashMap<(i32, u32), i64>,
) -> MonthMetric {
    let mut masks = base_masks.to_vec();
    let m = &mut masks[ti];
    match flip {
        Ruler::Performance => m.performance = !m.performance,
        Ruler::CostOfLiving => m.cost_of_living = !m.cost_of_living,
        Ruler::Savings => m.savings = !m.savings,
        Ruler::DailyAvg => m.daily_avg = !m.daily_avg,
    }
    month_metric_under(today, events, &masks, screen, annotation)
}

/// Contribuição marginal (contando − excluído) de cada tag a cada régua. O sinal é ESTÁVEL: seja o
/// interruptor ligado ou desligado, o número é o mesmo — quando a tag já exclui a régua, a base é o
/// "excluído" e a reexecução é o "contando" (inv − base); senão a base é o "contando" (base − inv).
/// A anotação da aba entra na reexecução, então o efeito respeita a fronteira do `max` (pode ser 0).
fn compute_effects(
    today: NaiveDate,
    events: &[TaggedEvent],
    tags: &[TagRuler],
    screen: (i32, u32),
    annotation: &HashMap<(i32, u32), i64>,
) -> Vec<TagEffectsDto> {
    let base_masks: Vec<RulerMask> = tags.iter().map(TagRuler::base_mask).collect();
    let base = month_metric_under(today, events, &base_masks, screen, annotation);

    tags.iter()
        .enumerate()
        .map(|(ti, tag)| {
            let flipped = |flip: Ruler| {
                metric_with_flipped(today, events, &base_masks, ti, flip, screen, annotation)
            };
            // (contando − excluído): direção fixa por régua, independente do estado atual. Quando a
            // tag já exclui a régua, a base é o "excluído" e a reexecução é o "contando".
            let signed = |excluded: bool, base_val: i64, inv: i64| {
                if excluded {
                    inv - base_val
                } else {
                    base_val - inv
                }
            };

            let perf = flipped(Ruler::Performance);
            let performance_delta_cents = signed(
                tag.exclude_from_performance,
                base.performance_cents,
                perf.performance_cents,
            );
            let cost = flipped(Ruler::CostOfLiving);
            let cost_delta_cents = signed(
                tag.exclude_from_cost_of_living,
                base.cost_of_living_cents,
                cost.cost_of_living_cents,
            );
            let sav = flipped(Ruler::Savings);
            let savings_base_delta_cents = signed(
                tag.exclude_from_savings,
                base.income_cents,
                sav.income_cents,
            );
            let savings_amount_delta_cents = signed(
                tag.exclude_from_savings,
                base.economia_cents,
                sav.economia_cents,
            );
            let daily = flipped(Ruler::DailyAvg);
            let daily_avg_delta_cents = signed(
                tag.exclude_from_daily_avg,
                base.real_daily_avg_cents,
                daily.real_daily_avg_cents,
            );

            TagEffectsDto {
                performance_delta_cents,
                cost_delta_cents,
                savings_base_delta_cents,
                savings_amount_delta_cents,
                daily_avg_delta_cents,
            }
        })
        .collect()
}

/// Custo de vida atual e "com tudo ligado" (todas as tags contando em custo). A cauda "sem as
/// exceções, contariam …" força só o bit de custo; as outras réguas ficam como estão.
fn cost_verdict(
    today: NaiveDate,
    events: &[TaggedEvent],
    tags: &[TagRuler],
    screen: (i32, u32),
    annotation: &HashMap<(i32, u32), i64>,
) -> (i64, i64) {
    let base_masks: Vec<RulerMask> = tags.iter().map(TagRuler::base_mask).collect();
    let cost_current =
        month_metric_under(today, events, &base_masks, screen, annotation).cost_of_living_cents;
    let all_on: Vec<RulerMask> = base_masks
        .iter()
        .map(|m| RulerMask {
            cost_of_living: true,
            ..*m
        })
        .collect();
    let cost_all_on =
        month_metric_under(today, events, &all_on, screen, annotation).cost_of_living_cents;
    (cost_current, cost_all_on)
}

// --- Núcleo puro: dinheiro de terceiros ------------------------------------------------------

/// Agregados de uma pessoa antes de virar linha do DTO. Fluxos são do mês da tela; a expectativa
/// pode vir de meses anteriores (dívida não expira na virada).
#[derive(Debug, Clone)]
struct ThirdPartyAgg {
    person_id: String,
    name: String,
    out_cents: i64,
    back_cents: i64,
    expected_cents: i64,
    /// Saída/expectativa mais antiga por voltar — idade do "em aberto".
    open_since: Option<NaiveDate>,
    /// Última volta realizada — data da quitação.
    settled_on: Option<NaiveDate>,
    /// Série de reembolso vinculada (parcelas realizadas, total).
    series: Option<(u32, u32)>,
}

/// Máquina de estados de terceiro (view-model puro). Precedência: série (sinal estrutural mais
/// específico) → a favor → sem registro → quitado → em aberto.
fn third_party_line(agg: &ThirdPartyAgg, today: NaiveDate) -> ThirdPartyLine {
    let base = |state: &str| ThirdPartyLine {
        person_id: agg.person_id.clone(),
        name: agg.name.clone(),
        out_cents: agg.out_cents,
        back_cents: agg.back_cents,
        expected_cents: agg.expected_cents,
        state: state.to_string(),
        open_since_days: None,
        series_done: None,
        series_total: None,
        settled_date: None,
    };

    if let Some((done, total)) = agg.series {
        return ThirdPartyLine {
            series_done: Some(done),
            series_total: Some(total),
            ..base("series")
        };
    }
    if agg.back_cents > agg.out_cents {
        return base("favor");
    }
    if agg.out_cents == 0 && agg.back_cents == 0 && agg.expected_cents == 0 {
        return base("none");
    }
    if agg.expected_cents == 0 && agg.out_cents > 0 && agg.back_cents == agg.out_cents {
        return ThirdPartyLine {
            settled_date: agg.settled_on.map(|d| d.format("%Y-%m-%d").to_string()),
            ..base("settled")
        };
    }
    let open_since_days = agg.open_since.map(|d| (today - d).num_days().max(0) as u32);
    ThirdPartyLine {
        open_since_days,
        ..base("open")
    }
}

/// Média mensal do `out` de terceiros na janela, só sobre os meses COM movimento. `None` quando
/// nenhum mês teve detecção (a manchete cai para o estado C).
fn third_party_monthly_average(monthly_out: &[i64]) -> Option<i64> {
    let with_movement: Vec<i64> = monthly_out.iter().copied().filter(|&v| v > 0).collect();
    if with_movement.is_empty() {
        return None;
    }
    Some(with_movement.iter().sum::<i64>() / with_movement.len() as i64)
}

// --- Shell: carga dos eventos do mês com as tags por lançamento ------------------------------

/// Eventos de métrica do mês da tela, cada um com os índices das tags do lançamento-pai, já com a
/// precedência da fatura aplicada (Cartão futuro vira lump sintético sem tag). Espelha a construção
/// final do motor para o número do custo fechar com a visão Este mês.
async fn load_tagged_month_events(
    pool: &SqlitePool,
    today: NaiveDate,
    tag_index: &HashMap<String, usize>,
    first: NaiveDate,
    end_exclusive: NaiveDate,
) -> Result<Vec<TaggedEvent>, String> {
    let raw = forecast_cmds::load_raw_db_events(pool, first, end_exclusive, Some(today)).await?;

    // transaction_id → índices das tags (na ordem de `tags`).
    let tag_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT tt.transaction_id, tt.tag_id FROM transaction_tag tt \
         JOIN \"transaction\" t ON t.id = tt.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL",
    )
    .bind(first.format("%Y-%m-%d").to_string())
    .bind(end_exclusive.format("%Y-%m-%d").to_string())
    .fetch_all(pool)
    .await
    .map_err(|e| format!("tags por lançamento: {e}"))?;
    let mut idx_by_txn: HashMap<String, Vec<usize>> = HashMap::new();
    for (txn, tag) in tag_rows {
        if let Some(&i) = tag_index.get(&tag) {
            idx_by_txn.entry(txn).or_default().push(i);
        }
    }

    let tagged: Vec<TaggedEvent> = raw
        .into_iter()
        .map(|r| TaggedEvent {
            event: r.event,
            tag_idx: idx_by_txn
                .get(&r.transaction_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect();

    // Precedência da fatura: o Cartão cru futuro some e a fatura entra como lump SINTÉTICO (sem
    // tag → conta em todas as réguas, igual ao motor). Genérica no envelope via
    // `apply_card_invoice_precedence`.
    let (has_card, invoices) =
        forecast_cmds::load_card_invoice_events(pool, today, first, Some(end_exclusive)).await?;
    Ok(forecast_cmds::apply_card_invoice_precedence(
        today,
        has_card,
        tagged,
        &invoices,
        |te: &TaggedEvent| te.event.kind == forecast::EventKind::Cartao && te.event.date > today,
        |invoice| TaggedEvent {
            event: CashflowEvent {
                date: invoice.due_date,
                kind: forecast::EventKind::Cartao,
                amount_cents: invoice.amount_cents,
                realized: false,
            },
            tag_idx: Vec::new(),
        },
    ))
}

// --- Shell: dinheiro de terceiros ------------------------------------------------------------

/// Agrega o dinheiro de terceiros do mês por pessoa a partir das quatro fontes estruturais
/// (marcadores de nota, splits, cartão vinculado, expectativas). O titular nunca entra: só quem
/// possui vínculo de terceiro (split, derivado, conta vinculada) vira candidato.
async fn load_third_parties(
    pool: &SqlitePool,
    today: NaiveDate,
    year: i32,
    month: u32,
) -> Result<Vec<ThirdPartyAgg>, String> {
    let ym = format!("{year:04}-{month:02}");
    let last = forecast::last_day_of_month(year, month);
    let ym_s = ym.clone();
    let today_s = today.format("%Y-%m-%d").to_string();
    let last_s = last.format("%Y-%m-%d").to_string();

    // Candidatos: donos de vínculo de terceiro. A conta principal do titular (não vinculada) não
    // gera linha.
    let people: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.id, p.name FROM person p WHERE p.id IN ( \
            SELECT owner_person_id FROM split \
            UNION SELECT counterparty_person_id FROM \"transaction\" \
                  WHERE counterparty_person_id IS NOT NULL \
            UNION SELECT owner_person_id FROM account WHERE linked_account_id IS NOT NULL) \
         ORDER BY p.name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("terceiros candidatos: {e}"))?;

    let mut aggs: HashMap<String, ThirdPartyAgg> = people
        .into_iter()
        .map(|(id, name)| {
            (
                id.clone(),
                ThirdPartyAgg {
                    person_id: id,
                    name,
                    out_cents: 0,
                    back_cents: 0,
                    expected_cents: 0,
                    open_since: None,
                    settled_on: None,
                    series: None,
                },
            )
        })
        .collect();

    let bump_out = |a: &mut ThirdPartyAgg, cents: i64, date: Option<NaiveDate>| {
        a.out_cents += cents;
        if let Some(d) = date {
            a.open_since = Some(a.open_since.map_or(d, |cur| cur.min(d)));
        }
    };

    // Fonte 1 — marcadores de reembolso: a perna "saiu" = valor integral da linha (a Entrada
    // derivada carrega o valor fronteado). Splits cobrem a perna "saiu" do #dividir:.
    // Derivada JÁ ligada a fatura de conta VINCULADA pertence à fonte CARTÃO (o import liga
    // #reembolso: de linha de Cartão à sub-fatura) — sem o corte, out e back dobrariam.
    let reembolso_out: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT counterparty_person_id, SUM(ABS(amount)), MIN(date) \
         FROM \"transaction\" \
         WHERE id LIKE 'derived:reembolso:%' AND counterparty_person_id IS NOT NULL \
           AND substr(date,1,7) = ?1 AND scenario_id IS NULL \
           AND NOT EXISTS (SELECT 1 FROM invoice iv JOIN account av ON av.id = iv.account_id \
                WHERE iv.id = \"transaction\".refund_invoice_id \
                  AND av.linked_account_id IS NOT NULL) \
         GROUP BY counterparty_person_id",
    )
    .bind(&ym_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("reembolso out: {e}"))?;
    for (pid, cents, min_date) in reembolso_out {
        if let Some(a) = aggs.get_mut(&pid) {
            bump_out(
                a,
                cents,
                NaiveDate::parse_from_str(&min_date, "%Y-%m-%d").ok(),
            );
        }
    }

    // Fonte 2 — splits: parte de terceiro numa saída.
    let split_out: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT s.owner_person_id, SUM(ABS(s.amount)), MIN(t.date) \
         FROM split s JOIN \"transaction\" t ON t.id = s.transaction_id \
         WHERE substr(t.date,1,7) = ?1 AND t.scenario_id IS NULL \
         GROUP BY s.owner_person_id",
    )
    .bind(&ym_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("split out: {e}"))?;
    for (pid, cents, min_date) in split_out {
        if let Some(a) = aggs.get_mut(&pid) {
            bump_out(
                a,
                cents,
                NaiveDate::parse_from_str(&min_date, "%Y-%m-%d").ok(),
            );
        }
    }

    // Fonte 3 — cartão vinculado: "saiu" = total efetivo das sub-faturas da pessoa no ciclo
    // (vencimento no mês da tela). Total declarado tem precedência sobre a soma das compras.
    let card_out: Vec<(String, Option<i64>, i64, String)> = sqlx::query_as(
        "SELECT a.owner_person_id, i.stated_total_cents, \
                COALESCE(SUM(ABS(t.amount)), 0), i.due_date \
         FROM invoice i JOIN account a ON a.id = i.account_id \
         LEFT JOIN \"transaction\" t ON t.invoice_id = i.id AND t.type = 'expense' \
              AND t.scenario_id IS NULL \
         WHERE a.linked_account_id IS NOT NULL AND substr(i.due_date,1,7) = ?1 \
         GROUP BY i.id",
    )
    .bind(&ym_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("cartão vinculado out: {e}"))?;
    for (pid, stated, psum, due) in card_out {
        if let Some(a) = aggs.get_mut(&pid) {
            let effective = crate::cards::effective_total_cents(stated, psum);
            bump_out(
                a,
                effective,
                NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok(),
            );
        }
    }

    // "Voltou" (realizado, mês da tela): Entradas derivadas realizadas + reembolsos de cartão
    // vinculado realizados.
    let back_derived: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT counterparty_person_id, SUM(ABS(amount)), MAX(date) \
         FROM \"transaction\" \
         WHERE (id LIKE 'derived:reembolso:%' OR id LIKE 'derived:dividir:%') \
           AND counterparty_person_id IS NOT NULL AND is_projection = 0 AND date <= ?2 \
           AND substr(date,1,7) = ?1 AND scenario_id IS NULL \
           AND NOT EXISTS (SELECT 1 FROM invoice iv JOIN account av ON av.id = iv.account_id \
                WHERE iv.id = \"transaction\".refund_invoice_id \
                  AND av.linked_account_id IS NOT NULL) \
         GROUP BY counterparty_person_id",
    )
    .bind(&ym_s)
    .bind(&today_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("voltou derivado: {e}"))?;
    let back_card: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT a.owner_person_id, SUM(ABS(t.amount)), MAX(t.date) \
         FROM \"transaction\" t JOIN invoice i ON i.id = t.refund_invoice_id \
         JOIN account a ON a.id = i.account_id \
         WHERE t.type = 'income' AND t.is_projection = 0 AND t.date <= ?2 \
           AND a.linked_account_id IS NOT NULL AND substr(t.date,1,7) = ?1 \
           AND t.scenario_id IS NULL \
         GROUP BY a.owner_person_id",
    )
    .bind(&ym_s)
    .bind(&today_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("voltou cartão: {e}"))?;
    for (pid, cents, max_date) in back_derived.into_iter().chain(back_card) {
        if let Some(a) = aggs.get_mut(&pid) {
            a.back_cents += cents;
            if let Ok(d) = NaiveDate::parse_from_str(&max_date, "%Y-%m-%d") {
                a.settled_on = Some(a.settled_on.map_or(d, |cur| cur.max(d)));
            }
        }
    }

    // Fonte 4 — expectativas: Entrada vinculada não realizada (projeção ou data futura). Abertos de
    // meses anteriores continuam (até o fim do mês da tela), então a janela é `date <= último dia`.
    let expected_derived: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT counterparty_person_id, SUM(ABS(amount)), MIN(date) \
         FROM \"transaction\" \
         WHERE (id LIKE 'derived:reembolso:%' OR id LIKE 'derived:dividir:%') \
           AND counterparty_person_id IS NOT NULL AND (is_projection = 1 OR date > ?2) \
           AND date <= ?3 AND scenario_id IS NULL \
           AND NOT EXISTS (SELECT 1 FROM invoice iv JOIN account av ON av.id = iv.account_id \
                WHERE iv.id = \"transaction\".refund_invoice_id \
                  AND av.linked_account_id IS NOT NULL) \
         GROUP BY counterparty_person_id",
    )
    .bind(&ym_s)
    .bind(&today_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("expectativa derivada: {e}"))?;
    let expected_card: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT a.owner_person_id, SUM(ABS(t.amount)), MIN(t.date) \
         FROM \"transaction\" t JOIN invoice i ON i.id = t.refund_invoice_id \
         JOIN account a ON a.id = i.account_id \
         WHERE t.type = 'income' AND (t.is_projection = 1 OR t.date > ?1) AND t.date <= ?2 \
           AND a.linked_account_id IS NOT NULL AND t.scenario_id IS NULL \
         GROUP BY a.owner_person_id",
    )
    .bind(&today_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("expectativa cartão: {e}"))?;
    for (pid, cents, min_date) in expected_derived.into_iter().chain(expected_card) {
        if let Some(a) = aggs.get_mut(&pid) {
            a.expected_cents += cents;
            if let Ok(d) = NaiveDate::parse_from_str(&min_date, "%Y-%m-%d") {
                a.open_since = Some(a.open_since.map_or(d, |cur| cur.min(d)));
            }
        }
    }

    // Série de reembolso vinculada (parcelamento com `count`): parcelas realizadas até o fim do
    // mês × total, com `done` escopado à PRÓPRIA série — nunca a soma cruzada das séries da
    // pessoa. Com mais de uma série viva no mês, a do reembolso mais RECENTE representa a linha
    // (ordem ascendente + sobrescrita = a última vence, deterministicamente).
    let series_rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT a.owner_person_id, cs.count, \
                (SELECT COUNT(*) FROM \"transaction\" t2 \
                  WHERE t2.refund_series_id = cs.id AND t2.is_projection = 0 \
                    AND t2.date <= ?2 AND t2.scenario_id IS NULL) \
         FROM \"transaction\" t JOIN card_series cs ON cs.id = t.refund_series_id \
         JOIN account a ON a.id = cs.account_id \
         WHERE cs.count IS NOT NULL \
           AND substr(t.date,1,7) = ?1 AND t.scenario_id IS NULL \
         GROUP BY a.owner_person_id, cs.id, cs.count \
         ORDER BY MAX(t.date) ASC",
    )
    .bind(&ym_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("série de reembolso: {e}"))?;
    for (pid, total, done) in series_rows {
        if let Some(a) = aggs.get_mut(&pid) {
            a.series = Some((done.max(0) as u32, total.max(0) as u32));
        }
    }

    let mut out: Vec<ThirdPartyAgg> = aggs.into_values().collect();
    out.sort_by(|a, b| {
        b.out_cents
            .cmp(&a.out_cents)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

// --- Shell: montagem do DTO ------------------------------------------------------------------

/// Implementação com `today` injetado (determinística, testável). O comando fino usa o relógio.
pub(crate) async fn tags_screen_dto(
    pool: &SqlitePool,
    today: NaiveDate,
    year: i32,
    month: u32,
) -> Result<TagsScreenDto, String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or("mês inválido")?;
    let last = forecast::last_day_of_month(year, month);
    let end_exclusive = last.succ_opt().ok_or("mês inválido para intervalo")?;
    let screen = (year, month);
    let annotation = forecast_cmds::load_economia_annotation(pool, &[year]).await?;

    // Tags e a ordem estável (índice) que os eventos referenciam.
    let tags = crate::tags::list_tags(pool).await?;
    let tag_index: HashMap<String, usize> = tags
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.clone(), i))
        .collect();
    let rulers: Vec<TagRuler> = tags
        .iter()
        .map(|t| TagRuler {
            exclude_from_performance: t.exclude_from_performance,
            exclude_from_cost_of_living: t.exclude_from_cost_of_living,
            exclude_from_savings: t.exclude_from_savings,
            exclude_from_daily_avg: t.exclude_from_daily_avg,
        })
        .collect();

    let events = load_tagged_month_events(pool, today, &tag_index, first, end_exclusive).await?;

    let effects = compute_effects(today, &events, &rulers, screen, &annotation);
    let (cost_current_cents, cost_all_on_cents) =
        cost_verdict(today, &events, &rulers, screen, &annotation);

    // Total e contagem por tag (semântica do tag_totals: saída/transfer do mês, valor absoluto).
    let totals: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT t.id, COALESCE(SUM(ABS(tr.amount)), 0), COUNT(tr.id) \
         FROM tag t \
         LEFT JOIN transaction_tag tt ON tt.tag_id = t.id \
         LEFT JOIN \"transaction\" tr ON tr.id = tt.transaction_id \
                AND substr(tr.date, 1, 7) = ?1 \
                AND tr.type IN ('expense', 'transfer') \
                AND tr.scenario_id IS NULL \
         GROUP BY t.id",
    )
    .bind(format!("{year:04}-{month:02}"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("totais por tag: {e}"))?;
    let totals_by_id: HashMap<String, (i64, i64)> = totals
        .into_iter()
        .map(|(id, total, count)| (id, (total, count)))
        .collect();

    let has_exceptions = rulers.iter().any(|r| {
        r.exclude_from_performance
            || r.exclude_from_cost_of_living
            || r.exclude_from_savings
            || r.exclude_from_daily_avg
    });

    let mut tag_rows: Vec<TagRow> = tags
        .iter()
        .zip(rulers.iter())
        .zip(effects)
        .map(|((t, r), effects)| {
            let (month_total_cents, txn_count) = totals_by_id.get(&t.id).copied().unwrap_or((0, 0));
            TagRow {
                id: t.id.clone(),
                name: t.name.clone(),
                color: t.color.clone(),
                emoji: t.emoji.clone(),
                is_special: t.is_special,
                counts_in: TagCountsIn {
                    performance: !r.exclude_from_performance,
                    cost_of_living: !r.exclude_from_cost_of_living,
                    savings: !r.exclude_from_savings,
                    daily_avg: !r.exclude_from_daily_avg,
                },
                month_total_cents,
                txn_count,
                effects,
            }
        })
        .collect();
    tag_rows.sort_by(|a, b| {
        b.is_special
            .cmp(&a.is_special)
            .then_with(|| b.month_total_cents.cmp(&a.month_total_cents))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // Terceiros do mês + janela (12 meses completos + o corrente da tela) para a média da manchete.
    let third_party_aggs = load_third_parties(pool, today, year, month).await?;
    let third_parties: Vec<ThirdPartyLine> = third_party_aggs
        .iter()
        .map(|a| third_party_line(a, today))
        .collect();

    let mut monthly_out: Vec<i64> = Vec::with_capacity(13);
    let mut people_in_window: std::collections::HashSet<String> = std::collections::HashSet::new();
    for back in 0..13i64 {
        let (wy, wm) = months_back(year, month, back);
        let window = load_third_parties(pool, today, wy, wm).await?;
        let total: i64 = window.iter().map(|a| a.out_cents).sum();
        monthly_out.push(total);
        for a in &window {
            if a.out_cents > 0 {
                people_in_window.insert(a.person_id.clone());
            }
        }
    }
    let third_party_avg_cents = third_party_monthly_average(&monthly_out);
    let third_party_people = people_in_window.len() as u32;

    let last_sync_at = crate::commands::sheets_import::last_sync_at_query(pool).await?;

    Ok(TagsScreenDto {
        month: format!("{year:04}-{month:02}"),
        verdict: TagsScreenVerdict {
            cost_current_cents,
            cost_all_on_cents,
            third_party_avg_cents,
            third_party_people,
            has_exceptions,
        },
        third_parties,
        tags: tag_rows,
        last_sync_at,
    })
}

/// `(ano, mês)` recuado `back` meses (calendário). `back = 0` = o próprio mês.
fn months_back(year: i32, month: u32, back: i64) -> (i32, u32) {
    let zero = (year as i64) * 12 + (month as i64 - 1) - back;
    let y = zero.div_euclid(12) as i32;
    let m = zero.rem_euclid(12) as u32 + 1;
    (y, m)
}

#[tauri::command]
pub async fn get_tags_screen(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<TagsScreenDto, String> {
    tags_screen_dto(pool.inner(), chrono::Local::now().date_naive(), year, month).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn ev(date: &str, kind: forecast::EventKind, amount_cents: i64) -> CashflowEvent {
        CashflowEvent {
            date: d(date),
            kind,
            amount_cents,
            realized: true,
        }
    }
    fn tag(perf: bool, cost: bool, savings: bool, daily: bool) -> TagRuler {
        TagRuler {
            exclude_from_performance: perf,
            exclude_from_cost_of_living: cost,
            exclude_from_savings: savings,
            exclude_from_daily_avg: daily,
        }
    }
    fn no_annotation() -> HashMap<(i32, u32), i64> {
        HashMap::new()
    }

    // Manchete do TDD: tag "Gio" com Income 497.764 + Daily 407.764 marcados, excluída de
    // performance+custo. A MESMA exclusão move a Performance pelo LÍQUIDO (90.000, piora ao sair) e
    // o Custo pela SAÍDA (407.764) — sinal estável seja o interruptor lido ligado ou desligado.
    #[test]
    fn gio_effect_performance_by_net_cost_by_outflow() {
        let gio = tag(true, true, false, false); // fora de performance e custo
        let events = [
            TaggedEvent {
                event: ev("2026-03-08", forecast::EventKind::Income, 497_764),
                tag_idx: vec![0],
            },
            TaggedEvent {
                event: ev("2026-03-10", forecast::EventKind::Daily, 407_764),
                tag_idx: vec![0],
            },
        ];
        let effects = compute_effects(
            d("2026-03-31"),
            &events,
            &[gio],
            (2026, 3),
            &no_annotation(),
        );
        assert_eq!(effects[0].performance_delta_cents, 90_000);
        assert_eq!(effects[0].cost_delta_cents, 407_764);
    }

    // O sinal é estável: uma tag CONTANDO (interruptor ligado) reporta o mesmo efeito que a mesma
    // tag excluída — só a base da reexecução troca de lado.
    #[test]
    fn effect_sign_is_state_independent() {
        let events = [
            TaggedEvent {
                event: ev("2026-03-08", forecast::EventKind::Income, 497_764),
                tag_idx: vec![0],
            },
            TaggedEvent {
                event: ev("2026-03-10", forecast::EventKind::Daily, 407_764),
                tag_idx: vec![0],
            },
        ];
        let on = compute_effects(
            d("2026-03-31"),
            &events,
            &[tag(false, false, false, false)],
            (2026, 3),
            &no_annotation(),
        );
        let off = compute_effects(
            d("2026-03-31"),
            &events,
            &[tag(true, true, false, false)],
            (2026, 3),
            &no_annotation(),
        );
        assert_eq!(on[0].performance_delta_cents, 90_000);
        assert_eq!(off[0].performance_delta_cents, 90_000);
        assert_eq!(on[0].cost_delta_cents, 407_764);
        assert_eq!(off[0].cost_delta_cents, 407_764);
    }

    // Efeito de savings = 0 quando a anotação da aba domina o `max`: a anotação não tem tag, então
    // desligar a Economia de uma tag não muda o numerador (fronteira honesta).
    #[test]
    fn savings_effect_zero_when_annotation_dominates() {
        let events = [
            TaggedEvent {
                event: ev("2026-03-05", forecast::EventKind::Income, 1_000_000),
                tag_idx: vec![],
            },
            TaggedEvent {
                event: ev("2026-03-20", forecast::EventKind::Economia, 100_000),
                tag_idx: vec![0],
            },
        ];
        let mut annotation = no_annotation();
        annotation.insert((2026, 3), 100_000); // domina o derivado da tag
        let effects = compute_effects(
            d("2026-03-31"),
            &events,
            &[tag(false, false, false, false)],
            (2026, 3),
            &annotation,
        );
        assert_eq!(
            effects[0].savings_amount_delta_cents, 0,
            "anotação sustenta o numerador: efeito honesto R$ 0"
        );
    }

    // Sem anotação, desligar a Economia da tag tira a economia registrada do numerador (e a renda
    // da base, se a renda também for tagueada).
    #[test]
    fn savings_effect_moves_amount_and_base() {
        let events = [
            TaggedEvent {
                event: ev("2026-03-05", forecast::EventKind::Income, 1_000_000),
                tag_idx: vec![0],
            },
            TaggedEvent {
                event: ev("2026-03-20", forecast::EventKind::Economia, 250_000),
                tag_idx: vec![0],
            },
        ];
        let effects = compute_effects(
            d("2026-03-31"),
            &events,
            &[tag(false, false, false, false)],
            (2026, 3),
            &no_annotation(),
        );
        assert_eq!(effects[0].savings_base_delta_cents, 1_000_000);
        assert_eq!(effects[0].savings_amount_delta_cents, 250_000);
    }

    // Diário médio pela reexecução: desligar o Diário da tag tira o gasto do numerador (D/N).
    #[test]
    fn daily_avg_effect_uses_elapsed_days() {
        let events = [TaggedEvent {
            event: ev("2026-03-10", forecast::EventKind::Daily, 310_000),
            tag_idx: vec![0],
        }];
        // 31 dias decorridos (visão do fim do mês): 310.000 / 31 = 10.000/dia.
        let effects = compute_effects(
            d("2026-03-31"),
            &events,
            &[tag(false, false, false, false)],
            (2026, 3),
            &no_annotation(),
        );
        assert_eq!(effects[0].daily_avg_delta_cents, 10_000);
    }

    // Veredito: cost_current respeita os interruptores; cost_all_on força todas as tags em custo.
    #[test]
    fn cost_verdict_current_vs_all_on() {
        let events = [
            TaggedEvent {
                event: ev("2026-03-10", forecast::EventKind::Daily, 200_000),
                tag_idx: vec![],
            },
            TaggedEvent {
                event: ev("2026-03-12", forecast::EventKind::Daily, 100_000),
                tag_idx: vec![0], // fora do custo
            },
        ];
        let (current, all_on) = cost_verdict(
            d("2026-03-31"),
            &events,
            &[tag(false, true, false, false)],
            (2026, 3),
            &no_annotation(),
        );
        assert_eq!(current, 200_000, "a tag fora do custo não conta");
        assert_eq!(all_on, 300_000, "sem exceções, contaria os dois");
    }

    // Máquina de estados de terceiro (view-model puro).
    fn agg(out: i64, back: i64, expected: i64) -> ThirdPartyAgg {
        ThirdPartyAgg {
            person_id: "p".into(),
            name: "Gio".into(),
            out_cents: out,
            back_cents: back,
            expected_cents: expected,
            open_since: None,
            settled_on: None,
            series: None,
        }
    }

    #[test]
    fn third_party_states() {
        // favor: voltou mais do que saiu.
        assert_eq!(
            third_party_line(&agg(100, 150, 0), d("2026-03-31")).state,
            "favor"
        );
        // settled: saiu e voltou batem, nada pendente.
        let mut s = agg(100, 100, 0);
        s.settled_on = Some(d("2026-03-20"));
        let line = third_party_line(&s, d("2026-03-31"));
        assert_eq!(line.state, "settled");
        assert_eq!(line.settled_date.as_deref(), Some("2026-03-20"));
        // open: expectativa viva → idade desde a saída.
        let mut o = agg(100, 0, 100);
        o.open_since = Some(d("2026-03-01"));
        let line = third_party_line(&o, d("2026-03-31"));
        assert_eq!(line.state, "open");
        assert_eq!(line.open_since_days, Some(30));
        // series: precede tudo.
        let mut se = agg(100, 100, 0);
        se.series = Some((2, 5));
        let line = third_party_line(&se, d("2026-03-31"));
        assert_eq!(line.state, "series");
        assert_eq!((line.series_done, line.series_total), (Some(2), Some(5)));
        // none: pessoa conhecida sem movimento.
        assert_eq!(
            third_party_line(&agg(0, 0, 0), d("2026-03-31")).state,
            "none"
        );
    }

    // Média mensal da manchete B: só os meses com movimento entram na média.
    #[test]
    fn monthly_average_over_active_months() {
        assert_eq!(third_party_monthly_average(&[0, 0, 0]), None);
        assert_eq!(third_party_monthly_average(&[300, 0, 100, 0]), Some(200));
    }

    // Recuo de meses no calendário (janela da manchete): cruza a virada do ano.
    #[test]
    fn months_back_crosses_year_boundary() {
        assert_eq!(months_back(2026, 3, 0), (2026, 3));
        assert_eq!(months_back(2026, 3, 3), (2025, 12));
        assert_eq!(months_back(2026, 1, 1), (2025, 12));
        assert_eq!(months_back(2026, 3, 12), (2025, 3));
    }

    // --- Integração (pool) ---------------------------------------------------------------

    use sqlx::sqlite::SqlitePoolOptions;

    async fn mem_pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn add_person(p: &SqlitePool, id: &str, name: &str) {
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(id)
            .bind(name)
            .execute(p)
            .await
            .unwrap();
    }

    async fn add_account(p: &SqlitePool, id: &str, ty: &str, owner: &str, linked: Option<&str>) {
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, linked_account_id) \
             VALUES (?1, ?1, ?2, ?3, ?4)",
        )
        .bind(id)
        .bind(ty)
        .bind(owner)
        .bind(linked)
        .execute(p)
        .await
        .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    async fn add_txn(
        p: &SqlitePool,
        id: &str,
        ty: &str,
        amount: i64,
        date: &str,
        is_projection: i64,
        counterparty: Option<&str>,
        refund_invoice_id: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, is_projection, counterparty_person_id, refund_invoice_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(id)
        .bind(ty)
        .bind(amount)
        .bind(date)
        .bind(is_projection)
        .bind(counterparty)
        .bind(refund_invoice_id)
        .execute(p)
        .await
        .unwrap();
    }

    async fn add_invoice(p: &SqlitePool, id: &str, account_id: &str, due: &str, stated: i64) {
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, \
             stated_total_cents) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(account_id)
        .bind(&due[..7])
        .bind(due)
        .bind(due)
        .bind(stated)
        .execute(p)
        .await
        .unwrap();
    }

    async fn add_split(p: &SqlitePool, id: &str, txn: &str, amount: i64, owner: &str) {
        sqlx::query(
            "INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1,?2,?3,?4)",
        )
        .bind(id)
        .bind(txn)
        .bind(amount)
        .bind(owner)
        .execute(p)
        .await
        .unwrap();
    }

    fn line<'a>(lines: &'a [ThirdPartyLine], name: &str) -> &'a ThirdPartyLine {
        lines.iter().find(|l| l.name == name).unwrap()
    }

    // As quatro fontes estruturais e seus estados, num mês só. Titular (dono de conta principal
    // não vinculada) nunca vira linha.
    #[tokio::test]
    async fn third_parties_by_source_and_state() {
        let p = mem_pool().await;
        let today = d("2026-03-31");
        add_person(&p, "tit", "Titular").await;
        add_person(&p, "ana", "Ana").await;
        add_person(&p, "bru", "Bru").await;
        add_person(&p, "cau", "Cau").await;
        add_person(&p, "dan", "Dan").await;
        add_person(&p, "eva", "Eva").await;
        add_account(&p, "main", "bank", "tit", None).await;

        // Marcador de reembolso realizado → quitado (saiu = voltou).
        add_txn(&p, "e1", "expense", -50000, "2026-03-05", 0, None, None).await;
        add_txn(
            &p,
            "derived:reembolso:e1:0",
            "income",
            50000,
            "2026-03-05",
            0,
            Some("ana"),
            None,
        )
        .await;

        // Split sem retorno → em aberto (deve).
        add_txn(&p, "e2", "expense", -30000, "2026-03-10", 0, None, None).await;
        add_split(&p, "s2", "e2", 20000, "bru").await;

        // Cartão vinculado + reembolso maior que a fatura → a favor.
        add_account(&p, "card_c", "credit_card", "cau", Some("main")).await;
        add_invoice(&p, "inv_c", "card_c", "2026-03-15", 40000).await;
        add_txn(
            &p,
            "r_c",
            "income",
            60000,
            "2026-03-20",
            0,
            None,
            Some("inv_c"),
        )
        .await;

        // Cartão vinculado + reembolso ainda PROJETADO → expectativa (em aberto).
        add_account(&p, "card_d", "credit_card", "dan", Some("main")).await;
        add_invoice(&p, "inv_d", "card_d", "2026-03-15", 25000).await;
        add_txn(
            &p,
            "r_d",
            "income",
            25000,
            "2026-03-20",
            1,
            None,
            Some("inv_d"),
        )
        .await;

        // Eva é candidata por um split em OUTRO mês, sem movimento em março → sem registro.
        add_txn(&p, "e_prev", "expense", -10000, "2026-01-10", 0, None, None).await;
        add_split(&p, "s_prev", "e_prev", 5000, "eva").await;

        let aggs = load_third_parties(&p, today, 2026, 3).await.unwrap();
        let lines: Vec<ThirdPartyLine> = aggs.iter().map(|a| third_party_line(a, today)).collect();

        assert!(
            lines.iter().all(|l| l.name != "Titular"),
            "titular não vira linha"
        );

        let ana = line(&lines, "Ana");
        assert_eq!((ana.out_cents, ana.back_cents), (50000, 50000));
        assert_eq!(ana.state, "settled");
        assert_eq!(ana.settled_date.as_deref(), Some("2026-03-05"));

        let bru = line(&lines, "Bru");
        assert_eq!(
            (bru.out_cents, bru.back_cents, bru.expected_cents),
            (20000, 0, 0)
        );
        assert_eq!(bru.state, "open");
        assert_eq!(bru.open_since_days, Some(21)); // 2026-03-10 → 2026-03-31

        let cau = line(&lines, "Cau");
        assert_eq!((cau.out_cents, cau.back_cents), (40000, 60000));
        assert_eq!(cau.state, "favor");

        let dan = line(&lines, "Dan");
        assert_eq!(
            (dan.out_cents, dan.back_cents, dan.expected_cents),
            (25000, 0, 25000)
        );
        assert_eq!(dan.state, "open");

        let eva = line(&lines, "Eva");
        assert_eq!((eva.out_cents, eva.back_cents), (0, 0));
        assert_eq!(eva.state, "none");
    }

    // Média mensal da manchete B: só os meses com movimento entram. Ana movimenta em jan e mar;
    // fev fica sem detecção → média = (jan + mar) / 2.
    #[tokio::test]
    async fn headline_monthly_average_over_window() {
        let p = mem_pool().await;
        let today = d("2026-03-31");
        add_person(&p, "ana", "Ana").await;
        // Janeiro: reembolso de 10.000.
        add_txn(&p, "ej", "expense", -10000, "2026-01-05", 0, None, None).await;
        add_txn(
            &p,
            "derived:reembolso:ej:0",
            "income",
            10000,
            "2026-01-05",
            0,
            Some("ana"),
            None,
        )
        .await;
        // Março: reembolso de 30.000.
        add_txn(&p, "em", "expense", -30000, "2026-03-05", 0, None, None).await;
        add_txn(
            &p,
            "derived:reembolso:em:0",
            "income",
            30000,
            "2026-03-05",
            0,
            Some("ana"),
            None,
        )
        .await;

        let dto = tags_screen_dto(&p, today, 2026, 3).await.unwrap();
        assert_eq!(dto.verdict.third_party_avg_cents, Some(20000)); // (10.000 + 30.000)/2
        assert_eq!(dto.verdict.third_party_people, 1);
    }

    // Backfill do vínculo de pessoa (migração 20260723000003): derivado legado sem
    // counterparty_person_id recebe o id da pessoa pela descrição; reexecutar não muda o valor.
    #[tokio::test]
    async fn counterparty_backfill_is_idempotent() {
        let p = mem_pool().await;
        add_person(&p, "gio", "Gio").await;
        // Linha legada: descrição no formato do import, vínculo ainda vazio.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection) \
             VALUES ('derived:reembolso:x:0', 'income', 5000, 'Reembolso: Gio', '2026-03-05', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        const BACKFILL: &str = "UPDATE \"transaction\" SET counterparty_person_id = ( \
              SELECT pp.id FROM person pp \
              WHERE LOWER(pp.name) = LOWER(TRIM(substr(description, instr(description, ':') + 1)))) \
             WHERE id LIKE 'derived:reembolso:%' OR id LIKE 'derived:dividir:%'";
        for _ in 0..2 {
            sqlx::query(BACKFILL).execute(&p).await.unwrap();
        }

        let got: (Option<String>,) = sqlx::query_as(
            "SELECT counterparty_person_id FROM \"transaction\" WHERE id = 'derived:reembolso:x:0'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(got.0.as_deref(), Some("gio"));
    }

    // Regressão do pool de 1 conexão (deadlock conhecido): o DTO completo roda sem travar,
    // porque nenhuma query corre com uma transação de escrita aberta.
    #[tokio::test]
    async fn tags_screen_dto_survives_single_connection_pool() {
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        add_person(&p, "ana", "Ana").await;
        add_txn(&p, "e1", "expense", -50000, "2026-03-05", 0, None, None).await;
        add_txn(
            &p,
            "derived:reembolso:e1:0",
            "income",
            50000,
            "2026-03-05",
            0,
            Some("ana"),
            None,
        )
        .await;

        let dto = tags_screen_dto(&p, d("2026-03-31"), 2026, 3).await.unwrap();
        assert_eq!(dto.month, "2026-03");
        let ana = line(&dto.third_parties, "Ana");
        assert_eq!(ana.state, "settled");
    }

    async fn add_series(p: &SqlitePool, id: &str, account: &str, count: i64) {
        sqlx::query(
            "INSERT INTO card_series (id, account_id, description, amount_cents, count, \
             start_cycle_month) VALUES (?1, ?2, 'Parcelado', 1000, ?3, '2026-01')",
        )
        .bind(id)
        .bind(account)
        .bind(count)
        .execute(p)
        .await
        .unwrap();
    }

    async fn add_series_refund(p: &SqlitePool, id: &str, series: &str, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection, \
             refund_series_id) VALUES (?1, 'income', 1000, ?2, 0, ?3)",
        )
        .bind(id)
        .bind(date)
        .bind(series)
        .execute(p)
        .await
        .unwrap();
    }

    // Pessoa com DUAS séries de reembolso vivas no mês: `done` é escopado à própria série
    // (nunca a soma cruzada de todas as séries da pessoa) e a linha representa a série do
    // reembolso mais RECENTE do mês — decisão explícita, não loteria de ordem de GROUP BY.
    #[tokio::test]
    async fn two_series_same_person_keep_scoped_done_and_latest_series() {
        let p = mem_pool().await;
        let today = d("2026-03-31");
        add_person(&p, "tit", "Titular").await;
        add_person(&p, "gio", "Gio").await;
        add_account(&p, "main", "bank", "tit", None).await;
        add_account(&p, "card_g", "credit_card", "gio", Some("main")).await;

        // Série A (3 parcelas): 2 realizadas, a do mês em 08/03.
        add_series(&p, "sa", "card_g", 3).await;
        add_series_refund(&p, "ra1", "sa", "2026-02-10").await;
        add_series_refund(&p, "ra2", "sa", "2026-03-08").await;
        // Série B (5 parcelas): 3 realizadas, a do mês em 20/03 — a mais recente.
        add_series(&p, "sb", "card_g", 5).await;
        add_series_refund(&p, "rb1", "sb", "2026-01-15").await;
        add_series_refund(&p, "rb2", "sb", "2026-02-15").await;
        add_series_refund(&p, "rb3", "sb", "2026-03-20").await;

        let aggs = load_third_parties(&p, today, 2026, 3).await.unwrap();
        let gio = aggs.iter().find(|a| a.name == "Gio").unwrap();
        assert_eq!(
            gio.series,
            Some((3, 5)),
            "a série do reembolso mais recente (B) representa a linha, com done da PRÓPRIA série"
        );
    }

    // `#reembolso:` numa linha de Cartão de conta VINCULADA: o import grava counterparty E
    // refund_invoice_id na mesma Entrada derivada. A fonte CARTÃO é a autoridade — a derivada
    // não pode dobrar em out (linha + fatura) nem em back (derivado + reembolso da fatura).
    #[tokio::test]
    async fn marker_refund_on_linked_card_line_counts_once() {
        let p = mem_pool().await;
        let today = d("2026-03-31");
        add_person(&p, "tit", "Titular").await;
        add_person(&p, "gio", "Gio").await;
        add_account(&p, "main", "bank", "tit", None).await;
        add_account(&p, "card_g", "credit_card", "gio", Some("main")).await;
        add_invoice(&p, "inv_g", "card_g", "2026-03-15", 40000).await;

        // A Entrada derivada do marcador, JÁ ligada à fatura da conta vinculada
        // (o que import::link_card_refunds produz).
        add_txn(
            &p,
            "derived:reembolso:e9:0",
            "income",
            15000,
            "2026-03-05",
            0,
            Some("gio"),
            Some("inv_g"),
        )
        .await;

        let aggs = load_third_parties(&p, today, 2026, 3).await.unwrap();
        let gio = aggs.iter().find(|a| a.name == "Gio").unwrap();
        assert_eq!(
            gio.out_cents, 40000,
            "saiu = só o total efetivo da fatura (a linha marcada vive dentro dela)"
        );
        assert_eq!(gio.back_cents, 15000, "voltou conta UMA vez, nunca dobrado");
    }
}
