//! CRUD do teto de diário e do orçamento por categoria: cadastro (upsert), leitura de tela e a
//! cerimônia de proposta (aceite/dispensa). A LEITURA que alimenta a projeção — qual teto é o
//! driver do forecast — vive em `forecast_cmds` (`effective_daily_ceiling`/`daily_ceiling_reading`);
//! este módulo é só o que o dono cadastra e a tela de configuração lê de volta.
use super::*;

/// Núcleo puro do upsert do teto diário (testável sem o `State` do Tauri).
/// Depreca TODOS os registros ativos anteriores e insere um novo com `status='active'` quando
/// `amount_cents > 0`. `amount_cents = 0` apenas depreca (desativa o teto explícito — o engine
/// cai no fallback de média do mês anterior em `effective_daily_ceiling`).
pub(crate) async fn upsert_daily_budget_inner(
    pool: &SqlitePool,
    amount_cents: i64,
) -> Result<(), String> {
    // Obtém o person_id do primeiro perfil (padrão single-user).
    let person: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("upsert_daily_budget (person): {e}"))?;
    let Some((person_id,)) = person else {
        // Nenhum perfil ainda — silencioso (usuário novo sem import).
        return Ok(());
    };
    // Depreca os registros ativos anteriores (todos, não só o primeiro).
    sqlx::query("UPDATE daily_budget SET status='deprecated' WHERE status='active'")
        .execute(pool)
        .await
        .map_err(|e| format!("upsert_daily_budget (deprecate): {e}"))?;

    if amount_cents > 0 {
        let id = uuid::Uuid::new_v4().to_string();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status) \
             VALUES (?1, ?2, ?3, ?4, 'active')",
        )
        .bind(&id)
        .bind(&person_id)
        .bind(amount_cents)
        .bind(&today)
        .execute(pool)
        .await
        .map_err(|e| format!("upsert_daily_budget (insert): {e}"))?;
    }
    Ok(())
}

/// Grava (ou atualiza) o teto diário configurado pelo dono (gasto variável ativo).
/// Adapter fino sobre `upsert_daily_budget_inner` (funcional-core / imperative-shell).
#[tauri::command]
pub async fn upsert_daily_budget(
    pool: State<'_, SqlitePool>,
    amount_cents: i64,
) -> Result<(), String> {
    upsert_daily_budget_inner(pool.inner(), amount_cents).await
}

// --- Quebra por categoria do orçamento Diário ---

/// Uma categoria do orçamento mensal do Diário (leitura). `amount_cents` é o alvo mensal positivo.
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct DailyBudgetCategoryRow {
    pub id: String,
    pub name: String,
    pub amount_cents: i64,
    pub position: i64,
}

/// Entrada de categoria vinda do app (escrita). `position` é a ordem 0-based de exibição.
#[derive(serde::Deserialize)]
pub struct CategoryInput {
    pub name: String,
    pub amount_cents: i64,
    pub position: i64,
}

/// Puro: valor MENSAL → teto por DIA. O teto diário do método é o orçamento mensal dividido
/// pelo divisor da cerimônia, arredondando o resto PARA CIMA (teto é teto: a cerimônia real
/// declara 40,33 para 1250 ÷ 31 = 40,3225…). `days_in_month = 0` → 0 (sem panic). Fonte da
/// verdade da fórmula — a derivação da tela do teto (TypeScript) espelha esta regra 1:1.
#[allow(dead_code)]
pub(crate) fn monthly_to_daily_rate(amount_cents: i64, days_in_month: u32) -> i64 {
    if days_in_month == 0 {
        return 0;
    }
    let days = i64::from(days_in_month);
    (amount_cents + days - 1) / days
}

/// Núcleo puro: grava o teto total do Diário + uma quebra opcional por categoria.
///
/// A substituição do orçamento ativo ocorre numa única `sqlx::Transaction`: desativa os registros
/// ativos, insere o total sucessor e troca as categorias. Uma falha parcial não pode deixar um
/// orçamento ativo sem categorias ou com categorias de outro total.
/// `upsert_daily_budget_inner` atende o caminho simples.
///
/// 1. Espelha o `upsert_daily_budget_inner`: depreca os ativos e insere o novo TOTAL (engine
///    inalterado — `effective_daily_ceiling` lê `daily_budget WHERE status='active' AND amount>0`).
/// 2. Se `amount_cents > 0` e `categories` não-vazio: usa o id recém-inserido (sem SELECT extra) e
///    substitui (DELETE + INSERT) as categorias.
/// 3. `categories` vazio: nada a fazer na tabela de categorias (o total-only continua válido).
///
/// Validação: cada `category.amount_cents > 0`; senão retorna Err sem tocar no banco.
pub(crate) async fn upsert_daily_budget_with_categories_inner(
    pool: &SqlitePool,
    amount_cents: i64,
    categories: &[CategoryInput],
    divisor_days: Option<i64>,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("upsert daily budget (begin): {e}"))?;
    upsert_daily_budget_with_categories_tx(&mut tx, amount_cents, categories, divisor_days, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| format!("upsert daily budget (commit): {e}"))?;
    Ok(())
}

