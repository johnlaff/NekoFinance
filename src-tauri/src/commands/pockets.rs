use super::*;

// --- Pockets & liquidity ---

#[derive(serde::Serialize)]
pub struct PocketAccount {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub liquidity: Option<String>,
    pub balance: i64,
    pub institution: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Pockets {
    pub liquid_cents: i64,
    pub reserve_cents: i64,
    pub restricted_cents: i64,
    pub illiquid_cents: i64,
    /// liquid + reserve + illiquid; restricted (vale) is tracked apart and the
    /// credit-card liability belongs to the invoice slice.
    pub net_worth_cents: i64,
    pub accounts: Vec<PocketAccount>,
}

/// Deterministic liquidity class per account type.
pub(crate) fn liquidity_for_type(account_type: &str) -> Option<&'static str> {
    match account_type {
        "bank" | "wallet" | "business" => Some("liquid"),
        "savings" => Some("reserve"),
        "meal_voucher" => Some("restricted"),
        "pension" | "fgts" => Some("illiquid"),
        _ => None, // credit_card: liability, not a pocket
    }
}

/// Pure aggregation over the account list (functional core, unit-tested).
pub(crate) fn aggregate_pockets(accounts: Vec<PocketAccount>) -> Pockets {
    let sum = |class: &str| -> i64 {
        accounts
            .iter()
            .filter(|a| a.liquidity.as_deref() == Some(class))
            .map(|a| a.balance)
            .sum()
    };
    let (liquid, reserve, restricted, illiquid) = (
        sum("liquid"),
        sum("reserve"),
        sum("restricted"),
        sum("illiquid"),
    );
    Pockets {
        liquid_cents: liquid,
        reserve_cents: reserve,
        restricted_cents: restricted,
        illiquid_cents: illiquid,
        net_worth_cents: liquid + reserve + illiquid,
        accounts,
    }
}

#[tauri::command]
pub async fn get_pockets(pool: State<'_, SqlitePool>) -> Result<Pockets, String> {
    pockets(pool.inner()).await
}

pub(crate) async fn pockets(pool: &SqlitePool) -> Result<Pockets, String> {
    type PocketRow = (String, String, String, Option<String>, i64, Option<String>);
    let rows: Vec<PocketRow> = sqlx::query_as(
        "SELECT id, name, type, liquidity, balance, institution FROM account \
         WHERE type != 'credit_card' ORDER BY created_at, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    Ok(aggregate_pockets(
        rows.into_iter()
            .map(
                |(id, name, t, liquidity, balance, institution)| PocketAccount {
                    id,
                    name,
                    r#type: t,
                    liquidity,
                    balance,
                    institution,
                },
            )
            .collect(),
    ))
}

#[tauri::command]
pub async fn create_account(
    pool: State<'_, SqlitePool>,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    create_account_inner(pool.inner(), name, account_type, balance_cents, institution).await
}

pub(crate) async fn create_account_inner(
    pool: &SqlitePool,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    let liquidity = liquidity_for_type(&account_type)
        .ok_or_else(|| format!("tipo inválido: {account_type}"))?;

    // Pockets exist before any sheet import; ensure the default owner person.
    // Atomic insert-if-empty so concurrent calls cannot both bootstrap an "Eu".
    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .map_err(|e| format!("create person: {e}"))?;
    let (owner_id,): (String,) =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, institution, balance, liquidity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&id)
    .bind(name)
    .bind(&account_type)
    .bind(&owner_id)
    .bind(&institution)
    .bind(balance_cents)
    .bind(liquidity)
    .execute(pool)
    .await
    .map_err(|e| format!("create account: {e}"))?;

    Ok(id)
}
