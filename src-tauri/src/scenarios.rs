//! Plano 072 (slice B) — o motor do "what-if": CRUD de cenários hipotéticos + o compare de
//! forecast (real × cenário) + a ferramenta determinística de empréstimo (tabela PRICE).
//!
//! Um `scenario` é só um rótulo (nome + autoria); as linhas hipotéticas em si são
//! `"transaction"` rows com `scenario_id` setado (slice A). Um `scenario_override` é uma AÇÃO
//! sobre o livro-razão REAL, escopada a uma obrigação (plano 069) ou a uma série recorrente —
//! nunca sobre o cenário em si. Nada aqui muta o livro-razão real: os overrides só afetam a
//! PROJEÇÃO do cenário (`get_scenario_forecast`); o forecast real (`get_forecast`) continua
//! cego à existência de qualquer cenário (slice A garantiu isso via `scenario_id IS NULL`).
//!
//! CONVENÇÕES DE MARCA NA DESCRIÇÃO (documentadas aqui porque não há coluna própria; a UI da
//! slice C REMOVE os sufixos ao exibir):
//!
//! - EMPRÉSTIMO: a UI marca as linhas hipotéticas de um empréstimo anexando
//!   `" #loan:<group_id>:<taxa_bps>"` ao FINAL da `description` de CADA linha do grupo (a
//!   Entrada do principal + as N Saídas/Cartão das parcelas). `get_scenario_forecast` detecta o
//!   grupo por essa marca (ancorada ao fim — um "#loan:" no meio do texto não conta), usa a
//!   linha de tipo `income` como o principal e as `expense` como as parcelas (magnitude da
//!   primeira parcela = o valor da prestação, assumidas iguais — a UI usa `price_installment`
//!   para gerá-las). Só o PRIMEIRO grupo (ordem data,id) vira `loan`; os grupos seguintes
//!   aparecem como entradas "add" comuns em `changes` (limitação aceita desta slice).
//!
//! - SUBSTITUIÇÃO (`replace`): quando `set_scenario_override` recebe `op = "replace"` com um
//!   `replacement` preenchido, ele mesmo cria a linha hipotética de substituição, anexando
//!   `" #repl:<override_id>"` ao final da descrição. É esse marcador que permite ao compare
//!   FUNDIR o par velho→novo numa única entrada `{op:"replace", old, new}` de `changes` (e
//!   excluir a linha da lista de "add"). Não há remoção individual de override: a limpeza das
//!   linhas pareadas ocorre em cascata quando o cenário é apagado.

use crate::commands::forecast_cmds::{
    self, forecast_horizon_end, load_economia_annotation, load_forecast_events, load_metric_events,
    projection_seed, reserve_floor,
};
use crate::commands::map_cashflow_row;
use crate::forecast::{self, CashflowEvent};
use crate::obligations;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tauri::State;

// --- CRUD: scenario ---

#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub person_id: String,
}

/// Mesmo bootstrap de "Eu" usado em `obligations::create_obligation`/`create_account_inner`:
/// `scenario.person_id` é só autoria (nunca entra em nenhuma regra de negócio).
async fn bootstrap_person(pool: &SqlitePool) -> Result<String, String> {
    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .map_err(|e| format!("bootstrap person: {e}"))?;
    let (owner_id,): (String,) =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;
    Ok(owner_id)
}

pub async fn create_scenario(pool: &SqlitePool, name: &str) -> Result<Scenario, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome do cenário obrigatório".into());
    }
    let person_id = bootstrap_person(pool).await?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO scenario (id, name, person_id) VALUES (?1, ?2, ?3)")
        .bind(&id)
        .bind(name)
        .bind(&person_id)
        .execute(pool)
        .await
        .map_err(|e| format!("create_scenario: {e}"))?;
    Ok(Scenario {
        id,
        name: name.to_string(),
        person_id,
    })
}

pub async fn list_scenarios(pool: &SqlitePool) -> Result<Vec<Scenario>, String> {
    sqlx::query_as::<_, Scenario>("SELECT id, name, person_id FROM scenario ORDER BY created_at")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("list_scenarios: {e}"))
}

/// Apaga o cenário. `transaction.scenario_id` e `scenario_override.scenario_id` são
/// `ON DELETE CASCADE` (slice A/A.1) — apagar aqui já limpa as linhas hipotéticas e os overrides.
pub async fn delete_scenario(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let rows = sqlx::query("DELETE FROM scenario WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete_scenario: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!("scenario not found: {id}"));
    }
    Ok(())
}

// --- CRUD: transações hipotéticas do cenário ---

async fn scenario_exists(pool: &SqlitePool, scenario_id: &str) -> Result<bool, String> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scenario WHERE id = ?1")
        .bind(scenario_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("scenario_exists: {e}"))?;
    Ok(n > 0)
}

/// Insere uma linha hipotética "e se" no cenário. Espelha `create_transaction_inner`
/// (mesmas validações de tipo/conta-destino), MAS nunca toca `account.balance` — o caminho real
/// já não muta saldo no lançamento manual, então basta reusar o mesmo INSERT com `scenario_id`
/// setado. `description` é OBRIGATÓRIA aqui (diferente do caminho real): uma linha hipotética
/// sem rótulo não tem como aparecer como "Adição" legível no compare (`ScenarioChange`).
#[allow(clippy::too_many_arguments)]
pub async fn add_scenario_transaction(
    pool: &SqlitePool,
    scenario_id: &str,
    txn_type: &str,
    amount_cents: i64,
    description: &str,
    date: &str,
    payment_method: Option<&str>,
    is_fixed: bool,
    to_account_id: Option<&str>,
    due_date: Option<&str>,
) -> Result<String, String> {
    if !scenario_exists(pool, scenario_id).await? {
        return Err(format!("scenario not found: {scenario_id}"));
    }
    let description = description.trim();
    if description.is_empty() {
        return Err("descrição obrigatória para uma linha de cenário".into());
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let dest = match txn_type {
        "income" | "expense" => {
            if to_account_id.is_some_and(|s| !s.is_empty()) {
                return Err("conta-destino só se aplica a transfer (Economia)".into());
            }
            None
        }
        "transfer" => {
            let dest_id = to_account_id
                .filter(|s| !s.is_empty())
                .ok_or("transfer requer conta-destino (to_account_id)")?;
            let row: Option<(String,)> =
                sqlx::query_as("SELECT COALESCE(liquidity,'') FROM account WHERE id = ?1")
                    .bind(dest_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| format!("query account: {e}"))?;
            match row {
                None => return Err("conta-destino não encontrada".into()),
                Some((liq,)) if liq == "reserve" || liq == "illiquid" => {}
                Some((liq,)) => {
                    return Err(format!(
                        "conta-destino deve ter liquidez 'reserve' ou 'illiquid', encontrado '{liq}'"
                    ));
                }
            }
            Some(dest_id)
        }
        other => return Err(format!("tipo inválido: {other}")),
    };
    let start = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| format!("data: {e}"))?;
    let today = chrono::Local::now().date_naive();
    // A projeção do cenário só varre do mês corrente em diante (janela de métrica começa no dia
    // 1º do mês de hoje): uma linha anterior a isso sumiria SILENCIOSAMENTE dos dois ramos.
    // Rejeitar é o comportamento honesto mais simples.
    let month_start =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("data de hoje inválida")?;
    if start < month_start {
        return Err("data anterior ao mês corrente não entra na projeção do cenário".into());
    }
    let is_projection = start > today;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO \"transaction\" \
           (id, type, amount, description, date, payment_method, is_fixed, to_account_id, \
            is_projection, due_date, scenario_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
    )
    .bind(&id)
    .bind(txn_type)
    .bind(amount_cents)
    .bind(description)
    .bind(date)
    .bind(payment_method)
    .bind(is_fixed as i64)
    .bind(dest)
    .bind(is_projection as i64)
    .bind(due_date)
    .bind(scenario_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("add_scenario_transaction: {e}"))?;
    Ok(id)
}

/// Apaga uma linha hipotética. Só apaga se `scenario_id` casar — impede apagar uma linha REAL ou
/// de OUTRO cenário pelo id.
pub async fn delete_scenario_transaction(
    pool: &SqlitePool,
    scenario_id: &str,
    txn_id: &str,
) -> Result<(), String> {
    let rows = sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1 AND scenario_id = ?2")
        .bind(txn_id)
        .bind(scenario_id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete_scenario_transaction: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!(
            "scenario transaction not found: {txn_id} (scenario {scenario_id})"
        ));
    }
    Ok(())
}

/// Uma linha hipotética crua do cenário, para a UI listar/apagar (fatia C). A descrição chega
/// com os sufixos de marca (`#loan:`/`#repl:`) ainda anexados — a UI é quem os remove ao exibir
/// (ver banner do módulo); mantê-los aqui preserva a identidade do grupo/par para quem precisar.
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq)]
pub struct ScenarioTransactionRow {
    pub id: String,
    pub r#type: String,
    pub amount: i64,
    pub description: String,
    pub date: String,
}