/// Proveniência da cerimônia que produziu o teto: a nota da planilha que a tela reproduz e o mês
/// em que a cerimônia foi feita. `None` no upsert = cerimônia do rito no app (mês corrente, sem
/// nota) — o registro sucessor nasce limpo, então a nota de uma proposta antiga nunca sobrevive a
/// uma cerimônia nova.
pub(crate) struct CeremonyProvenance<'a> {
    pub source_note: Option<&'a str>,
    /// `YYYY-MM` da cerimônia (o mês da nota, não o do aceite).
    pub ceremony_month: &'a str,
}

/// Núcleo em transação JÁ ABERTA — o caller é dono do commit (o aceite de proposta compõe este
/// upsert com a marcação da proposta no mesmo tudo-ou-nada).
pub(crate) async fn upsert_daily_budget_with_categories_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    amount_cents: i64,
    categories: &[CategoryInput],
    divisor_days: Option<i64>,
    provenance: Option<CeremonyProvenance<'_>>,
) -> Result<(), String> {
    // Valida ANTES de escrever (atomicidade lógica: ou tudo válido, ou nada muda).
    for c in categories {
        if c.amount_cents <= 0 {
            return Err("cada categoria deve ter valor positivo (magnitude)".into());
        }
    }
    if let Some(d) = divisor_days
        && d <= 0
    {
        return Err("o divisor de dias deve ser positivo".into());
    }

    // Obtém o person_id do primeiro perfil (padrão single-user) — igual ao `upsert_daily_budget_inner`.
    let person: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| format!("upsert_daily_budget (person): {e}"))?;
    let Some((person_id,)) = person else {
        // Nenhum perfil ainda — silencioso (usuário novo sem import).
        return Ok(());
    };

    // Depreca os registros ativos anteriores (todos, não só o primeiro).
    sqlx::query("UPDATE daily_budget SET status='deprecated' WHERE status='active'")
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("upsert_daily_budget (deprecate): {e}"))?;

    if amount_cents > 0 {
        let budget_id = uuid::Uuid::new_v4().to_string();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let ceremony_month = provenance
            .as_ref()
            .map_or_else(|| today[..7].to_string(), |p| p.ceremony_month.to_string());
        sqlx::query(
            "INSERT INTO daily_budget \
             (id, person_id, amount, start_date, status, divisor_days, source_note, ceremony_month) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7)",
        )
        .bind(&budget_id)
        .bind(&person_id)
        .bind(amount_cents)
        .bind(&today)
        .bind(divisor_days)
        .bind(provenance.as_ref().and_then(|p| p.source_note))
        .bind(&ceremony_month)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("upsert_daily_budget (insert): {e}"))?;

        // Só anexa categorias quando há um teto explícito ativo E uma quebra informada. Usa o
        // `budget_id` recém-inserido (sem SELECT extra) → não há janela entre inserir e categorizar.
        if !categories.is_empty() {
            sqlx::query("DELETE FROM daily_budget_category WHERE budget_id = ?1")
                .bind(&budget_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("upsert categories (clear): {e}"))?;
            for c in categories {
                sqlx::query(
                    "INSERT INTO daily_budget_category (id, budget_id, name, amount_cents, position) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&budget_id)
                .bind(&c.name)
                .bind(c.amount_cents)
                .bind(c.position)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("upsert categories (insert): {e}"))?;
            }
        }
    }
    Ok(())
}

/// Núcleo puro: categorias do orçamento Diário ATIVO (vazio quando não há orçamento/quebra).
pub(crate) async fn get_daily_budget_categories_inner(
    pool: &SqlitePool,
) -> Result<Vec<DailyBudgetCategoryRow>, String> {
    sqlx::query_as::<_, DailyBudgetCategoryRow>(
        "SELECT dbc.id, dbc.name, dbc.amount_cents, dbc.position \
         FROM daily_budget_category dbc \
         JOIN daily_budget db ON db.id = dbc.budget_id \
         WHERE db.status='active' ORDER BY dbc.position",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("get daily budget categories: {e}"))
}

