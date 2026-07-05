//! Plan 069: user-confirmed "obligation" identity — the recurring series the spreadsheet
//! doesn't store. `obligation` is a NEKO EXTENSION (not a method artifact): the method has no
//! concept linking twelve monthly "Aluguel" line items into one series. The user names a
//! recurring item ONCE ("Aluguel"); every line item whose (normalized) description — and,
//! optionally, section — matches is treated as an occurrence, ALWAYS shown via a confirm-preview
//! before saving. Never inferred, never silent.
//!
//! `line_item` rows are RE-DERIVED from the cell note on every import (see
//! `google_sheets::import`), so identity can't live on the row itself — it would be wiped on the
//! next sync. `obligation` stores a MATCH RULE (normalized description + optional normalized
//! section) and membership is resolved at query time. This is a view/index: it never mutates
//! amounts and never touches the cell-owns-total rule.

use crate::google_sheets::import::{self, ItemKind};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct Obligation {
    pub id: String,
    pub person_id: String,
    pub name: String,
    pub match_desc: String,
    pub match_section: Option<String>,
    pub kind: String,
}

/// Uma ocorrência de `line_item` que casou com a regra de uma obrigação. `date` vem do
/// lançamento pai (a nota não carrega data própria) — usada para agrupar por mês em
/// `obligation_history`.
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct ObligationLineItem {
    pub line_item_id: String,
    pub transaction_id: String,
    pub amount_cents: i64,
    pub description: String,
    pub date: String,
}

/// Total de um mês (`YYYY-MM`) para uma obrigação: soma + nº de ocorrências.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ObligationMonthTotal {
    pub year: i32,
    pub month: u32,
    pub total_cents: i64,
    pub count: i64,
}

// --- Normalização (locais a este módulo; ver banner acima sobre por que não há coluna) ---

/// Remove um contador de parcela final NÃO ESPAÇADO (`"3/36"`) e o espaço que o precede.
/// Uma parcela real embute esse contador MUTÁVEL dentro da descrição (ele muda a cada mês:
/// `1/36`, `2/36`, ...), então uma comparação exata falharia entre meses sem este passo.
/// Puro, sem regex: varre os dois grupos de dígitos separados por `/` a partir do fim.
fn strip_trailing_installment_counter(s: &str) -> &str {
    let s = s.trim_end();
    let bytes = s.as_bytes();

    // Segundo número (depois da barra).
    let mut i = s.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i == s.len() {
        return s; // não termina em dígito.
    }
    if i == 0 || bytes[i - 1] != b'/' {
        return s; // dígito final não é precedido por uma barra.
    }
    let slash = i - 1;

    // Primeiro número (antes da barra).
    let mut j = slash;
    while j > 0 && bytes[j - 1].is_ascii_digit() {
        j -= 1;
    }
    if j == slash {
        return s; // nada antes da barra.
    }

    s[..j].trim_end()
}

/// Casefold + accent-fold, LOCAL a este módulo (deliberado — plano 069 rejeita compartilhar o
/// helper do plano 071, que não fazia accent-fold e por isso quebrava "Aluguel" x "ALUGUÉL").
fn fold_case_accents(s: &str) -> String {
    let mut normalized = String::with_capacity(s.len());
    for ch in s.chars().flat_map(char::to_lowercase) {
        match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => normalized.push('a'),
            'é' | 'è' | 'ê' | 'ë' => normalized.push('e'),
            'í' | 'ì' | 'î' | 'ï' => normalized.push('i'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => normalized.push('o'),
            'ú' | 'ù' | 'û' | 'ü' => normalized.push('u'),
            'ç' => normalized.push('c'),
            other => normalized.push(other),
        }
    }
    normalized
}

