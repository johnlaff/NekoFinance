use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Estado da fatura derivado exclusivamente do calendário, para que banco e interface não
/// precisem sincronizar uma cópia perecível desse estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceStatus {
    Prevista,
    Aberta,
    Fechada,
    Paga,
}

impl InvoiceStatus {
    /// Valor estável exposto nas fronteiras que representam o estado derivado da fatura.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Prevista => "prevista",
            Self::Aberta => "aberta",
            Self::Fechada => "fechada",
            Self::Paga => "paga",
        }
    }
}

/// Estado de uma perna determinística da legitimidade do modo cartão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateLeg {
    Alive,
    Below,
    Unknown,
}

impl GateLeg {
    /// Valor estável exposto nos DTOs de resumo do dashboard.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alive => "alive",
            Self::Below => "below",
            Self::Unknown => "unknown",
        }
    }
}

/// Classifica a perna da economia contra o piso da faixa (`SAVINGS_FLOOR_BPS`): sem registro é
/// desconhecida, nunca aprovada por omissão.
pub fn economy_gate_leg(economia_bps: Option<i64>) -> GateLeg {
    match economia_bps {
        None => GateLeg::Unknown,
        Some(bps) if bps >= crate::forecast::SAVINGS_FLOOR_BPS => GateLeg::Alive,
        Some(_) => GateLeg::Below,
    }
}

/// Classifica a perna da reserva contra o mínimo de meses (`RESERVE_MIN_MONTHS`): sem registro é
/// desconhecida, nunca aprovada por omissão.
pub fn reserve_gate_leg(reserve_months: Option<f64>) -> GateLeg {
    match reserve_months {
        None => GateLeg::Unknown,
        Some(months) if months >= crate::forecast::RESERVE_MIN_MONTHS as f64 => GateLeg::Alive,
        Some(_) => GateLeg::Below,
    }
}

/// Combina as pernas de economia e reserva sem transformar ausência de dado em aprovação. Cada
/// perna lê sua própria evidência já resolvida pelo motor (régua anual, meses de reserva) — este
/// módulo só classifica e compõe, nunca recalcula.
pub fn compose_card_gate(economy: GateLeg, reserve: GateLeg) -> GateLeg {
    if matches!(economy, GateLeg::Below) || matches!(reserve, GateLeg::Below) {
        GateLeg::Below
    } else if matches!((economy, reserve), (GateLeg::Alive, GateLeg::Alive)) {
        GateLeg::Alive
    } else {
        GateLeg::Unknown
    }
}

/// Retorna o ano e mês deslocados sem depender de uma data intermediária, que poderia não
/// existir em meses mais curtos.
fn shift_month(year: i32, month: u32, delta: i32) -> Option<(i32, u32)> {
    let absolute_month = i64::from(year) * 12 + i64::from(month) - 1 + i64::from(delta);
    let year = i32::try_from(absolute_month.div_euclid(12)).ok()?;
    let month = u32::try_from(absolute_month.rem_euclid(12) + 1).ok()?;
    Some((year, month))
}

/// Data de fechamento da fatura que recebe a compra.
///
/// Uma compra após o fechamento pertence ao ciclo que fecha no mês seguinte; isso preserva a
/// ordem temporal entre compra, fechamento e vencimento, ao contrário de agrupar a compra no
/// ciclo já encerrado.
pub fn cycle_close_for_purchase(purchase: NaiveDate, closing_day: u32) -> NaiveDate {
    let in_purchase_month = closing_day_in(purchase.year(), purchase.month(), closing_day);
    let (year, month) = if purchase.day() <= in_purchase_month {
        (purchase.year(), purchase.month())
    } else {
        shift_month(purchase.year(), purchase.month(), 1).expect("mês posterior representável")
    };

    NaiveDate::from_ymd_opt(year, month, closing_day_in(year, month, closing_day))
        .expect("dia de fechamento válido")
}

/// O dia em que o fechamento acontece NAQUELE mês. Um cartão que fecha dia 29, 30 ou 31 é comum;
/// o mês curto é problema de derivar a data, não do cadastro — a mesma regra que o vencimento já
/// segue. Encurtar (28/fev) preserva o ciclo; recuar para um 28 fixo empurraria a compra do dia
/// 29 para a fatura seguinte, um mês inteiro de atraso.
pub(crate) fn closing_day_in(year: i32, month: u32, closing_day: u32) -> u32 {
    crate::calendar::clamp_day_of_month(closing_day, year, month)
}

/// Primeiro vencimento estritamente posterior ao fechamento.
///
/// O vencimento no mesmo mês só é possível quando seu dia ainda não passou; o clamp evita que
/// uma preferência de dia 29–31 torne fevereiro e meses curtos impossíveis de representar.
pub fn due_date_for_close(close: NaiveDate, due_day: u32) -> NaiveDate {
    let (mut year, mut month) = if due_day > close.day() {
        (close.year(), close.month())
    } else {
        shift_month(close.year(), close.month(), 1).expect("mês posterior representável")
    };
    let mut day = crate::calendar::clamp_day_of_month(due_day, year, month);
    let mut due = NaiveDate::from_ymd_opt(year, month, day).expect("dia de vencimento válido");

    if due <= close {
        (year, month) = shift_month(year, month, 1).expect("mês posterior representável");
        day = crate::calendar::clamp_day_of_month(due_day, year, month);
        due = NaiveDate::from_ymd_opt(year, month, day).expect("dia de vencimento válido");
    }

    due
}

