use super::*;
use crate::cards;
use sqlx::{SqliteConnection, SqlitePool};

#[derive(serde::Serialize, Clone)]
pub struct InvoiceSummaryDto {
    pub id: String,
    pub cycle_month: String,
    pub closing_date: String,
    pub due_date: String,
    pub status: String,
    pub stated_total_cents: Option<i64>,
    pub purchases_sum_cents: i64,
    pub effective_total_cents: i64,
    pub reconciliation_delta_cents: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct CardDto {
    pub id: String,
    pub name: String,
    pub institution: Option<String>,
    pub owner_name: String,
    pub linked_account_id: Option<String>,
    pub closing_day: u32,
    pub due_day: u32,
    pub credit_limit_cents: Option<i64>,
    pub aliases: Vec<String>,
    pub open_invoice: Option<InvoiceSummaryDto>,
    pub next_due: Option<InvoiceSummaryDto>,
}

#[derive(serde::Serialize)]
pub struct CardPurchaseDto {
    pub txn_id: String,
    pub date: String,
    pub description: String,
    pub amount_cents: i64,
    pub owner_name: String,
    pub series_id: Option<String>,
    pub installment_label: Option<String>,
    pub is_projection: bool,
}

#[derive(serde::Serialize)]
pub struct RefundDto {
    pub txn_id: String,
    pub date: String,
    pub amount_cents: i64,
    pub description: String,
    pub is_projection: bool,
}

#[derive(serde::Serialize)]
pub struct SubInvoiceDto {
    pub account_id: String,
    pub card_name: String,
    pub owner_name: String,
    pub effective_total_cents: i64,
}

#[derive(serde::Serialize)]
pub struct InvoiceDetailDto {
    #[serde(flatten)]
    pub summary: InvoiceSummaryDto,
    pub purchases: Vec<CardPurchaseDto>,
    pub refunds: Vec<RefundDto>,
    pub sub_invoices: Vec<SubInvoiceDto>,
    pub emitter_total_cents: i64,
}

#[derive(serde::Serialize)]
pub struct CardProposalDto {
    pub id: String,
    pub alias: String,
    pub display_name: String,
    pub source_month: String,
    pub status: String,
    /// Todas as formas com que a planilha nomeia este cartão, a identidade inclusa. O cadastro
    /// nasce reconhecendo as variantes de ciclo em vez de propor um cartão por anotação.
    pub aliases: Vec<String>,
}

type InvoiceRow = (String, String, String, String, Option<i64>);
type HolderRow = (String, Option<String>, Option<i64>, Option<i64>);
type CardListRow = (
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<i64>,
);
type InvoiceDetailRow = (
    String,
    String,
    String,
    String,
    Option<i64>,
    String,
    Option<String>,
);
type PurchaseRow = (
    String,
    String,
    String,
    i64,
    Option<String>,
    i64,
    Option<i64>,
    Option<String>,
);
type SeriesRow = (String, String, Option<i64>, String, Option<String>);

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "data inválida".into())
}

fn validate_cycle(closing_day: Option<i64>, due_day: Option<i64>) -> Result<(u32, u32), String> {
    let closing = closing_day.ok_or("fechamento obrigatório")?;
    let due = due_day.ok_or("vencimento obrigatório")?;
    if !(1..=31).contains(&closing) {
        return Err("fechamento deve ser entre 1 e 31".into());
    }
    if !(1..=31).contains(&due) {
        return Err("vencimento deve ser entre 1 e 31".into());
    }
    // Fechamento e vencimento no MESMO mês precisam sobreviver a fevereiro: acima do dia 28 os
    // dois encurtam para o último dia e colidem, e um ciclo que fecha no dia em que vence não
    // existe. Fechar depois do vencimento não tem esse problema — o fechamento é do mês anterior.
    if closing < due && closing >= 28 {
        return Err(format!(
            "com fechamento no dia {closing}, o vencimento precisa vir antes dele (do mês seguinte) \
             ou até o dia 27 — em fevereiro os dois cairiam no mesmo dia"
        ));
    }
    Ok((closing as u32, due as u32))
}

/// O adicional herda o ciclo do titular para que as duas sub-faturas compartilhem a identidade mensal.
pub(crate) async fn effective_cycle(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<(u32, u32), String> {
    let row: Option<(Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT linked_account_id, closing_day, due_day FROM account WHERE id = ?1 AND type = 'credit_card'",
    ).bind(account_id).fetch_optional(pool).await.map_err(|e| format!("cartão: {e}"))?;
    let (linked, closing, due) = row.ok_or("cartão não encontrado")?;
    if let Some(holder) = linked {
        let holder: Option<(Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT linked_account_id, closing_day, due_day FROM account WHERE id = ?1 AND type = 'credit_card'",
        ).bind(holder).fetch_optional(pool).await.map_err(|e| format!("titular: {e}"))?;
        let (nested, closing, due) = holder.ok_or("titular do cartão não encontrado")?;
        if nested.is_some() {
            return Err("adicional deve apontar para um cartão titular".into());
        }
        validate_cycle(closing, due)
    } else {
        validate_cycle(closing, due)
    }
}

/// Encontra ou cria uma fatura dentro da transação do chamador para não disputar a única conexão do pool.
pub(crate) async fn ensure_invoice(
    conn: &mut SqliteConnection,
    account_id: &str,
    cycle_month: &str,
    closing_day: u32,
    due_day: u32,
) -> Result<String, String> {
    if let Some((id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM invoice WHERE account_id = ?1 AND cycle_month = ?2",
    )
    .bind(account_id)
    .bind(cycle_month)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("fatura: {e}"))?
    {
        return Ok(id);
    }
    let (closing, due) =
        cards::dates_for_cycle_month(cycle_month, closing_day, due_day).ok_or("ciclo inválido")?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(&id).bind(account_id).bind(cycle_month).bind(closing.to_string()).bind(due.to_string())
        .execute(&mut *conn).await.map_err(|e| format!("criar fatura: {e}"))?;
    Ok(id)
}

async fn invoice_purchases_sum(pool: &SqlitePool, invoice_id: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM \"transaction\" WHERE invoice_id = ?1 AND type = 'expense'")
        .bind(invoice_id).fetch_one(pool).await.map_err(|e| format!("compras: {e}"))
}

async fn adjust_stated(
    conn: &mut SqliteConnection,
    invoice_id: &str,
    delta: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE invoice SET stated_total_cents = MAX(0, stated_total_cents + ?2) WHERE id = ?1 AND stated_total_cents IS NOT NULL")
        .bind(invoice_id).bind(delta).execute(&mut *conn).await.map_err(|e| format!("ajustar fatura: {e}"))?;
    Ok(())
}

async fn summary(
    pool: &SqlitePool,
    row: InvoiceRow,
    today: NaiveDate,
) -> Result<InvoiceSummaryDto, String> {
    let purchases_sum_cents = invoice_purchases_sum(pool, &row.0).await?;
    let closing = parse_date(&row.2)?;
    let due = parse_date(&row.3)?;
    Ok(InvoiceSummaryDto {
        id: row.0,
        cycle_month: row.1,
        closing_date: row.2,
        due_date: row.3,
        status: cards::invoice_status(today, closing, due).as_str().into(),
        stated_total_cents: row.4,
        purchases_sum_cents,
        effective_total_cents: cards::effective_total_cents(row.4, purchases_sum_cents),
        reconciliation_delta_cents: cards::reconciliation_delta_cents(row.4, purchases_sum_cents),
    })
}

async fn invoice_summaries(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<Vec<InvoiceSummaryDto>, String> {
    let rows: Vec<InvoiceRow> = sqlx::query_as("SELECT id, cycle_month, closing_date, due_date, stated_total_cents FROM invoice WHERE account_id = ?1 ORDER BY cycle_month DESC")
        .bind(account_id).fetch_all(pool).await.map_err(|e| format!("faturas: {e}"))?;
    let today = chrono::Local::now().date_naive();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(summary(pool, row, today).await?);
    }
    Ok(out)
}

async fn owner_id(conn: &mut SqliteConnection, owner_name: Option<&str>) -> Result<String, String> {
    if let Some(name) = owner_name.map(str::trim).filter(|n| !n.is_empty()) {
        if let Some((id,)) = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM person WHERE name = ?1 COLLATE NOCASE LIMIT 1",
        )
        .bind(name)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("pessoa: {e}"))?
        {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&id)
            .bind(name)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("criar pessoa: {e}"))?;
        return Ok(id);
    }
    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("criar pessoa: {e}"))?;
    sqlx::query_scalar("SELECT id FROM person ORDER BY created_at, id LIMIT 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("pessoa: {e}"))
}