/// Normaliza uma descrição de item para casar entre meses: remove o contador de parcela final
/// (quando há um nome textual além do contador), funde caixa/acento. NUNCA usado para exibição
/// — só para comparação da regra de casamento.
pub(crate) fn normalize_desc(description: &str) -> String {
    let trimmed = description.trim();
    let stripped = strip_trailing_installment_counter(trimmed);
    // Só descarta o contador se o prefixo restante tiver pelo menos duas letras (um nome
    // mínimo); senão "R$ 3/4" colapsaria para "r$" e "3/4" para vazio, gerando falsos
    // positivos / regras vazias.
    let alphabetic_count = stripped.chars().filter(|c| c.is_alphabetic()).count();
    if alphabetic_count >= 2 {
        fold_case_accents(stripped)
    } else {
        fold_case_accents(trimmed)
    }
}

/// Normaliza uma seção para casar contra `obligation.match_section`. Reusa
/// `google_sheets::import::normalize_item_section` — mesma função que já resolve "CONTAS" (2025,
/// sem `:`) e "FATURAS:" (2026, com `:`) para o mesmo valor, então a fronteira de ano não quebra
/// o casamento.
pub(crate) fn normalize_section(section: &str) -> String {
    import::normalize_item_section(section)
}

fn kind_slug(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Saida => "saida",
        ItemKind::Cartao => "cartao",
        ItemKind::Diario => "diario",
        ItemKind::Economia => "economia",
        ItemKind::Patrimonio => "patrimonio",
        ItemKind::Ajuste => "ajuste",
    }
}

/// Uma linha candidata: TODO line_item + a data do lançamento pai. Buscamos o universo inteiro
/// (sem filtro de pessoa — `obligation.person_id` é só autoria, nunca faz parte do casamento) e
/// filtramos em Rust, porque o casamento depende de `normalize_desc`/`normalize_section`
/// (fold de acento + strip de contador de parcela), que não têm equivalente direto em SQL puro.
#[derive(Debug, sqlx::FromRow)]
struct CandidateRow {
    line_item_id: String,
    transaction_id: String,
    amount_cents: i64,
    description: String,
    section: Option<String>,
    date: String,
}

async fn fetch_candidate_items(pool: &SqlitePool) -> Result<Vec<CandidateRow>, String> {
    sqlx::query_as::<_, CandidateRow>(
        "SELECT li.id AS line_item_id, li.transaction_id, li.amount_cents, li.description, \
                li.section, t.date \
         FROM line_item li \
         JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.scenario_id IS NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("fetch_candidate_items: {e}"))
}

/// Regra de casamento (idêntica no preview e no resolver de uma obrigação salva): descrição
/// normalizada IGUAL, e (seção-alvo ausente OU seção normalizada IGUAL).
fn item_matches(row: &CandidateRow, match_desc: &str, match_section: Option<&str>) -> bool {
    if normalize_desc(&row.description) != match_desc {
        return false;
    }
    match match_section {
        None => true,
        Some(want) => row
            .section
            .as_deref()
            .map(|s| normalize_section(s) == want)
            .unwrap_or(false),
    }
}

fn to_obligation_line_item(row: CandidateRow) -> ObligationLineItem {
    ObligationLineItem {
        line_item_id: row.line_item_id,
        transaction_id: row.transaction_id,
        amount_cents: row.amount_cents,
        description: row.description,
        date: row.date,
    }
}

/// Prévia do casamento ANTES de salvar (confirm-preview obrigatório, plano 069): mesma regra do
/// resolver, sem persistir nada. `match_desc_raw`/`match_section_raw` chegam em texto livre (o
/// que o usuário digitou/a seção crua); normalizamos aqui do mesmo jeito que `create_obligation`
/// normaliza antes de gravar, então "nº mostrado no preview" == "nº agrupado após salvar".
pub async fn preview_obligation_matches(
    pool: &SqlitePool,
    match_desc_raw: &str,
    match_section_raw: Option<&str>,
) -> Result<Vec<ObligationLineItem>, String> {
    let match_desc = normalize_desc(match_desc_raw);
    let match_section = match_section_raw.map(normalize_section);
    let rows = fetch_candidate_items(pool).await?;
    Ok(rows
        .into_iter()
        .filter(|r| item_matches(r, &match_desc, match_section.as_deref()))
        .map(to_obligation_line_item)
        .collect())
}

/// Cria a obrigação (regra normalizada + autoria). NÃO é o preview — a UI deve chamar
/// `preview_obligation_matches` primeiro e só chegar aqui após confirmação explícita do usuário.
pub async fn create_obligation(
    pool: &SqlitePool,
    name: &str,
    match_desc_raw: &str,
    match_section_raw: Option<&str>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    let match_desc = normalize_desc(match_desc_raw);
    if match_desc.is_empty() {
        return Err("descrição de casamento vazia após normalizar".into());
    }
    let match_section = match_section_raw.map(normalize_section);
    let kind = kind_slug(import::classify_line_item(match_section.as_deref(), ""));

    // Mesmo bootstrap de "Eu" usado por `create_account_inner`: obligation.person_id é só
    // autoria (nunca entra no casamento), então não exigimos seleção de pessoa na UI.
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
        "INSERT INTO obligation (id, person_id, name, match_desc, match_section, kind) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(&owner_id)
    .bind(name)
    .bind(&match_desc)
    .bind(&match_section)
    .bind(kind)
    .execute(pool)
    .await
    .map_err(|e| format!("create_obligation: {e}"))?;
    Ok(id)
}

pub async fn list_obligations(pool: &SqlitePool) -> Result<Vec<Obligation>, String> {
    sqlx::query_as::<_, Obligation>(
        "SELECT id, person_id, name, match_desc, match_section, kind FROM obligation \
         ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_obligations: {e}"))
}