/// Lista as linhas hipotéticas (`transaction.scenario_id = ?`) de um cenário, mais recentes
/// primeiro. Só leitura — não participa de nenhum cálculo, é a fonte da lista editável do
/// side-sheet (a UI some sem isto: sem um id de volta, não dá pra oferecer apagar uma linha
/// depois que a sessão perde o retorno de `add_scenario_transaction`).
pub async fn list_scenario_transactions(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<Vec<ScenarioTransactionRow>, String> {
    sqlx::query_as::<_, ScenarioTransactionRow>(
        "SELECT id, type, amount, COALESCE(description,'') AS description, date \
         FROM \"transaction\" WHERE scenario_id = ?1 ORDER BY date DESC, id DESC",
    )
    .bind(scenario_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_scenario_transactions: {e}"))
}

// --- CRUD: scenario_override ---

#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct ScenarioOverride {
    pub id: String,
    pub scenario_id: String,
    pub op: String,
    pub from_date: String,
    pub obligation_id: Option<String>,
    pub recurrence_id: Option<String>,
}

/// A linha hipotética de SUBSTITUIÇÃO que acompanha um override `replace` (opcional). Quando
/// presente, `set_scenario_override` cria a linha ele mesmo e a marca com `#repl:<override_id>`
/// no fim da descrição — o pareamento determinístico velho→novo do compare (ver banner do
/// módulo). Defaults: `txn_type = "expense"`, `is_fixed = true` (uma obrigação substituída é
/// tipicamente uma Saída fixa), `description = nome genérico`.
#[derive(Debug, serde::Deserialize, Clone)]
pub struct ReplacementInput {
    pub amount_cents: i64,
    pub date: String,
    pub description: Option<String>,
    pub txn_type: Option<String>,
    pub payment_method: Option<String>,
    pub is_fixed: Option<bool>,
}

/// Cria um override (`suppress`/`replace`) sobre uma obrigação OU uma recorrência (exatamente
/// uma — o CHECK XOR do banco endurece isso; validamos antes para devolver um erro legível em
/// vez do texto cru do driver SQLite). Um SEGUNDO override para o mesmo alvo no mesmo cenário é
/// rejeitado: dois overrides somariam a mesma supressão duas vezes e derrubariam irmãos de uma
/// célula multi-item (ver `build_suppression_plan`, que ainda deduplica por defesa em
/// profundidade). Para `op = "replace"`, um `replacement` opcional cria a linha hipotética de
/// substituição pareada (marca `#repl:<override_id>`).
pub async fn set_scenario_override(
    pool: &SqlitePool,
    scenario_id: &str,
    op: &str,
    from_date: &str,
    obligation_id: Option<&str>,
    recurrence_id: Option<&str>,
    replacement: Option<ReplacementInput>,
) -> Result<String, String> {
    if !scenario_exists(pool, scenario_id).await? {
        return Err(format!("scenario not found: {scenario_id}"));
    }
    if op != "suppress" && op != "replace" {
        return Err(format!(
            "op inválido: {op} (esperado 'suppress' ou 'replace')"
        ));
    }
    if replacement.is_some() && op != "replace" {
        return Err("replacement só se aplica a op = 'replace'".into());
    }
    NaiveDate::parse_from_str(from_date, "%Y-%m-%d").map_err(|e| format!("from_date: {e}"))?;
    let obligation_id = obligation_id.filter(|s| !s.is_empty());
    let recurrence_id = recurrence_id.filter(|s| !s.is_empty());
    match (obligation_id, recurrence_id) {
        (Some(_), Some(_)) => {
            return Err(
                "informe exatamente um alvo: obligation_id OU recurrence_id, não os dois".into(),
            );
        }
        (None, None) => {
            return Err("informe um alvo: obligation_id ou recurrence_id".into());
        }
        _ => {}
    }

    // Rejeita alvo duplicado no MESMO cenário (a comparação `col = NULL` é NULL→falsa em SQL,
    // então cada bind só casa o braço do alvo realmente setado).
    let (dup,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM scenario_override \
         WHERE scenario_id = ?1 AND (obligation_id = ?2 OR recurrence_id = ?3)",
    )
    .bind(scenario_id)
    .bind(obligation_id)
    .bind(recurrence_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("set_scenario_override (dup check): {e}"))?;
    if dup > 0 {
        return Err(if obligation_id.is_some() {
            "já existe uma alteração para esta obrigação neste cenário".into()
        } else {
            "já existe uma alteração para esta recorrência neste cenário".into()
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO scenario_override (id, scenario_id, op, from_date, obligation_id, recurrence_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(scenario_id)
    .bind(op)
    .bind(from_date)
    .bind(obligation_id)
    .bind(recurrence_id)
    .execute(pool)
    .await
    .map_err(|e| format!("set_scenario_override: {e}"))?;

    // Linha de substituição pareada (op=replace): criada AQUI para o pareamento ser
    // determinístico via `#repl:<override_id>`. Falhou a linha → desfaz o override (sem par
    // órfão) e propaga o erro.
    if let Some(repl) = replacement {
        let base = repl
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Substituição")
            .to_string();
        let tagged = format!("{base} #repl:{id}");
        let created = add_scenario_transaction(
            pool,
            scenario_id,
            repl.txn_type.as_deref().unwrap_or("expense"),
            repl.amount_cents,
            &tagged,
            &repl.date,
            repl.payment_method.as_deref(),
            repl.is_fixed.unwrap_or(true),
            None,
            None,
        )
        .await;
        if let Err(e) = created {
            let _ = sqlx::query("DELETE FROM scenario_override WHERE id = ?1")
                .bind(&id)
                .execute(pool)
                .await;
            return Err(format!("linha de substituição inválida: {e}"));
        }
    }
    Ok(id)
}

pub async fn list_scenario_overrides(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<Vec<ScenarioOverride>, String> {
    sqlx::query_as::<_, ScenarioOverride>(
        "SELECT id, scenario_id, op, from_date, obligation_id, recurrence_id \
         FROM scenario_override WHERE scenario_id = ?1 ORDER BY from_date",
    )
    .bind(scenario_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_scenario_overrides: {e}"))
}

// --- Empréstimo (ferramenta determinística; sem matemática livre de LLM) ---

/// Tabela PRICE (parcelas fixas, juros compostos): `PMT = PV · i / (1 − (1+i)^−n)`.
/// `monthly_rate_bps` = taxa mensal em basis points (100 = 1%). `n = 0` devolve 0 (sem série).
/// `monthly_rate_bps = 0` cai no caso degenerado (parcela = principal ÷ n), evitando divisão por
/// zero na fórmula (o limite de PRICE quando i→0 é justamente PV/n).
pub fn price_installment(principal_cents: i64, monthly_rate_bps: i64, n: u32) -> i64 {
    if n == 0 {
        return 0;
    }
    if monthly_rate_bps == 0 {
        return (principal_cents as f64 / n as f64).round() as i64;
    }
    let i = monthly_rate_bps as f64 / 10_000.0;
    let pv = principal_cents as f64;
    let factor = 1.0 - (1.0 + i).powi(-(n as i32));
    (pv * i / factor).round() as i64
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct LoanBreakdown {
    pub loan_principal_cents: i64,
    pub loan_installment_cents: i64,
    pub loan_term_months: u32,
    pub loan_monthly_rate_bps: i64,
    pub loan_total_paid_cents: i64,
    pub loan_total_cost_cents: i64,
    pub reserve_months_after_financing: Option<f64>,
}

/// Extrai `(group_id, rate_bps)` da marca `" #loan:<group_id>:<rate_bps>"` ao FINAL da descrição
/// (convenção documentada no banner do módulo). Ancorada: a marca precisa estar no fim (o parse
/// da taxa consome até o último caractere) e precedida de espaço/início — um "#loan:" solto no
/// meio do texto não é varrido. `None` se a descrição não carrega a marca.
fn parse_loan_marker(description: &str) -> Option<(String, i64)> {
    let description = description.trim_end();
    let idx = description.rfind("#loan:")?;
    if idx > 0 && !description[..idx].ends_with(char::is_whitespace) {
        return None; // colado em outra palavra: não é a marca da convenção.
    }
    let rest = &description[idx + "#loan:".len()..];
    let (group_id, rate_str) = rest.split_once(':')?;
    let group_id = group_id.trim();
    if group_id.is_empty() || group_id.contains(char::is_whitespace) {
        return None;
    }
    // `parse::<i64>` só aceita dígitos até o fim → âncora natural no final da descrição.
    let rate_bps: i64 = rate_str.trim().parse().ok()?;
    Some((group_id.to_string(), rate_bps))
}

/// Extrai o `override_id` da marca `" #repl:<override_id>"` ao FINAL da descrição de uma linha
/// de substituição criada por `set_scenario_override` (op=replace). Mesma âncora do `#loan:`.
fn parse_repl_marker(description: &str) -> Option<String> {
    let description = description.trim_end();
    let idx = description.rfind("#repl:")?;
    if idx > 0 && !description[..idx].ends_with(char::is_whitespace) {
        return None;
    }
    let id = description[idx + "#repl:".len()..].trim();
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    Some(id.to_string())
}

// --- Compare de forecast (real x cenário) ---

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ScenarioChange {
    pub op: String,
    pub description: String,
    pub from_date: String,
    pub old_amount_cents: Option<i64>,
    pub new_amount_cents: Option<i64>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioMonthEnd {
    pub year: i32,
    pub month: u32,
    pub real_balance_cents: i64,
    pub scenario_balance_cents: i64,
    pub delta_cents: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ScenarioCompareDto {
    pub scenario_id: String,
    pub scenario_name: String,

    pub real_today: String,
    pub real_horizon_end: String,
    pub real_month_end: Vec<forecast_cmds::MonthEndDto>,
    pub real_deepest_deficit: Option<forecast_cmds::DayPointDto>,
    pub real_performance_cents: i64,
    pub real_safe_to_spend_today_cents: i64,
    pub real_binding_guardrail: String,
    pub real_cost_of_living_cents: i64,
    /// Renda do mês corrente (Entradas) — plano 074 (fatia B): a UI classifica Custo de vida
    /// ("Dentro da renda"/"Acima da renda") sem re-derivar a renda; o motor já a calcula.
    pub real_income_cents: i64,

    pub scenario_month_end: Vec<forecast_cmds::MonthEndDto>,
    pub scenario_deepest_deficit: Option<forecast_cmds::DayPointDto>,
    pub scenario_performance_cents: i64,
    pub scenario_safe_to_spend_today_cents: i64,
    pub scenario_binding_guardrail: String,
    pub scenario_cost_of_living_cents: i64,
    pub scenario_income_cents: i64,

    pub month_end: Vec<ScenarioMonthEnd>,
    pub deepest_deficit_delta_cents: Option<i64>,
    pub performance_delta_cents: i64,
    pub safe_to_spend_delta_cents: i64,
    pub cost_of_living_delta_cents: i64,

    pub changes: Vec<ScenarioChange>,
    pub loan: Option<LoanBreakdown>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
struct RawTxnRow {
    id: String,
    #[sqlx(rename = "type")]
    ttype: String,
    amount: i64,
    date: String,
    payment_method: String,
    is_fixed: i64,
    is_projection: i64,
    to_liquidity: String,
    recurrence_id: Option<String>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
struct HypoTxnRow {
    #[allow(dead_code)]
    id: String,
    #[sqlx(rename = "type")]
    ttype: String,
    amount: i64,
    date: String,
    payment_method: String,
    is_fixed: i64,
    is_projection: i64,
    to_liquidity: String,
    #[allow(dead_code)]
    recurrence_id: Option<String>,
    description: Option<String>,
}

/// Linhas REAIS (`scenario_id IS NULL`) num intervalo `[start, end]`. `inclusive_start` decide
/// `>=` (janela de métrica, cobre o mês corrente) vs `>` (janela de encadeamento, evita dobrar o
/// que a semente já embute) — o mesmo corte que `load_forecast_events`/`load_metric_events` usam.
/// SQL literal por `&'static str` (sqlx exige) — sem string dinâmica.
async fn load_real_rows(
    pool: &SqlitePool,
    start: &str,
    inclusive_start: bool,
    end: &str,
) -> Result<Vec<RawTxnRow>, String> {
    const INCLUSIVE: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, t.recurrence_id \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date <= ?2 AND t.scenario_id IS NULL";
    const EXCLUSIVE: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, t.recurrence_id \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?1 AND t.date <= ?2 AND t.scenario_id IS NULL";
    let sql = if inclusive_start {
        INCLUSIVE
    } else {
        EXCLUSIVE
    };
    sqlx::query_as(sql)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load_real_rows: {e}"))
}

/// Superset-select das linhas HIPOTÉTICAS do cenário (`t.scenario_id = ?`), com `id`/`description`/
/// `recurrence_id` a mais (o cenário nunca sofre override; a marca de empréstimo vive na descrição).
async fn load_hypothetical_rows(
    pool: &SqlitePool,
    scenario_id: &str,
    start: &str,
    inclusive_start: bool,
    end: &str,
) -> Result<Vec<HypoTxnRow>, String> {
    const INCLUSIVE: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, \
         t.recurrence_id, t.description \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?2 AND t.date <= ?3 AND t.scenario_id = ?1";
    const EXCLUSIVE: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, \
         t.recurrence_id, t.description \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?2 AND t.date <= ?3 AND t.scenario_id = ?1";
    let sql = if inclusive_start {
        INCLUSIVE
    } else {
        EXCLUSIVE
    };
    sqlx::query_as(sql)
        .bind(scenario_id)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load_hypothetical_rows: {e}"))
}

/// TODAS as linhas hipotéticas do cenário, SEM janela de data — a detecção de empréstimo e a
/// lista de `changes` precisam ver o grupo INTEIRO, independentemente das janelas de data do
/// encadeamento (`date >= today`) e das métricas (`date >= month_start`), que existem só para o
/// cálculo de saldo e recortariam linhas do grupo (uma parcela além do horizonte, uma linha antes
/// da janela). `ORDER BY date, id` torna determinística a escolha do "primeiro" grupo `#loan`
/// reportado.
async fn load_all_hypothetical_rows(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<Vec<HypoTxnRow>, String> {
    const SQL: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, \
         t.recurrence_id, t.description \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.scenario_id = ?1 ORDER BY t.date, t.id";
    sqlx::query_as(SQL)
        .bind(scenario_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load_all_hypothetical_rows: {e}"))
}

/// Plano de supressão RESOLVIDO a partir dos overrides salvos: quanto suprimir por
/// `transaction_id` (obrigação, escopo LINE-ITEM — nunca derruba a célula inteira, só o(s)
/// item(ns) casado(s)) e quais `(recurrence_id, from_date)` suprimir POR INTEIRO (série
/// recorrente é de propósito único; não há item para preservar).
#[derive(Default)]
struct SuppressionPlan {
    line_item_suppressed_cents: HashMap<String, i64>,
    recurrence_suppressed: Vec<(String, NaiveDate)>,
    /// Para `changes`: soma suprimida por override (na ordem em que foram lidos).
    per_override_suppressed_cents: HashMap<String, i64>,
}

async fn build_suppression_plan(
    pool: &SqlitePool,
    overrides: &[ScenarioOverride],
) -> Result<SuppressionPlan, String> {
    let mut plan = SuppressionPlan::default();
    // Defesa em profundidade contra alvos duplicados (que `set_scenario_override` já rejeita,
    // mas linhas pré-existentes/inseridas por fora não passam pelo guard): cada `line_item` só
    // contribui UMA vez para a supressão, mesmo que dois overrides o casem — somar duas vezes
    // zeraria a célula e derrubaria os irmãos não suprimidos.
    let mut seen_line_items: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ov in overrides {
        let from_date = NaiveDate::parse_from_str(&ov.from_date, "%Y-%m-%d")
            .map_err(|e| format!("override {} from_date inválida: {e}", ov.id))?;
        if let Some(obligation_id) = &ov.obligation_id {
            let items = obligations::obligation_items(pool, obligation_id).await?;
            let mut suppressed_here = 0i64;
            for item in items {
                let Ok(d) = NaiveDate::parse_from_str(&item.date, "%Y-%m-%d") else {
                    continue;
                };
                if d >= from_date && seen_line_items.insert(item.line_item_id.clone()) {
                    let magnitude = item.amount_cents.abs();
                    *plan
                        .line_item_suppressed_cents
                        .entry(item.transaction_id.clone())
                        .or_insert(0) += magnitude;
                    suppressed_here += magnitude;
                }
            }
            plan.per_override_suppressed_cents
                .insert(ov.id.clone(), suppressed_here);
        } else if let Some(recurrence_id) = &ov.recurrence_id {
            plan.recurrence_suppressed
                .push((recurrence_id.clone(), from_date));
            // Magnitude suprimida da série (linhas reais ≥ from_date) — alimenta o
            // `old_amount_cents` de `changes`, igual ao braço de obrigação.
            let (suppressed_here,): (i64,) = sqlx::query_as(
                "SELECT COALESCE(SUM(ABS(amount)), 0) FROM \"transaction\" \
                 WHERE recurrence_id = ?1 AND date >= ?2 AND scenario_id IS NULL",
            )
            .bind(recurrence_id)
            .bind(&ov.from_date)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("recurrence suppressed sum: {e}"))?;
            plan.per_override_suppressed_cents
                .insert(ov.id.clone(), suppressed_here);
        }
    }
    Ok(plan)
}

/// Aplica o plano de supressão a linhas REAIS cruas: recorrência casada (data ≥ from_date) derruba
/// a linha INTEIRA (série de propósito único); supressão por obrigação subtrai a magnitude
/// suprimida do total da célula e só derruba a linha se o resto chegar a 0 — os IRMÃOS de uma
/// célula multi-item continuam intactos (a linha só carrega o total, não os itens individuais;
/// subtrair preserva a contribuição dos itens não suprimidos).
fn apply_suppression(rows: Vec<RawTxnRow>, plan: &SuppressionPlan) -> Vec<RawTxnRow> {
    rows.into_iter()
        .filter_map(|mut row| {
            if let Some(recurrence_id) = &row.recurrence_id
                && let Ok(d) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
                && plan
                    .recurrence_suppressed
                    .iter()
                    .any(|(id, from)| id == recurrence_id && d >= *from)
            {
                return None;
            }
            if let Some(&suppressed) = plan.line_item_suppressed_cents.get(&row.id) {
                let new_amount = (row.amount.abs() - suppressed).max(0);
                if new_amount == 0 {
                    return None;
                }
                row.amount = new_amount;
            }
            Some(row)
        })
        .collect()
}

fn map_raw_rows(rows: Vec<RawTxnRow>) -> Vec<CashflowEvent> {
    rows.into_iter()
        .filter_map(|r| {
            map_cashflow_row((
                r.ttype,
                r.amount,
                r.date,
                r.payment_method,
                r.is_fixed,
                r.is_projection,
                r.to_liquidity,
            ))
        })
        .collect()
}

fn map_hypo_rows(rows: &[HypoTxnRow]) -> Vec<CashflowEvent> {
    rows.iter()
        .cloned()
        .filter_map(|r| {
            map_cashflow_row((
                r.ttype,
                r.amount,
                r.date,
                r.payment_method,
                r.is_fixed,
                r.is_projection,
                r.to_liquidity,
            ))
        })
        .collect()
}

/// Detecta um grupo de empréstimo entre TODAS as linhas hipotéticas do cenário (convenção
/// `#loan:<id>:<taxa>` no banner do módulo) e monta o `LoanBreakdown`. Só o PRIMEIRO grupo na
/// ordem (data, id) é reportado — devolve também o `group_id` para o builder de `changes`
/// excluir SÓ as linhas desse grupo (as de um segundo financiamento aparecem como "add" comuns,
/// nunca somem do DTO). `None` se nenhuma linha carrega a marca.
fn detect_loan(
    hypo_rows: &[HypoTxnRow],
    scenario_cost_of_living_cents: i64,
    scenario_reserve_after_cents: i64,
) -> Option<(String, LoanBreakdown)> {
    let mut group_id: Option<String> = None;
    let mut rate_bps = 0i64;
    let mut principal_cents = 0i64;
    let mut installment_cents = 0i64;
    let mut term_months = 0u32;

    for row in hypo_rows {
        let Some(desc) = &row.description else {
            continue;
        };
        let Some((gid, rate)) = parse_loan_marker(desc) else {
            continue;
        };
        if let Some(existing) = &group_id {
            if existing != &gid {
                continue; // só o primeiro grupo (ordem data,id) é reportado.
            }
        } else {
            group_id = Some(gid);
            rate_bps = rate;
        }
        if row.ttype == "income" {
            principal_cents = row.amount.abs();
        } else if row.ttype == "expense" {
            if installment_cents == 0 {
                installment_cents = row.amount.abs();
            }
            term_months += 1;
        }
    }

    let group_id = group_id?;
    if term_months == 0 {
        return None;
    }

    let total_paid_cents = installment_cents * term_months as i64;
    let total_cost_cents = total_paid_cents - principal_cents;
    // Reserva-em-meses após o financiamento: colchão restante (piso já respeitado pelo forecast) ÷
    // custo de vida do cenário. Documentado como aproximação (não há helper de "meses de reserva"
    // pronto no forecast core): usa o mesmo custo de vida do compare.
    let reserve_months_after_financing = (scenario_cost_of_living_cents > 0)
        .then(|| scenario_reserve_after_cents as f64 / scenario_cost_of_living_cents as f64);

    Some((
        group_id,
        LoanBreakdown {
            loan_principal_cents: principal_cents,
            loan_installment_cents: installment_cents,
            loan_term_months: term_months,
            loan_monthly_rate_bps: rate_bps,
            loan_total_paid_cents: total_paid_cents,
            loan_total_cost_cents: total_cost_cents,
            reserve_months_after_financing,
        },
    ))
}

/// Custo de vida "do momento": o mês corrente do `Forecast` (mesma definição canônica do motor —
/// fixas + diário realizado + cartão, plano 060), ou 0 se o mês corrente não aparece nos meses do
/// horizonte (nunca deveria faltar, já que `today` sempre inicia o horizonte).
fn current_month_cost_of_living(fc: &forecast::Forecast, today: NaiveDate) -> i64 {
    fc.months
        .iter()
        .find(|m| m.year == today.year() && m.month == today.month())
        .map(|m| m.cost_of_living_cents)
        .unwrap_or(0)
}

fn current_month_performance(fc: &forecast::Forecast, today: NaiveDate) -> i64 {
    fc.months
        .iter()
        .find(|m| m.year == today.year() && m.month == today.month())
        .map(|m| m.performance_cents)
        .unwrap_or(0)
}

/// Renda do mês corrente (`MonthMetric.income_cents`) — plano 074 (fatia B): exposta para os
/// cards do compare classificarem Custo de vida ("Dentro da renda"/"Acima da renda") na UI sem
/// RE-DERIVAR a renda no comando (fonte única: o motor já soma as Entradas do mês).
fn current_month_income(fc: &forecast::Forecast, today: NaiveDate) -> i64 {
    fc.months
        .iter()
        .find(|m| m.year == today.year() && m.month == today.month())
        .map(|m| m.income_cents)
        .unwrap_or(0)
}

pub(crate) async fn get_scenario_forecast_inner(
    pool: &SqlitePool,
    scenario_id: &str,
    today: NaiveDate,
) -> Result<ScenarioCompareDto, String> {
    let scenario: Scenario =
        sqlx::query_as("SELECT id, name, person_id FROM scenario WHERE id = ?1")
            .bind(scenario_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("get_scenario_forecast: {e}"))?
            .ok_or_else(|| format!("scenario not found: {scenario_id}"))?;

    let real_horizon_end = forecast_horizon_end(pool, today).await?;
    let scenario_max: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM \"transaction\" WHERE scenario_id = ?1")
            .bind(scenario_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("scenario max date: {e}"))?;
    let scenario_max_date = scenario_max
        .0
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let horizon_end = match scenario_max_date {
        Some(d) if d > real_horizon_end => d,
        _ => real_horizon_end,
    };

    let seed = projection_seed(pool, today).await?;
    let years: Vec<i32> = (today.year()..=horizon_end.year()).collect();
    let annotation = load_economia_annotation(pool, &years).await?;

    // --- Ramo REAL (baseline, intocado — os mesmos loaders do forecast de produção). ---
    let real_chain_events = load_forecast_events(pool, today, horizon_end).await?;
    let real_metric_events = load_metric_events(pool, today, horizon_end).await?;
    let real_fc = forecast::project_with_metrics(
        seed,
        today,
        &real_chain_events,
        &real_metric_events,
        horizon_end,
        &annotation,
    );

    // --- Ramo CENÁRIO: linhas reais AJUSTADAS pelos overrides + linhas hipotéticas do cenário. ---
    let overrides = list_scenario_overrides(pool, scenario_id).await?;
    let plan = build_suppression_plan(pool, &overrides).await?;

    let today_str = today.format("%Y-%m-%d").to_string();
    let horizon_str = horizon_end.format("%Y-%m-%d").to_string();
    let month_start_str = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or("data de hoje inválida")?
        .format("%Y-%m-%d")
        .to_string();

    // Ramo REAL: `date > today` no encadeamento — o movimento real de hoje já está embutido no
    // saldo-semente da conta; incluí-lo dobraria. As linhas HIPOTÉTICAS não tocam saldo nenhum
    // (não há semente), então o encadeamento do CENÁRIO inclui HOJE (`date >= today`): um evento
    // hipotético de hoje (ex.: o principal desembolsado hoje) tem de entrar na trajetória, senão
    // ela não sobe com o dinheiro recebido e o guardrail de caixa fica apertado. As métricas
    // cobrem o mês inteiro (`date >= month_start`) nos dois ramos — pipeline separada, sem
    // double-count com o encadeamento.
    let real_chain_raw = load_real_rows(pool, &today_str, false, &horizon_str).await?;
    let real_metric_raw = load_real_rows(pool, &month_start_str, true, &horizon_str).await?;
    let hypo_chain_rows =
        load_hypothetical_rows(pool, scenario_id, &today_str, true, &horizon_str).await?;
    let hypo_metric_rows =
        load_hypothetical_rows(pool, scenario_id, &month_start_str, true, &horizon_str).await?;

    let scenario_chain_adjusted = apply_suppression(real_chain_raw, &plan);
    let scenario_metric_adjusted = apply_suppression(real_metric_raw, &plan);

    let mut scenario_chain_events = map_raw_rows(scenario_chain_adjusted);
    scenario_chain_events.extend(map_hypo_rows(&hypo_chain_rows));
    let mut scenario_metric_events = map_raw_rows(scenario_metric_adjusted);
    scenario_metric_events.extend(map_hypo_rows(&hypo_metric_rows));

    // Previsão de diário reutiliza o MESMO teto/dia do ramo real — o orçamento de Diário não muda
    // por cenário; só o encadeamento de caixa/hipotéticas mudam.
    let daily_ceiling = forecast_cmds::effective_daily_ceiling(pool, today).await?;
    let days_with_daily_chain: std::collections::HashSet<NaiveDate> = scenario_chain_events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    scenario_chain_events.extend(forecast::project_daily_ceiling(
        daily_ceiling,
        today,
        horizon_end,
        &days_with_daily_chain,
    ));
    let days_with_daily_metric: std::collections::HashSet<NaiveDate> = scenario_metric_events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    scenario_metric_events.extend(forecast::project_daily_ceiling(
        daily_ceiling,
        today,
        horizon_end,
        &days_with_daily_metric,
    ));

    let scenario_fc = forecast::project_with_metrics(
        seed,
        today,
        &scenario_chain_events,
        &scenario_metric_events,
        horizon_end,
        &annotation,
    );

    // --- Guardrails (mesma fórmula do forecast real, poupança anual REALIZADA — não muda por
    // cenário; o "e se" só reprojeta o caixa/performance do mês, não reescreve o ano já realizado). ---
    let reserve_floor_cents = reserve_floor(pool, today).await?;
    let (annual_income, _) = forecast_cmds::realized_annual_savings(pool, today).await?;
    let annual_economia = forecast_cmds::realized_annual_economia(pool, today).await?;

    let real_sts = forecast::safe_to_spend_today(
        &real_fc,
        annual_income,
        annual_economia,
        forecast_cmds::SAVINGS_TARGET_BPS,
        reserve_floor_cents,
    );
    let scenario_sts = forecast::safe_to_spend_today(
        &scenario_fc,
        annual_income,
        annual_economia,
        forecast_cmds::SAVINGS_TARGET_BPS,
        reserve_floor_cents,
    );
    let guardrail_str = |g: forecast::Guardrail| {
        match g {
            forecast::Guardrail::Cash => "cash",
            forecast::Guardrail::Savings => "savings",
        }
        .to_string()
    };

    let real_cost_of_living_cents = current_month_cost_of_living(&real_fc, today);
    let scenario_cost_of_living_cents = current_month_cost_of_living(&scenario_fc, today);
    let real_performance_cents = current_month_performance(&real_fc, today);
    let scenario_performance_cents = current_month_performance(&scenario_fc, today);
    let real_income_cents = current_month_income(&real_fc, today);
    let scenario_income_cents = current_month_income(&scenario_fc, today);

    let real_month_end: Vec<forecast_cmds::MonthEndDto> = real_fc
        .month_end
        .iter()
        .map(|m| forecast_cmds::MonthEndDto {
            year: m.year,
            month: m.month,
            balance_cents: m.balance_cents,
        })
        .collect();
    let scenario_month_end: Vec<forecast_cmds::MonthEndDto> = scenario_fc
        .month_end
        .iter()
        .map(|m| forecast_cmds::MonthEndDto {
            year: m.year,
            month: m.month,
            balance_cents: m.balance_cents,
        })
        .collect();

    let mut month_end = Vec::new();
    for r in &real_month_end {
        if let Some(s) = scenario_month_end
            .iter()
            .find(|s| s.year == r.year && s.month == r.month)
        {
            month_end.push(ScenarioMonthEnd {
                year: r.year,
                month: r.month,
                real_balance_cents: r.balance_cents,
                scenario_balance_cents: s.balance_cents,
                delta_cents: s.balance_cents - r.balance_cents,
            });
        }
    }

    let real_deepest_deficit = real_fc.deepest_deficit.map(|p| forecast_cmds::DayPointDto {
        date: p.date.format("%Y-%m-%d").to_string(),
        balance_cents: p.balance_cents,
    });
    let scenario_deepest_deficit =
        scenario_fc
            .deepest_deficit
            .map(|p| forecast_cmds::DayPointDto {
                date: p.date.format("%Y-%m-%d").to_string(),
                balance_cents: p.balance_cents,
            });
    let deepest_deficit_delta_cents = match (&real_fc.deepest_deficit, &scenario_fc.deepest_deficit)
    {
        (Some(r), Some(s)) => Some(s.balance_cents - r.balance_cents),
        _ => None,
    };

    // --- Empréstimo + changes: usam TODAS as linhas hipotéticas do cenário (sem janela de data).
    // As janelas de encadeamento/métrica existem só para o cálculo de saldo; a detecção e a lista
    // de `changes` precisam do grupo inteiro — senão `loan_total_cost` poderia superestimar o
    // custo por não ver todas as linhas do financiamento.
    let all_hypo_rows = load_all_hypothetical_rows(pool, scenario_id).await?;
    let scenario_reserve_after_cents = scenario_fc
        .deepest_deficit
        .map(|p| p.balance_cents)
        .unwrap_or(0)
        - reserve_floor_cents;
    let (loan_group_id, loan) = match detect_loan(
        &all_hypo_rows,
        scenario_cost_of_living_cents,
        scenario_reserve_after_cents,
    ) {
        Some((gid, breakdown)) => (Some(gid), Some(breakdown)),
        None => (None, None),
    };

    // Linhas de substituição pareadas por `#repl:<override_id>` (ver banner do módulo): fundem
    // velho→novo numa única entrada `replace` de `changes`. Só marca com override EXISTENTE
    // conta como par (um `#repl:` órfão apareceria como "add" normal, nunca some do DTO).
    let mut replacement_by_override: HashMap<String, i64> = HashMap::new();
    for row in &all_hypo_rows {
        if let Some(desc) = &row.description
            && let Some(ov_id) = parse_repl_marker(desc)
            && overrides.iter().any(|o| o.id == ov_id)
        {
            replacement_by_override
                .entry(ov_id)
                .or_insert(row.amount.abs());
        }
    }

    let mut changes = Vec::new();
    for ov in &overrides {
        let (label, from_date_naive) = if let Some(obligation_id) = &ov.obligation_id {
            let name: Option<(String,)> =
                sqlx::query_as("SELECT name FROM obligation WHERE id = ?1")
                    .bind(obligation_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| format!("obligation name: {e}"))?;
            (
                name.map(|(n,)| n).unwrap_or_else(|| obligation_id.clone()),
                ov.from_date.clone(),
            )
        } else {
            (
                ov.recurrence_id.clone().unwrap_or_default(),
                ov.from_date.clone(),
            )
        };
        let op_label = if ov.op == "suppress" {
            "remove"
        } else {
            "replace"
        };
        changes.push(ScenarioChange {
            op: op_label.to_string(),
            description: label,
            from_date: from_date_naive,
            old_amount_cents: plan.per_override_suppressed_cents.get(&ov.id).copied(),
            // Fundido via `#repl:<override_id>`; `None` se o replace foi criado sem a linha de
            // substituição pareada (a UI então mostra só o "removido").
            new_amount_cents: replacement_by_override.get(&ov.id).copied(),
        });
    }
    for row in &all_hypo_rows {
        let desc = row.description.as_deref().unwrap_or("");
        // Linha do grupo de empréstimo REPORTADO via `loan` → não duplica. Um SEGUNDO grupo
        // `#loan:` (não coberto pelo `loan` desta slice) entra como "add" normal.
        if let Some((gid, _)) = parse_loan_marker(desc)
            && loan_group_id.as_deref() == Some(gid.as_str())
        {
            continue;
        }
        // Linha de substituição pareada → já fundida na entrada `replace` acima.
        if parse_repl_marker(desc).is_some_and(|ov_id| overrides.iter().any(|o| o.id == ov_id)) {
            continue;
        }
        changes.push(ScenarioChange {
            op: "add".to_string(),
            description: row.description.clone().unwrap_or_default(),
            from_date: row.date.clone(),
            old_amount_cents: None,
            new_amount_cents: Some(row.amount.abs()),
        });
    }

    Ok(ScenarioCompareDto {
        scenario_id: scenario.id,
        scenario_name: scenario.name,

        real_today: today_str,
        real_horizon_end: real_horizon_end.format("%Y-%m-%d").to_string(),
        real_month_end,
        real_deepest_deficit,
        real_performance_cents,
        real_safe_to_spend_today_cents: real_sts.amount_cents,
        real_binding_guardrail: guardrail_str(real_sts.binding),
        real_cost_of_living_cents,
        real_income_cents,

        scenario_month_end,
        scenario_deepest_deficit,
        scenario_performance_cents,
        scenario_safe_to_spend_today_cents: scenario_sts.amount_cents,
        scenario_binding_guardrail: guardrail_str(scenario_sts.binding),
        scenario_cost_of_living_cents,
        scenario_income_cents,

        month_end,
        deepest_deficit_delta_cents,
        performance_delta_cents: scenario_performance_cents - real_performance_cents,
        safe_to_spend_delta_cents: scenario_sts.amount_cents - real_sts.amount_cents,
        cost_of_living_delta_cents: scenario_cost_of_living_cents - real_cost_of_living_cents,

        changes,
        loan,
    })
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn create_scenario_cmd(
    pool: State<'_, SqlitePool>,
    name: String,
) -> Result<Scenario, String> {
    create_scenario(pool.inner(), &name).await
}

#[tauri::command]
pub async fn list_scenarios_cmd(pool: State<'_, SqlitePool>) -> Result<Vec<Scenario>, String> {
    list_scenarios(pool.inner()).await
}

#[tauri::command]
pub async fn delete_scenario_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    delete_scenario(pool.inner(), &id).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_scenario_transaction_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
    txn_type: String,
    amount_cents: i64,
    description: String,
    date: String,
    payment_method: Option<String>,
    is_fixed: bool,
    to_account_id: Option<String>,
    due_date: Option<String>,
) -> Result<String, String> {
    add_scenario_transaction(
        pool.inner(),
        &scenario_id,
        &txn_type,
        amount_cents,
        &description,
        &date,
        payment_method.as_deref(),
        is_fixed,
        to_account_id.as_deref(),
        due_date.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn delete_scenario_transaction_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
    txn_id: String,
) -> Result<(), String> {
    delete_scenario_transaction(pool.inner(), &scenario_id, &txn_id).await
}

#[tauri::command]
pub async fn list_scenario_transactions_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
) -> Result<Vec<ScenarioTransactionRow>, String> {
    list_scenario_transactions(pool.inner(), &scenario_id).await
}

#[tauri::command]
pub async fn set_scenario_override_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
    op: String,
    from_date: String,
    obligation_id: Option<String>,
    recurrence_id: Option<String>,
    replacement: Option<ReplacementInput>,
) -> Result<String, String> {
    set_scenario_override(
        pool.inner(),
        &scenario_id,
        &op,
        &from_date,
        obligation_id.as_deref(),
        recurrence_id.as_deref(),
        replacement,
    )
    .await
}

/// Ferramenta determinística exposta à UI (nunca matemática livre de LLM): calcula a parcela
/// PRICE antes do usuário confirmar as linhas hipotéticas do empréstimo.
#[tauri::command]
pub fn price_installment_cmd(principal_cents: i64, monthly_rate_bps: i64, term_months: u32) -> i64 {
    price_installment(principal_cents, monthly_rate_bps, term_months)
}

#[tauri::command]
pub async fn get_scenario_forecast_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
) -> Result<ScenarioCompareDto, String> {
    get_scenario_forecast_inner(
        pool.inner(),
        &scenario_id,
        chrono::Local::now().date_naive(),
    )
    .await
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

    async fn txn(pool: &SqlitePool, id: &str, ttype: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES (?1,?2,?3,?4,0)",
        )
        .bind(id)
        .bind(ttype)
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

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    // --- CRUD básico ---

    #[tokio::test]
    async fn create_list_delete_scenario_roundtrip() {
        let p = pool().await;
        let sc = create_scenario(&p, "E se eu comprar um carro")
            .await
            .unwrap();
        assert_eq!(sc.name, "E se eu comprar um carro");

        let all = list_scenarios(&p).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, sc.id);

        delete_scenario(&p, &sc.id).await.unwrap();
        assert!(list_scenarios(&p).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_scenario_rejects_empty_name() {
        let p = pool().await;
        assert!(create_scenario(&p, "   ").await.is_err());
    }

    // Teste 9: apagar o cenário cascateia transações + overrides.
    #[tokio::test]
    async fn delete_scenario_cascades_transactions_and_overrides() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let txn_id = add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            10000,
            "Parcela",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await
        .unwrap();

        delete_scenario(&p, &sc.id).await.unwrap();

        let (txn_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id = ?1")
                .bind(&txn_id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(txn_count, 0, "transação hipotética cascateia");

        let (ov_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE id = ?1")
                .bind(&ov_id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(ov_count, 0, "override cascateia");
    }

    // Teste 10: adicionar uma transação de cenário não muta account.balance.
    #[tokio::test]
    async fn add_scenario_transaction_does_not_change_account_balance() {
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Eu')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) \
             VALUES ('acc-1', 'Conta', 'bank', 'pe-1', 500000)",
        )
        .execute(&p)
        .await
        .unwrap();
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            999900,
            "Compra gigante",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let (balance,): (i64,) = sqlx::query_as("SELECT balance FROM account WHERE id = 'acc-1'")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(
            balance, 500000,
            "add_scenario_transaction não muta account.balance"
        );
    }

    #[tokio::test]
    async fn add_scenario_transaction_rejects_empty_description() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let result = add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            1000,
            "  ",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_scenario_transactions_only_returns_own_scenario_rows() {
        let p = pool().await;
        let sc1 = create_scenario(&p, "Um").await.unwrap();
        let sc2 = create_scenario(&p, "Dois").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc1.id,
            "expense",
            1000,
            "Linha do cenário 1",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        add_scenario_transaction(
            &p,
            &sc2.id,
            "expense",
            2000,
            "Linha do cenário 2",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let rows = list_scenario_transactions(&p, &sc1.id).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].description, "Linha do cenário 1");
    }

    #[tokio::test]
    async fn delete_scenario_transaction_rejects_wrong_scenario() {
        let p = pool().await;
        let sc1 = create_scenario(&p, "Um").await.unwrap();
        let sc2 = create_scenario(&p, "Dois").await.unwrap();
        let txn_id = add_scenario_transaction(
            &p,
            &sc1.id,
            "expense",
            1000,
            "X",
            "2026-08-01",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(
            delete_scenario_transaction(&p, &sc2.id, &txn_id)
                .await
                .is_err()
        );
        delete_scenario_transaction(&p, &sc1.id, &txn_id)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn set_scenario_override_rejects_both_targets() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let result = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&ob_id),
            Some("rec-1"),
            None,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn set_scenario_override_rejects_no_target() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let result =
            set_scenario_override(&p, &sc.id, "suppress", "2026-08-01", None, None, None).await;
        assert!(result.is_err());
    }

    // --- price_installment / PRICE ---

    // Teste 6a: taxa zero degenera para principal / n (parcela exata, sem juros).
    #[test]
    fn price_installment_zero_rate_is_principal_over_n() {
        let pmt = price_installment(120_000, 0, 12);
        assert_eq!(pmt, 10_000);
        let total_paid = pmt * 12;
        assert_eq!(total_paid, 120_000);
        assert_eq!(total_paid - 120_000, 0, "sem juros a taxa zero");
    }

    // Teste 6b: valor conhecido com juros (verificado externamente: PV=1000,00, i=2%/mês, n=10 →
    // parcela 111,33; total pago 1113,30; juros 113,30 — todos em centavos abaixo).
    #[test]
    fn price_installment_known_value_matches_hand_computed() {
        let pmt = price_installment(100_000, 200, 10);
        assert_eq!(pmt, 11_133);
        let total_paid = pmt * 10;
        assert_eq!(total_paid, 111_330);
        assert_eq!(total_paid - 100_000, 11_330, "juros = total - principal");
    }

    #[test]
    fn price_installment_zero_term_is_zero() {
        assert_eq!(price_installment(100_000, 200, 0), 0);
    }

    // --- get_scenario_forecast: determinismo/idempotência ---

    async fn seed_baseline(p: &SqlitePool) {
        txn(p, "inc-1", "income", 500_000, "2026-08-01").await;
    }

    // Teste 1 + 2: determinismo e idempotência — mesmas entradas, mesma saída, recomputado 2x.
    #[tokio::test]
    async fn get_scenario_forecast_is_deterministic_and_idempotent() {
        let p = pool().await;
        seed_baseline(&p).await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            20_000,
            "Assinatura nova",
            "2026-08-10",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let a = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        let b = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        assert_eq!(a.scenario_month_end, b.scenario_month_end);
        assert_eq!(a.real_month_end, b.real_month_end);
        assert_eq!(
            a.scenario_cost_of_living_cents,
            b.scenario_cost_of_living_cents
        );

        // Idempotência: recomputar sem mudar nada dá o MESMO resultado de novo.
        let c = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        assert_eq!(b.scenario_month_end, c.scenario_month_end);
    }

    // Plano 074 (fatia B): a UI classifica Custo de vida ("Dentro da renda"/"Acima da renda")
    // usando a renda do mês exposta no DTO — o comando expõe (não re-deriva) o que o motor já
    // soma em `MonthMetric.income_cents`; a Entrada hipotética do cenário entra na renda do
    // CENÁRIO, o real fica intocado.
    #[tokio::test]
    async fn compare_exposes_current_month_income_for_custo_de_vida_classification() {
        let p = pool().await;
        seed_baseline(&p).await; // Entrada real: 500.000 em 2026-08-01.
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "income",
            100_000,
            "Bico extra",
            "2026-08-05",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        assert_eq!(compare.real_income_cents, 500_000);
        assert_eq!(
            compare.scenario_income_cents, 600_000,
            "a Entrada hipotética soma à renda do mês só no ramo cenário"
        );
    }

    // Teste 7: isolamento de cenário segue intocado — o forecast REAL não vê a linha hipotética.
    #[tokio::test]
    async fn scenario_isolation_still_holds() {
        let p = pool().await;
        seed_baseline(&p).await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            400_000,
            "Compra enorme",
            "2026-08-10",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let horizon = d("2026-08-31");
        let real_events = forecast_cmds::load_forecast_events(&p, today, horizon)
            .await
            .unwrap();
        assert!(
            real_events.iter().all(|e| e.amount_cents != 400_000),
            "a linha hipotética não aparece no forecast real"
        );
    }

    // Teste 3: replace (suprime + hipotética) não conta o valor antigo e o novo juntos.
    #[tokio::test]
    async fn replace_override_does_not_double_count() {
        let p = pool().await;
        seed_baseline(&p).await;
        txn(&p, "aluguel-1", "expense", 150_000, "2026-08-05").await;
        line_item(&p, "li-1", "aluguel-1", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let sc = create_scenario(&p, "Cenário").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2026-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await
        .unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            200_000,
            "Aluguel novo",
            "2026-08-05",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-07-31");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        // Real: renda 500.000 − aluguel 150.000 = 350.000.
        assert_eq!(month.real_balance_cents, 350_000);
        // Cenário: renda 500.000 − aluguel NOVO 200.000 (velho suprimido, NÃO soma 150+200).
        assert_eq!(
            month.scenario_balance_cents, 300_000,
            "replace não conta o valor antigo e o novo juntos"
        );
    }

    // Teste 4: override sobre UM item de uma célula multi-item preserva a contribuição do irmão.
    #[tokio::test]
    async fn suppress_one_sibling_leaves_others_intact() {
        let p = pool().await;
        seed_baseline(&p).await;
        // Célula única (mesma transação) com 2 itens: Aluguel 100.000 + Internet 8.000.
        txn(&p, "cell-1", "expense", 108_000, "2026-08-05").await;
        line_item(&p, "li-aluguel", "cell-1", 100_000, "Aluguel", None).await;
        line_item(&p, "li-internet", "cell-1", 8_000, "Internet", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let sc = create_scenario(&p, "Cenário").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-07-31");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();
        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        // Real: 500.000 − 108.000 = 392.000.
        assert_eq!(month.real_balance_cents, 392_000);
        // Cenário: só o Aluguel (100.000) suprimido; Internet (8.000) continua pesando.
        // 500.000 − 8.000 = 492.000 (não 500.000, que apagaria o irmão também).
        assert_eq!(
            month.scenario_balance_cents, 492_000,
            "Internet (irmão não suprimido) continua descontando o saldo"
        );
    }

    // Teste 8: suprimir uma obrigação some as ocorrências (≥ from_date) do ramo cenário, mas
    // permanece no ramo real.
    #[tokio::test]
    async fn suppress_removes_occurrences_from_scenario_branch_only() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 1_000_000, "2026-08-01").await;
        txn(&p, "aluguel-ago", "expense", 150_000, "2026-08-05").await;
        line_item(&p, "li-ago", "aluguel-ago", 150_000, "Aluguel", None).await;
        txn(&p, "aluguel-set", "expense", 150_000, "2026-09-05").await;
        line_item(&p, "li-set", "aluguel-set", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let sc = create_scenario(&p, "Cenário").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();

        let sep_real = compare
            .real_month_end
            .iter()
            .find(|m| m.month == 9)
            .unwrap();
        let sep_scenario = compare
            .scenario_month_end
            .iter()
            .find(|m| m.month == 9)
            .unwrap();
        assert!(
            sep_scenario.balance_cents > sep_real.balance_cents,
            "setembro no cenário fica mais alto (aluguel suprimido), real não muda"
        );
    }

    // Teste 5: o principal do empréstimo ELEVA o saldo na data do desembolso antes das parcelas.
    #[tokio::test]
    async fn loan_principal_raises_balance_before_installments_pull_down() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-01").await;

        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "income",
            1_000_000,
            "Empréstimo desembolso #loan:carro:150",
            "2026-08-05",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            90_000,
            "Parcela 1 #loan:carro:150",
            "2026-08-10",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-07-31");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();

        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        // Real: 500.000. Cenário: 500.000 + 1.000.000 (principal) − 90.000 (parcela) = 1.410.000.
        assert_eq!(month.real_balance_cents, 500_000);
        assert_eq!(
            month.scenario_balance_cents, 1_410_000,
            "principal soma antes da parcela puxar pra baixo"
        );

        let loan = compare.loan.expect("grupo de empréstimo detectado");
        assert_eq!(loan.loan_principal_cents, 1_000_000);
        assert_eq!(loan.loan_installment_cents, 90_000);
        assert_eq!(loan.loan_term_months, 1);
        assert_eq!(loan.loan_monthly_rate_bps, 150);
        assert_eq!(loan.loan_total_paid_cents, 90_000);
        assert_eq!(loan.loan_total_cost_cents, 90_000 - 1_000_000);
    }

    // --- Revisão adversarial (rodada 1 da slice B) ---

    // Detecção robusta: a detecção do empréstimo usa TODAS as linhas hipotéticas (sem janela de
    // data), então o principal desembolsado no próprio `today` nunca some do grupo — o custo total
    // não pode ser superestimado pelo principal inteiro, em silêncio.
    #[tokio::test]
    async fn loan_detects_principal_disbursed_today() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-02").await;

        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "income",
            1_000_000,
            "Desembolso #loan:moto:100",
            "2026-08-01", // == today injetado abaixo
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            90_000,
            "Parcela 1 #loan:moto:100",
            "2026-08-10",
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-08-01"))
            .await
            .unwrap();
        let loan = compare
            .loan
            .expect("principal de hoje ainda é detectado no grupo");
        assert_eq!(
            loan.loan_principal_cents, 1_000_000,
            "principal desembolsado hoje não pode virar 0"
        );
        assert_eq!(
            loan.loan_total_cost_cents,
            90_000 - 1_000_000,
            "custo total = pago − principal, não pago − 0"
        );
    }

    // Plano 078: um evento HIPOTÉTICO datado do próprio `today` (ex.: o principal do empréstimo,
    // desembolsado hoje) precisa entrar no ENCADEAMENTO de saldo do cenário, não só nas métricas.
    // Linhas reais de hoje já estão no saldo-semente (`date > today` é correto lá); linhas
    // hipotéticas não têm semente, então o evento de hoje se perderia da trajetória — afundando o
    // menor ponto e apertando o guardrail de caixa artificialmente.
    #[tokio::test]
    async fn hypothetical_income_today_enters_balance_chain() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-02").await; // real, depois de hoje

        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "income",
            1_000_000,
            "Principal desembolsado hoje",
            "2026-08-01", // == today
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();

        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        // O principal de hoje eleva o saldo encadeado do cenário; o real fica intocado.
        assert_eq!(
            month.scenario_balance_cents,
            month.real_balance_cents + 1_000_000,
            "principal hipotético de hoje entra no encadeamento do cenário"
        );
        // A trajetória do cenário é paralela à real (o teto de Diário é o mesmo nos dois ramos),
        // deslocada pelo principal a partir de hoje: o menor ponto do cenário fica MENOS fundo que
        // o real pelo mesmo valor (delta = cenário − real = +principal ≥ 0).
        assert_eq!(
            compare.deepest_deficit_delta_cents,
            Some(1_000_000),
            "o principal de hoje levanta o menor ponto do cenário (não fica mais fundo que o real)"
        );
    }

    // Plano 078 (simetria): a mesma fronteira vale para saída — uma despesa hipotética de hoje
    // REDUZ o saldo encadeado do cenário (não some da trajetória).
    #[tokio::test]
    async fn hypothetical_expense_today_lowers_balance_chain() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-02").await;

        let sc = create_scenario(&p, "Gasto de hoje").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            200_000,
            "Compra hoje",
            "2026-08-01", // == today
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();

        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        assert_eq!(
            month.scenario_balance_cents,
            month.real_balance_cents - 200_000,
            "despesa hipotética de hoje entra no encadeamento e reduz o saldo do cenário"
        );
    }

    // Plano 078 (anti-double-count): o fix só move a fronteira de HOJE. Um hipotético datado de
    // AMANHÃ (já dentro da janela antes e depois) precisa contribuir EXATAMENTE uma vez para o
    // encadeamento — métricas e encadeamento são pipelines separadas; o de amanhã não pode passar
    // a somar em dobro.
    #[tokio::test]
    async fn hypothetical_tomorrow_counts_once_in_chain() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-10").await;

        let sc = create_scenario(&p, "Amanhã").await.unwrap();
        add_scenario_transaction(
            &p,
            &sc.id,
            "income",
            300_000,
            "Entrada amanhã",
            "2026-08-02", // > today (2026-08-01)
            None,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        let today = d("2026-08-01");
        let compare = get_scenario_forecast_inner(&p, &sc.id, today)
            .await
            .unwrap();

        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        assert_eq!(
            month.scenario_balance_cents,
            month.real_balance_cents + 300_000,
            "hipotético de amanhã soma uma única vez ao encadeamento (sem double-count)"
        );
    }

    // MAJOR 2: um SEGUNDO grupo `#loan:` não vira `loan` (só o primeiro, ordem data,id), mas as
    // linhas dele PRECISAM aparecer em `changes` como "add" — nunca somem do DTO.
    #[tokio::test]
    async fn second_loan_group_rows_surface_in_changes() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-01").await;

        let sc = create_scenario(&p, "Dois financiamentos").await.unwrap();
        // Grupo A (primeiro por data): reportado via `loan`.
        for (ttype, amount, desc, date) in [
            (
                "income",
                1_000_000,
                "Desembolso #loan:carro:150",
                "2026-08-05",
            ),
            ("expense", 90_000, "Parcela 1 #loan:carro:150", "2026-08-10"),
        ] {
            add_scenario_transaction(
                &p, &sc.id, ttype, amount, desc, date, None, false, None, None,
            )
            .await
            .unwrap();
        }
        // Grupo B (depois): deve aparecer em `changes`.
        for (ttype, amount, desc, date) in [
            ("income", 500_000, "Desembolso #loan:moto:200", "2026-09-05"),
            ("expense", 55_000, "Parcela 1 #loan:moto:200", "2026-09-10"),
        ] {
            add_scenario_transaction(
                &p, &sc.id, ttype, amount, desc, date, None, false, None, None,
            )
            .await
            .unwrap();
        }

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-07-31"))
            .await
            .unwrap();

        let loan = compare.loan.expect("primeiro grupo detectado");
        assert_eq!(
            loan.loan_monthly_rate_bps, 150,
            "o loan reportado é o grupo A"
        );

        let group_b_changes: Vec<_> = compare
            .changes
            .iter()
            .filter(|c| c.description.contains("#loan:moto:200"))
            .collect();
        assert_eq!(
            group_b_changes.len(),
            2,
            "as 2 linhas do segundo financiamento aparecem em changes"
        );
        assert!(group_b_changes.iter().all(|c| c.op == "add"));
        assert!(
            !compare
                .changes
                .iter()
                .any(|c| c.description.contains("#loan:carro:150")),
            "as linhas do grupo reportado via `loan` não duplicam em changes"
        );
    }

    // MAJOR 3 (guard): um segundo override para a MESMA obrigação no mesmo cenário é rejeitado.
    #[tokio::test]
    async fn duplicate_override_same_obligation_rejected() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await
        .unwrap();
        let second = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2026-09-01",
            Some(&ob_id),
            None,
            None,
        )
        .await;
        assert!(
            second.is_err() && second.unwrap_err().contains("já existe"),
            "segundo override para a mesma obrigação é rejeitado com erro limpo"
        );
    }

    // MAJOR 3 (guard, braço recorrência): mesma regra para recurrence_id.
    #[tokio::test]
    async fn duplicate_override_same_recurrence_rejected() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        sqlx::query(
            "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
             VALUES ('rec-1', 'mensal', 0, 2, '2026-08-05')",
        )
        .execute(&p)
        .await
        .unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            None,
            Some("rec-1"),
            None,
        )
        .await
        .unwrap();
        let second = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-09-01",
            None,
            Some("rec-1"),
            None,
        )
        .await;
        assert!(
            second.is_err() && second.unwrap_err().contains("já existe"),
            "segundo override para a mesma recorrência é rejeitado"
        );
    }

    // MAJOR 3 (defesa em profundidade): mesmo com DUAS linhas de override duplicadas já no banco
    // (por fora do guard), a supressão do item conta UMA vez — a célula multi-item não zera e o
    // irmão continua contribuindo.
    #[tokio::test]
    async fn preexisting_duplicate_override_rows_do_not_double_suppress() {
        let p = pool().await;
        seed_baseline(&p).await;
        txn(&p, "cell-1", "expense", 108_000, "2026-08-05").await;
        line_item(&p, "li-aluguel", "cell-1", 100_000, "Aluguel", None).await;
        line_item(&p, "li-internet", "cell-1", 8_000, "Internet", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let sc = create_scenario(&p, "Cenário").await.unwrap();

        // Duas linhas duplicadas direto no banco (o CHECK XOR permite; não há UNIQUE).
        for ov_id in ["ov-1", "ov-2"] {
            sqlx::query(
                "INSERT INTO scenario_override (id, scenario_id, op, from_date, obligation_id) \
                 VALUES (?1, ?2, 'suppress', '2026-08-01', ?3)",
            )
            .bind(ov_id)
            .bind(&sc.id)
            .bind(&ob_id)
            .execute(&p)
            .await
            .unwrap();
        }

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-07-31"))
            .await
            .unwrap();
        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        // 500.000 − 8.000 (Internet intacta): o Aluguel é subtraído UMA vez (100.000), não duas
        // (o que zeraria a célula e apagaria a Internet junto).
        assert_eq!(
            month.scenario_balance_cents, 492_000,
            "supressão duplicada não subtrai duas vezes nem derruba o irmão"
        );
    }

    // Pareamento replace: dois replaces CONCORRENTES viram duas entradas fundidas
    // {op:"replace", old, new} corretamente pareadas via `#repl:<override_id>` — sem "add" avulso.
    #[tokio::test]
    async fn two_concurrent_replaces_pair_old_and_new() {
        let p = pool().await;
        seed_baseline(&p).await;
        txn(&p, "t-aluguel", "expense", 150_000, "2026-08-05").await;
        line_item(&p, "li-a", "t-aluguel", 150_000, "Aluguel", None).await;
        txn(&p, "t-internet", "expense", 8_000, "2026-08-06").await;
        line_item(&p, "li-i", "t-internet", 8_000, "Internet", None).await;
        let ob_aluguel = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ob_internet = obligations::create_obligation(&p, "Internet", "Internet", None)
            .await
            .unwrap();

        let sc = create_scenario(&p, "Mudança").await.unwrap();
        let repl = |amount: i64, date: &str, label: &str| ReplacementInput {
            amount_cents: amount,
            date: date.to_string(),
            description: Some(label.to_string()),
            txn_type: None,
            payment_method: None,
            is_fixed: None,
        };
        set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2026-08-01",
            Some(&ob_aluguel),
            None,
            Some(repl(200_000, "2026-08-05", "Aluguel novo")),
        )
        .await
        .unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2026-08-01",
            Some(&ob_internet),
            None,
            Some(repl(10_000, "2026-08-06", "Internet nova")),
        )
        .await
        .unwrap();

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-07-31"))
            .await
            .unwrap();

        let replaces: Vec<_> = compare
            .changes
            .iter()
            .filter(|c| c.op == "replace")
            .collect();
        assert_eq!(replaces.len(), 2, "duas entradas replace fundidas");
        let aluguel = replaces
            .iter()
            .find(|c| c.description == "Aluguel")
            .unwrap();
        assert_eq!(aluguel.old_amount_cents, Some(150_000));
        assert_eq!(
            aluguel.new_amount_cents,
            Some(200_000),
            "par velho→novo do Aluguel"
        );
        let internet = replaces
            .iter()
            .find(|c| c.description == "Internet")
            .unwrap();
        assert_eq!(internet.old_amount_cents, Some(8_000));
        assert_eq!(
            internet.new_amount_cents,
            Some(10_000),
            "par velho→novo da Internet"
        );
        assert!(
            !compare.changes.iter().any(|c| c.op == "add"),
            "as linhas de substituição pareadas não aparecem como add avulso"
        );

        // E a matemática segue sem dupla contagem: 500.000 − 200.000 − 10.000 = 290.000.
        let month = compare
            .month_end
            .iter()
            .find(|m| m.year == 2026 && m.month == 8)
            .unwrap();
        assert_eq!(month.scenario_balance_cents, 290_000);
    }

    // MINOR: o braço de recorrência também preenche `old_amount_cents` em changes.
    #[tokio::test]
    async fn recurrence_override_reports_old_amount_in_changes() {
        let p = pool().await;
        seed_baseline(&p).await;
        sqlx::query(
            "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
             VALUES ('rec-1', 'mensal', 0, 2, '2026-08-05')",
        )
        .execute(&p)
        .await
        .unwrap();
        for (id, date) in [("r-1", "2026-08-05"), ("r-2", "2026-09-05")] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, date, is_projection, recurrence_id) \
                 VALUES (?1, 'expense', 50000, ?2, 1, 'rec-1')",
            )
            .bind(id)
            .bind(date)
            .execute(&p)
            .await
            .unwrap();
        }

        let sc = create_scenario(&p, "Cenário").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            None,
            Some("rec-1"),
            None,
        )
        .await
        .unwrap();

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-07-31"))
            .await
            .unwrap();
        let change = compare
            .changes
            .iter()
            .find(|c| c.op == "remove")
            .expect("entrada remove da recorrência");
        assert_eq!(
            change.old_amount_cents,
            Some(100_000),
            "soma das 2 ocorrências suprimidas (2 × 50.000)"
        );
    }

    // MINOR: âncora das marcas — `#loan:`/`#repl:` no meio do texto ou colados não são varridos.
    #[test]
    fn markers_are_anchored_to_the_end_of_the_description() {
        assert_eq!(
            parse_loan_marker("Parcela 1 #loan:carro:150"),
            Some(("carro".to_string(), 150))
        );
        assert_eq!(
            parse_loan_marker("Compra #loan:carro:150 sapatos"),
            None,
            "marca no meio do texto não conta"
        );
        assert_eq!(
            parse_loan_marker("nota#loan:carro:150"),
            None,
            "marca colada em outra palavra não conta"
        );
        assert_eq!(parse_loan_marker("sem marca"), None);

        assert_eq!(
            parse_repl_marker("Aluguel novo #repl:ov-123"),
            Some("ov-123".to_string())
        );
        assert_eq!(parse_repl_marker("nota #repl:ov-1 depois"), None);
        assert_eq!(parse_repl_marker("colado#repl:ov-1"), None);
        assert_eq!(parse_repl_marker("sem marca"), None);
    }

    // MINOR: data anterior ao mês corrente sumiria SILENCIOSAMENTE dos dois ramos → rejeitada.
    #[tokio::test]
    async fn add_scenario_transaction_rejects_date_before_current_month() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let result = add_scenario_transaction(
            &p,
            &sc.id,
            "expense",
            1000,
            "Antiga",
            "2020-01-01",
            None,
            false,
            None,
            None,
        )
        .await;
        assert!(
            result.is_err() && result.unwrap_err().contains("mês corrente"),
            "data no passado (antes do mês corrente) é rejeitada com erro limpo"
        );
    }
}