/// Identidade mensal da fatura, ancorada no mês em que ela vence.
pub fn cycle_month_of(due_date: NaiveDate) -> String {
    format!("{:04}-{:02}", due_date.year(), due_date.month())
}

/// Lê somente a identidade canônica `YYYY-MM`, para que chaves de série ordenem sem ambiguidade.
pub fn parse_cycle_month(s: &str) -> Option<(i32, u32)> {
    let bytes = s.as_bytes();
    if bytes.len() != 7
        || bytes[4] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..].iter().all(u8::is_ascii_digit)
    {
        return None;
    }

    let year = s[..4].parse().ok()?;
    let month = s[5..].parse().ok()?;
    (1..=12).contains(&month).then_some((year, month))
}

/// Desloca a identidade mensal sem passar por um dia arbitrário do calendário.
pub fn add_cycle_months(cycle_month: &str, delta: i32) -> Option<String> {
    let (year, month) = parse_cycle_month(cycle_month)?;
    let (year, month) = shift_month(year, month, delta)?;
    (0..=9_999)
        .contains(&year)
        .then(|| format!("{year:04}-{month:02}"))
}

/// Reconstrói as datas explícitas de uma fatura a partir da sua identidade mensal de vencimento.
/// Isso preserva a chave cartão×mês quando uma fatura ainda não existe, sem depender de compras.
pub fn dates_for_cycle_month(
    cycle_month: &str,
    closing_day: u32,
    due_day: u32,
) -> Option<(NaiveDate, NaiveDate)> {
    let (due_year, due_month) = parse_cycle_month(cycle_month)?;
    let due_date = NaiveDate::from_ymd_opt(
        due_year,
        due_month,
        crate::calendar::clamp_day_of_month(due_day, due_year, due_month),
    )?;
    // O mês do fechamento sai dos dias PEDIDOS — é a intenção do molde. Cada um encurta depois,
    // no seu próprio mês: fechamento e vencimento podem cair em meses de tamanhos diferentes.
    let (closing_year, closing_month) = if closing_day < due_day {
        (due_year, due_month)
    } else {
        shift_month(due_year, due_month, -1)?
    };
    let closing_date = NaiveDate::from_ymd_opt(
        closing_year,
        closing_month,
        closing_day_in(closing_year, closing_month, closing_day),
    )?;
    (closing_date < due_date).then_some((closing_date, due_date))
}

/// Posição 1-based de uma ocorrência em sua série, usada para derivar `n/N` sem persistir
/// informação que pode divergir da ancoragem da série.
pub fn cycle_index(start_cycle_month: &str, cycle_month: &str) -> Option<i64> {
    let (start_year, start_month) = parse_cycle_month(start_cycle_month)?;
    let (cycle_year, cycle_month) = parse_cycle_month(cycle_month)?;
    let start = i64::from(start_year) * 12 + i64::from(start_month);
    let cycle = i64::from(cycle_year) * 12 + i64::from(cycle_month);
    (cycle >= start).then_some(cycle - start + 1)
}

/// Primeiro dia do ciclo que termina em `closing_date`, respeitando meses com tamanhos distintos.
pub fn cycle_start(closing_date: NaiveDate) -> NaiveDate {
    let (year, month) = shift_month(closing_date.year(), closing_date.month(), -1)
        .expect("mês anterior representável");
    let previous_day = closing_date
        .day()
        .min(crate::calendar::last_day_of_month(year, month).day());
    NaiveDate::from_ymd_opt(year, month, previous_day)
        .and_then(|date| date.succ_opt())
        .expect("início de ciclo representável")
}

/// Classifica a fatura pelo calendário, sem armazenar status que poderia divergir das datas.
pub fn invoice_status(
    today: NaiveDate,
    closing_date: NaiveDate,
    due_date: NaiveDate,
) -> InvoiceStatus {
    if today > due_date {
        InvoiceStatus::Paga
    } else if today > closing_date {
        InvoiceStatus::Fechada
    } else if today >= cycle_start(closing_date) {
        InvoiceStatus::Aberta
    } else {
        InvoiceStatus::Prevista
    }
}

/// Total exibido e usado pela fatura: o valor declarado resolve importações e ajustes manuais.
pub fn effective_total_cents(stated_total_cents: Option<i64>, purchases_sum_cents: i64) -> i64 {
    stated_total_cents.unwrap_or(purchases_sum_cents)
}