pub async fn delete_obligation(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let rows = sqlx::query("DELETE FROM obligation WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete_obligation: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!("obligation not found: {id}"));
    }
    Ok(())
}

/// Todas as ocorrências (line items) casadas pela regra salva de uma obrigação.
pub async fn obligation_items(
    pool: &SqlitePool,
    obligation_id: &str,
) -> Result<Vec<ObligationLineItem>, String> {
    let ob: Obligation = sqlx::query_as(
        "SELECT id, person_id, name, match_desc, match_section, kind FROM obligation \
         WHERE id = ?1",
    )
    .bind(obligation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("obligation_items: {e}"))?
    .ok_or_else(|| format!("obligation not found: {obligation_id}"))?;

    let rows = fetch_candidate_items(pool).await?;
    Ok(rows
        .into_iter()
        .filter(|r| item_matches(r, &ob.match_desc, ob.match_section.as_deref()))
        .map(to_obligation_line_item)
        .collect())
}

/// Totais por mês (`YYYY-MM`) das ocorrências de uma obrigação — a série que a planilha não
/// guarda. Meses sem ocorrência simplesmente não aparecem (não há "mês zero" sintético).
pub async fn obligation_history(
    pool: &SqlitePool,
    obligation_id: &str,
) -> Result<Vec<ObligationMonthTotal>, String> {
    let items = obligation_items(pool, obligation_id).await?;
    let mut by_month: std::collections::BTreeMap<(i32, u32), (i64, i64)> =
        std::collections::BTreeMap::new();
    for item in &items {
        // `date` é sempre ISO "YYYY-MM-DD" (mesma convenção usada em todo o resto do app).
        let Some(year) = item.date.get(0..4).and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let Some(month) = item.date.get(5..7).and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let entry = by_month.entry((year, month)).or_insert((0, 0));
        entry.0 += item.amount_cents;
        entry.1 += 1;
    }
    Ok(by_month
        .into_iter()
        .map(
            |((year, month), (total_cents, count))| ObligationMonthTotal {
                year,
                month,
                total_cents,
                count,
            },
        )
        .collect())
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn preview_obligation_matches_cmd(
    pool: State<'_, SqlitePool>,
    match_desc: String,
    match_section: Option<String>,
) -> Result<Vec<ObligationLineItem>, String> {
    preview_obligation_matches(pool.inner(), &match_desc, match_section.as_deref()).await
}

#[tauri::command]
pub async fn create_obligation_cmd(
    pool: State<'_, SqlitePool>,
    name: String,
    match_desc: String,
    match_section: Option<String>,
) -> Result<String, String> {
    create_obligation(pool.inner(), &name, &match_desc, match_section.as_deref()).await
}

#[tauri::command]
pub async fn list_obligations_cmd(pool: State<'_, SqlitePool>) -> Result<Vec<Obligation>, String> {
    list_obligations(pool.inner()).await
}

#[tauri::command]
pub async fn delete_obligation_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    delete_obligation(pool.inner(), &id).await
}