/// Grava o teto total do Diário + a quebra por categoria. Adapter fino sobre o núcleo puro.
#[tauri::command]
pub async fn upsert_daily_budget_with_categories_cmd(
    pool: State<'_, SqlitePool>,
    amount_cents: i64,
    categories: Vec<CategoryInput>,
    divisor_days: Option<i64>,
) -> Result<(), String> {
    upsert_daily_budget_with_categories_inner(pool.inner(), amount_cents, &categories, divisor_days)
        .await
}

/// Lê as categorias do orçamento Diário ativo (vazio = sem quebra). Adapter fino.
#[tauri::command]
pub async fn get_daily_budget_categories_cmd(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<DailyBudgetCategoryRow>, String> {
    get_daily_budget_categories_inner(pool.inner()).await
}

/// Orçamento Diário ativo por inteiro (valor/dia + divisor da cerimônia + itens mensais) — a
/// leitura da tela do teto. `per_day_cents = 0` ⇒ sem teto estipulado.
#[derive(serde::Serialize)]
pub struct DailyBudgetDto {
    pub per_day_cents: i64,
    pub divisor_days: Option<i64>,
    /// `YYYY-MM` em que a cerimônia foi feita — a idade que a tela conta para convidar à
    /// recalibração. `None` só em orçamento sem registro.
    pub ceremony_month: Option<String>,
    /// Nota crua da célula que documenta a cerimônia, quando o teto nasceu de uma proposta
    /// aceita. `None` = cerimônia feita no app (não há nota da planilha para reproduzir).
    pub source_note: Option<String>,
    pub categories: Vec<DailyBudgetCategoryRow>,
}

/// Linha do orçamento ativo como o banco a devolve (o DTO acrescenta as categorias).
#[derive(sqlx::FromRow, Default)]
struct ActiveBudgetRow {
    amount: i64,
    divisor_days: Option<i64>,
    ceremony_month: Option<String>,
    source_note: Option<String>,
}

pub(crate) async fn get_daily_budget_inner(pool: &SqlitePool) -> Result<DailyBudgetDto, String> {
    let active = sqlx::query_as::<_, ActiveBudgetRow>(
        "SELECT amount, divisor_days, ceremony_month, source_note FROM daily_budget \
         WHERE status='active' AND amount > 0 ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("get_daily_budget: {e}"))?
    .unwrap_or_default();
    Ok(DailyBudgetDto {
        per_day_cents: active.amount,
        divisor_days: active.divisor_days,
        ceremony_month: active.ceremony_month,
        source_note: active.source_note,
        categories: get_daily_budget_categories_inner(pool).await?,
    })
}

#[tauri::command]
pub async fn get_daily_budget_cmd(pool: State<'_, SqlitePool>) -> Result<DailyBudgetDto, String> {
    get_daily_budget_inner(pool.inner()).await
}

/// Proposta de teto pendente lida da cerimônia da planilha (uma por vez, por construção).
#[derive(serde::Serialize)]
pub struct CeilingProposalDto {
    pub id: String,
    pub per_day_cents: i64,
    pub divisor_days: i64,
    pub source_month: String,
    /// Nota crua da célula, reproduzida na tela como prova. `None` em propostas registradas
    /// antes da coluna de proveniência existir.
    pub raw_note: Option<String>,
    pub items: Vec<CeilingProposalItemDto>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CeilingProposalItemDto {
    pub name: String,
    pub amount_cents: i64,
}

pub(crate) async fn get_ceiling_proposal_inner(
    pool: &SqlitePool,
) -> Result<Option<CeilingProposalDto>, String> {
    let row: Option<(String, i64, i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, per_day_cents, divisor_days, source_month, items_json, raw_note \
         FROM ceiling_proposal WHERE status='pending' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("get_ceiling_proposal: {e}"))?;
    let Some((id, per_day_cents, divisor_days, source_month, items_json, raw_note)) = row else {
        return Ok(None);
    };
    // Fronteira interna, mas ainda parse-validado: um items_json corrompido não pode derrubar a
    // tela — degrada para proposta sem itens.
    let items: Vec<CeilingProposalItemDto> = serde_json::from_str(&items_json).unwrap_or_default();
    Ok(Some(CeilingProposalDto {
        id,
        per_day_cents,
        divisor_days,
        source_month,
        raw_note,
        items,
    }))
}

#[tauri::command]
pub async fn get_ceiling_proposal_cmd(
    pool: State<'_, SqlitePool>,
) -> Result<Option<CeilingProposalDto>, String> {
    get_ceiling_proposal_inner(pool.inner()).await
}

/// Aceite EXPLÍCITO da proposta: grava o orçamento (valor/dia + itens + divisor) e marca a
/// proposta como aceita, no mesmo tudo-ou-nada.
pub(crate) async fn accept_ceiling_proposal_inner(
    pool: &SqlitePool,
    proposal_id: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("accept proposal (begin): {e}"))?;
    let row: Option<(i64, i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT per_day_cents, divisor_days, items_json, source_month, raw_note \
         FROM ceiling_proposal WHERE id = ?1 AND status = 'pending'",
    )
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("accept proposal (lookup): {e}"))?;
    let Some((per_day_cents, divisor_days, items_json, source_month, raw_note)) = row else {
        return Err("proposta de teto não encontrada ou já resolvida".into());
    };
    let items: Vec<CeilingProposalItemDto> = serde_json::from_str(&items_json).unwrap_or_default();
    let categories: Vec<CategoryInput> = items
        .into_iter()
        .enumerate()
        .map(|(i, it)| CategoryInput {
            name: it.name,
            amount_cents: it.amount_cents,
            position: i as i64,
        })
        .collect();
    // A cerimônia aceita continua sendo a cerimônia da NOTA: a idade que a tela conta corre do
    // mês em que o dono a escreveu na planilha, não do dia do aceite.
    upsert_daily_budget_with_categories_tx(
        &mut tx,
        per_day_cents,
        &categories,
        Some(divisor_days),
        Some(CeremonyProvenance {
            source_note: raw_note.as_deref(),
            ceremony_month: &source_month,
        }),
    )
    .await?;
    sqlx::query(
        "UPDATE ceiling_proposal SET status='accepted', resolved_at=datetime('now') WHERE id=?1",
    )
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("accept proposal (mark): {e}"))?;
    tx.commit()
        .await
        .map_err(|e| format!("accept proposal (commit): {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn accept_ceiling_proposal_cmd(
    pool: State<'_, SqlitePool>,
    proposal_id: String,
) -> Result<(), String> {
    accept_ceiling_proposal_inner(pool.inner(), &proposal_id).await
}

/// Dispensa a proposta: some da UI e a MESMA nota nunca re-propõe (identidade por hash).
pub(crate) async fn dismiss_ceiling_proposal_inner(
    pool: &SqlitePool,
    proposal_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE ceiling_proposal SET status='dismissed', resolved_at=datetime('now') \
         WHERE id = ?1 AND status = 'pending'",
    )
    .bind(proposal_id)
    .execute(pool)
    .await
    .map_err(|e| format!("dismiss proposal: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn dismiss_ceiling_proposal_cmd(
    pool: State<'_, SqlitePool>,
    proposal_id: String,
) -> Result<(), String> {
    dismiss_ceiling_proposal_inner(pool.inner(), &proposal_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::forecast_cmds::effective_daily_ceiling;

    async fn pool() -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    /// Insere um perfil — pré-condição de `upsert_daily_budget_inner` (escreve por person_id).
    async fn seed_person(pool: &SqlitePool) {
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-045', 'Tester')")
            .execute(pool)
            .await
            .unwrap();
    }

    fn cat(name: &str, amount_cents: i64, position: i64) -> CategoryInput {
        CategoryInput {
            name: name.into(),
            amount_cents,
            position,
        }
    }

    #[tokio::test]
    async fn upsert_daily_budget_with_categories_stores_breakdown() {
        let p = pool().await;
        seed_person(&p).await;
        // Total 1250,00 quebrado em 3 categorias genéricas que somam o total.
        let cats = vec![
            cat("Transport", 30000, 0),
            cat("Groceries", 50000, 1),
            cat("Leisure", 45000, 2),
        ];
        upsert_daily_budget_with_categories_inner(&p, 125000, &cats, None)
            .await
            .unwrap();

        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert_eq!(rows.len(), 3, "as 3 categorias persistem");
        let sum: i64 = rows.iter().map(|r| r.amount_cents).sum();
        assert_eq!(sum, 125000, "a soma das categorias bate com o total");
        // A ordem segue `position`.
        assert_eq!(rows[0].name, "Transport");
        assert_eq!(rows[2].name, "Leisure");

        // O TOTAL continua na tabela daily_budget (engine inalterado).
        let total = effective_daily_ceiling(&p, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap())
            .await
            .unwrap();
        assert_eq!(total, 125000, "o teto total ativo é o escrito");

        // Reescrever substitui (clear + reinsert), não acumula.
        upsert_daily_budget_with_categories_inner(&p, 125000, &[cat("Shopping", 125000, 0)], None)
            .await
            .unwrap();
        let rows2 = get_daily_budget_categories_inner(&p).await.unwrap();
        assert_eq!(rows2.len(), 1, "a quebra anterior foi substituída");
        assert_eq!(rows2[0].name, "Shopping");
    }

    #[tokio::test]
    async fn upsert_daily_budget_with_categories_without_cats_ok() {
        let p = pool().await;
        seed_person(&p).await;
        // Sem quebra: grava só o total; nenhuma categoria inserida.
        upsert_daily_budget_with_categories_inner(&p, 60000, &[], None)
            .await
            .unwrap();
        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert!(rows.is_empty(), "total-only não cria categorias");
        let total = effective_daily_ceiling(&p, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap())
            .await
            .unwrap();
        assert_eq!(total, 60000);
    }

    #[tokio::test]
    async fn upsert_daily_budget_with_categories_deprecates_old() {
        let p = pool().await;
        seed_person(&p).await;
        upsert_daily_budget_with_categories_inner(&p, 100000, &[cat("Groceries", 100000, 0)], None)
            .await
            .unwrap();
        // Segunda chamada depreca o orçamento anterior e cria nova quebra no novo orçamento ativo.
        upsert_daily_budget_with_categories_inner(&p, 80000, &[cat("Transport", 80000, 0)], None)
            .await
            .unwrap();

        // Só UM orçamento ativo (o novo); o anterior foi deprecado.
        let active: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_budget WHERE status='active'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            active.0, 1,
            "um único orçamento ativo após o segundo upsert"
        );

        // A leitura traz só as categorias do orçamento ATIVO.
        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Transport");
    }

    #[tokio::test]
    async fn upsert_daily_budget_with_categories_rejects_zero_category() {
        let p = pool().await;
        seed_person(&p).await;
        let err = upsert_daily_budget_with_categories_inner(&p, 50000, &[cat("Bad", 0, 0)], None)
            .await
            .unwrap_err();
        assert!(err.contains("positivo"), "err: {err}");
        // Nada foi escrito (validação antes de qualquer write).
        let any: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_budget WHERE status='active'")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(any.0, 0, "categoria inválida não grava nem o total");
    }

    #[tokio::test]
    async fn upsert_daily_budget_with_categories_is_atomic() {
        // Total + categorias gravam numa ÚNICA transação. No caminho feliz, ambos
        // confirmam juntos; nenhum orçamento ATIVO fica sem suas categorias.
        let p = pool().await;
        seed_person(&p).await;

        upsert_daily_budget_with_categories_inner(
            &p,
            10000,
            &[cat("Alpha", 6000, 0), cat("Beta", 4000, 1)],
            None,
        )
        .await
        .unwrap();

        // Exatamente um orçamento ativo e suas duas categorias, ambos presentes (atômico).
        let active: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_budget WHERE status='active'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(active.0, 1, "um orçamento ativo");
        let cats: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_budget_category")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(
            cats.0, 2,
            "as duas categorias foram gravadas junto com o total"
        );

        // Nenhuma categoria pende de um orçamento deprecado (todas referenciam o ativo).
        let orphan: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM daily_budget_category c \
             JOIN daily_budget b ON b.id = c.budget_id \
             WHERE b.status <> 'active'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(orphan.0, 0, "nenhuma categoria sob orçamento deprecado");

        // Desativar (amount_cents = 0): nenhum orçamento ativo; a leitura do ATIVO não traz categorias.
        upsert_daily_budget_with_categories_inner(&p, 0, &[], None)
            .await
            .unwrap();
        let active_after: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM daily_budget WHERE status='active'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(active_after.0, 0, "desativado: nenhum orçamento ativo");
        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert!(
            rows.is_empty(),
            "sem orçamento ativo, a quebra ativa lida é vazia"
        );
    }

    #[tokio::test]
    async fn get_daily_budget_categories_returns_empty_without_budget() {
        let p = pool().await;
        // Sem orçamento ativo → vetor vazio (não-panic).
        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn monthly_to_daily_rate_divides_correctly() {
        assert_eq!(monthly_to_daily_rate(3100, 31), 100); // divisão exata
        // Resto arredonda PARA CIMA (teto é teto): 4032,25… → 40,33, como a cerimônia real.
        assert_eq!(monthly_to_daily_rate(125000, 31), 4033);
        assert_eq!(monthly_to_daily_rate(100, 0), 0, "dias=0 não causa panic");
    }

    #[tokio::test]
    async fn accept_and_dismiss_ceiling_proposal() {
        let p = pool().await;
        seed_person(&p).await;
        sqlx::query(
            "INSERT INTO ceiling_proposal (id, note_hash, per_day_cents, divisor_days, items_json, source_month, status) \
             VALUES ('cp-1', 'h1', 4033, 31, '[{\"name\":\"Mercado\",\"amount_cents\":125000}]', '2026-05', 'pending')",
        )
        .execute(&p)
        .await
        .unwrap();

        accept_ceiling_proposal_inner(&p, "cp-1").await.unwrap();
        let budget = get_daily_budget_inner(&p).await.unwrap();
        assert_eq!(budget.per_day_cents, 4_033);
        assert_eq!(budget.divisor_days, Some(31));
        assert_eq!(budget.categories.len(), 1);
        assert_eq!(budget.categories[0].name, "Mercado");
        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM ceiling_proposal WHERE id='cp-1'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(status, "accepted");
        // Aceite repetido de proposta já resolvida é erro honesto, não regrava.
        assert!(accept_ceiling_proposal_inner(&p, "cp-1").await.is_err());

        sqlx::query(
            "INSERT INTO ceiling_proposal (id, note_hash, per_day_cents, divisor_days, items_json, source_month, status) \
             VALUES ('cp-2', 'h2', 2000, 30, '[]', '2026-06', 'pending')",
        )
        .execute(&p)
        .await
        .unwrap();
        dismiss_ceiling_proposal_inner(&p, "cp-2").await.unwrap();
        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM ceiling_proposal WHERE id='cp-2'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(status, "dismissed");
        assert!(get_ceiling_proposal_inner(&p).await.unwrap().is_none());
    }

    // A proveniência da cerimônia sobrevive ao aceite: a nota crua vira a prova reproduzida na
    // tela e a idade da cerimônia corre do mês da NOTA, não do dia do aceite. Uma cerimônia
    // feita depois, no app, nasce sem nota — a prova passa a ser a do app.
    #[tokio::test]
    async fn ceiling_provenance_survives_accept_and_resets_on_app_ceremony() {
        let p = pool().await;
        seed_person(&p).await;
        let note = "Mensal  R$ 1250,00  Variável\nR$ 1250,00 / 31 Dias = R$ 40,33";
        sqlx::query(
            "INSERT INTO ceiling_proposal \
             (id, note_hash, per_day_cents, divisor_days, items_json, source_month, status, raw_note) \
             VALUES ('cp-9', 'h9', 4033, 31, '[{\"name\":\"Variável\",\"amount_cents\":125000}]', '2025-09', 'pending', ?1)",
        )
        .bind(note)
        .execute(&p)
        .await
        .unwrap();

        let pending = get_ceiling_proposal_inner(&p).await.unwrap().unwrap();
        assert_eq!(pending.raw_note.as_deref(), Some(note));

        accept_ceiling_proposal_inner(&p, "cp-9").await.unwrap();
        let budget = get_daily_budget_inner(&p).await.unwrap();
        assert_eq!(budget.source_note.as_deref(), Some(note));
        assert_eq!(budget.ceremony_month.as_deref(), Some("2025-09"));

        // Cerimônia refeita no app: registro sucessor sem nota, com o mês corrente.
        upsert_daily_budget_with_categories_inner(
            &p,
            4355,
            &[cat("Variável", 135_000, 0)],
            Some(31),
        )
        .await
        .unwrap();
        let refeito = get_daily_budget_inner(&p).await.unwrap();
        assert_eq!(refeito.per_day_cents, 4_355);
        assert_eq!(refeito.source_note, None, "a nota antiga não sobrevive");
        assert_eq!(
            refeito.ceremony_month.as_deref(),
            Some(&chrono::Local::now().format("%Y-%m").to_string()[..]),
        );
    }
}