/// Diferença visível de reconciliação; ela não altera nem substitui nenhuma compra vinculada.
pub fn reconciliation_delta_cents(
    stated_total_cents: Option<i64>,
    purchases_sum_cents: i64,
) -> Option<i64> {
    stated_total_cents
        .filter(|stated| *stated != purchases_sum_cents)
        .map(|stated| stated - purchases_sum_cents)
}

/// Normaliza aliases pela mesma regra que interpreta seções importadas, para que texto com
/// caixa, acentos ou dois-pontos não crie identidades paralelas.
pub(crate) fn normalize_alias(s: &str) -> String {
    crate::google_sheets::import::normalize_item_section(s)
}

/// Formas com que uma linha de nota se apresenta como fatura de cartão. Do mais específico ao
/// mais genérico: `fatura ` sozinho consumiria só a primeira palavra de "fatura cartão X".
const INVOICE_PREFIXES: [&str; 3] = ["fatura do cartao ", "fatura cartao ", "fatura "];

/// Alias que uma linha de nota declara: o texto antes do marcador `#`, normalizado.
pub(crate) fn declared_alias(description: &str) -> String {
    normalize_alias(description.split('#').next().unwrap_or("").trim())
}

/// Raiz de um alias: o nome antes de um sufixo entre parênteses. A planilha distingue ciclos
/// do mesmo cartão no próprio nome (`Nubank (26/09)`), e essa distinção é anotação humana, não
/// identidade — a raiz é o que agrupa. Um nome que é só o parêntese não tem raiz para agrupar.
pub(crate) fn root_alias(alias: &str) -> String {
    let Some(open) = alias.find('(') else {
        return alias.to_string();
    };
    let root = alias[..open].trim();
    if root.is_empty() {
        alias.to_string()
    } else {
        root.to_string()
    }
}

/// Rótulo exibido para a raiz de um nome, preservando caixa e acento do texto original — o que
/// `root_alias` faz com a identidade, este faz com a apresentação.
pub(crate) fn root_display(name: &str) -> &str {
    let name = name.split('#').next().unwrap_or("").trim();
    match name.find('(') {
        Some(open) if !name[..open].trim().is_empty() => name[..open].trim(),
        _ => name,
    }
}

/// `true` quando a linha se apresenta como fatura de cartão. Insumo exclusivo de DIAGNÓSTICO:
/// reconhecer o formato não classifica dinheiro — só a seção e o léxico fazem isso.
pub(crate) fn looks_like_invoice_line(description: &str) -> bool {
    let alias = declared_alias(description);
    INVOICE_PREFIXES
        .iter()
        .any(|prefix| alias.starts_with(prefix))
}

/// Identidades de cartão que o domínio já conhece, na forma que resolve a linha de uma nota.
///
/// Existe porque a planilha declara a fatura de duas maneiras: sob o cabeçalho de seção
/// (`CARTÕES`) e, quando o dono esqueceu o cabeçalho, como uma linha comum que nomeia o cartão
/// (`Fatura Bradesco`). A segunda forma só é reconhecível contra identidades que o próprio dono
/// já declarou — nunca por palavra-chave de banco ou emissor, que classificaria "Fatura Vivo"
/// como cartão.
///
/// `T` é o que o chamador precisa da identidade: `account_id` na leitura de eventos, alias
/// canônico na varredura do import.
#[derive(Debug, Clone)]
pub(crate) struct CardLexicon<T> {
    by_alias: HashMap<String, T>,
}

impl<T: Clone + PartialEq> CardLexicon<T> {
    /// Indexa cada alias declarado e, quando ela não é ambígua, a sua raiz — é a raiz que faz
    /// "Fatura Nubank" alcançar o cartão que a planilha declarou como "Nubank (26/09)". Raiz
    /// compartilhada por identidades diferentes não vira atalho: ambiguidade não se resolve só.
    pub(crate) fn from_entries(entries: impl IntoIterator<Item = (String, T)>) -> Self {
        let mut by_alias: HashMap<String, T> = HashMap::new();
        let mut by_root: HashMap<String, Option<T>> = HashMap::new();
        for (alias, value) in entries {
            if alias.is_empty() {
                continue;
            }
            let root = root_alias(&alias);
            if root != alias {
                by_root
                    .entry(root)
                    .and_modify(|current| {
                        if current.as_ref() != Some(&value) {
                            *current = None;
                        }
                    })
                    .or_insert_with(|| Some(value.clone()));
            }
            by_alias.insert(alias, value);
        }
        for (root, value) in by_root {
            // Um alias declarado com esse mesmo texto sempre vence o atalho pela raiz.
            if let Some(value) = value
                && !by_alias.contains_key(&root)
            {
                by_alias.insert(root, value);
            }
        }
        Self { by_alias }
    }