async fn replace_aliases(
    conn: &mut SqliteConnection,
    account_id: &str,
    name: &str,
    aliases: &[String],
) -> Result<(), String> {
    let implicit = cards::normalize_alias(name);
    let mut normalized = std::collections::HashSet::new();
    for alias in aliases {
        let alias = cards::normalize_alias(alias);
        if alias.is_empty() {
            return Err("alias obrigatório".into());
        }
        // O nome já é alias implícito: repetir essa mesma identidade não cria uma segunda linha.
        if alias == implicit {
            continue;
        }
        if !normalized.insert(alias.clone()) {
            return Err("alias duplicado".into());
        }
    }
    let others: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM account WHERE type = 'credit_card' AND id != ?1")
            .bind(account_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("aliases: {e}"))?;
    for (_, other_name) in others {
        if cards::normalize_alias(&other_name) == implicit
            || normalized.contains(&cards::normalize_alias(&other_name))
        {
            return Err("alias já pertence a outro cartão".into());
        }
    }
    let used: Vec<(String,)> =
        sqlx::query_as("SELECT alias FROM card_alias WHERE account_id != ?1")
            .bind(account_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("aliases: {e}"))?;
    if used
        .iter()
        .any(|(a,)| a == &implicit || normalized.contains(a))
    {
        return Err("alias já pertence a outro cartão".into());
    }
    sqlx::query("DELETE FROM card_alias WHERE account_id = ?1")
        .bind(account_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("limpar aliases: {e}"))?;
    for alias in normalized {
        sqlx::query("INSERT INTO card_alias (id, account_id, alias) VALUES (?1, ?2, ?3)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(account_id)
            .bind(alias)
            .execute(&mut *conn)
            .await
            .map_err(|_| "alias já pertence a outro cartão".to_string())?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_card_on_conn(
    conn: &mut SqliteConnection,
    name: &str,
    institution: Option<&str>,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    credit_limit_cents: Option<i64>,
    owner_person_name: Option<&str>,
    linked_account_id: Option<&str>,
    aliases: &[String],
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    if credit_limit_cents.is_some_and(|v| v < 0) {
        return Err("limite não pode ser negativo".into());
    }
    let (closing, due) = if let Some(holder) = linked_account_id.filter(|v| !v.is_empty()) {
        if closing_day.is_some() || due_day.is_some() {
            return Err("cartão adicional herda o ciclo e não aceita dias".into());
        }
        let holder: Option<HolderRow> = sqlx::query_as(
            "SELECT type, linked_account_id, closing_day, due_day FROM account WHERE id = ?1",
        )
        .bind(holder)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("titular: {e}"))?;
        let Some((kind, nested, closing, due)) = holder else {
            return Err("titular não encontrado".into());
        };
        if kind != "credit_card" || nested.is_some() {
            return Err("vínculo deve apontar para cartão titular".into());
        }
        validate_cycle(closing, due)?;
        (None, None)
    } else {
        let (c, d) = validate_cycle(closing_day, due_day)?;
        (Some(c as i64), Some(d as i64))
    };
    let owner = owner_id(conn, owner_person_name).await?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO account (id, name, type, owner_person_id, institution, balance, liquidity, closing_day, due_day, credit_limit, linked_account_id) VALUES (?1, ?2, 'credit_card', ?3, ?4, 0, NULL, ?5, ?6, ?7, ?8)")
        .bind(&id).bind(name).bind(owner).bind(institution.map(str::trim).filter(|s| !s.is_empty())).bind(closing).bind(due).bind(credit_limit_cents).bind(linked_account_id.filter(|s| !s.is_empty()))
        .execute(&mut *conn).await.map_err(|e| format!("criar cartão: {e}"))?;
    replace_aliases(conn, &id, name, aliases).await?;
    Ok(id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_card_account(
    pool: State<'_, SqlitePool>,
    name: String,
    institution: Option<String>,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    credit_limit_cents: Option<i64>,
    owner_person_name: Option<String>,
    linked_account_id: Option<String>,
    aliases: Vec<String>,
) -> Result<String, String> {
    create_card_account_inner(
        pool.inner(),
        &name,
        institution.as_deref(),
        closing_day,
        due_day,
        credit_limit_cents,
        owner_person_name.as_deref(),
        linked_account_id.as_deref(),
        &aliases,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_card_account_inner(
    pool: &SqlitePool,
    name: &str,
    institution: Option<&str>,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    credit_limit_cents: Option<i64>,
    owner_person_name: Option<&str>,
    linked_account_id: Option<&str>,
    aliases: &[String],
) -> Result<String, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("criar cartão: {e}"))?;
    let id = create_card_on_conn(
        &mut tx,
        name,
        institution,
        closing_day,
        due_day,
        credit_limit_cents,
        owner_person_name,
        linked_account_id,
        aliases,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| format!("criar cartão: {e}"))?;
    Ok(id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_card_account(
    pool: State<'_, SqlitePool>,
    account_id: String,
    name: String,
    institution: Option<String>,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    credit_limit_cents: Option<i64>,
    aliases: Vec<String>,
) -> Result<(), String> {
    update_card_account_inner(
        pool.inner(),
        &account_id,
        &name,
        institution.as_deref(),
        closing_day,
        due_day,
        credit_limit_cents,
        &aliases,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_card_account_inner(
    pool: &SqlitePool,
    account_id: &str,
    name: &str,
    institution: Option<&str>,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    credit_limit_cents: Option<i64>,
    aliases: &[String],
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("editar cartão: {e}"))?;
    let linked: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT linked_account_id FROM account WHERE id = ?1 AND type = 'credit_card'",
    )
    .bind(account_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("cartão: {e}"))?;
    let (linked,) = linked.ok_or("cartão não encontrado")?;
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    if credit_limit_cents.is_some_and(|v| v < 0) {
        return Err("limite não pode ser negativo".into());
    }
    let (closing, due) = if linked.is_some() {
        if closing_day.is_some() || due_day.is_some() {
            return Err("cartão adicional herda o ciclo e não aceita dias".into());
        }
        (None, None)
    } else {
        let (c, d) = validate_cycle(closing_day, due_day)?;
        (Some(c as i64), Some(d as i64))
    };
    sqlx::query("UPDATE account SET name=?2, institution=?3, closing_day=?4, due_day=?5, credit_limit=?6 WHERE id=?1")
        .bind(account_id).bind(name).bind(institution.map(str::trim).filter(|s| !s.is_empty())).bind(closing).bind(due).bind(credit_limit_cents).execute(&mut *tx).await.map_err(|e| format!("editar cartão: {e}"))?;
    replace_aliases(&mut tx, account_id, name, aliases).await?;
    tx.commit().await.map_err(|e| format!("editar cartão: {e}"))
}

#[tauri::command]
pub async fn list_cards(pool: State<'_, SqlitePool>) -> Result<Vec<CardDto>, String> {
    list_cards_inner(pool.inner()).await
}

pub(crate) async fn list_cards_inner(pool: &SqlitePool) -> Result<Vec<CardDto>, String> {
    let rows: Vec<CardListRow> = sqlx::query_as(
        "SELECT a.id,a.name,a.institution,p.name,a.linked_account_id,a.credit_limit FROM account a JOIN person p ON p.id=a.owner_person_id WHERE a.type='credit_card' ORDER BY a.name COLLATE NOCASE",
    ).fetch_all(pool).await.map_err(|e| format!("cartões: {e}"))?;
    let today = chrono::Local::now().date_naive();
    let mut cards_out = Vec::with_capacity(rows.len());
    for (id, name, institution, owner_name, linked_account_id, credit_limit_cents) in rows {
        let (closing_day, due_day) = effective_cycle(pool, &id).await?;
        let aliases: Vec<(String,)> =
            sqlx::query_as("SELECT alias FROM card_alias WHERE account_id=?1 ORDER BY alias")
                .bind(&id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("aliases: {e}"))?;
        let all = invoice_summaries(pool, &id).await?;
        let open_invoice = all.iter().find(|i| i.status == "aberta").cloned();
        let next_due = all
            .iter()
            .filter(|i| parse_date(&i.due_date).is_ok_and(|d| d >= today))
            .min_by_key(|i| i.due_date.clone())
            .cloned();
        cards_out.push(CardDto {
            id,
            name,
            institution,
            owner_name,
            linked_account_id,
            closing_day,
            due_day,
            credit_limit_cents,
            aliases: aliases.into_iter().map(|a| a.0).collect(),
            open_invoice,
            next_due,
        });
    }
    Ok(cards_out)
}

#[tauri::command]
pub async fn list_invoices(
    pool: State<'_, SqlitePool>,
    account_id: String,
) -> Result<Vec<InvoiceSummaryDto>, String> {
    list_invoices_inner(pool.inner(), &account_id).await
}
pub(crate) async fn list_invoices_inner(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<Vec<InvoiceSummaryDto>, String> {
    invoice_summaries(pool, account_id).await
}

#[tauri::command]
pub async fn get_invoice(
    pool: State<'_, SqlitePool>,
    invoice_id: String,
) -> Result<InvoiceDetailDto, String> {
    get_invoice_inner(pool.inner(), &invoice_id).await
}

pub(crate) async fn get_invoice_inner(
    pool: &SqlitePool,
    invoice_id: &str,
) -> Result<InvoiceDetailDto, String> {
    let invoice: Option<InvoiceDetailRow> = sqlx::query_as(
        "SELECT i.id,i.cycle_month,i.closing_date,i.due_date,i.stated_total_cents,i.account_id,a.linked_account_id FROM invoice i JOIN account a ON a.id=i.account_id WHERE i.id=?1",
    ).bind(invoice_id).fetch_optional(pool).await.map_err(|e| format!("fatura: {e}"))?;
    let (id, cycle_month, closing_date, due_date, stated, account_id, linked_account_id) =
        invoice.ok_or("fatura não encontrada")?;
    let summary = summary(
        pool,
        (
            id.clone(),
            cycle_month.clone(),
            closing_date,
            due_date,
            stated,
        ),
        chrono::Local::now().date_naive(),
    )
    .await?;
    let owner: String = sqlx::query_scalar(
        "SELECT p.name FROM account a JOIN person p ON p.id=a.owner_person_id WHERE a.id=?1",
    )
    .bind(&account_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("dono: {e}"))?;
    let rows: Vec<PurchaseRow> = sqlx::query_as(
        "SELECT t.id,t.date,COALESCE(t.description,''),t.amount,t.card_series_id,t.is_projection,s.count,s.start_cycle_month FROM \"transaction\" t LEFT JOIN card_series s ON s.id=t.card_series_id WHERE t.invoice_id=?1 AND t.type='expense' ORDER BY t.date,t.id",
    ).bind(invoice_id).fetch_all(pool).await.map_err(|e| format!("compras: {e}"))?;
    let purchases = rows
        .into_iter()
        .map(
            |(txn_id, date, description, amount_cents, series_id, is_projection, count, start)| {
                let installment_label = match (count, start) {
                    (Some(count), Some(start)) => {
                        cards::cycle_index(&start, &cycle_month).map(|n| format!("{n}/{count}"))
                    }
                    _ => None,
                };
                CardPurchaseDto {
                    txn_id,
                    date,
                    description,
                    amount_cents,
                    owner_name: owner.clone(),
                    series_id,
                    installment_label,
                    is_projection: is_projection != 0,
                }
            },
        )
        .collect();
    // Um reembolso vinculado à SÉRIE (não a uma ocorrência) pertence a UMA fatura — a da 1ª
    // ocorrência (`s.start_cycle_month`), nunca repetido em todo ciclo em que a série materializa
    // (a lente líquida do drill dobraria a subtração a cada fatura). `refund_txn_id` casa por
    // compra: a Entrada aparece na fatura DELA, não em toda fatura da série.
    let refunds: Vec<RefundDto> = sqlx::query_as::<_, (String, String, i64, String, i64)>(
        "SELECT DISTINCT r.id,r.date,r.amount,COALESCE(r.description,''),r.is_projection \
         FROM \"transaction\" r \
         LEFT JOIN card_series s ON s.id = r.refund_series_id \
         LEFT JOIN \"transaction\" purchase ON purchase.id = r.refund_txn_id \
         WHERE r.type = 'income' \
           AND ( \
                r.refund_invoice_id = ?1 \
                OR purchase.invoice_id = ?1 \
                OR (s.account_id = ?2 AND s.start_cycle_month = ?3) \
           ) \
         ORDER BY r.date,r.id",
    )
    .bind(invoice_id)
    .bind(&account_id)
    .bind(&cycle_month)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("reembolsos: {e}"))?
    .into_iter()
    .map(
        |(txn_id, date, amount_cents, description, is_projection)| RefundDto {
            txn_id,
            date,
            amount_cents,
            description,
            is_projection: is_projection != 0,
        },
    )
    .collect();
    let holder_id = linked_account_id.as_deref().unwrap_or(&account_id);
    let sub_rows: Vec<(String,String,String,String,Option<i64>)> = sqlx::query_as(
        "SELECT i.account_id,a.name,p.name,i.id,i.stated_total_cents FROM invoice i JOIN account a ON a.id=i.account_id JOIN person p ON p.id=a.owner_person_id WHERE a.linked_account_id=?1 AND i.cycle_month=?2",
    ).bind(holder_id).bind(&cycle_month).fetch_all(pool).await.map_err(|e| format!("sub-faturas: {e}"))?;
    let mut sub_invoices = Vec::new();
    let mut emitter_total = if linked_account_id.is_some() {
        let holder: Option<(String, Option<i64>)> = sqlx::query_as(
            "SELECT id, stated_total_cents FROM invoice WHERE account_id=?1 AND cycle_month=?2",
        )
        .bind(holder_id)
        .bind(&cycle_month)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("fatura titular: {e}"))?;
        match holder {
            Some((holder_invoice_id, stated)) => cards::effective_total_cents(
                stated,
                invoice_purchases_sum(pool, &holder_invoice_id).await?,
            ),
            None => 0,
        }
    } else {
        summary.effective_total_cents
    };
    for (sub_account_id, card_name, owner_name, sub_id, sub_stated) in sub_rows {
        let sum = invoice_purchases_sum(pool, &sub_id).await?;
        let effective = cards::effective_total_cents(sub_stated, sum);
        emitter_total += effective;
        sub_invoices.push(SubInvoiceDto {
            account_id: sub_account_id,
            card_name,
            owner_name,
            effective_total_cents: effective,
        });
    }
    Ok(InvoiceDetailDto {
        summary,
        purchases,
        refunds,
        sub_invoices,
        emitter_total_cents: emitter_total,
    })
}

#[tauri::command]
pub async fn register_card_purchase(
    pool: State<'_, SqlitePool>,
    card_account_id: String,
    amount_cents: i64,
    description: Option<String>,
    date: String,
    refund_cents: Option<i64>,
    tag_ids: Vec<String>,
) -> Result<String, String> {
    register_card_purchase_with_refund_inner(
        pool.inner(),
        &card_account_id,
        amount_cents,
        description.as_deref(),
        &date,
        refund_cents,
        &tag_ids,
    )
    .await
}
#[cfg(test)]
pub(crate) async fn register_card_purchase_inner(
    pool: &SqlitePool,
    card_account_id: &str,
    amount_cents: i64,
    description: Option<&str>,
    date: &str,
    tag_ids: &[String],
) -> Result<String, String> {
    register_card_purchase_with_refund_inner(
        pool,
        card_account_id,
        amount_cents,
        description,
        date,
        None,
        tag_ids,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn register_card_purchase_with_refund_inner(
    pool: &SqlitePool,
    card_account_id: &str,
    amount_cents: i64,
    description: Option<&str>,
    date: &str,
    refund_cents: Option<i64>,
    tag_ids: &[String],
) -> Result<String, String> {
    if amount_cents <= 0 {
        return Err("valor deve ser positivo".into());
    }
    if refund_cents.is_some_and(|value| value <= 0) {
        return Err("reembolso deve ser positivo".into());
    }
    let date = parse_date(date)?;
    let (closing, due) = effective_cycle(pool, card_account_id).await?;
    let close = cards::cycle_close_for_purchase(date, closing);
    let cycle = cards::cycle_month_of(cards::due_date_for_close(close, due));
    let mut tx = pool.begin().await.map_err(|e| format!("compra: {e}"))?;
    let invoice_id = ensure_invoice(&mut tx, card_account_id, &cycle, closing, due).await?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO \"transaction\" (id,type,amount,description,date,payment_method,is_fixed,is_projection,invoice_id) VALUES (?1,'expense',?2,?3,?4,'credit',0,?5,?6)")
        .bind(&id).bind(amount_cents).bind(description.map(str::trim).filter(|s|!s.is_empty())).bind(date.to_string()).bind((date>chrono::Local::now().date_naive()) as i64).bind(&invoice_id).execute(&mut *tx).await.map_err(|e|format!("registrar compra: {e}"))?;
    adjust_stated(&mut tx, &invoice_id, amount_cents).await?;
    if let Some(refund_cents) = refund_cents {
        let due: String = sqlx::query_scalar("SELECT due_date FROM invoice WHERE id = ?1")
            .bind(&invoice_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("reembolso: {e}"))?;
        let due_date = parse_date(&due)?;
        sqlx::query("INSERT INTO \"transaction\" (id,type,amount,date,is_fixed,is_projection,refund_invoice_id) VALUES (?1,'income',?2,?3,0,?4,?5)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(refund_cents)
            .bind(&due)
            .bind((due_date > chrono::Local::now().date_naive()) as i64)
            .bind(&invoice_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("criar reembolso: {e}"))?;
    }
    crate::tags::set_transaction_tags_on_conn(&mut tx, &id, tag_ids).await?;
    tx.commit().await.map_err(|e| format!("compra: {e}"))?;
    Ok(id)
}

#[tauri::command]
pub async fn move_card_purchase(
    pool: State<'_, SqlitePool>,
    txn_id: String,
    target_cycle_month: String,
) -> Result<(), String> {
    move_card_purchase_inner(pool.inner(), &txn_id, &target_cycle_month).await
}
pub(crate) async fn move_card_purchase_inner(
    pool: &SqlitePool,
    txn_id: &str,
    target_cycle_month: &str,
) -> Result<(), String> {
    if cards::parse_cycle_month(target_cycle_month).is_none() {
        return Err("ciclo inválido".into());
    }
    let info: Option<(String,Option<String>,String)> = sqlx::query_as("SELECT i.account_id,t.card_series_id,t.invoice_id FROM \"transaction\" t JOIN invoice i ON i.id=t.invoice_id WHERE t.id=?1 AND t.type='expense'").bind(txn_id).fetch_optional(pool).await.map_err(|e|format!("compra: {e}"))?;
    let (account_id, series_id, origin) = info.ok_or("compra de cartão não encontrada")?;
    if series_id.is_some() {
        return Err("ocorrência de série deve ser alterada pela série".into());
    }
    let (closing, due) = effective_cycle(pool, &account_id).await?;
    let mut tx = pool.begin().await.map_err(|e| format!("remanejar: {e}"))?;
    let target = ensure_invoice(&mut tx, &account_id, target_cycle_month, closing, due).await?;
    if target != origin {
        let amount: i64 = sqlx::query_scalar("SELECT amount FROM \"transaction\" WHERE id=?1")
            .bind(txn_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("compra: {e}"))?;
        adjust_stated(&mut tx, &origin, -amount).await?;
        adjust_stated(&mut tx, &target, amount).await?;
        sqlx::query("UPDATE \"transaction\" SET invoice_id=?2 WHERE id=?1")
            .bind(txn_id)
            .bind(&target)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("remanejar: {e}"))?;
    }
    tx.commit().await.map_err(|e| format!("remanejar: {e}"))
}

#[tauri::command]
pub async fn set_invoice_stated_total(
    pool: State<'_, SqlitePool>,
    invoice_id: String,
    stated_total_cents: Option<i64>,
) -> Result<(), String> {
    set_invoice_stated_total_inner(pool.inner(), &invoice_id, stated_total_cents).await
}
pub(crate) async fn set_invoice_stated_total_inner(
    pool: &SqlitePool,
    invoice_id: &str,
    stated_total_cents: Option<i64>,
) -> Result<(), String> {
    if stated_total_cents.is_some_and(|v| v < 0) {
        return Err("total declarado não pode ser negativo".into());
    }
    let result = sqlx::query("UPDATE invoice SET stated_total_cents=?2 WHERE id=?1")
        .bind(invoice_id)
        .bind(stated_total_cents)
        .execute(pool)
        .await
        .map_err(|e| format!("fatura: {e}"))?;
    if result.rows_affected() == 0 {
        return Err("fatura não encontrada".into());
    }
    Ok(())
}

/// Corrige as datas de UMA fatura, sem tocar no molde do cartão.
///
/// O ciclo cadastrado é um molde para derivar ciclos que ainda não existem; o banco, porém, move
/// o fechamento de um mês para outro (dia útil, feriado, decisão do emissor). A fatura já guarda
/// as próprias datas — só faltava a porta para corrigi-las quando a realidade diverge do molde.
///
/// O fechamento corrigido persiste: a varredura do import não o reescreve. O vencimento é
/// diferente — quando a planilha declara aquela fatura, ela volta a mandar no próximo import,
/// porque a data da linha é o vencimento observado.
#[tauri::command]
pub async fn set_invoice_dates(
    pool: State<'_, SqlitePool>,
    invoice_id: String,
    closing_date: String,
    due_date: String,
) -> Result<(), String> {
    set_invoice_dates_inner(pool.inner(), &invoice_id, &closing_date, &due_date).await
}
pub(crate) async fn set_invoice_dates_inner(
    pool: &SqlitePool,
    invoice_id: &str,
    closing_date: &str,
    due_date: &str,
) -> Result<(), String> {
    let closing =
        parse_date(closing_date).map_err(|_| "data de fechamento inválida".to_string())?;
    let due = parse_date(due_date).map_err(|_| "data de vencimento inválida".to_string())?;
    if closing >= due {
        return Err("o fechamento precisa ser anterior ao vencimento".into());
    }
    let cycle_month: Option<(String,)> =
        sqlx::query_as("SELECT cycle_month FROM invoice WHERE id = ?1")
            .bind(invoice_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("fatura: {e}"))?;
    let (cycle_month,) = cycle_month.ok_or("fatura não encontrada")?;
    // A identidade da fatura é cartão×mês-do-vencimento: mover o vencimento para outro mês faria
    // a chave mentir. Para isso existe outra fatura, a daquele mês.
    if crate::cards::cycle_month_of(due) != cycle_month {
        return Err(format!(
            "o vencimento precisa continuar em {cycle_month} — é o mês que identifica esta fatura"
        ));
    }
    sqlx::query("UPDATE invoice SET closing_date = ?2, due_date = ?3 WHERE id = ?1")
        .bind(invoice_id)
        .bind(closing.to_string())
        .bind(due.to_string())
        .execute(pool)
        .await
        .map_err(|e| format!("fatura: {e}"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn materialize_series(
    conn: &mut SqliteConnection,
    series_id: &str,
    account_id: &str,
    description: &str,
    amount: i64,
    count: Option<i64>,
    start_cycle_month: &str,
    canceled_from: Option<&str>,
    closing: u32,
    due: u32,
    display_day: u32,
) -> Result<(), String> {
    let mut cycles = Vec::new();
    let length = match count {
        Some(n) => n,
        None => {
            // Assinatura: janela rolante até dezembro do ano corrente (mínimo 3 ocorrências),
            // para que as faturas futuras do ano-planilha enxerguem a cobrança que não cessa.
            let (year, month) =
                cards::parse_cycle_month(start_cycle_month).ok_or("ciclo inválido")?;
            let now = chrono::Local::now().date_naive();
            let months = (now.year() - year) * 12 + (12 - month as i32) + 1;
            i64::from(months.max(3))
        }
    };
    for offset in 0..length {
        let cycle =
            cards::add_cycle_months(start_cycle_month, offset as i32).ok_or("ciclo inválido")?;
        if canceled_from.is_some_and(|end| cycle.as_str() >= end) {
            break;
        }
        cycles.push(cycle);
    }
    for cycle in cycles {
        let invoice_id = ensure_invoice(conn, account_id, &cycle, closing, due).await?;
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id=?1 AND invoice_id=?2",
        )
        .bind(series_id)
        .bind(&invoice_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("ocorrências: {e}"))?;
        if existing > 0 {
            continue;
        }
        let closing_date: String =
            sqlx::query_scalar("SELECT closing_date FROM invoice WHERE id=?1")
                .bind(&invoice_id)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| format!("fatura: {e}"))?;
        let close = parse_date(&closing_date)?;
        let day =
            display_day.min(crate::forecast::last_day_of_month(close.year(), close.month()).day());
        let date =
            NaiveDate::from_ymd_opt(close.year(), close.month(), day).ok_or("data inválida")?;
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO \"transaction\" (id,type,amount,description,date,payment_method,is_fixed,is_projection,invoice_id,card_series_id) VALUES (?1,'expense',?2,?3,?4,'credit',0,?5,?6,?7)")
            .bind(&id).bind(amount).bind(description).bind(date.to_string()).bind((date>chrono::Local::now().date_naive()) as i64).bind(&invoice_id).bind(series_id).execute(&mut *conn).await.map_err(|e|format!("materializar série: {e}"))?;
        adjust_stated(conn, &invoice_id, amount).await?;
    }
    Ok(())
}

/// Estende assinaturas ativas até dezembro do ano corrente ao abrir o banco. A inserção de cada
/// ocorrência é idempotente por série × fatura, portanto reabrir o app não duplica cobranças.
pub async fn advance_active_subscriptions(pool: &SqlitePool) -> Result<(), String> {
    let subscriptions: Vec<(String, String, String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, account_id, description, amount_cents, start_cycle_month, canceled_from_cycle_month \
         FROM card_series WHERE count IS NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("assinaturas: {e}"))?;
    let today = chrono::Local::now().date_naive();

    for (series_id, account_id, description, amount_cents, start_cycle, canceled_from) in
        subscriptions
    {
        let (closing, due) = effective_cycle(pool, &account_id).await?;
        let current_cycle = cards::cycle_month_of(cards::due_date_for_close(
            cards::cycle_close_for_purchase(today, closing),
            due,
        ));
        if canceled_from
            .as_deref()
            .is_some_and(|cycle| cycle <= current_cycle.as_str())
        {
            continue;
        }
        let display_day = sqlx::query_scalar::<_, String>(
            "SELECT date FROM \"transaction\" WHERE card_series_id = ?1 ORDER BY date, id LIMIT 1",
        )
        .bind(&series_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("assinatura: {e}"))?
        .and_then(|date| parse_date(&date).ok().map(|date| date.day()))
        .unwrap_or(1);

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("avançar assinatura: {e}"))?;
        materialize_series(
            &mut tx,
            &series_id,
            &account_id,
            &description,
            amount_cents,
            None,
            &start_cycle,
            canceled_from.as_deref(),
            closing,
            due,
            display_day,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| format!("avançar assinatura: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_card_series(
    pool: State<'_, SqlitePool>,
    card_account_id: String,
    description: String,
    amount_cents: i64,
    count: Option<i64>,
    start_date: String,
    refund_cents: Option<i64>,
    tag_ids: Vec<String>,
) -> Result<String, String> {
    create_card_series_with_refund_inner(
        pool.inner(),
        &card_account_id,
        &description,
        amount_cents,
        count,
        &start_date,
        refund_cents,
        &tag_ids,
    )
    .await
}
#[cfg(test)]
pub(crate) async fn create_card_series_inner(
    pool: &SqlitePool,
    card_account_id: &str,
    description: &str,
    amount_cents: i64,
    count: Option<i64>,
    start_date: &str,
) -> Result<String, String> {
    create_card_series_with_refund_inner(
        pool,
        card_account_id,
        description,
        amount_cents,
        count,
        start_date,
        None,
        &[],
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_card_series_with_refund_inner(
    pool: &SqlitePool,
    card_account_id: &str,
    description: &str,
    amount_cents: i64,
    count: Option<i64>,
    start_date: &str,
    refund_cents: Option<i64>,
    tag_ids: &[String],
) -> Result<String, String> {
    if description.trim().is_empty() {
        return Err("descrição obrigatória".into());
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo".into());
    }
    if count.is_some_and(|n| !(1..=120).contains(&n)) {
        return Err("quantidade deve ser entre 1 e 120".into());
    }
    if refund_cents.is_some_and(|value| value <= 0) {
        return Err("reembolso deve ser positivo".into());
    }
    let start = parse_date(start_date)?;
    let (closing, due) = effective_cycle(pool, card_account_id).await?;
    let start_cycle = cards::cycle_month_of(cards::due_date_for_close(
        cards::cycle_close_for_purchase(start, closing),
        due,
    ));
    let mut tx = pool.begin().await.map_err(|e| format!("série: {e}"))?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO card_series (id,account_id,description,amount_cents,count,start_cycle_month) VALUES (?1,?2,?3,?4,?5,?6)").bind(&id).bind(card_account_id).bind(description.trim()).bind(amount_cents).bind(count).bind(&start_cycle).execute(&mut *tx).await.map_err(|e|format!("criar série: {e}"))?;
    materialize_series(
        &mut tx,
        &id,
        card_account_id,
        description.trim(),
        amount_cents,
        count,
        &start_cycle,
        None,
        closing,
        due,
        start.day(),
    )
    .await?;
    let occurrence_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM \"transaction\" WHERE card_series_id = ?1 ORDER BY date, id",
    )
    .bind(&id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("série: {e}"))?;
    for (occurrence_id,) in occurrence_ids {
        crate::tags::set_transaction_tags_on_conn(&mut tx, &occurrence_id, tag_ids).await?;
    }
    if let Some(refund_cents) = refund_cents {
        let due: String = sqlx::query_scalar(
            "SELECT i.due_date FROM \"transaction\" t \
             JOIN invoice i ON i.id = t.invoice_id \
             WHERE t.card_series_id = ?1 ORDER BY i.cycle_month, t.id LIMIT 1",
        )
        .bind(&id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("reembolso da série: {e}"))?;
        let due_date = parse_date(&due)?;
        sqlx::query("INSERT INTO \"transaction\" (id,type,amount,date,is_fixed,is_projection,refund_series_id) VALUES (?1,'income',?2,?3,0,?4,?5)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(refund_cents)
            .bind(&due)
            .bind((due_date > chrono::Local::now().date_naive()) as i64)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("criar reembolso da série: {e}"))?;
    }
    tx.commit().await.map_err(|e| format!("série: {e}"))?;
    Ok(id)
}

async fn active_series_occurrences(
    conn: &mut SqliteConnection,
    series_id: &str,
) -> Result<Vec<(String, String, i64, String)>, String> {
    sqlx::query_as("SELECT t.id,t.invoice_id,t.amount,i.due_date FROM \"transaction\" t JOIN invoice i ON i.id=t.invoice_id WHERE t.card_series_id=?1")
        .bind(series_id).fetch_all(&mut *conn).await.map_err(|e|format!("ocorrências: {e}"))
}

#[tauri::command]
pub async fn update_card_series(
    pool: State<'_, SqlitePool>,
    series_id: String,
    description: String,
    amount_cents: i64,
) -> Result<(), String> {
    update_card_series_inner(pool.inner(), &series_id, &description, amount_cents).await
}
pub(crate) async fn update_card_series_inner(
    pool: &SqlitePool,
    series_id: &str,
    description: &str,
    amount_cents: i64,
) -> Result<(), String> {
    if description.trim().is_empty() {
        return Err("descrição obrigatória".into());
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo".into());
    }
    let series:Option<SeriesRow>=sqlx::query_as("SELECT account_id,description,count,start_cycle_month,canceled_from_cycle_month FROM card_series WHERE id=?1").bind(series_id).fetch_optional(pool).await.map_err(|e|format!("série: {e}"))?;
    let (account_id, _old, count, start, canceled) = series.ok_or("série não encontrada")?;
    let (closing, due) = effective_cycle(pool, &account_id).await?;
    let mut tx = pool.begin().await.map_err(|e| format!("série: {e}"))?;
    let today = chrono::Local::now().date_naive();
    let occurrences = active_series_occurrences(&mut tx, series_id).await?;
    let display_day = 1;
    for (id, invoice_id, amount, due_date) in occurrences {
        let invoice_status = cards::invoice_status(
            today,
            parse_date(
                &sqlx::query_scalar::<_, String>("SELECT closing_date FROM invoice WHERE id=?1")
                    .bind(&invoice_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("fatura: {e}"))?,
            )?,
            parse_date(&due_date)?,
        );
        if matches!(
            invoice_status,
            cards::InvoiceStatus::Aberta | cards::InvoiceStatus::Prevista
        ) {
            adjust_stated(&mut tx, &invoice_id, -amount).await?;
            sqlx::query("DELETE FROM \"transaction\" WHERE id=?1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("regenerar série: {e}"))?;
        }
    }
    sqlx::query("UPDATE card_series SET description=?2,amount_cents=?3 WHERE id=?1")
        .bind(series_id)
        .bind(description.trim())
        .bind(amount_cents)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("série: {e}"))?;
    materialize_series(
        &mut tx,
        series_id,
        &account_id,
        description.trim(),
        amount_cents,
        count,
        &start,
        canceled.as_deref(),
        closing,
        due,
        display_day,
    )
    .await?;
    tx.commit().await.map_err(|e| format!("série: {e}"))
}

#[tauri::command]
pub async fn cancel_card_series(
    pool: State<'_, SqlitePool>,
    series_id: String,
    from_cycle_month: String,
) -> Result<(), String> {
    cancel_card_series_inner(pool.inner(), &series_id, &from_cycle_month).await
}
pub(crate) async fn cancel_card_series_inner(
    pool: &SqlitePool,
    series_id: &str,
    from_cycle_month: &str,
) -> Result<(), String> {
    if cards::parse_cycle_month(from_cycle_month).is_none() {
        return Err("ciclo inválido".into());
    }
    let series: Option<(String, Option<i64>)> =
        sqlx::query_as("SELECT account_id,count FROM card_series WHERE id=?1")
            .bind(series_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("série: {e}"))?;
    let (account_id, count) = series.ok_or("série não encontrada")?;
    if count.is_some() {
        return Err("somente assinaturas podem ser canceladas".into());
    }
    let (c, d) = effective_cycle(pool, &account_id).await?;
    let today = chrono::Local::now().date_naive();
    let current = cards::cycle_month_of(cards::due_date_for_close(
        cards::cycle_close_for_purchase(today, c),
        d,
    ));
    if from_cycle_month < current.as_str() {
        return Err("cancelamento não pode anteceder o ciclo aberto".into());
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("cancelar série: {e}"))?;
    let rows:Vec<(String,String,i64,String)>=sqlx::query_as("SELECT t.id,t.invoice_id,t.amount,i.cycle_month FROM \"transaction\" t JOIN invoice i ON i.id=t.invoice_id WHERE t.card_series_id=?1").bind(series_id).fetch_all(&mut *tx).await.map_err(|e|format!("ocorrências: {e}"))?;
    for (id, invoice, amount, cycle) in rows {
        if cycle.as_str() >= from_cycle_month {
            adjust_stated(&mut tx, &invoice, -amount).await?;
            sqlx::query("DELETE FROM \"transaction\" WHERE id=?1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("cancelar série: {e}"))?;
        }
    }
    // O reembolso de série (`refund_series_id`) é uma Entrada ÚNICA presa à data de vencimento da
    // 1ª ocorrência — não é uma ocorrência (`card_series_id`), então o loop acima não a toca.
    // Cancelar a partir de um ciclo que cobre essa data tem que levá-la junto: senão a Entrada
    // projetada sobra sem nenhuma Saída que a justifique.
    sqlx::query("DELETE FROM \"transaction\" WHERE refund_series_id=?1 AND substr(date,1,7)>=?2")
        .bind(series_id)
        .bind(from_cycle_month)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("cancelar reembolso da série: {e}"))?;
    sqlx::query("UPDATE card_series SET canceled_from_cycle_month=?2 WHERE id=?1")
        .bind(series_id)
        .bind(from_cycle_month)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("cancelar série: {e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("cancelar série: {e}"))
}

#[tauri::command]
pub async fn delete_card_series(
    pool: State<'_, SqlitePool>,
    series_id: String,
) -> Result<(), String> {
    delete_card_series_inner(pool.inner(), &series_id).await
}
pub(crate) async fn delete_card_series_inner(
    pool: &SqlitePool,
    series_id: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("apagar série: {e}"))?;
    let rows = active_series_occurrences(&mut tx, series_id).await?;
    if rows.is_empty() {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM card_series WHERE id=?1")
            .bind(series_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("série: {e}"))?;
        if exists == 0 {
            return Err("série não encontrada".into());
        }
    }
    for (_, invoice, amount, _) in rows {
        adjust_stated(&mut tx, &invoice, -amount).await?;
    }
    // `refund_series_id` é `ON DELETE SET NULL`: apagar `card_series` primeiro anularia a FK e
    // deixaria a Entrada de reembolso viva, sem vínculo — renda fantasma. Apagar a Entrada
    // explicitamente ANTES garante que ela morre junto com a série que a originou.
    sqlx::query("DELETE FROM \"transaction\" WHERE refund_series_id=?1")
        .bind(series_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("apagar reembolso da série: {e}"))?;
    sqlx::query("DELETE FROM card_series WHERE id=?1")
        .bind(series_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("apagar série: {e}"))?;
    tx.commit().await.map_err(|e| format!("apagar série: {e}"))
}

#[tauri::command]
pub async fn create_refund_expectation(
    pool: State<'_, SqlitePool>,
    invoice_id: String,
    amount_cents: i64,
    description: Option<String>,
) -> Result<String, String> {
    create_refund_expectation_inner(
        pool.inner(),
        &invoice_id,
        amount_cents,
        description.as_deref(),
    )
    .await
}
pub(crate) async fn create_refund_expectation_inner(
    pool: &SqlitePool,
    invoice_id: &str,
    amount_cents: i64,
    description: Option<&str>,
) -> Result<String, String> {
    if amount_cents <= 0 {
        return Err("valor deve ser positivo".into());
    }
    let due: Option<(String,)> = sqlx::query_as("SELECT due_date FROM invoice WHERE id=?1")
        .bind(invoice_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("fatura: {e}"))?;
    let (due,) = due.ok_or("fatura não encontrada")?;
    let date = parse_date(&due)?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO \"transaction\" (id,type,amount,description,date,is_fixed,is_projection,refund_invoice_id) VALUES (?1,'income',?2,?3,?4,0,?5,?6)").bind(&id).bind(amount_cents).bind(description.map(str::trim).filter(|s|!s.is_empty())).bind(&due).bind((date>chrono::Local::now().date_naive())as i64).bind(invoice_id).execute(pool).await.map_err(|e|format!("criar reembolso: {e}"))?;
    Ok(id)
}

#[tauri::command]
pub async fn link_refund(
    pool: State<'_, SqlitePool>,
    txn_id: String,
    refund_invoice_id: Option<String>,
    refund_txn_id: Option<String>,
    refund_series_id: Option<String>,
) -> Result<(), String> {
    link_refund_inner(
        pool.inner(),
        &txn_id,
        refund_invoice_id.as_deref(),
        refund_txn_id.as_deref(),
        refund_series_id.as_deref(),
    )
    .await
}
pub(crate) async fn link_refund_inner(
    pool: &SqlitePool,
    txn_id: &str,
    refund_invoice_id: Option<&str>,
    refund_txn_id: Option<&str>,
    refund_series_id: Option<&str>,
) -> Result<(), String> {
    let targets = [refund_invoice_id, refund_txn_id, refund_series_id];
    if targets
        .iter()
        .filter(|v| v.is_some_and(|s| !s.is_empty()))
        .count()
        != 1
    {
        return Err("reembolso exige exatamente um alvo".into());
    }
    let kind: Option<(String,)> = sqlx::query_as("SELECT type FROM \"transaction\" WHERE id=?1")
        .bind(txn_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("reembolso: {e}"))?;
    if kind.ok_or("lançamento não encontrado")?.0 != "income" {
        return Err("reembolso deve ser uma entrada".into());
    }
    if let Some(id) = refund_invoice_id {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice WHERE id=?1")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("fatura: {e}"))?;
        if n == 0 {
            return Err("fatura não encontrada".into());
        }
    }
    if let Some(id) = refund_txn_id {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM \"transaction\" WHERE id=?1 AND invoice_id IS NOT NULL",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("compra: {e}"))?;
        if n == 0 {
            return Err("compra de cartão não encontrada".into());
        }
    }
    if let Some(id) = refund_series_id {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM card_series WHERE id=?1")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("série: {e}"))?;
        if n == 0 {
            return Err("série não encontrada".into());
        }
    }
    sqlx::query("UPDATE \"transaction\" SET refund_invoice_id=?2,refund_txn_id=?3,refund_series_id=?4 WHERE id=?1").bind(txn_id).bind(refund_invoice_id).bind(refund_txn_id).bind(refund_series_id).execute(pool).await.map_err(|e|format!("vincular reembolso: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn unlink_refund(pool: State<'_, SqlitePool>, txn_id: String) -> Result<(), String> {
    unlink_refund_inner(pool.inner(), &txn_id).await
}
pub(crate) async fn unlink_refund_inner(pool: &SqlitePool, txn_id: &str) -> Result<(), String> {
    let result=sqlx::query("UPDATE \"transaction\" SET refund_invoice_id=NULL,refund_txn_id=NULL,refund_series_id=NULL WHERE id=?1 AND type='income'").bind(txn_id).execute(pool).await.map_err(|e|format!("desvincular reembolso: {e}"))?;
    if result.rows_affected() == 0 {
        return Err("entrada não encontrada".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn list_card_proposals(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<CardProposalDto>, String> {
    list_card_proposals_inner(pool.inner()).await
}
pub(crate) async fn list_card_proposals_inner(
    pool: &SqlitePool,
) -> Result<Vec<CardProposalDto>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id,alias,display_name,source_month,status FROM card_proposal \
         WHERE status='pending' ORDER BY created_at,id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("propostas: {e}"))?;

    let mut proposals = Vec::with_capacity(rows.len());
    for (id, alias, display_name, source_month, status) in rows {
        let aliases = proposal_aliases(pool, &id, &alias).await?;
        proposals.push(CardProposalDto {
            id,
            alias,
            display_name,
            source_month,
            status,
            aliases,
        });
    }
    Ok(proposals)
}

/// Apelidos de uma proposta, com a identidade sempre na frente. Uma proposta anterior à
/// consolidação por raiz não tem linhas próprias: ela responde pela identidade e só.
async fn proposal_aliases(
    pool: &SqlitePool,
    proposal_id: &str,
    alias: &str,
) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT alias FROM card_proposal_alias WHERE proposal_id = ?1 ORDER BY alias",
    )
    .bind(proposal_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("apelidos da proposta: {e}"))?;

    let mut aliases = vec![alias.to_string()];
    aliases.extend(rows.into_iter().map(|(a,)| a).filter(|a| a != alias));
    Ok(aliases)
}

#[tauri::command]
pub async fn accept_card_proposal(
    pool: State<'_, SqlitePool>,
    proposal_id: String,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    owner_person_name: Option<String>,
    linked_account_id: Option<String>,
) -> Result<String, String> {
    accept_card_proposal_inner(
        pool.inner(),
        &proposal_id,
        closing_day,
        due_day,
        owner_person_name.as_deref(),
        linked_account_id.as_deref(),
    )
    .await
}
pub(crate) async fn accept_card_proposal_inner(
    pool: &SqlitePool,
    proposal_id: &str,
    closing_day: Option<i64>,
    due_day: Option<i64>,
    owner_person_name: Option<&str>,
    linked_account_id: Option<&str>,
) -> Result<String, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("aceitar proposta: {e}"))?;
    let proposal: Option<(String, String)> = sqlx::query_as(
        "SELECT alias,display_name FROM card_proposal WHERE id=?1 AND status='pending'",
    )
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("proposta: {e}"))?;
    let (alias, name) = proposal.ok_or("proposta pendente não encontrada")?;
    let aliases: Vec<String> = {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT alias FROM card_proposal_alias WHERE proposal_id = ?1")
                .bind(proposal_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| format!("apelidos da proposta: {e}"))?;
        let mut all = vec![alias.clone()];
        all.extend(rows.into_iter().map(|(a,)| a).filter(|a| *a != alias));
        all
    };
    let id = create_card_on_conn(
        &mut tx,
        &name,
        None,
        closing_day,
        due_day,
        None,
        owner_person_name,
        linked_account_id,
        &aliases,
    )
    .await?;
    sqlx::query(
        "UPDATE card_proposal SET status='accepted',resolved_at=datetime('now') WHERE id=?1",
    )
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("aceitar proposta: {e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("aceitar proposta: {e}"))?;
    Ok(id)
}

/// Resolve a proposta como APELIDO de um cartão que já existe, em vez de criar outro.
///
/// A planilha nomeia o mesmo cartão de várias maneiras ao longo dos anos — o nome muda quando o
/// dono passa a distinguir portadores, ou quando escreve só o emissor. Sem este caminho, cada
/// grafia virava uma conta, e a fatura de um mesmo cartão aparecia repartida.
#[tauri::command]
pub async fn attach_card_proposal(
    pool: State<'_, SqlitePool>,
    proposal_id: String,
    account_id: String,
) -> Result<(), String> {
    attach_card_proposal_inner(pool.inner(), &proposal_id, &account_id).await
}
pub(crate) async fn attach_card_proposal_inner(
    pool: &SqlitePool,
    proposal_id: &str,
    account_id: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("vincular proposta: {e}"))?;
    let is_card: Option<(String,)> =
        sqlx::query_as("SELECT id FROM account WHERE id = ?1 AND type = 'credit_card'")
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("cartão de destino: {e}"))?;
    if is_card.is_none() {
        return Err("cartão de destino não encontrado".into());
    }
    let proposal: Option<(String,)> =
        sqlx::query_as("SELECT alias FROM card_proposal WHERE id = ?1 AND status = 'pending'")
            .bind(proposal_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("proposta: {e}"))?;
    let (alias,) = proposal.ok_or("proposta pendente não encontrada")?;

    let variants: Vec<(String,)> =
        sqlx::query_as("SELECT alias FROM card_proposal_alias WHERE proposal_id = ?1")
            .bind(proposal_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("apelidos da proposta: {e}"))?;
    for candidate in std::iter::once(alias).chain(variants.into_iter().map(|(a,)| a)) {
        let normalized = crate::cards::normalize_alias(&candidate);
        if normalized.is_empty() {
            continue;
        }
        // O apelido pertence a um cartão só; um já tomado por outra conta não é reatribuído em
        // silêncio, porque isso mudaria a leitura de faturas passadas sem o dono pedir.
        sqlx::query(
            "INSERT INTO card_alias (id, account_id, alias) VALUES (?1, ?2, ?3) \
             ON CONFLICT(alias) DO NOTHING",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(account_id)
        .bind(&normalized)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("vincular apelido: {e}"))?;
    }

    sqlx::query(
        "UPDATE card_proposal SET status='accepted',resolved_at=datetime('now') WHERE id=?1",
    )
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("vincular proposta: {e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("vincular proposta: {e}"))
}

#[tauri::command]
pub async fn dismiss_card_proposal(
    pool: State<'_, SqlitePool>,
    proposal_id: String,
) -> Result<(), String> {
    dismiss_card_proposal_inner(pool.inner(), &proposal_id).await
}
pub(crate) async fn dismiss_card_proposal_inner(
    pool: &SqlitePool,
    proposal_id: &str,
) -> Result<(), String> {
    let updated=sqlx::query("UPDATE card_proposal SET status='dismissed',resolved_at=datetime('now') WHERE id=?1 AND status='pending'").bind(proposal_id).execute(pool).await.map_err(|e|format!("dispensar proposta: {e}"))?;
    if updated.rows_affected() == 0 {
        return Err("proposta pendente não encontrada".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn card(pool: &SqlitePool, name: &str, closing: i64, due: i64) -> String {
        create_card_account_inner(
            pool,
            name,
            None,
            Some(closing),
            Some(due),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap()
    }

    /// Materializa a fatura de um ciclo pelo mesmo caminho da produção (o molde do cartão).
    async fn ensure_invoice_for_test(
        pool: &SqlitePool,
        account_id: &str,
        cycle_month: &str,
    ) -> String {
        let (closing, due) = effective_cycle(pool, account_id).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        ensure_invoice(&mut conn, account_id, cycle_month, closing, due)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn creates_holder_and_validates_additional_cycle_and_aliases() {
        let pool = pool().await;
        let holder = create_card_account_inner(
            &pool,
            "Cartão Azul",
            None,
            Some(20),
            Some(10),
            None,
            Some("Ana"),
            None,
            &["azul".into()],
        )
        .await
        .unwrap();
        let owner: (String,) = sqlx::query_as(
            "SELECT p.name FROM account a JOIN person p ON p.id=a.owner_person_id WHERE a.id=?1",
        )
        .bind(&holder)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(owner.0, "Ana");
        assert!(
            create_card_account_inner(
                &pool,
                "Sem ciclo",
                None,
                None,
                Some(10),
                None,
                None,
                None,
                &[]
            )
            .await
            .unwrap_err()
            .contains("fechamento")
        );
        assert!(
            create_card_account_inner(
                &pool,
                "Adicional",
                None,
                Some(5),
                None,
                None,
                None,
                Some(&holder),
                &[]
            )
            .await
            .unwrap_err()
            .contains("herda")
        );
        let additional = create_card_account_inner(
            &pool,
            "Adicional",
            None,
            None,
            None,
            None,
            None,
            Some(&holder),
            &[],
        )
        .await
        .unwrap();
        assert_eq!(effective_cycle(&pool, &additional).await.unwrap(), (20, 10));
        assert!(
            create_card_account_inner(
                &pool,
                "Outro",
                None,
                Some(10),
                Some(5),
                None,
                None,
                None,
                &["azul".into()]
            )
            .await
            .unwrap_err()
            .contains("alias")
        );
    }

    #[tokio::test]
    async fn rejects_cycle_that_collides_after_february_due_date_clamp() {
        let pool = pool().await;
        let err = create_card_account_inner(
            &pool,
            "Ciclo inválido",
            None,
            Some(28),
            Some(29),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap_err();
        assert!(err.contains("fechamento no dia 28"), "erro: {err}");

        card(&pool, "Último dia seguro", 28, 28).await;
        card(&pool, "Vencimento 29 seguro", 20, 29).await;
    }

    #[tokio::test]
    async fn purchase_uses_correct_cycle_reuses_invoice_and_never_deadlocks_single_connection() {
        let pool = pool().await;
        let id = card(&pool, "Neko", 20, 10).await;
        let first =
            register_card_purchase_inner(&pool, &id, 2_500, Some("antes"), "2026-01-15", &[])
                .await
                .unwrap();
        let second =
            register_card_purchase_inner(&pool, &id, 1_000, Some("depois"), "2026-01-25", &[])
                .await
                .unwrap();
        let invoices = list_invoices_inner(&pool, &id).await.unwrap();
        assert_eq!(invoices.len(), 2);
        let first_invoice: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id=?1")
                .bind(&first)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            invoices
                .iter()
                .find(|i| i.id == first_invoice)
                .unwrap()
                .cycle_month,
            "2026-02"
        );
        assert_eq!(sqlx::query_scalar::<_,String>("SELECT i.cycle_month FROM invoice i JOIN \"transaction\" t ON t.invoice_id=i.id WHERE t.id=?1").bind(&second).fetch_one(&pool).await.unwrap(),"2026-03");
        let series = create_card_series_inner(&pool, &id, "Serviço", 100, Some(3), "2026-01-15")
            .await
            .unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id=?1")
                .bind(series)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn series_labels_and_stated_total_follow_the_authoritative_invoice() {
        let pool = pool().await;
        let id = card(&pool, "Neko", 20, 10).await;
        let series = create_card_series_inner(&pool, &id, "Parcelado", 500, Some(3), "2026-01-15")
            .await
            .unwrap();
        let inv = list_invoices_inner(&pool, &id).await.unwrap();
        for (position, invoice) in inv.iter().rev().enumerate() {
            let detail = get_invoice_inner(&pool, &invoice.id).await.unwrap();
            assert_eq!(
                detail.purchases[0].installment_label,
                Some(format!("{}/3", position + 1))
            );
        }
        let invoice = inv.last().unwrap();
        set_invoice_stated_total_inner(&pool, &invoice.id, Some(10_000))
            .await
            .unwrap();
        let purchase =
            register_card_purchase_inner(&pool, &id, 2_500, Some("extra"), "2026-01-15", &[])
                .await
                .unwrap();
        let detail = get_invoice_inner(&pool, &invoice.id).await.unwrap();
        assert_eq!(detail.summary.stated_total_cents, Some(12_500));
        assert_eq!(detail.summary.effective_total_cents, 12_500);
        move_card_purchase_inner(&pool, &purchase, "2026-03")
            .await
            .unwrap();
        let origin = get_invoice_inner(&pool, &invoice.id).await.unwrap();
        assert_eq!(origin.summary.stated_total_cents, Some(10_000));
        let _ = series;
    }

    #[tokio::test]
    async fn subscription_materializes_through_december_of_the_current_year() {
        let pool = pool().await;
        let id = card(&pool, "Neko", 20, 10).await;
        let today = chrono::Local::now().date_naive();
        let start = today.format("%Y-%m-%d").to_string();
        create_card_series_inner(&pool, &id, "Streaming", 4_990, None, &start)
            .await
            .unwrap();
        let cycles: Vec<(String,)> = sqlx::query_as(
            "SELECT i.cycle_month FROM \"transaction\" t JOIN invoice i ON i.id=t.invoice_id \
             WHERE t.card_series_id IS NOT NULL ORDER BY i.cycle_month",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let start_cycle = cards::cycle_month_of(cards::due_date_for_close(
            cards::cycle_close_for_purchase(today, 20),
            10,
        ));
        let (start_year, start_month) = cards::parse_cycle_month(&start_cycle).unwrap();
        let expected = ((today.year() - start_year) * 12 + (12 - start_month as i32) + 1).max(3);
        assert_eq!(cycles.len() as i32, expected);
        assert_eq!(cycles.first().unwrap().0, start_cycle);
        if expected > 3 {
            assert_eq!(cycles.last().unwrap().0, format!("{}-12", today.year()));
        }
    }

    #[tokio::test]
    async fn advancing_active_subscriptions_extends_an_old_subscription_through_current_december_idempotently()
     {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let today = chrono::Local::now().date_naive();
        let start_cycle = format!("{}-01", today.year() - 1);
        sqlx::query(
            "INSERT INTO card_series \
             (id, account_id, description, amount_cents, count, start_cycle_month) \
             VALUES ('old-subscription', ?1, 'Streaming', 4_990, NULL, ?2)",
        )
        .bind(&card_id)
        .bind(&start_cycle)
        .execute(&pool)
        .await
        .unwrap();

        advance_active_subscriptions(&pool).await.unwrap();
        let cycles: Vec<(String,)> = sqlx::query_as(
            "SELECT i.cycle_month FROM \"transaction\" t \
             JOIN invoice i ON i.id = t.invoice_id \
             WHERE t.card_series_id = 'old-subscription' ORDER BY i.cycle_month",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            cycles.first().map(|cycle| cycle.0.as_str()),
            Some(start_cycle.as_str())
        );
        assert_eq!(
            cycles.last().map(|cycle| cycle.0.as_str()),
            Some(format!("{}-12", today.year()).as_str())
        );
        let count_after_first_run = cycles.len();

        advance_active_subscriptions(&pool).await.unwrap();
        let count_after_second_run: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id = 'old-subscription'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count_after_second_run as usize, count_after_first_run);
    }

    #[tokio::test]
    async fn additive_accounting_floors_at_zero_and_follows_delete_and_cancel() {
        let pool = pool().await;
        let id = card(&pool, "Neko", 20, 10).await;
        let purchase = register_card_purchase_inner(&pool, &id, 2_500, None, "2026-01-15", &[])
            .await
            .unwrap();
        let invoice: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id=?1")
                .bind(&purchase)
                .fetch_one(&pool)
                .await
                .unwrap();
        // Sem stated: gestos não ajustam nada.
        let detail = get_invoice_inner(&pool, &invoice).await.unwrap();
        assert_eq!(detail.summary.stated_total_cents, None);
        assert_eq!(detail.summary.effective_total_cents, 2_500);
        // Piso em zero: stated menor que a compra removida nunca fica negativo.
        set_invoice_stated_total_inner(&pool, &invoice, Some(1_000))
            .await
            .unwrap();
        crate::commands::transactions::delete_transaction_inner(&pool, &purchase)
            .await
            .unwrap();
        let stated: Option<i64> =
            sqlx::query_scalar("SELECT stated_total_cents FROM invoice WHERE id=?1")
                .bind(&invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stated, Some(0));
        // Cancelamento devolve as ocorrências futuras ao stated das faturas.
        let series = create_card_series_inner(&pool, &id, "Streaming", 500, None, "2026-01-15")
            .await
            .unwrap();
        let future_cycle: String = sqlx::query_scalar(
            "SELECT i.cycle_month FROM \"transaction\" t JOIN invoice i ON i.id=t.invoice_id \
             WHERE t.card_series_id=?1 ORDER BY i.cycle_month DESC LIMIT 1",
        )
        .bind(&series)
        .fetch_one(&pool)
        .await
        .unwrap();
        let (before,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id=?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();
        cancel_card_series_inner(&pool, &series, &future_cycle)
            .await
            .unwrap();
        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id=?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, before - 1);
    }

    #[tokio::test]
    async fn refund_links_are_exclusive_and_visible_on_invoice() {
        let pool = pool().await;
        let card = card(&pool, "Neko", 20, 10).await;
        let purchase = register_card_purchase_inner(&pool, &card, 900, None, "2026-01-15", &[])
            .await
            .unwrap();
        let invoice: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id=?1")
                .bind(&purchase)
                .fetch_one(&pool)
                .await
                .unwrap();
        let refund = create_refund_expectation_inner(&pool, &invoice, 300, Some("parte"))
            .await
            .unwrap();
        assert!(
            link_refund_inner(&pool, &refund, Some(&invoice), Some(&purchase), None)
                .await
                .unwrap_err()
                .contains("exatamente")
        );
        let detail = get_invoice_inner(&pool, &invoice).await.unwrap();
        assert_eq!(detail.refunds.len(), 1);
        unlink_refund_inner(&pool, &refund).await.unwrap();
        let linked: Option<String> =
            sqlx::query_scalar("SELECT refund_invoice_id FROM \"transaction\" WHERE id=?1")
                .bind(&refund)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, None);
    }

    #[tokio::test]
    async fn registering_card_purchase_with_refund_links_the_same_invoice_and_rolls_back_on_tag_failure()
     {
        let card_pool = pool().await;
        let card_id = card(&card_pool, "Neko", 20, 10).await;
        let purchase = register_card_purchase_with_refund_inner(
            &card_pool,
            &card_id,
            2_500,
            Some("compra"),
            "2026-01-15",
            Some(800),
            &[],
        )
        .await
        .unwrap();
        let purchase_invoice: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id = ?1")
                .bind(&purchase)
                .fetch_one(&card_pool)
                .await
                .unwrap();
        let refund_invoice: Option<String> = sqlx::query_scalar(
            "SELECT refund_invoice_id FROM \"transaction\" WHERE type = 'income'",
        )
        .fetch_one(&card_pool)
        .await
        .unwrap();
        assert_eq!(refund_invoice.as_deref(), Some(purchase_invoice.as_str()));

        let rollback_pool = pool().await;
        let rollback_card = card(&rollback_pool, "Neko", 20, 10).await;
        let err = register_card_purchase_with_refund_inner(
            &rollback_pool,
            &rollback_card,
            2_500,
            Some("compra"),
            "2026-01-15",
            Some(800),
            &["tag-inexistente".into()],
        )
        .await
        .unwrap_err();
        assert!(err.contains("attach tag"), "erro: {err}");
        let transactions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(&rollback_pool)
            .await
            .unwrap();
        let invoices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
            .fetch_one(&rollback_pool)
            .await
            .unwrap();
        assert_eq!(transactions, 0, "falha não persiste compra nem reembolso");
        assert_eq!(invoices, 0, "falha também reverte a fatura criada");
    }

    #[tokio::test]
    async fn creating_card_series_with_refund_creates_one_income_linked_to_the_series() {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let series = create_card_series_with_refund_inner(
            &pool,
            &card_id,
            "Assinatura compartilhada",
            1_500,
            None,
            "2026-01-15",
            Some(500),
            &[],
        )
        .await
        .unwrap();
        let refunds: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT refund_series_id, refund_invoice_id FROM \"transaction\" WHERE type = 'income'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(refunds, vec![(Some(series), None)]);
    }

    /// Reembolso de série (`refund_series_id`) é criado UMA VEZ, junto da série, preso à data de
    /// vencimento da 1ª ocorrência. Cancelar a partir dessa mesma ocorrência tem de levar o
    /// reembolso junto — senão a Entrada projetada sobra sem nenhuma Saída que a justifique.
    #[tokio::test]
    async fn canceling_a_series_before_its_first_occurrence_removes_the_projected_refund_income() {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let start = chrono::Local::now().date_naive().to_string();
        let series = create_card_series_with_refund_inner(
            &pool,
            &card_id,
            "Assinatura compartilhada",
            1_500,
            None,
            &start,
            Some(500),
            &[],
        )
        .await
        .unwrap();
        let start_cycle_month: String =
            sqlx::query_scalar("SELECT start_cycle_month FROM card_series WHERE id = ?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();

        cancel_card_series_inner(&pool, &series, &start_cycle_month)
            .await
            .unwrap();

        let refunds: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM \"transaction\" WHERE refund_series_id = ?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            refunds, 0,
            "cancelar a partir da 1ª ocorrência não pode deixar a Entrada de reembolso projetada"
        );
    }

    /// `refund_series_id` tem `ON DELETE SET NULL` — apagar a série sem tratar o reembolso
    /// deixaria a Entrada com FK anulada: renda fantasma que não traceia mais para série nenhuma.
    #[tokio::test]
    async fn deleting_a_series_deletes_its_projected_refund_income_instead_of_orphaning_it() {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let start = chrono::Local::now().date_naive().to_string();
        let series = create_card_series_with_refund_inner(
            &pool,
            &card_id,
            "Assinatura compartilhada",
            1_500,
            None,
            &start,
            Some(500),
            &[],
        )
        .await
        .unwrap();

        delete_card_series_inner(&pool, &series).await.unwrap();

        let orphan_income: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM \"transaction\" WHERE type = 'income'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            orphan_income, 0,
            "apagar a série apaga a Entrada de reembolso — não deixa renda fantasma com FK nula"
        );
    }

    /// `transaction.card_series_id` é `ON DELETE CASCADE` (migração do domínio do cartão): apagar a
    /// série leva as ocorrências junto sem precisar de um DELETE explícito aqui. O `adjust_stated`
    /// que roda ANTES do delete precisa continuar descontando exatamente o valor das ocorrências —
    /// nem inflado (se o ajuste não rodasse) nem negativo (o piso em zero de `adjust_stated` cobre
    /// isso). Este teste trava o contrato para não regredir se a FK mudar de CASCADE para outra coisa.
    #[tokio::test]
    async fn deleting_a_series_cascades_its_occurrences_and_decrements_the_stated_total_without_going_negative()
     {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let series =
            create_card_series_inner(&pool, &card_id, "Parcelado", 500, Some(2), "2026-01-15")
                .await
                .unwrap();
        let occurrences: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT id, invoice_id, amount FROM \"transaction\" WHERE card_series_id = ?1 ORDER BY date",
        )
        .bind(&series)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            occurrences.len(),
            2,
            "série de 2 parcelas materializa 2 ocorrências"
        );
        let (_, first_invoice, first_amount) = &occurrences[0];
        let (_, second_invoice, second_amount) = &occurrences[1];
        assert_ne!(
            first_invoice, second_invoice,
            "cada parcela cai num ciclo/fatura diferente"
        );

        // Fatura A: stated ABAIXO da ocorrência — sem o piso em zero, o desconto ficaria negativo.
        set_invoice_stated_total_inner(&pool, first_invoice, Some(first_amount - 300))
            .await
            .unwrap();
        // Fatura B: stated bem acima da ocorrência — precisa DESCONTAR, não ficar inflada em 2_000.
        set_invoice_stated_total_inner(&pool, second_invoice, Some(2_000))
            .await
            .unwrap();

        delete_card_series_inner(&pool, &series).await.unwrap();

        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM \"transaction\" WHERE card_series_id = ?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            remaining, 0,
            "a FK CASCADE apaga as ocorrências junto da série"
        );

        let stated_a: Option<i64> =
            sqlx::query_scalar("SELECT stated_total_cents FROM invoice WHERE id = ?1")
                .bind(first_invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            stated_a,
            Some(0),
            "desconto não fica negativo — piso em zero"
        );

        let stated_b: Option<i64> =
            sqlx::query_scalar("SELECT stated_total_cents FROM invoice WHERE id = ?1")
                .bind(second_invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            stated_b,
            Some(2_000 - second_amount),
            "desconto aplicado — não fica inflado no valor anterior"
        );
    }

    #[tokio::test]
    async fn invoice_detail_lists_refund_linked_to_its_card_series() {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let start = chrono::Local::now().date_naive().to_string();
        let series = create_card_series_with_refund_inner(
            &pool,
            &card_id,
            "Assinatura compartilhada",
            1_500,
            Some(1),
            &start,
            Some(500),
            &[],
        )
        .await
        .unwrap();
        let invoice_id: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE card_series_id = ?1")
                .bind(&series)
                .fetch_one(&pool)
                .await
                .unwrap();

        let detail = get_invoice_inner(&pool, &invoice_id).await.unwrap();

        assert_eq!(detail.refunds.len(), 1);
        assert_eq!(detail.refunds[0].amount_cents, 500);
        assert_eq!(detail.refunds[0].description, "");
    }

    /// `refund_series_id` aponta para a SÉRIE, não para uma ocorrência — um único reembolso não
    /// pode aparecer em TODAS as faturas do ciclo (a lente líquida do drill dobraria a subtração
    /// a cada ciclo). Ele pertence a UMA fatura: a da 1ª ocorrência.
    #[tokio::test]
    async fn series_refund_is_listed_once_on_the_first_occurrences_invoice_only() {
        let pool = pool().await;
        let card_id = card(&pool, "Neko", 20, 10).await;
        let start = chrono::Local::now().date_naive().to_string();
        let series = create_card_series_with_refund_inner(
            &pool,
            &card_id,
            "Assinatura compartilhada",
            1_500,
            None,
            &start,
            Some(500),
            &[],
        )
        .await
        .unwrap();
        let occurrence_invoices: Vec<(String,)> = sqlx::query_as(
            "SELECT t.invoice_id FROM \"transaction\" t JOIN invoice i ON i.id = t.invoice_id \
             WHERE t.card_series_id = ?1 ORDER BY i.cycle_month",
        )
        .bind(&series)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(
            occurrence_invoices.len() >= 2,
            "assinatura materializa mais de um ciclo"
        );

        let first_detail = get_invoice_inner(&pool, &occurrence_invoices[0].0)
            .await
            .unwrap();
        assert_eq!(
            first_detail.refunds.len(),
            1,
            "a 1ª fatura lista o reembolso da série"
        );

        let later_detail = get_invoice_inner(&pool, &occurrence_invoices[1].0)
            .await
            .unwrap();
        assert_eq!(
            later_detail.refunds.len(),
            0,
            "faturas seguintes não repetem o mesmo reembolso"
        );
    }

    #[tokio::test]
    async fn holder_invoice_includes_each_additional_once() {
        let pool = pool().await;
        let holder = card(&pool, "Titular", 20, 10).await;
        let additional = create_card_account_inner(
            &pool,
            "Adicional",
            None,
            None,
            None,
            None,
            Some("Bia"),
            Some(&holder),
            &[],
        )
        .await
        .unwrap();
        let holder_purchase =
            register_card_purchase_inner(&pool, &holder, 1_000, None, "2026-01-15", &[])
                .await
                .unwrap();
        register_card_purchase_inner(&pool, &additional, 250, None, "2026-01-15", &[])
            .await
            .unwrap();
        let invoice: String =
            sqlx::query_scalar("SELECT invoice_id FROM \"transaction\" WHERE id=?1")
                .bind(holder_purchase)
                .fetch_one(&pool)
                .await
                .unwrap();
        let detail = get_invoice_inner(&pool, &invoice).await.unwrap();
        assert_eq!(detail.sub_invoices.len(), 1);
        assert_eq!(detail.emitter_total_cents, 1_250);
    }

    #[tokio::test]
    async fn proposal_acceptance_is_atomic_and_pending_list_excludes_dismissed() {
        let pool = pool().await;
        sqlx::query("INSERT INTO card_proposal (id,alias,display_name,source_month,status) VALUES ('p1','nubank','Nubank','2026-01','pending'),('p2','old','Old','2026-01','pending')").execute(&pool).await.unwrap();
        assert!(
            accept_card_proposal_inner(&pool, "p1", Some(0), Some(10), None, None)
                .await
                .is_err()
        );
        let state: String = sqlx::query_scalar("SELECT status FROM card_proposal WHERE id='p1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(state, "pending");
        let id = accept_card_proposal_inner(&pool, "p1", Some(20), Some(10), None, None)
            .await
            .unwrap();
        assert!(!id.is_empty());
        dismiss_card_proposal_inner(&pool, "p2").await.unwrap();
        assert!(list_card_proposals_inner(&pool).await.unwrap().is_empty());
    }

    /// O banco move o fechamento de um mês para outro e o molde do cartão não acompanha. A fatura
    /// já guardava as próprias datas; o que faltava era poder corrigi-las.
    #[tokio::test]
    async fn invoice_dates_are_correctable_without_touching_the_card_cycle() {
        let pool = pool().await;
        let card = card(&pool, "Cartão", 29, 12).await;
        let invoice = ensure_invoice_for_test(&pool, &card, "2026-02").await;

        let before: (String, String) =
            sqlx::query_as("SELECT closing_date, due_date FROM invoice WHERE id = ?1")
                .bind(&invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, ("2026-01-29".into(), "2026-02-12".into()));

        // Neste mês o emissor fechou dia 27.
        set_invoice_dates_inner(&pool, &invoice, "2026-01-27", "2026-02-12")
            .await
            .unwrap();
        let after: (String, String) =
            sqlx::query_as("SELECT closing_date, due_date FROM invoice WHERE id = ?1")
                .bind(&invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, ("2026-01-27".into(), "2026-02-12".into()));

        // O molde do cartão permanece: só aquele ciclo foi corrigido.
        assert_eq!(effective_cycle(&pool, &card).await.unwrap(), (29, 12));
    }

    #[tokio::test]
    async fn invoice_dates_reject_an_inverted_cycle_or_a_move_across_months() {
        let pool = pool().await;
        let card = card(&pool, "Cartão", 29, 12).await;
        let invoice = ensure_invoice_for_test(&pool, &card, "2026-02").await;

        assert!(
            set_invoice_dates_inner(&pool, &invoice, "2026-02-12", "2026-02-12")
                .await
                .is_err(),
            "fechar no dia do vencimento não é um ciclo"
        );
        // Mover o vencimento de mês faria a identidade cartão×mês mentir.
        let moved = set_invoice_dates_inner(&pool, &invoice, "2026-02-05", "2026-03-12").await;
        assert!(moved.is_err());
        assert!(moved.unwrap_err().contains("2026-02"));

        let untouched: (String, String) =
            sqlx::query_as("SELECT closing_date, due_date FROM invoice WHERE id = ?1")
                .bind(&invoice)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(untouched, ("2026-01-29".into(), "2026-02-12".into()));
    }

    /// Aceitar a proposta leva TODAS as grafias junto — é isso que impede a mesma fatura de ser
    /// lida por um cartão num mês e por nenhum no outro.
    #[tokio::test]
    async fn accepting_a_proposal_carries_every_alias_the_sheet_uses() {
        let pool = pool().await;
        sqlx::query(
            "INSERT INTO card_proposal (id,alias,display_name,source_month,status) \
             VALUES ('p1','nubank','Nubank','2026-01','pending')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO card_proposal_alias (id,proposal_id,alias) \
             VALUES ('a1','p1','nubank (26/09)'),('a2','p1','nubank (26/12)')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let listed = list_card_proposals_inner(&pool).await.unwrap();
        assert_eq!(listed[0].aliases.len(), 3);
        assert_eq!(listed[0].aliases[0], "nubank", "a identidade vem primeiro");

        let account = accept_card_proposal_inner(&pool, "p1", Some(20), Some(10), None, None)
            .await
            .unwrap();
        let aliases: Vec<(String,)> =
            sqlx::query_as("SELECT alias FROM card_alias WHERE account_id = ?1 ORDER BY alias")
                .bind(&account)
                .fetch_all(&pool)
                .await
                .unwrap();
        let aliases: Vec<String> = aliases.into_iter().map(|(a,)| a).collect();
        assert!(aliases.contains(&"nubank (26/09)".to_string()));
        assert!(aliases.contains(&"nubank (26/12)".to_string()));
    }

    /// O caminho que faltava: a proposta é outra grafia de um cartão que já existe. Sem ele,
    /// aceitar sempre criava conta nova e repartia a fatura de um mesmo cartão em duas.
    #[tokio::test]
    async fn attaching_a_proposal_adds_aliases_to_an_existing_card_without_creating_another() {
        let pool = pool().await;
        let emissor = card(&pool, "Emissor", 20, 12).await;
        sqlx::query(
            "INSERT INTO card_proposal (id,alias,display_name,source_month,status) \
             VALUES ('p1','emissor titular','Emissor Titular','2025-10','pending')",
        )
        .execute(&pool)
        .await
        .unwrap();

        attach_card_proposal_inner(&pool, "p1", &emissor)
            .await
            .unwrap();

        let cards: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account WHERE type='credit_card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cards, 1, "nenhuma conta nova");
        let owner: Option<String> =
            sqlx::query_scalar("SELECT account_id FROM card_alias WHERE alias = 'emissor titular'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(owner.as_deref(), Some(emissor.as_str()));
        assert!(
            list_card_proposals_inner(&pool).await.unwrap().is_empty(),
            "a proposta sai da fila resolvida"
        );
    }

    #[tokio::test]
    async fn attaching_a_proposal_rejects_a_destination_that_is_not_a_card() {
        let pool = pool().await;
        sqlx::query(
            "INSERT INTO card_proposal (id,alias,display_name,source_month,status) \
             VALUES ('p1','nubank','Nubank','2026-01','pending')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            attach_card_proposal_inner(&pool, "p1", "conta-inexistente")
                .await
                .is_err()
        );
        let state: String = sqlx::query_scalar("SELECT status FROM card_proposal WHERE id='p1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(state, "pending", "destino inválido não resolve a proposta");
    }

    #[tokio::test]
    async fn proposal_acceptance_as_additional_inherits_cycle_and_rejects_days() {
        let pool = pool().await;
        let holder = card(&pool, "Titular", 20, 10).await;
        sqlx::query(
            "INSERT INTO card_proposal (id,alias,display_name,source_month,status) \
             VALUES ('p1','adicional','Cartão adicional','2026-01','pending'), \
                    ('p2','com-dias','Com dias','2026-01','pending')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let additional =
            accept_card_proposal_inner(&pool, "p1", None, None, Some("Bia"), Some(&holder))
                .await
                .unwrap();
        assert_eq!(effective_cycle(&pool, &additional).await.unwrap(), (20, 10));

        let error =
            accept_card_proposal_inner(&pool, "p2", Some(20), Some(10), Some("Bia"), Some(&holder))
                .await
                .unwrap_err();
        assert!(error.contains("herda"));
    }
}