#[tauri::command]
pub async fn obligation_items_cmd(
    pool: State<'_, SqlitePool>,
    obligation_id: String,
) -> Result<Vec<ObligationLineItem>, String> {
    obligation_items(pool.inner(), &obligation_id).await
}

#[tauri::command]
pub async fn obligation_history_cmd(
    pool: State<'_, SqlitePool>,
    obligation_id: String,
) -> Result<Vec<ObligationMonthTotal>, String> {
    obligation_history(pool.inner(), &obligation_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn txn(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES (?1,'expense',?2,?3,0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn line_item(
        pool: &SqlitePool,
        id: &str,
        txn_id: &str,
        amount_cents: i64,
        description: &str,
        section: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES (?1,?2,?3,?4,0,?5)",
        )
        .bind(id)
        .bind(txn_id)
        .bind(amount_cents)
        .bind(description)
        .bind(section)
        .execute(pool)
        .await
        .unwrap();
    }

    // --- normalize_desc ---

    #[test]
    fn normalize_desc_folds_case_and_accents() {
        assert_eq!(normalize_desc("Aluguel "), "aluguel");
        assert_eq!(normalize_desc("aluguel"), "aluguel");
        assert_eq!(normalize_desc("ALUGUEL"), "aluguel");
        assert_eq!(normalize_desc("Água"), "agua");
    }

    #[test]
    fn normalize_desc_strips_trailing_installment_counter() {
        assert_eq!(normalize_desc("Netflix 1/36"), "netflix");
        assert_eq!(normalize_desc("Netflix 2/36"), "netflix");
        assert_eq!(normalize_desc("Netflix 12/36"), "netflix");
        // Sem contador: passa direto (só normaliza caixa/acento).
        assert_eq!(normalize_desc("Netflix"), "netflix");
        // Números que não são um contador de parcela (não terminam a descrição) ficam intactos.
        assert_eq!(normalize_desc("Compra 12/2026 loja"), "compra 12/2026 loja");
        // O contador muda entre meses, mas a base nomeada permanece — deve casar.
        assert_eq!(
            normalize_desc("Aluguel 12/36"),
            normalize_desc("Aluguel 5/36")
        );
    }

    #[test]
    fn normalize_desc_does_not_strip_counter_when_prefix_has_few_letters() {
        // Apenas o contador: sem letras, não há nome para casar — mantém tudo.
        assert_eq!(normalize_desc("3/4"), "3/4");
        // Prefixo com menos de duas letras (símbolo monetário): o "3/4" é parte do valor,
        // não uma parcela — não descarta.
        assert_eq!(normalize_desc("R$ 3/4"), "r$ 3/4");
    }

    // --- normalize_section ---

    #[test]
    fn normalize_section_matches_across_year_boundary_note_grammar() {
        // "CONTAS" (2025, sem ":") e "CONTAS:" (2026, com ":") -> mesmo valor normalizado.
        assert_eq!(normalize_section("CONTAS"), normalize_section("CONTAS:"));
        assert_eq!(normalize_section("Contas:"), "contas");
    }

    // --- resolver / preview ---

    #[tokio::test]
    async fn preview_and_resolver_agree_on_the_same_count() {
        let p = pool().await;
        txn(&p, "t1", -150000, "2026-01-05").await;
        txn(&p, "t2", -150000, "2026-02-05").await;
        txn(&p, "t3", -150000, "2026-03-05").await;
        txn(&p, "t4", -8000, "2026-03-06").await; // não relacionado

        line_item(&p, "li1", "t1", 150000, "Aluguel", Some("CONTAS:")).await;
        line_item(&p, "li2", "t2", 150000, "aluguel", Some("CONTAS:")).await;
        line_item(&p, "li3", "t3", 150000, "ALUGUEL", Some("CONTAS:")).await;
        line_item(&p, "li4", "t4", 8000, "Mercado", Some("DIÁRIO:")).await;

        let preview = preview_obligation_matches(&p, "Aluguel", Some("contas"))
            .await
            .unwrap();
        assert_eq!(
            preview.len(),
            3,
            "3 meses de aluguel, sem o item não relacionado"
        );

        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", Some("contas"))
            .await
            .unwrap();
        let resolved = obligation_items(&p, &ob_id).await.unwrap();
        assert_eq!(
            resolved.len(),
            preview.len(),
            "nº mostrado no preview == nº agrupado após salvar"
        );

        let preview_ids: std::collections::HashSet<_> =
            preview.iter().map(|li| &li.line_item_id).collect();
        let resolved_ids: std::collections::HashSet<_> =
            resolved.iter().map(|li| &li.line_item_id).collect();
        assert_eq!(
            preview_ids, resolved_ids,
            "preview e resolver devem devolver o MESMO conjunto de line_item ids"
        );

        let history = obligation_history(&p, &ob_id).await.unwrap();
        assert_eq!(history.len(), 3, "3 meses distintos");
        for month_total in &history {
            assert_eq!(month_total.total_cents, 150000);
            assert_eq!(month_total.count, 1);
        }
    }

    #[tokio::test]
    async fn resolver_matches_across_year_boundary_note_grammar() {
        let p = pool().await;
        txn(&p, "t1", -150000, "2025-12-05").await;
        txn(&p, "t2", -150000, "2026-01-05").await;

        // 2025: seção sem ":". 2026: seção com ":". Mesma obrigação deve casar as duas.
        line_item(&p, "li1", "t1", 150000, "Aluguel", Some("CONTAS")).await;
        line_item(&p, "li2", "t2", 150000, "Aluguel", Some("CONTAS:")).await;

        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", Some("CONTAS:"))
            .await
            .unwrap();
        let resolved = obligation_items(&p, &ob_id).await.unwrap();
        assert_eq!(
            resolved.len(),
            2,
            "sem quebra silenciosa na fronteira de ano"
        );
    }

    #[tokio::test]
    async fn resolver_strips_installment_counter_across_months() {
        let p = pool().await;
        txn(&p, "t1", -5000, "2026-01-10").await;
        txn(&p, "t2", -5000, "2026-02-10").await;
        txn(&p, "t3", -5000, "2026-03-10").await;

        line_item(
            &p,
            "li1",
            "t1",
            5000,
            "Financiamento 1/36",
            Some("CARTÕES:"),
        )
        .await;
        line_item(
            &p,
            "li2",
            "t2",
            5000,
            "Financiamento 2/36",
            Some("CARTÕES:"),
        )
        .await;
        line_item(
            &p,
            "li3",
            "t3",
            5000,
            "Financiamento 3/36",
            Some("CARTÕES:"),
        )
        .await;

        let ob_id = create_obligation(&p, "Financiamento", "Financiamento 1/36", Some("CARTÕES:"))
            .await
            .unwrap();
        let resolved = obligation_items(&p, &ob_id).await.unwrap();
        assert_eq!(
            resolved.len(),
            3,
            "contador de parcela mutável não deve quebrar o casamento entre meses"
        );
    }

    #[tokio::test]
    async fn create_obligation_derives_kind_from_section() {
        let p = pool().await;
        let ob_id = create_obligation(&p, "Fatura cartão", "Fatura", Some("FATURAS:"))
            .await
            .unwrap();
        let obs = list_obligations(&p).await.unwrap();
        let ob = obs.iter().find(|o| o.id == ob_id).unwrap();
        assert_eq!(ob.kind, "cartao");
    }

    #[tokio::test]
    async fn create_obligation_without_section_defaults_kind_to_saida() {
        let p = pool().await;
        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let obs = list_obligations(&p).await.unwrap();
        let ob = obs.iter().find(|o| o.id == ob_id).unwrap();
        assert_eq!(ob.kind, "saida");
        assert_eq!(ob.match_section, None);
    }

    // --- delete: cascade + never mutates line items ---

    #[tokio::test]
    async fn delete_obligation_leaves_line_items_untouched() {
        let p = pool().await;
        txn(&p, "t1", -150000, "2026-01-05").await;
        line_item(&p, "li1", "t1", 150000, "Aluguel", Some("CONTAS:")).await;
        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", Some("CONTAS:"))
            .await
            .unwrap();

        delete_obligation(&p, &ob_id).await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM line_item WHERE id = 'li1'")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, 1, "apagar a obrigação nunca apaga/edita line_item");
        let (amount,): (i64,) =
            sqlx::query_as("SELECT amount_cents FROM line_item WHERE id = 'li1'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            amount, 150000,
            "valor do item intacto (view/index, nunca muta)"
        );
    }

    #[tokio::test]
    async fn delete_obligation_rejects_unknown_id() {
        let p = pool().await;
        assert!(delete_obligation(&p, "nope").await.is_err());
    }

    #[tokio::test]
    async fn deleting_person_cascades_to_obligation() {
        let p = pool().await;
        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", Some("CONTAS:"))
            .await
            .unwrap();
        let (person_id,): (String,) =
            sqlx::query_as("SELECT person_id FROM obligation WHERE id = ?1")
                .bind(&ob_id)
                .fetch_one(&p)
                .await
                .unwrap();

        sqlx::query("DELETE FROM person WHERE id = ?1")
            .bind(&person_id)
            .execute(&p)
            .await
            .unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM obligation WHERE id = ?1")
            .bind(&ob_id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, 0, "ON DELETE CASCADE de person remove a obrigação");
    }

    // --- accepted limitation: renaming an item's description drops it from the match ---

    #[tokio::test]
    async fn renaming_a_matched_item_description_drops_it_from_the_match() {
        // Limitação aceita (plano 069): a identidade é a REGRA de descrição normalizada, não um
        // vínculo persistido no line_item (ele é re-derivado a cada import). Se o usuário edita a
        // descrição via `update_transaction_items_cmd`, o item deixa de casar — documentado, não
        // corrigido aqui (corrigir exigiria uma FK em line_item que não sobrevive ao re-import).
        let p = pool().await;
        txn(&p, "t1", -150000, "2026-01-05").await;
        line_item(&p, "li1", "t1", 150000, "Aluguel", Some("CONTAS:")).await;
        let ob_id = create_obligation(&p, "Aluguel", "Aluguel", Some("CONTAS:"))
            .await
            .unwrap();
        assert_eq!(obligation_items(&p, &ob_id).await.unwrap().len(), 1);

        // Simula a reescrita de `update_transaction_items_cmd` (clear + reinsert com nova
        // descrição) sem depender do módulo de comandos.
        sqlx::query("UPDATE line_item SET description = ?1 WHERE id = 'li1'")
            .bind("Aluguel apto novo")
            .execute(&p)
            .await
            .unwrap();

        let resolved = obligation_items(&p, &ob_id).await.unwrap();
        assert!(
            resolved.is_empty(),
            "renomear a descrição tira o item do casamento (limitação aceita, não um bug)"
        );
    }
}