    /// A identidade que a descrição nomeia, ou `None` quando a linha não nomeia cartão conhecido.
    pub(crate) fn resolve(&self, description: &str) -> Option<T> {
        let alias = declared_alias(description);
        if alias.is_empty() {
            return None;
        }
        if let Some(value) = self.by_alias.get(&alias) {
            return Some(value.clone());
        }
        // Só a linha que se apresenta como fatura tenta de novo sem o prefixo: mencionar o
        // emissor ("Seguro Bradesco") nunca transforma uma saída comum em fatura.
        let named = INVOICE_PREFIXES
            .iter()
            .find_map(|prefix| alias.strip_prefix(prefix))?
            .trim();
        self.by_alias.get(named).cloned()
    }
}

/// `true` quando ao menos um cartão está configurado — insumo da relabelagem de crédito órfão
/// (compra crua sem fatura vinculada vira Saída fixa só quando há cartão no domínio).
pub(crate) async fn has_any_card_account(pool: &SqlitePool) -> Result<bool, String> {
    sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM account WHERE type = 'credit_card')")
        .fetch_one(pool)
        .await
        .map(|n| n != 0)
        .map_err(|e| format!("cartões: {e}"))
}

/// Vincula compras de crédito legadas à fatura do ciclo correto quando há exatamente um cartão
/// configurado. A fatura é a identidade persistida do vencimento; o backfill só estabelece a FK e
/// preserva o total declarado, que pode já refletir a planilha importada.
pub async fn backfill_legacy_credit_purchases(pool: &SqlitePool) -> Result<(), String> {
    let (card_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM account WHERE type = 'credit_card'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("contar cartões para backfill: {e}"))?;
    if card_count != 1 {
        return Ok(());
    }

    let card: Option<(String, i64, i64)> = sqlx::query_as(
        "SELECT id, closing_day, due_day FROM account \
         WHERE type = 'credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("backfill cartão: {e}"))?;
    let Some((account_id, closing_day, due_day)) = card else {
        return Ok(());
    };

    let purchases: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, date FROM \"transaction\" \
         WHERE type = 'expense' AND payment_method = 'credit' AND invoice_id IS NULL \
           AND scenario_id IS NULL ORDER BY date, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("backfill compras: {e}"))?;
    if purchases.is_empty() {
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("backfill cartão (begin): {e}"))?;
    for (purchase_id, purchase_date) in purchases {
        let purchase = NaiveDate::parse_from_str(&purchase_date, "%Y-%m-%d")
            .map_err(|_| format!("data de compra de crédito inválida: {purchase_date}"))?;
        let closing = cycle_close_for_purchase(purchase, closing_day as u32);
        let due = due_date_for_close(closing, due_day as u32);
        let cycle_month = cycle_month_of(due);
        let invoice_id: Option<(String,)> =
            sqlx::query_as("SELECT id FROM invoice WHERE account_id = ?1 AND cycle_month = ?2")
                .bind(&account_id)
                .bind(&cycle_month)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("backfill buscar fatura: {e}"))?;
        if let Some((invoice_id,)) = invoice_id {
            sqlx::query(
                "UPDATE \"transaction\" SET invoice_id = ?1 WHERE id = ?2 AND invoice_id IS NULL",
            )
            .bind(&invoice_id)
            .bind(&purchase_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("backfill vincular compra: {e}"))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| format!("backfill cartão (commit): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    fn d(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("data de teste válida")
    }

    /// Léxico de teste que mapeia cada alias declarado para ele mesmo.
    fn known(aliases: &[&str]) -> CardLexicon<String> {
        CardLexicon::from_entries(aliases.iter().map(|a| ((*a).to_string(), (*a).to_string())))
    }

    // --- ciclo com dia que não cabe em todo mês -------------------------------------------

    /// Um cartão que fecha dia 29, 30 ou 31 é comum; fevereiro é problema da DERIVAÇÃO da data,
    /// não do cadastro. A regra é a mesma já aplicada ao vencimento: encurtar para o último dia
    /// do mês, nunca recuar para um dia 28 fixo que atrasaria a compra um ciclo inteiro.
    #[test]
    fn closing_day_past_the_short_month_shortens_to_its_last_day() {
        // Fevereiro comum: o fechamento do dia 29 acontece no dia 28.
        assert_eq!(
            cycle_close_for_purchase(d("2026-02-10"), 29),
            d("2026-02-28")
        );
        // Bissexto: o mesmo fechamento cabe no dia 29.
        assert_eq!(
            cycle_close_for_purchase(d("2028-02-10"), 29),
            d("2028-02-29")
        );
        // Mês de 30 dias com fechamento no 31.
        assert_eq!(
            cycle_close_for_purchase(d("2026-04-10"), 31),
            d("2026-04-30")
        );
        // Mês longo: o dia pedido é o dia usado.
        assert_eq!(
            cycle_close_for_purchase(d("2026-03-10"), 29),
            d("2026-03-29")
        );
    }

    #[test]
    fn a_purchase_on_the_shortened_closing_day_still_belongs_to_that_cycle() {
        // 28/fev É o fechamento de fevereiro quando o molde diz 29 — a compra desse dia entra na
        // fatura que fecha ali, não na seguinte.
        assert_eq!(
            cycle_close_for_purchase(d("2026-02-28"), 29),
            d("2026-02-28")
        );
        // Depois do fechamento, a compra pertence ao ciclo seguinte.
        assert_eq!(
            cycle_close_for_purchase(d("2026-03-30"), 29),
            d("2026-04-29")
        );
    }

    #[test]
    fn cycle_dates_reconstruct_with_a_closing_day_past_the_short_month() {
        // Fecha 29 do mês anterior, vence 12 — o ciclo do João, em fevereiro.
        assert_eq!(
            dates_for_cycle_month("2026-02", 29, 12),
            Some((d("2026-01-29"), d("2026-02-12")))
        );
        // Vencimento em março: o fechamento cai no fevereiro curto e encurta.
        assert_eq!(
            dates_for_cycle_month("2026-03", 29, 12),
            Some((d("2026-02-28"), d("2026-03-12")))
        );
        assert_eq!(
            dates_for_cycle_month("2028-03", 29, 12),
            Some((d("2028-02-29"), d("2028-03-12")))
        );
    }

    // --- alias declarado pela linha -------------------------------------------------------

    #[test]
    fn declared_alias_drops_the_marker_and_normalizes_case_and_accent() {
        assert_eq!(declared_alias("Itaú"), "itau");
        assert_eq!(declared_alias("Bradesco João #reembolso"), "bradesco joao");
        assert_eq!(declared_alias("  Mercado Pago  "), "mercado pago");
        assert_eq!(declared_alias("#só marcador"), "");
    }

    // --- raiz do alias (agrupa apelidos variantes) ----------------------------------------

    #[test]
    fn root_alias_drops_a_parenthesized_suffix() {
        assert_eq!(root_alias("nubank (26/02)"), "nubank");
        assert_eq!(root_alias("itau (usei a feature virar fatura)"), "itau");
        assert_eq!(root_alias("nubank"), "nubank");
    }

    #[test]
    fn root_alias_keeps_a_name_that_is_only_a_parenthesis() {
        // Sem raiz antes do parêntese não há o que agrupar: a identidade original permanece.
        assert_eq!(root_alias("(26/02)"), "(26/02)");
    }

    // --- resolução contra identidades já declaradas ---------------------------------------

    #[test]
    fn resolves_a_line_that_names_a_known_card_directly() {
        let lexicon = known(&["nubank", "bradesco"]);
        assert_eq!(lexicon.resolve("Nubank"), Some("nubank".to_string()));
    }

    #[test]
    fn resolves_an_invoice_line_written_outside_the_cards_section() {
        // O caso que fazia a fatura sumir: a nota do mês não tem cabeçalho de seção e a linha
        // se apresenta como "Fatura <cartão>".
        let lexicon = known(&["bradesco", "amazon", "nubank"]);
        for (line, expected) in [
            ("Fatura Bradesco", "bradesco"),
            ("Fatura Amazon", "amazon"),
            ("Fatura Cartão Amazon", "amazon"),
            ("Fatura do Cartão Nubank", "nubank"),
        ] {
            assert_eq!(
                lexicon.resolve(line),
                Some(expected.to_string()),
                "{line} nomeia um cartão declarado"
            );
        }
    }

    #[test]
    fn does_not_resolve_a_line_that_merely_mentions_the_issuer() {
        // A âncora da regra: sem prefixo de fatura, mencionar o emissor não faz da linha uma
        // fatura. "Seguro Bradesco" e "Festa Junina Bradesco" são saídas comuns.
        let lexicon = known(&["bradesco", "inter", "nubank"]);
        for line in [
            "Seguro Bradesco",
            "Festa Junina Bradesco",
            "Rendimentos Bradesco",
            "Dinheiro do Inter",
            "Empréstimo Nubank pago",
        ] {
            assert_eq!(lexicon.resolve(line), None, "{line} não é uma fatura");
        }
    }

    #[test]
    fn does_not_resolve_an_invoice_of_a_card_nobody_declared() {
        // Sem heurística de emissor: um nome que o dono nunca declarou como cartão continua
        // fora do balde, por mais que a linha se apresente como fatura.
        let lexicon = known(&["bradesco"]);
        assert_eq!(lexicon.resolve("Fatura Sicoob"), None);
        assert_eq!(lexicon.resolve("Fatura Vivo"), None);
    }

    #[test]
    fn resolves_through_the_root_when_only_the_variant_was_declared() {
        // O léxico guarda "nubank (26/09)" porque foi assim que a planilha declarou; uma linha
        // "Fatura Nubank" ainda nomeia esse cartão.
        let lexicon = known(&["nubank (26/09)"]);
        assert_eq!(
            lexicon.resolve("Fatura Nubank"),
            Some("nubank (26/09)".to_string())
        );
    }

    #[test]
    fn an_invoice_prefix_alone_names_no_card() {
        let lexicon = known(&["bradesco"]);
        assert_eq!(lexicon.resolve("Fatura"), None);
        assert_eq!(lexicon.resolve("Fatura "), None);
    }

    #[test]
    fn looks_like_an_invoice_line_only_with_the_prefix() {
        // Insumo do diagnóstico: reporta, nunca decide.
        assert!(looks_like_invoice_line("Fatura Sicoob"));
        assert!(looks_like_invoice_line("fatura cartão XPTO"));
        assert!(!looks_like_invoice_line("Seguro Bradesco"));
        assert!(!looks_like_invoice_line("Aluguel"));
    }

    #[test]
    fn cycle_close_for_purchase_uses_the_current_or_next_closing_date() {
        assert_eq!(
            cycle_close_for_purchase(d("2026-01-15"), 20),
            d("2026-01-20")
        );
        assert_eq!(
            cycle_close_for_purchase(d("2026-01-25"), 20),
            d("2026-02-20")
        );
        assert_eq!(
            cycle_close_for_purchase(d("2026-12-25"), 20),
            d("2027-01-20")
        );
        // Fechamento no dia 31: a compra do próprio 31 pertence ao ciclo que fecha ali. O mês
        // curto só encurta o dia quando o ciclo cai nele — nunca empurra a compra um ciclo à
        // frente.
        assert_eq!(
            cycle_close_for_purchase(d("2026-01-31"), 31),
            d("2026-01-31")
        );
        assert_eq!(
            cycle_close_for_purchase(d("2026-02-28"), 31),
            d("2026-02-28")
        );
        assert_eq!(
            cycle_close_for_purchase(d("2026-01-15"), 0),
            d("2026-02-01")
        );
    }

    #[test]
    fn due_date_for_close_is_the_first_due_day_strictly_after_closing() {
        assert_eq!(due_date_for_close(d("2026-01-20"), 10), d("2026-02-10"));
        assert_eq!(due_date_for_close(d("2026-01-05"), 25), d("2026-01-25"));
        assert_eq!(due_date_for_close(d("2026-12-20"), 10), d("2027-01-10"));
        assert_eq!(due_date_for_close(d("2026-01-31"), 31), d("2026-02-28"));
    }

    #[test]
    fn due_date_for_close_moves_past_a_february_clamp_that_would_equal_closing() {
        let close = d("2026-02-28");

        assert_eq!(due_date_for_close(close, 29), d("2026-03-29"));
        assert_eq!(due_date_for_close(close, 30), d("2026-03-30"));
        assert_eq!(due_date_for_close(close, 31), d("2026-03-31"));
    }

    #[test]
    fn purchase_cycle_due_date_stays_strictly_after_its_closing_date_after_february_clamp() {
        let close = cycle_close_for_purchase(d("2026-01-29"), 28);
        let due = due_date_for_close(close, 29);

        assert_eq!(close, d("2026-02-28"));
        assert_eq!(due, d("2026-03-29"));
        assert!(due > close);
    }

    #[test]
    fn purchase_after_closing_has_a_due_date_after_the_purchase() {
        let close = cycle_close_for_purchase(d("2026-01-25"), 20);
        assert_eq!(due_date_for_close(close, 10), d("2026-03-10"));
    }

    #[test]
    fn invoice_status_covers_ranges_and_exact_boundaries() {
        let closing = d("2026-02-20");
        let due = d("2026-03-10");
        let start = d("2026-01-21");

        assert_eq!(
            invoice_status(d("2026-01-20"), closing, due),
            InvoiceStatus::Prevista
        );
        assert_eq!(invoice_status(start, closing, due), InvoiceStatus::Aberta);
        assert_eq!(invoice_status(closing, closing, due), InvoiceStatus::Aberta);
        assert_eq!(
            invoice_status(d("2026-02-21"), closing, due),
            InvoiceStatus::Fechada
        );
        assert_eq!(invoice_status(due, closing, due), InvoiceStatus::Fechada);
        assert_eq!(
            invoice_status(d("2026-03-11"), closing, due),
            InvoiceStatus::Paga
        );
        assert_eq!(InvoiceStatus::Prevista.as_str(), "prevista");
        assert_eq!(InvoiceStatus::Aberta.as_str(), "aberta");
        assert_eq!(InvoiceStatus::Fechada.as_str(), "fechada");
        assert_eq!(InvoiceStatus::Paga.as_str(), "paga");
    }

    #[test]
    fn cycle_index_is_one_based_and_does_not_precede_the_series() {
        assert_eq!(cycle_index("2026-03", "2026-03"), Some(1));
        assert_eq!(cycle_index("2026-03", "2026-07"), Some(5));
        assert_eq!(cycle_index("2026-11", "2027-02"), Some(4));
        assert_eq!(cycle_index("2026-03", "2026-02"), None);
    }

    #[test]
    fn cycle_month_helpers_validate_and_move_months() {
        assert_eq!(cycle_month_of(d("2026-03-10")), "2026-03");
        assert_eq!(add_cycle_months("2026-11", 3), Some("2027-02".to_owned()));
        assert_eq!(add_cycle_months("2026-03", -4), Some("2025-11".to_owned()));
        assert_eq!(parse_cycle_month("2026-13"), None);
        assert_eq!(parse_cycle_month("2026-1"), None);
        assert_eq!(parse_cycle_month("lixo"), None);
    }

    #[test]
    fn dates_for_cycle_month_round_trips_the_purchase_cycle() {
        let purchase = d("2026-01-25");
        let closing = cycle_close_for_purchase(purchase, 20);
        let due = due_date_for_close(closing, 10);
        assert_eq!(
            dates_for_cycle_month(&cycle_month_of(due), 20, 10),
            Some((closing, due))
        );
    }

    #[test]
    fn stated_total_is_authoritative_and_reconciliation_only_exists_for_divergence() {
        assert_eq!(effective_total_cents(Some(1_200), 1_000), 1_200);
        assert_eq!(effective_total_cents(None, 1_000), 1_000);
        assert_eq!(reconciliation_delta_cents(Some(1_000), 1_000), None);
        assert_eq!(reconciliation_delta_cents(Some(1_200), 1_000), Some(200));
        assert_eq!(reconciliation_delta_cents(None, 1_000), None);
    }

    #[test]
    fn card_gate_composition_follows_the_full_three_state_matrix() {
        use GateLeg::{Alive, Below, Unknown};

        for (economy, reserve, expected) in [
            (Alive, Alive, Alive),
            (Alive, Below, Below),
            (Alive, Unknown, Unknown),
            (Below, Alive, Below),
            (Below, Below, Below),
            (Below, Unknown, Below),
            (Unknown, Alive, Unknown),
            (Unknown, Below, Below),
            (Unknown, Unknown, Unknown),
        ] {
            assert_eq!(compose_card_gate(economy, reserve), expected);
        }
    }

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool SQLite em memória");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações aplicadas");
        pool
    }

    #[tokio::test]
    async fn card_domain_migration_creates_invoice_constraints_and_transaction_links() {
        let pool = pool().await;

        for table in ["invoice", "card_series", "card_alias", "card_proposal"] {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(exists, 1, "tabela {table} existe");
        }
        let transaction_links: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('transaction') \
             WHERE name IN ('invoice_id', 'card_series_id', 'refund_invoice_id', \
                            'refund_txn_id', 'refund_series_id')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(transaction_links, 5);

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id) \
             VALUES ('card-1', 'Cartão', 'credit_card', 'person-1')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('invoice-1', 'card-1', '2026-03', '2026-02-20', '2026-03-10')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let duplicate = sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('invoice-2', 'card-1', '2026-03', '2026-02-20', '2026-03-10')",
        )
        .execute(&pool)
        .await;
        assert!(duplicate.is_err());

        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, is_projection, invoice_id, refund_invoice_id) \
             VALUES ('transaction-1', 'income', 1_000, '2026-03-10', 0, 'invoice-1', 'invoice-1')",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn legacy_credit_purchase_backfill_links_the_next_cycle_without_changing_stated_total() {
        let pool = pool().await;
        let year = chrono::Local::now().year() + 1;
        let purchase = NaiveDate::from_ymd_opt(year, 1, 25).unwrap();
        let closing = cycle_close_for_purchase(purchase, 20);
        let due = due_date_for_close(closing, 10);
        let cycle_month = cycle_month_of(due);

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-1', 'Cartão', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
               (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-1', 'card-1', ?1, ?2, ?3, 99_999)",
        )
        .bind(&cycle_month)
        .bind(closing.to_string())
        .bind(due.to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
               (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES ('legacy-purchase', 'expense', 1_234, ?1, 'credit', 0, 0)",
        )
        .bind(purchase.to_string())
        .execute(&pool)
        .await
        .unwrap();

        backfill_legacy_credit_purchases(&pool).await.unwrap();
        let linked: (String, Option<i64>) = sqlx::query_as(
            "SELECT invoice_id, (SELECT stated_total_cents FROM invoice WHERE id = invoice_id) \
             FROM \"transaction\" WHERE id = 'legacy-purchase'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(linked, ("invoice-1".into(), Some(99_999)));

        backfill_legacy_credit_purchases(&pool).await.unwrap();
        let linked_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id = 'legacy-purchase' AND invoice_id = 'invoice-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            linked_count, 1,
            "reexecutar não altera a compra já vinculada"
        );
    }

    #[tokio::test]
    async fn legacy_credit_purchase_backfill_leaves_purchase_unlinked_with_multiple_cards() {
        let pool = pool().await;
        let year = chrono::Local::now().year() + 1;
        let purchase = NaiveDate::from_ymd_opt(year, 1, 25).unwrap();
        let closing = cycle_close_for_purchase(purchase, 20);
        let due = due_date_for_close(closing, 10);

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account \
             (id, name, type, owner_person_id, closing_day, due_day, created_at) \
             VALUES ('visa', 'Visa', 'credit_card', 'person-1', 20, 10, '2026-01-01T00:00:00Z'), \
                    ('mastercard', 'Mastercard', 'credit_card', 'person-1', 20, 10, '2026-01-02T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice \
             (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('visa-invoice', 'visa', ?1, ?2, ?3)",
        )
        .bind(cycle_month_of(due))
        .bind(closing.to_string())
        .bind(due.to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES ('orphan-credit', 'expense', 1_234, ?1, 'credit', 0, 0)",
        )
        .bind(purchase.to_string())
        .execute(&pool)
        .await
        .unwrap();

        backfill_legacy_credit_purchases(&pool).await.unwrap();

        let invoice_id: Option<String> =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id = 'orphan-credit'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            invoice_id, None,
            "sem evidência, compra não escolhe um cartão"
        );
    }

    #[tokio::test]
    async fn legacy_credit_purchase_backfill_leaves_credit_without_any_card_untouched() {
        let pool = pool().await;
        sqlx::query(
            "INSERT INTO \"transaction\" \
               (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES ('legacy-purchase', 'expense', 1_234, '2030-01-25', 'credit', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        backfill_legacy_credit_purchases(&pool).await.unwrap();
        let invoice_id: Option<String> = sqlx::query_scalar(
            "SELECT invoice_id FROM \"transaction\" WHERE id = 'legacy-purchase'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(invoice_id, None);
    }

    #[tokio::test]
    async fn legacy_credit_purchase_backfill_does_not_create_an_invoice_without_a_scanned_invoice()
    {
        let pool = pool().await;
        let year = chrono::Local::now().year() + 1;
        let purchase = NaiveDate::from_ymd_opt(year, 1, 25).unwrap();

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-1', 'Pessoa')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, closing_day, due_day) \
             VALUES ('card-1', 'Cartão', 'credit_card', 'person-1', 20, 10)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" \
               (id, type, amount, date, payment_method, is_fixed, is_projection) \
             VALUES ('legacy-purchase', 'expense', 1_234, ?1, 'credit', 0, 0)",
        )
        .bind(purchase.to_string())
        .execute(&pool)
        .await
        .unwrap();

        backfill_legacy_credit_purchases(&pool).await.unwrap();
        let invoice_id: Option<String> = sqlx::query_scalar(
            "SELECT invoice_id FROM \"transaction\" WHERE id = 'legacy-purchase'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            invoice_id, None,
            "sem fatura escaneada, a compra permanece solta"
        );
        let invoices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(invoices, 0, "o backfill nunca fabrica uma fatura");

        backfill_legacy_credit_purchases(&pool).await.unwrap();
        let invoices_after_retry: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(invoices_after_retry, 0, "reexecutar sem fatura é no-op");
    }

    #[test]
    fn gate_leg_as_str_covers_every_variant() {
        assert_eq!(GateLeg::Alive.as_str(), "alive");
        assert_eq!(GateLeg::Below.as_str(), "below");
        assert_eq!(GateLeg::Unknown.as_str(), "unknown");
    }

    // Tabela 3×3: cada perna sem registro é DESCONHECIDA, nunca aprovada por omissão; qualquer
    // perna abaixo derruba o gate inteiro, e só a dupla viva libera o modo cartão.
    #[test]
    fn card_gate_composes_the_two_legs_as_a_three_by_three_table() {
        let alive_savings = Some(crate::forecast::SAVINGS_FLOOR_BPS);
        let below_savings = Some(crate::forecast::SAVINGS_FLOOR_BPS - 1);
        let alive_reserve = Some(crate::forecast::RESERVE_MIN_MONTHS as f64);
        let below_reserve = Some(crate::forecast::RESERVE_MIN_MONTHS as f64 - 0.1);

        let gate = |economia_bps: Option<i64>, reserve_months: Option<f64>| {
            compose_card_gate(
                economy_gate_leg(economia_bps),
                reserve_gate_leg(reserve_months),
            )
        };

        assert_eq!(
            gate(alive_savings, alive_reserve),
            GateLeg::Alive,
            "economia viva + reserva viva"
        );
        assert_eq!(
            gate(alive_savings, below_reserve),
            GateLeg::Below,
            "economia viva + reserva abaixo"
        );
        assert_eq!(
            gate(alive_savings, None),
            GateLeg::Unknown,
            "economia viva + reserva sem registro"
        );
        assert_eq!(
            gate(below_savings, alive_reserve),
            GateLeg::Below,
            "economia abaixo + reserva viva"
        );
        assert_eq!(
            gate(below_savings, below_reserve),
            GateLeg::Below,
            "economia abaixo + reserva abaixo"
        );
        assert_eq!(
            gate(below_savings, None),
            GateLeg::Below,
            "economia abaixo + reserva sem registro"
        );
        assert_eq!(
            gate(None, alive_reserve),
            GateLeg::Unknown,
            "economia sem registro + reserva viva"
        );
        assert_eq!(
            gate(None, below_reserve),
            GateLeg::Below,
            "economia sem registro + reserva abaixo"
        );
        assert_eq!(
            gate(None, None),
            GateLeg::Unknown,
            "economia sem registro + reserva sem registro"
        );
    }
}
