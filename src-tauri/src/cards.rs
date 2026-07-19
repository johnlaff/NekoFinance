use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;

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
    let closing_day = closing_day.clamp(1, 28);
    let (year, month) = if purchase.day() <= closing_day {
        (purchase.year(), purchase.month())
    } else {
        shift_month(purchase.year(), purchase.month(), 1).expect("mês posterior representável")
    };

    NaiveDate::from_ymd_opt(year, month, closing_day).expect("dia de fechamento válido")
}

/// Primeiro vencimento estritamente posterior ao fechamento.
///
/// O vencimento no mesmo mês só é possível quando seu dia ainda não passou; o clamp evita que
/// uma preferência de dia 29–31 torne fevereiro e meses curtos impossíveis de representar.
pub fn due_date_for_close(close: NaiveDate, due_day: u32) -> NaiveDate {
    let (year, month) = if due_day > close.day() {
        (close.year(), close.month())
    } else {
        shift_month(close.year(), close.month(), 1).expect("mês posterior representável")
    };
    let day = due_day.clamp(1, crate::forecast::last_day_of_month(year, month).day());

    NaiveDate::from_ymd_opt(year, month, day).expect("dia de vencimento válido")
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
    let due_day = due_day.clamp(
        1,
        crate::forecast::last_day_of_month(due_year, due_month).day(),
    );
    let due_date = NaiveDate::from_ymd_opt(due_year, due_month, due_day)?;
    let closing_day = closing_day.clamp(1, 28);
    let (closing_year, closing_month) = if closing_day < due_day {
        (due_year, due_month)
    } else {
        shift_month(due_year, due_month, -1)?
    };
    let closing_date = NaiveDate::from_ymd_opt(closing_year, closing_month, closing_day)?;
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
        .min(crate::forecast::last_day_of_month(year, month).day());
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

/// Vincula compras de crédito legadas à fatura do ciclo correto quando existe um cartão titular
/// configurado. A fatura é a identidade persistida do vencimento; o backfill só estabelece a FK e
/// preserva o total declarado, que pode já refletir a planilha importada.
pub async fn backfill_legacy_credit_purchases(pool: &SqlitePool) -> Result<(), String> {
    let card: Option<(String, i64, i64)> = sqlx::query_as(
        "SELECT id, closing_day, due_day FROM account \
         WHERE type = 'credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id LIMIT 1",
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
        let invoice_id = match invoice_id {
            Some((id,)) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(&id)
                .bind(&account_id)
                .bind(&cycle_month)
                .bind(closing.to_string())
                .bind(due.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("backfill criar fatura: {e}"))?;
                id
            }
        };
        sqlx::query(
            "UPDATE \"transaction\" SET invoice_id = ?1 WHERE id = ?2 AND invoice_id IS NULL",
        )
        .bind(&invoice_id)
        .bind(&purchase_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("backfill vincular compra: {e}"))?;
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
        assert_eq!(
            cycle_close_for_purchase(d("2026-01-31"), 31),
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
    async fn legacy_credit_purchase_backfill_creates_the_invoice_for_the_next_cycle() {
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
        let invoice: (String, String, String, Option<i64>) = sqlx::query_as(
            "SELECT i.cycle_month, i.closing_date, i.due_date, i.stated_total_cents \
             FROM invoice i JOIN \"transaction\" t ON t.invoice_id = i.id \
             WHERE t.id = 'legacy-purchase'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(invoice.0, format!("{year}-03"));
        assert_eq!(invoice.1, format!("{year}-02-20"));
        assert_eq!(invoice.2, format!("{year}-03-10"));
        assert_eq!(invoice.3, None, "o vínculo não inventa total declarado");
    }
}
