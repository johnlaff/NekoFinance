//! Motor de "what-if": CRUD de cenários hipotéticos, comparação real × cenário e ferramenta
//! determinística de empréstimo pela tabela PRICE.
//!
//! Um `scenario` é só um rótulo (nome + autoria); as linhas hipotéticas em si são
//! `"transaction"` rows com `scenario_id` setado. Um `scenario_override` é uma AÇÃO
//! sobre o livro-razão REAL, escopada a uma obrigação ou a uma série recorrente —
//! nunca sobre o cenário em si. Nada aqui muta o livro-razão real: os overrides só afetam a
//! PROJEÇÃO do cenário (`get_scenario_forecast`); o forecast real (`get_forecast`) continua
//! cego à existência de qualquer cenário por meio de `scenario_id IS NULL`.
//!
//! EMPRÉSTIMO: uma entidade `scenario_loan` guarda os PARÂMETROS (principal, taxa, prazo, datas
//! do desembolso e da 1ª parcela); as linhas hipotéticas do grupo (Entrada do principal + N
//! Saídas das parcelas, geradas por `price_installment`) apontam para ela via
//! `transaction.loan_id` (FK, `ON DELETE CASCADE`). Criar, editar e remover são cada um uma
//! única transação SQL; editar REGENERA a série inteira a partir dos parâmetros novos. Apagar a
//! última linha de um empréstimo apaga o registro na mesma transação — um empréstimo existe
//! enquanto tiver ao menos uma linha ("sem fantasma"). `get_scenario_forecast` reporta só o
//! PRIMEIRO grupo (ordem data,id) como `loan`; linhas de grupos seguintes aparecem como
//! entradas "add" comuns em `changes`. Bancos legados marcavam o grupo com o sufixo
//! `" #loan:<group_id>:<taxa_bps>"` na descrição — `backfill_scenario_loans` (startup) converte
//! esses grupos em entidades e remove os sufixos.
//!
//! SUBSTITUIÇÃO (`replace`): quando `set_scenario_override` recebe `op = "replace"` com um
//! `replacement` preenchido, ele gera UMA linha hipotética por ocorrência suprimida (datas dos
//! itens da obrigação, ou das linhas reais da recorrência, `date >= from_date`) — uma SÉRIE, não
//! "suprime N meses, repõe 1". Todas as linhas apontam para o `scenario_override` via
//! `transaction.override_id` (FK, `ON DELETE CASCADE`): é essa identidade que permite ao compare
//! FUNDIR o par velho→novo numa única entrada `{op:"replace", old, new}` de `changes` (e excluir
//! as linhas da lista de "add"), e que faz "substituir X por Y" morrer junto com a obrigação —
//! nunca degradar para "manter X e adicionar Y". Não há remoção individual de override: a série
//! some em cascata quando a obrigação/recorrência/cenário é apagado. Bancos legados marcavam a
//! linha com o sufixo `" #repl:<override_id>"` na descrição —
//! `backfill_scenario_override_replacements` (startup) converte esses marcadores em FK e os remove.

use crate::commands::forecast_cmds::{
    self, finalize_card_events, forecast_horizon_end, load_economia_annotation,
    load_forecast_events, load_metric_events, projection_seed, reserve_floor,
};
use crate::commands::map_cashflow_row;
use crate::forecast::{self, CashflowEvent};
use crate::obligations;
use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
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
/// `ON DELETE CASCADE`; apagar aqui já limpa as linhas hipotéticas e os overrides.
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
    let mut connection = pool
        .acquire()
        .await
        .map_err(|e| format!("add_scenario_transaction (acquire): {e}"))?;
    insert_scenario_transaction(
        &mut connection,
        scenario_id,
        txn_type,
        amount_cents,
        description,
        date,
        payment_method,
        is_fixed,
        to_account_id,
        due_date,
        None,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_scenario_transaction(
    connection: &mut SqliteConnection,
    scenario_id: &str,
    txn_type: &str,
    amount_cents: i64,
    description: &str,
    date: &str,
    payment_method: Option<&str>,
    is_fixed: bool,
    to_account_id: Option<&str>,
    due_date: Option<&str>,
    loan_id: Option<&str>,
    override_id: Option<&str>,
) -> Result<String, String> {
    let description = description.trim();
    if description.is_empty() {
        return Err("descrição obrigatória para uma linha de cenário".into());
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let destination = match txn_type {
        "income" | "expense" => {
            if to_account_id.is_some_and(|id| !id.is_empty()) {
                return Err("conta-destino só se aplica a transfer (Economia)".into());
            }
            None
        }
        "transfer" => {
            let destination_id = to_account_id
                .filter(|id| !id.is_empty())
                .ok_or("transfer requer conta-destino (to_account_id)")?;
            let row: Option<(String,)> =
                sqlx::query_as("SELECT COALESCE(liquidity,'') FROM account WHERE id = ?1")
                    .bind(destination_id)
                    .fetch_optional(&mut *connection)
                    .await
                    .map_err(|e| format!("query account: {e}"))?;
            match row {
                None => return Err("conta-destino não encontrada".into()),
                Some((liquidity,)) if liquidity == "reserve" || liquidity == "illiquid" => {}
                Some((liquidity,)) => {
                    return Err(format!(
                        "conta-destino deve ter liquidez 'reserve' ou 'illiquid', encontrado '{liquidity}'"
                    ));
                }
            }
            Some(destination_id)
        }
        other => return Err(format!("tipo inválido: {other}")),
    };
    let start = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| format!("data: {e}"))?;
    let today = chrono::Local::now().date_naive();
    let month_start =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("data de hoje inválida")?;
    if start < month_start {
        return Err("data anterior ao mês corrente não entra na projeção do cenário".into());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO \"transaction\" \
           (id, type, amount, description, date, payment_method, is_fixed, to_account_id, \
            is_projection, due_date, scenario_id, loan_id, override_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
    )
    .bind(&id)
    .bind(txn_type)
    .bind(amount_cents)
    .bind(description)
    .bind(date)
    .bind(payment_method)
    .bind(is_fixed as i64)
    .bind(destination)
    .bind((start > today) as i64)
    .bind(due_date)
    .bind(scenario_id)
    .bind(loan_id)
    .bind(override_id)
    .bind(&now)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("add_scenario_transaction: {e}"))?;
    Ok(id)
}

/// Apaga uma linha hipotética. Só apaga se `scenario_id` casar — impede apagar uma linha REAL ou
/// de OUTRO cenário pelo id. Se a linha era a ÚLTIMA de um empréstimo, o registro `scenario_loan`
/// morre na mesma transação — um empréstimo existe enquanto tiver ao menos uma linha (invariante
/// "sem fantasma"; o estado "sem parcelas restantes" não existe).
pub async fn delete_scenario_transaction(
    pool: &SqlitePool,
    scenario_id: &str,
    txn_id: &str,
) -> Result<(), String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("delete_scenario_transaction (begin): {e}"))?;
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT loan_id FROM \"transaction\" WHERE id = ?1 AND scenario_id = ?2")
            .bind(txn_id)
            .bind(scenario_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("delete_scenario_transaction: {e}"))?;
    let Some((loan_id,)) = row else {
        return Err(format!(
            "scenario transaction not found: {txn_id} (scenario {scenario_id})"
        ));
    };
    sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1 AND scenario_id = ?2")
        .bind(txn_id)
        .bind(scenario_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete_scenario_transaction: {e}"))?;
    if let Some(loan_id) = loan_id {
        sqlx::query(
            "DELETE FROM scenario_loan WHERE id = ?1 \
             AND NOT EXISTS (SELECT 1 FROM \"transaction\" WHERE loan_id = ?1)",
        )
        .bind(&loan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete_scenario_transaction (loan cleanup): {e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("delete_scenario_transaction (commit): {e}"))
}

/// Uma linha hipotética crua do cenário, para a UI listar/apagar. `loan_id` presente = linha de
/// um empréstimo (a UI agrupa por ele e busca os parâmetros em `list_scenario_loans`);
/// `override_id` presente = linha de uma série de substituição (some com o override em cascata).
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq)]
pub struct ScenarioTransactionRow {
    pub id: String,
    pub r#type: String,
    pub amount: i64,
    pub description: String,
    pub date: String,
    pub loan_id: Option<String>,
    pub override_id: Option<String>,
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
        "SELECT id, type, amount, COALESCE(description,'') AS description, date, loan_id, \
         override_id \
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

/// A SÉRIE de substituição que acompanha um override `replace` (opcional). `set_scenario_override`
/// gera UMA linha hipotética por ocorrência suprimida (datas derivadas do alvo — itens da
/// obrigação ou linhas reais da recorrência, `date >= from_date`), todas donas do `override_id`
/// via FK. Sem campo de data: as datas vêm do alvo, não de uma entrada única (o antigo modelo de
/// "suprime N, repõe 1"). `amount_cents` é o novo valor de CADA ocorrência. Defaults:
/// `txn_type = "expense"`, `is_fixed = true` (uma obrigação substituída é tipicamente uma Saída
/// fixa), `description = nome genérico`.
#[derive(Debug, serde::Deserialize, Clone)]
pub struct ReplacementInput {
    pub amount_cents: i64,
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
/// profundidade). Alvo-obrigação passa pela FRONTEIRA (`ensure_suppression_preserves_siblings`):
/// recusa o caso destrutivo em que suprimir zeraria uma célula com irmão vivo. Para
/// `op = "replace"`, um `replacement` gera a SÉRIE de substituição — uma linha por ocorrência
/// suprimida, todas donas do `override_id` (FK).
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
    let from_date_naive =
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

    // Fronteira: suprimir (op `suppress` OU o suprimir embutido no `replace`) uma obrigação não
    // pode zerar uma célula com irmão não suprimido. Leitura pura, antes de abrir a transação.
    if let Some(obligation_id) = obligation_id {
        ensure_suppression_preserves_siblings(pool, scenario_id, obligation_id, from_date_naive)
            .await?;
    }

    // As datas da série de substituição vêm do alvo (não do override recém-criado), então derivam
    // ANTES do `begin`: o pool é `max_connections(1)`, e ler com a transação aberta esperaria uma
    // segunda conexão que nunca vem (deadlock em produção; os testes em memória não expõem).
    let replacement_dates = match &replacement {
        Some(_) => {
            replacement_occurrence_dates(pool, obligation_id, recurrence_id, from_date_naive)
                .await?
        }
        None => Vec::new(),
    };

    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("set_scenario_override (begin): {e}"))?;
    let inserted = sqlx::query(
        "INSERT INTO scenario_override (id, scenario_id, op, from_date, obligation_id, recurrence_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(scenario_id)
    .bind(op)
    .bind(from_date)
    .bind(obligation_id)
    .bind(recurrence_id)
    .execute(&mut *tx)
    .await;
    if let Err(error) = inserted {
        if error
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
        {
            return Err(if obligation_id.is_some() {
                "já existe uma alteração para esta obrigação neste cenário".into()
            } else {
                "já existe uma alteração para esta recorrência neste cenário".into()
            });
        }
        return Err(format!("set_scenario_override: {error}"));
    }

    // Série de substituição (op=replace): UMA linha por ocorrência suprimida (datas já derivadas
    // acima), todas donas do `override_id` via FK. Falhou uma linha → rollback (sem série órfã) e
    // propaga o erro. `insert_scenario_transaction` recusa data antes do mês corrente, já filtrada.
    if let Some(repl) = replacement {
        let base = repl
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Substituição")
            .to_string();
        for date in replacement_dates {
            insert_scenario_transaction(
                &mut tx,
                scenario_id,
                repl.txn_type.as_deref().unwrap_or("expense"),
                repl.amount_cents,
                &base,
                &date,
                repl.payment_method.as_deref(),
                repl.is_fixed.unwrap_or(true),
                None,
                None,
                None,
                Some(&id),
            )
            .await
            .map_err(|e| format!("linha de substituição inválida: {e}"))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| format!("set_scenario_override (commit): {e}"))?;
    Ok(id)
}

/// Fronteira do override sobre uma obrigação: recusa a criação quando suprimir os itens casados
/// (`date >= from_date`) ZERARIA alguma célula que ainda tem itens irmãos não suprimidos — o único
/// caso em que a supressão line-item de `apply_suppression` derruba a linha inteira e, com ela, a
/// contribuição do irmão (nota SOBRE-explicada: itens somam mais que o total). A supressão é
/// CUMULATIVA: dois overrides em obrigações distintas podem casar itens da MESMA célula, então
/// somamos a footprint dos overrides já existentes no cenário + o novo (deduplicando por
/// line_item, igual a `apply_suppression`). Célula sub-explicada e célula sem irmão passam: a
/// subtração preserva o residual e o irmão. Recorrência não tem fronteira — série de propósito
/// único, sem line_item / irmão a preservar.
async fn ensure_suppression_preserves_siblings(
    pool: &SqlitePool,
    scenario_id: &str,
    new_obligation_id: &str,
    new_from_date: NaiveDate,
) -> Result<(), String> {
    let mut targets: Vec<(String, NaiveDate)> =
        vec![(new_obligation_id.to_string(), new_from_date)];
    for ov in list_scenario_overrides(pool, scenario_id).await? {
        if let Some(obligation_id) = ov.obligation_id
            && let Ok(from_date) = NaiveDate::parse_from_str(&ov.from_date, "%Y-%m-%d")
        {
            targets.push((obligation_id, from_date));
        }
    }

    // Só as células que a projeção realmente toca (`>= mês corrente`) entram na fronteira: uma
    // célula histórica nunca é suprimida de fato (`load_real_rows` começa em `month_start`), então
    // não pode derrubar irmão na projeção — bloquear por causa dela seria falso-positivo, mesmo
    // que a nota seja sobre-explicada. Mesmo piso de `replacement_occurrence_dates`.
    let today = chrono::Local::now().date_naive();
    let month_start =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("data de hoje inválida")?;
    // Footprint combinada, deduplicada por line_item (cada item conta UMA vez para a supressão da
    // célula, como o `seen_line_items` de `build_suppression_plan`). A ordem não altera o total.
    let mut suppressed_by_cell: HashMap<String, i64> = HashMap::new();
    let mut cell_date: HashMap<String, String> = HashMap::new();
    let mut matched_line_items: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (obligation_id, from_date) in &targets {
        let floor = (*from_date).max(month_start);
        for item in obligations::obligation_items(pool, obligation_id).await? {
            let Ok(d) = NaiveDate::parse_from_str(&item.date, "%Y-%m-%d") else {
                continue;
            };
            if d >= floor && matched_line_items.insert(item.line_item_id.clone()) {
                *suppressed_by_cell
                    .entry(item.transaction_id.clone())
                    .or_insert(0) += item.amount_cents.abs();
                cell_date
                    .entry(item.transaction_id.clone())
                    .or_insert(item.date.clone());
            }
        }
    }

    for (transaction_id, suppressed) in &suppressed_by_cell {
        let (total,): (i64,) =
            sqlx::query_as("SELECT ABS(amount) FROM \"transaction\" WHERE id = ?1")
                .bind(transaction_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("fronteira (total da célula): {e}"))?;
        if *suppressed < total {
            continue; // residual > 0: a linha sobrevive, irmão preservado.
        }
        // Zeraria a célula. Tem irmão vivo (line_item fora do matched, magnitude > 0)?
        let cell_items: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, ABS(amount_cents) FROM line_item WHERE transaction_id = ?1")
                .bind(transaction_id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("fronteira (irmãos da célula): {e}"))?;
        let sibling_sum: i64 = cell_items
            .iter()
            .filter(|(id, _)| !matched_line_items.contains(id))
            .map(|(_, magnitude)| *magnitude)
            .sum();
        if sibling_sum > 0 {
            let date = cell_date.get(transaction_id).cloned().unwrap_or_default();
            return Err(format!(
                "esta alteração zeraria a célula de {date}, que ainda tem itens que você não está \
                 alterando — a soma dos itens da nota passa do total, então suprimir este apagaria \
                 os irmãos junto. Ajuste o total da nota na origem antes de simular."
            ));
        }
    }
    Ok(())
}

/// Datas das ocorrências suprimidas de um alvo (`>= from_date`) para a série de substituição — uma
/// linha por data, deduplicadas e ordenadas (série determinística). Ocorrências antes do mês
/// corrente ficam de fora: são inertes na projeção (que começa hoje) e `insert_scenario_transaction`
/// as recusa. Obrigação → datas dos `obligation_items`; recorrência → datas das linhas reais da
/// série.
async fn replacement_occurrence_dates(
    pool: &SqlitePool,
    obligation_id: Option<&str>,
    recurrence_id: Option<&str>,
    from_date: NaiveDate,
) -> Result<Vec<String>, String> {
    let today = chrono::Local::now().date_naive();
    let month_start =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("data de hoje inválida")?;
    let floor = from_date.max(month_start);

    let mut dates: Vec<NaiveDate> = Vec::new();
    if let Some(obligation_id) = obligation_id {
        for item in obligations::obligation_items(pool, obligation_id).await? {
            if let Ok(d) = NaiveDate::parse_from_str(&item.date, "%Y-%m-%d")
                && d >= floor
            {
                dates.push(d);
            }
        }
    } else if let Some(recurrence_id) = recurrence_id {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT date FROM \"transaction\" \
             WHERE recurrence_id = ?1 AND scenario_id IS NULL AND date >= ?2 ORDER BY date",
        )
        .bind(recurrence_id)
        .bind(floor.format("%Y-%m-%d").to_string())
        .fetch_all(pool)
        .await
        .map_err(|e| format!("ocorrências da recorrência: {e}"))?;
        for (date,) in rows {
            if let Ok(d) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                dates.push(d);
            }
        }
    }
    dates.sort_unstable();
    dates.dedup();
    Ok(dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect())
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

/// Uma recorrência REAL (série do livro-razão) oferecível como alvo de override no seletor da UI.
/// A tabela `recurrence` não guarda rótulo — ele vive nas transações da série; expomos a descrição
/// da ocorrência mais antiga + a frequência. Só recorrências com ≥ 1 ocorrência real aparecem.
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct RecurrenceTarget {
    pub id: String,
    pub description: String,
    pub frequency: String,
    pub first_date: String,
}

pub async fn list_recurrence_targets(pool: &SqlitePool) -> Result<Vec<RecurrenceTarget>, String> {
    sqlx::query_as::<_, RecurrenceTarget>(
        "SELECT r.id, \
                COALESCE((SELECT description FROM \"transaction\" \
                          WHERE recurrence_id = r.id AND scenario_id IS NULL \
                          ORDER BY date, id LIMIT 1), '') AS description, \
                r.frequency, \
                COALESCE((SELECT MIN(date) FROM \"transaction\" \
                          WHERE recurrence_id = r.id AND scenario_id IS NULL), '') AS first_date \
         FROM recurrence r \
         WHERE EXISTS (SELECT 1 FROM \"transaction\" \
                       WHERE recurrence_id = r.id AND scenario_id IS NULL) \
         ORDER BY first_date DESC, r.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_recurrence_targets: {e}"))
}

/// Ocorrências reais de uma recorrência (data + magnitude) — o análogo de `obligation_items` para
/// recorrências: alimenta a prévia "afeta N ocorrências a partir de {data}" do seletor de alvo.
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct RecurrenceOccurrence {
    pub date: String,
    pub amount_cents: i64,
}

pub async fn recurrence_occurrences(
    pool: &SqlitePool,
    recurrence_id: &str,
) -> Result<Vec<RecurrenceOccurrence>, String> {
    sqlx::query_as::<_, RecurrenceOccurrence>(
        "SELECT date, ABS(amount) AS amount_cents FROM \"transaction\" \
         WHERE recurrence_id = ?1 AND scenario_id IS NULL ORDER BY date, id",
    )
    .bind(recurrence_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("recurrence_occurrences: {e}"))
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

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioLoanInput {
    pub scenario_id: String,
    pub principal_cents: i64,
    pub term_months: u32,
    pub rate_bps: i64,
    pub disbursement_date: String,
    pub first_installment_date: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScenarioId(String);

impl ScenarioId {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MoneyCents(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RateBps(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TermMonths(u32);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScenarioLoan {
    scenario_id: ScenarioId,
    principal: MoneyCents,
    term: TermMonths,
    rate: RateBps,
    disbursement_date: NaiveDate,
    first_installment_date: NaiveDate,
    description: String,
}

impl TryFrom<ScenarioLoanInput> for ScenarioLoan {
    type Error = String;

    fn try_from(input: ScenarioLoanInput) -> Result<Self, Self::Error> {
        let scenario_id = input.scenario_id.trim();
        if scenario_id.is_empty() {
            return Err("scenario_id obrigatório".into());
        }
        if input.principal_cents <= 0 {
            return Err("principal deve ser positivo".into());
        }
        if !(1..=480).contains(&input.term_months) {
            return Err("prazo deve estar entre 1 e 480 meses".into());
        }
        if input.rate_bps < 0 {
            return Err("taxa deve ser maior ou igual a zero".into());
        }
        let disbursement_date = NaiveDate::parse_from_str(&input.disbursement_date, "%Y-%m-%d")
            .map_err(|e| format!("data do desembolso: {e}"))?;
        let first_installment_date =
            NaiveDate::parse_from_str(&input.first_installment_date, "%Y-%m-%d")
                .map_err(|e| format!("data da primeira parcela: {e}"))?;
        let description = input.description.trim();

        Ok(Self {
            scenario_id: ScenarioId(scenario_id.to_string()),
            principal: MoneyCents(input.principal_cents),
            term: TermMonths(input.term_months),
            rate: RateBps(input.rate_bps),
            disbursement_date,
            first_installment_date,
            description: if description.is_empty() {
                "Empréstimo".into()
            } else {
                description.to_string()
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScenarioLoanLineKind {
    Principal,
    Installment,
}

impl ScenarioLoanLineKind {
    fn transaction_type(self) -> &'static str {
        match self {
            Self::Principal => "income",
            Self::Installment => "expense",
        }
    }

    fn is_fixed(self) -> bool {
        matches!(self, Self::Installment)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScenarioLoanLine {
    kind: ScenarioLoanLineKind,
    amount: MoneyCents,
    description: String,
    date: NaiveDate,
}

fn plan_scenario_loan(loan: &ScenarioLoan) -> Result<Vec<ScenarioLoanLine>, String> {
    let installment = MoneyCents(price_installment(
        loan.principal.0,
        loan.rate.0,
        loan.term.0,
    ));
    let mut lines = Vec::with_capacity(loan.term.0 as usize + 1);
    lines.push(ScenarioLoanLine {
        kind: ScenarioLoanLineKind::Principal,
        amount: loan.principal,
        description: loan.description.clone(),
        date: loan.disbursement_date,
    });

    for index in 0..loan.term.0 {
        let date = loan
            .first_installment_date
            .checked_add_months(Months::new(index))
            .ok_or("data de parcela fora do intervalo suportado")?;
        lines.push(ScenarioLoanLine {
            kind: ScenarioLoanLineKind::Installment,
            amount: installment,
            description: format!("{} parcela {}/{}", loan.description, index + 1, loan.term.0),
            date,
        });
    }

    Ok(lines)
}

/// Insere as linhas planejadas de um empréstimo dentro da transação corrente, todas apontando
/// para `loan_id`. Compartilhado entre criar e editar (que regenera a série inteira).
async fn insert_scenario_loan_lines(
    tx: &mut SqliteConnection,
    scenario_id: &str,
    loan_id: &str,
    lines: Vec<ScenarioLoanLine>,
) -> Result<(), String> {
    for line in lines {
        insert_scenario_transaction(
            tx,
            scenario_id,
            line.kind.transaction_type(),
            line.amount.0,
            &line.description,
            &line.date.format("%Y-%m-%d").to_string(),
            None,
            line.kind.is_fixed(),
            None,
            None,
            Some(loan_id),
            None,
        )
        .await?;
    }
    Ok(())
}

/// Cria o empréstimo hipotético: entidade `scenario_loan` + principal + N parcelas, numa única
/// transação. Devolve o id da entidade — a UI foca/realça o grupo recém-criado por ele.
pub async fn create_scenario_loan(
    pool: &SqlitePool,
    input: ScenarioLoanInput,
) -> Result<String, String> {
    let loan = ScenarioLoan::try_from(input)?;
    if !scenario_exists(pool, loan.scenario_id.as_str()).await? {
        return Err(format!("scenario not found: {}", loan.scenario_id.as_str()));
    }
    let lines = plan_scenario_loan(&loan)?;
    let loan_id = uuid::Uuid::new_v4().to_string();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("create_scenario_loan (begin): {e}"))?;

    insert_scenario_loan_row(&mut tx, &loan_id, &loan).await?;
    insert_scenario_loan_lines(&mut tx, loan.scenario_id.as_str(), &loan_id, lines).await?;

    tx.commit()
        .await
        .map_err(|e| format!("create_scenario_loan (commit): {e}"))?;
    Ok(loan_id)
}

async fn insert_scenario_loan_row(
    tx: &mut SqliteConnection,
    loan_id: &str,
    loan: &ScenarioLoan,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO scenario_loan \
           (id, scenario_id, principal_cents, rate_bps, term_months, disbursement_date, \
            first_installment_date, description) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(loan_id)
    .bind(loan.scenario_id.as_str())
    .bind(loan.principal.0)
    .bind(loan.rate.0)
    .bind(loan.term.0)
    .bind(loan.disbursement_date.format("%Y-%m-%d").to_string())
    .bind(loan.first_installment_date.format("%Y-%m-%d").to_string())
    .bind(&loan.description)
    .execute(tx)
    .await
    .map_err(|e| format!("insert scenario_loan: {e}"))?;
    Ok(())
}

/// Edita o empréstimo: valida a posse (`loan_id` pertence ao cenário do input), atualiza os
/// parâmetros e REGENERA a série inteira (DELETE + re-INSERT) na mesma transação — parcelas
/// removidas à mão não sobrevivem à re-parametrização (a série é sempre função determinística
/// dos parâmetros via PRICE; a UI avisa antes de restaurar).
pub async fn update_scenario_loan(
    pool: &SqlitePool,
    loan_id: &str,
    input: ScenarioLoanInput,
) -> Result<(), String> {
    let loan = ScenarioLoan::try_from(input)?;
    let owner: Option<(String,)> =
        sqlx::query_as("SELECT scenario_id FROM scenario_loan WHERE id = ?1")
            .bind(loan_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("update_scenario_loan: {e}"))?;
    match owner {
        None => return Err(format!("scenario loan not found: {loan_id}")),
        Some((scenario_id,)) if scenario_id != loan.scenario_id.as_str() => {
            return Err(format!(
                "scenario loan not found: {loan_id} (scenario {})",
                loan.scenario_id.as_str()
            ));
        }
        Some(_) => {}
    }
    let lines = plan_scenario_loan(&loan)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("update_scenario_loan (begin): {e}"))?;

    sqlx::query(
        "UPDATE scenario_loan SET principal_cents = ?2, rate_bps = ?3, term_months = ?4, \
         disbursement_date = ?5, first_installment_date = ?6, description = ?7 WHERE id = ?1",
    )
    .bind(loan_id)
    .bind(loan.principal.0)
    .bind(loan.rate.0)
    .bind(loan.term.0)
    .bind(loan.disbursement_date.format("%Y-%m-%d").to_string())
    .bind(loan.first_installment_date.format("%Y-%m-%d").to_string())
    .bind(&loan.description)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("update_scenario_loan (update): {e}"))?;

    sqlx::query("DELETE FROM \"transaction\" WHERE loan_id = ?1")
        .bind(loan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("update_scenario_loan (delete lines): {e}"))?;

    insert_scenario_loan_lines(&mut tx, loan.scenario_id.as_str(), loan_id, lines).await?;

    tx.commit()
        .await
        .map_err(|e| format!("update_scenario_loan (commit): {e}"))
}

/// Remove o empréstimo inteiro (entidade + principal + parcelas via `ON DELETE CASCADE`) — um
/// único DELETE, atômico por natureza. Só remove se o empréstimo pertencer ao cenário.
pub async fn delete_scenario_loan(
    pool: &SqlitePool,
    scenario_id: &str,
    loan_id: &str,
) -> Result<(), String> {
    let rows = sqlx::query("DELETE FROM scenario_loan WHERE id = ?1 AND scenario_id = ?2")
        .bind(loan_id)
        .bind(scenario_id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete_scenario_loan: {e}"))?;
    if rows.rows_affected() == 0 {
        return Err(format!(
            "scenario loan not found: {loan_id} (scenario {scenario_id})"
        ));
    }
    Ok(())
}

/// Parâmetros de um empréstimo do cenário, para a UI montar o cabeçalho do grupo e pré-preencher
/// o formulário de edição.
#[derive(Debug, Serialize, sqlx::FromRow, Clone, PartialEq, Eq)]
pub struct ScenarioLoanRow {
    pub id: String,
    pub scenario_id: String,
    pub principal_cents: i64,
    pub rate_bps: i64,
    pub term_months: u32,
    pub disbursement_date: String,
    pub first_installment_date: String,
    pub description: String,
}

pub async fn list_scenario_loans(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<Vec<ScenarioLoanRow>, String> {
    sqlx::query_as::<_, ScenarioLoanRow>(
        "SELECT id, scenario_id, principal_cents, rate_bps, term_months, disbursement_date, \
         first_installment_date, description \
         FROM scenario_loan WHERE scenario_id = ?1 ORDER BY created_at, id",
    )
    .bind(scenario_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("list_scenario_loans: {e}"))
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct LoanBreakdown {
    pub loan_principal_cents: i64,
    pub loan_installment_cents: i64,
    pub loan_term_months: u32,
    pub loan_monthly_rate_bps: i64,
    pub loan_total_paid_cents: i64,
    pub loan_total_cost_cents: i64,
    /// Régua canônica de reserva ANTES do financiamento: saldo das contas de reserva ÷ custo de
    /// vida típico (mediana dos meses realizados completos) — a mesma conta do dashboard, para a
    /// comparação antes → depois ser legítima. `None` quando não há mês completo (sem típico).
    pub reserve_months_before_financing: Option<f64>,
    /// A mesma régua com a parcela somada ao denominador (reserva ÷ (mediana + parcela)): quantos
    /// meses a reserva cobre DEPOIS de assumir o compromisso novo. Só a parcela entra — as demais
    /// mudanças hipotéticas do cenário já têm leitura própria na trajetória e no guardrail.
    pub reserve_months_after_financing: Option<f64>,
    /// Segunda perna do gate: percentual poupado típico ANTES da parcela, em bps
    /// (mediana da economia registrada ÷ mediana das entradas, últimos 6 meses completos —
    /// mesma janela e estimador da régua de reserva). `None` quando a mediana de entradas é 0
    /// (sem % possível; a linha some).
    pub savings_rate_before_bps: Option<i64>,
    /// O mesmo percentual com a parcela descontada da economia típica, BRUTO — pode ser
    /// negativo quando a parcela excede a economia típica; o clamp em 0% é só de exibição.
    /// Só a parcela desconta (simétrico à régua de reserva).
    pub savings_rate_after_bps: Option<i64>,
    /// Mediana mensal da economia registrada (centavos): alimenta a regra da metade
    /// (parcela > ½ × economia típica ⇒ zona amarela) e a frase em R$ do popover.
    pub economia_median_cents: i64,
}

/// Extrai `(group_id, rate_bps)` da marca LEGADA `" #loan:<group_id>:<rate_bps>"` ao FINAL da
/// descrição (só `backfill_scenario_loans` ainda a encontra — nenhum caminho novo a escreve).
/// Ancorada: a marca precisa estar no fim (o parse da taxa consome até o último caractere) e
/// precedida de espaço/início — um "#loan:" solto no meio do texto não é varrido. `None` se a
/// descrição não carrega a marca.
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

/// Remove a marca legada `#loan:` do fim da descrição (chamado só quando `parse_loan_marker`
/// casou — a âncora já foi validada).
fn strip_loan_marker(description: &str) -> String {
    let trimmed = description.trim_end();
    match trimmed.rfind("#loan:") {
        Some(idx) => trimmed[..idx].trim_end().to_string(),
        None => trimmed.to_string(),
    }
}

/// Rótulo `"… parcela <i>/<N>"` no FIM da descrição (já sem marcador) → `(i, N)`.
fn parse_installment_label(description: &str) -> Option<(u32, u32)> {
    let rest = description.trim_end().rsplit_once(" parcela ")?.1;
    let (i_str, n_str) = rest.split_once('/')?;
    let i: u32 = i_str.trim().parse().ok()?;
    let n: u32 = n_str.trim().parse().ok()?;
    Some((i, n))
}

#[derive(Debug, sqlx::FromRow)]
struct LegacyLoanRow {
    id: String,
    #[sqlx(rename = "type")]
    ttype: String,
    amount: i64,
    date: String,
    description: String,
    scenario_id: String,
}

/// Parâmetros derivados de um grupo legado — só quando a derivação é LIMPA (ver
/// `derive_legacy_loan`).
struct DerivedLoan {
    principal_cents: i64,
    rate_bps: i64,
    term_months: u32,
    disbursement_date: NaiveDate,
    first_installment_date: NaiveDate,
    description: String,
}

/// Deriva os parâmetros do empréstimo de um grupo legado marcado com `#loan:`. Derivação limpa
/// exige: taxa idêntica em todas as linhas; exatamente UMA linha `income` (principal + data do
/// desembolso); ≥1 parcelas `expense`, todas com rótulo `"parcela i/N"` de mesmo `N` (1–480),
/// `i` distintos e valor idêntico. Parcelas removidas à mão não impedem a derivação: o prazo vem
/// do rótulo e a data da 1ª parcela é recuada `i−1` meses a partir da parcela de menor `i`.
/// `None` = grupo vira linhas soltas (só perde o sufixo, nada é apagado).
fn derive_legacy_loan(rows: &[(&LegacyLoanRow, i64)]) -> Option<DerivedLoan> {
    let rate_bps = rows.first()?.1;
    if rows.iter().any(|(_, rate)| *rate != rate_bps) {
        return None;
    }

    let mut principal: Option<(&LegacyLoanRow, NaiveDate)> = None;
    let mut installments: Vec<(u32, u32, i64, NaiveDate)> = Vec::new();
    for (row, _) in rows {
        let date = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").ok()?;
        match row.ttype.as_str() {
            "income" => {
                if principal.is_some() {
                    return None; // dois principais: não há derivação única.
                }
                principal = Some((row, date));
            }
            "expense" => {
                let (i, n) = parse_installment_label(&strip_loan_marker(&row.description))?;
                installments.push((i, n, row.amount.abs(), date));
            }
            _ => return None,
        }
    }

    let (principal_row, disbursement_date) = principal?;
    let (_, term_months, first_amount, _) = *installments.first()?;
    if !(1..=480).contains(&term_months) {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    for &(i, n, amount, _) in &installments {
        if n != term_months || amount != first_amount || i < 1 || i > n || !seen.insert(i) {
            return None;
        }
    }
    let &(min_i, _, _, min_date) = installments.iter().min_by_key(|(i, ..)| *i)?;
    let first_installment_date = min_date.checked_sub_months(Months::new(min_i - 1))?;

    Some(DerivedLoan {
        principal_cents: principal_row.amount.abs(),
        rate_bps,
        term_months,
        disbursement_date,
        first_installment_date,
        description: strip_loan_marker(&principal_row.description),
    })
}

/// Converte grupos legados marcados com `" #loan:<group_id>:<rate_bps>"` em entidades
/// `scenario_loan`, apontando as linhas via `loan_id` e removendo o sufixo das descrições; grupo
/// sem derivação limpa só perde o sufixo (vira linhas soltas comuns — nada é apagado). Cada
/// grupo é processado numa transação própria. Roda no startup logo após as migrações e é
/// idempotente: depois de processada, nenhuma descrição termina com o marcador.
pub async fn backfill_scenario_loans(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<LegacyLoanRow> = sqlx::query_as(
        "SELECT id, type, amount, date, COALESCE(description,'') AS description, scenario_id \
         FROM \"transaction\" WHERE scenario_id IS NOT NULL AND loan_id IS NULL \
         ORDER BY date, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("backfill_scenario_loans: {e}"))?;

    let mut order: Vec<(String, String)> = Vec::new();
    let mut groups: HashMap<(String, String), Vec<(&LegacyLoanRow, i64)>> = HashMap::new();
    for row in &rows {
        let Some((group_id, rate_bps)) = parse_loan_marker(&row.description) else {
            continue;
        };
        let key = (row.scenario_id.clone(), group_id);
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push((row, rate_bps));
    }

    for key in order {
        let group = &groups[&key];
        let derived = derive_legacy_loan(group);
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("backfill_scenario_loans (begin): {e}"))?;
        let loan_id = match derived {
            Some(loan) => {
                let id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO scenario_loan \
                       (id, scenario_id, principal_cents, rate_bps, term_months, \
                        disbursement_date, first_installment_date, description) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .bind(&id)
                .bind(&key.0)
                .bind(loan.principal_cents)
                .bind(loan.rate_bps)
                .bind(loan.term_months)
                .bind(loan.disbursement_date.format("%Y-%m-%d").to_string())
                .bind(loan.first_installment_date.format("%Y-%m-%d").to_string())
                .bind(&loan.description)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("backfill_scenario_loans (insert): {e}"))?;
                Some(id)
            }
            None => None,
        };
        for (row, _) in group {
            sqlx::query("UPDATE \"transaction\" SET description = ?2, loan_id = ?3 WHERE id = ?1")
                .bind(&row.id)
                .bind(strip_loan_marker(&row.description))
                .bind(&loan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("backfill_scenario_loans (update): {e}"))?;
        }
        tx.commit()
            .await
            .map_err(|e| format!("backfill_scenario_loans (commit): {e}"))?;
    }
    Ok(())
}

/// Extrai o `override_id` da marca LEGADA `" #repl:<override_id>"` ao FINAL da descrição (só
/// `backfill_scenario_override_replacements` ainda a encontra — nenhum caminho novo a escreve;
/// o compare pareia por FK). Mesma âncora do `#loan:`.
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

fn strip_repl_marker(description: &str) -> String {
    let trimmed = description.trim_end();
    match trimmed.rfind("#repl:") {
        Some(idx) => trimmed[..idx].trim_end().to_string(),
        None => trimmed.to_string(),
    }
}

/// Converte as linhas de substituição LEGADAS (marcadas com `" #repl:<override_id>"` na descrição)
/// para a identidade por FK: seta `transaction.override_id` e remove o sufixo. Roda no startup,
/// logo após `backfill_scenario_loans`, e é IDEMPOTENTE — processa apenas linhas de cenário cuja
/// descrição ainda carrega o marcador (`override_id IS NULL`). Marcador com override morto (órfão)
/// só perde o sufixo: a linha vira uma adição comum, como já degradava antes da FK. Não expande a
/// linha única legada em série (preserva o dado existente, como o backfill do `#loan`); só novos
/// overrides geram série.
pub async fn backfill_scenario_override_replacements(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, COALESCE(description,'') AS description FROM \"transaction\" \
         WHERE scenario_id IS NOT NULL AND override_id IS NULL AND description LIKE '%#repl:%' \
         ORDER BY date, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("backfill_scenario_override_replacements: {e}"))?;

    for (id, description) in rows {
        let Some(override_id) = parse_repl_marker(&description) else {
            continue; // "#repl:" no meio do texto, não a marca da convenção — não toca.
        };
        let (override_exists,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE id = ?1")
                .bind(&override_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("backfill_scenario_override_replacements (lookup): {e}"))?;
        let fk = (override_exists > 0).then_some(override_id);
        sqlx::query("UPDATE \"transaction\" SET description = ?2, override_id = ?3 WHERE id = ?1")
            .bind(&id)
            .bind(strip_repl_marker(&description))
            .bind(&fk)
            .execute(pool)
            .await
            .map_err(|e| format!("backfill_scenario_override_replacements (update): {e}"))?;
    }
    Ok(())
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
    /// Renda do mês corrente (Entradas): a UI classifica Custo de vida
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
    loan_id: Option<String>,
    override_id: Option<String>,
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
/// `recurrence_id`/`loan_id` a mais (o cenário nunca sofre override; `loan_id` identifica o grupo
/// de empréstimo).
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
         t.recurrence_id, t.description, t.loan_id, t.override_id \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?2 AND t.date <= ?3 AND t.scenario_id = ?1";
    const EXCLUSIVE: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, \
         t.recurrence_id, t.description, t.loan_id, t.override_id \
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
/// da janela). `ORDER BY date, id` torna determinística a escolha do "primeiro" grupo de
/// empréstimo reportado.
async fn load_all_hypothetical_rows(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<Vec<HypoTxnRow>, String> {
    const SQL: &str = "SELECT t.id, t.type AS \"type\", t.amount, t.date, \
         COALESCE(t.payment_method,'') AS payment_method, \
         t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, \
         t.recurrence_id, t.description, t.loan_id, t.override_id \
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
    /// Para `changes` (suppress): soma suprimida por override (todas as ocorrências ≥ from_date).
    per_override_suppressed_cents: HashMap<String, i64>,
    /// Para `changes` (replace): magnitude suprimida da ocorrência REPRESENTATIVA (a mais antiga
    /// ≥ from_date) — o `old` mensal do par velho→novo, na mesma unidade que o `new` por
    /// ocorrência. Evita comparar total-do-horizonte (suppress) com valor mensal (replace).
    per_override_occurrence_cents: HashMap<String, i64>,
}

async fn build_suppression_plan(
    pool: &SqlitePool,
    overrides: &[ScenarioOverride],
    today: NaiveDate,
) -> Result<SuppressionPlan, String> {
    let mut plan = SuppressionPlan::default();
    // A ocorrência representativa (o `old` mensal do replace) só olha células que a projeção
    // realmente toca (`>= mês corrente`): uma célula histórica nunca é suprimida de fato
    // (`load_real_rows` começa em `month_start`), então não deve virar o "old" exibido.
    let month_start =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("data de hoje inválida")?;
    // Defesa em profundidade contra alvos duplicados (que `set_scenario_override` já rejeita,
    // mas linhas pré-existentes/inseridas por fora não passam pelo guard): cada `line_item` só
    // contribui UMA vez para a supressão, mesmo que dois overrides o casem — somar duas vezes
    // zeraria a célula e derrubaria os irmãos não suprimidos.
    let mut seen_line_items: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ov in overrides {
        let from_date = NaiveDate::parse_from_str(&ov.from_date, "%Y-%m-%d")
            .map_err(|e| format!("override {} from_date inválida: {e}", ov.id))?;
        let floor = from_date.max(month_start);
        if let Some(obligation_id) = &ov.obligation_id {
            let items = obligations::obligation_items(pool, obligation_id).await?;
            let mut suppressed_here = 0i64;
            // Magnitude por célula (só as ≥ floor) p/ achar a ocorrência representativa.
            let mut cell_by_txn: HashMap<String, (String, i64)> = HashMap::new();
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
                    if d >= floor {
                        cell_by_txn
                            .entry(item.transaction_id.clone())
                            .or_insert((item.date.clone(), 0))
                            .1 += magnitude;
                    }
                }
            }
            plan.per_override_suppressed_cents
                .insert(ov.id.clone(), suppressed_here);
            // Determinístico: menor `(data, transaction_id)`. `HashMap::values().min_by(data)`
            // deixaria o desempate de data à ordem (aleatória) de iteração do HashMap — um valor
            // financeiro exibido não pode depender do seed de hash do processo.
            if let Some((_, _, magnitude)) = cell_by_txn
                .iter()
                .map(|(txn_id, (date, mag))| (date.clone(), txn_id.clone(), *mag))
                .min()
            {
                plan.per_override_occurrence_cents
                    .insert(ov.id.clone(), magnitude);
            }
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
            // Ocorrência representativa (linha real mais antiga ≥ floor) — o `old` mensal.
            let occurrence: Option<(i64,)> = sqlx::query_as(
                "SELECT ABS(amount) FROM \"transaction\" \
                 WHERE recurrence_id = ?1 AND date >= ?2 AND scenario_id IS NULL \
                 ORDER BY date, id LIMIT 1",
            )
            .bind(recurrence_id)
            .bind(floor.format("%Y-%m-%d").to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("recurrence first occurrence: {e}"))?;
            if let Some((magnitude,)) = occurrence {
                plan.per_override_occurrence_cents
                    .insert(ov.id.clone(), magnitude);
            }
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

/// Detecta um grupo de empréstimo entre TODAS as linhas hipotéticas do cenário (linhas com
/// `loan_id`) e monta o `LoanBreakdown`. Só o PRIMEIRO grupo na ordem (data, id) é reportado —
/// devolve também o `loan_id` para o builder de `changes` excluir SÓ as linhas desse grupo (as
/// de um segundo financiamento aparecem como "add" comuns, nunca somem do DTO). A taxa vem da
/// entidade (`rate_by_loan`); principal/parcela/prazo derivam das LINHAS PRESENTES — apagar
/// parcelas finais (quitação antecipada simulada) reduz o total pago reportado. `None` se
/// nenhuma linha pertence a um empréstimo.
fn detect_loan(
    hypo_rows: &[HypoTxnRow],
    rate_by_loan: &HashMap<String, i64>,
    reserve_balance_cents: i64,
    baseline_cents: i64,
    income_median_cents: i64,
    economia_median_cents: i64,
) -> Option<(String, LoanBreakdown)> {
    let mut group_id: Option<String> = None;
    let mut principal_cents = 0i64;
    let mut installment_cents = 0i64;
    let mut term_months = 0u32;

    for row in hypo_rows {
        let Some(lid) = &row.loan_id else {
            continue;
        };
        if let Some(existing) = &group_id {
            if existing != lid {
                continue; // só o primeiro grupo (ordem data,id) é reportado.
            }
        } else {
            group_id = Some(lid.clone());
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
    let rate_bps = rate_by_loan.get(&group_id).copied().unwrap_or(0);
    if term_months == 0 {
        return None;
    }

    let total_paid_cents = installment_cents * term_months as i64;
    let total_cost_cents = total_paid_cents - principal_cents;
    // Régua de reserva do método (a mesma do dashboard): reserva ÷ custo de vida típico, e no
    // "depois" a parcela soma ao denominador como o único compromisso novo. `baseline == 0`
    // (nenhum mês completo) oculta a régua (`None`); reserva zerada é `Some(0.0)` — reserva
    // vazia é informação, o gate reprova.
    let reserve_months_before_financing =
        (baseline_cents > 0).then(|| reserve_balance_cents as f64 / baseline_cents as f64);
    let reserve_months_after_financing = (baseline_cents > 0)
        .then(|| reserve_balance_cents as f64 / (baseline_cents + installment_cents) as f64);
    // Segunda perna do gate (percentual poupado, mesma convenção bps do motor mensal:
    // valor * 10_000 / renda). O "depois" desconta SÓ a parcela e fica BRUTO — negativo quando
    // ela excede a economia típica; o frontend julga a escada no bruto e clampa só a exibição.
    let savings_rate_before_bps =
        (income_median_cents > 0).then(|| economia_median_cents * 10_000 / income_median_cents);
    let savings_rate_after_bps = (income_median_cents > 0)
        .then(|| (economia_median_cents - installment_cents) * 10_000 / income_median_cents);

    Some((
        group_id,
        LoanBreakdown {
            loan_principal_cents: principal_cents,
            loan_installment_cents: installment_cents,
            loan_term_months: term_months,
            loan_monthly_rate_bps: rate_bps,
            loan_total_paid_cents: total_paid_cents,
            loan_total_cost_cents: total_cost_cents,
            reserve_months_before_financing,
            reserve_months_after_financing,
            savings_rate_before_bps,
            savings_rate_after_bps,
            economia_median_cents,
        },
    ))
}

/// Custo de vida "do momento": o mês corrente do `Forecast` (mesma definição canônica do motor —
/// fixas + diário realizado + cartão), ou 0 se o mês corrente não aparece nos meses do
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

/// Renda do mês corrente (`MonthMetric.income_cents`): exposta para os
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
    let plan = build_suppression_plan(pool, &overrides, today).await?;

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

    let scenario_end_exclusive = horizon_end
        .succ_opt()
        .ok_or("horizonte inválido para faturas do cenário")?;
    let mut scenario_chain_events = map_raw_rows(scenario_chain_adjusted);
    scenario_chain_events.extend(map_hypo_rows(&hypo_chain_rows));
    scenario_chain_events = finalize_card_events(
        pool,
        today,
        today,
        scenario_end_exclusive,
        scenario_chain_events,
    )
    .await?;
    let mut scenario_metric_events = map_raw_rows(scenario_metric_adjusted);
    scenario_metric_events.extend(map_hypo_rows(&hypo_metric_rows));
    scenario_metric_events = finalize_card_events(
        pool,
        today,
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .ok_or("data de hoje inválida para faturas do cenário")?,
        scenario_end_exclusive,
        scenario_metric_events,
    )
    .await?;

    // Previsão de diário reutiliza o MESMO teto/dia do ramo real — o orçamento de Diário não muda
    // por cenário; só o encadeamento de caixa/hipotéticas mudam.
    let daily_ceiling = forecast_cmds::projection_daily_ceiling(pool, today).await?;
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
    // Insumos da régua de reserva — os MESMOS do `reserve_months` do dashboard (numerador =
    // contas de reserva; denominador = mediana dos meses completos), para "antes" coincidir com
    // o dashboard. O compare é somente-leitura: queries direto no pool, sem transação aberta.
    let reserve_balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query reserve balance: {e}"))?;
    let baseline_cents = forecast_cmds::realized_monthly_baseline(pool, today).await?;
    // Segunda perna do gate: medianas de renda e economia do MESMO "mês típico" da régua de
    // reserva (mesma janela de 6 meses completos, mesmo estimador).
    let (income_median_cents, economia_median_cents) =
        forecast_cmds::realized_savings_baseline(pool, today).await?;
    let rate_by_loan: HashMap<String, i64> = list_scenario_loans(pool, scenario_id)
        .await?
        .into_iter()
        .map(|l| (l.id, l.rate_bps))
        .collect();
    let (loan_group_id, loan) = match detect_loan(
        &all_hypo_rows,
        &rate_by_loan,
        reserve_balance.0,
        baseline_cents,
        income_median_cents,
        economia_median_cents,
    ) {
        Some((gid, breakdown)) => (Some(gid), Some(breakdown)),
        None => (None, None),
    };

    // Série de substituição pareada por `override_id` (FK — ver banner do módulo): funde velho→novo
    // numa única entrada `replace` de `changes`. Todas as N linhas da série têm o MESMO valor (o
    // novo valor por ocorrência), então o `new` mensal é o de qualquer linha — o primeiro. Uma FK
    // órfã não existe (o CASCADE apaga a série com o override), mas checamos o override existir por
    // simetria com o compare antigo.
    let mut replacement_by_override: HashMap<String, i64> = HashMap::new();
    for row in &all_hypo_rows {
        if let Some(ov_id) = &row.override_id
            && overrides.iter().any(|o| &o.id == ov_id)
        {
            replacement_by_override
                .entry(ov_id.clone())
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
            // `replace` reporta o par POR OCORRÊNCIA (mensal): o `old` é a ocorrência suprimida
            // representativa, na mesma unidade que o `new`. `suppress` reporta o total suprimido.
            old_amount_cents: if ov.op == "replace" {
                plan.per_override_occurrence_cents.get(&ov.id).copied()
            } else {
                plan.per_override_suppressed_cents.get(&ov.id).copied()
            },
            // Fundido via `override_id`; `None` se o replace foi criado sem série (a UI então
            // mostra só o "removido").
            new_amount_cents: replacement_by_override.get(&ov.id).copied(),
        });
    }
    for row in &all_hypo_rows {
        // Linha do grupo de empréstimo REPORTADO via `loan` → não duplica. Um SEGUNDO
        // empréstimo (não coberto pelo `loan` desta slice) entra como "add" normal.
        if row.loan_id.is_some() && row.loan_id == loan_group_id {
            continue;
        }
        // Linha da série de substituição pareada → já fundida na entrada `replace` acima.
        if row
            .override_id
            .as_ref()
            .is_some_and(|ov_id| overrides.iter().any(|o| &o.id == ov_id))
        {
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
pub async fn list_recurrence_targets_cmd(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<RecurrenceTarget>, String> {
    list_recurrence_targets(pool.inner()).await
}

#[tauri::command]
pub async fn recurrence_occurrences_cmd(
    pool: State<'_, SqlitePool>,
    recurrence_id: String,
) -> Result<Vec<RecurrenceOccurrence>, String> {
    recurrence_occurrences(pool.inner(), &recurrence_id).await
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

    /// Semeia um empréstimo direto no banco (entidade + linhas com `loan_id`), sem passar pela
    /// validação de datas do caminho de criação — os testes de compare usam datas fixas para
    /// serem determinísticos sob qualquer relógio.
    #[allow(clippy::too_many_arguments)]
    async fn seed_loan(
        p: &SqlitePool,
        scenario_id: &str,
        loan_id: &str,
        rate_bps: i64,
        principal_cents: i64,
        term_months: u32,
        disbursement_date: &str,
        rows: &[(&str, i64, &str, &str)],
    ) {
        sqlx::query(
            "INSERT INTO scenario_loan (id, scenario_id, principal_cents, rate_bps, \
             term_months, disbursement_date, first_installment_date, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 'Empréstimo')",
        )
        .bind(loan_id)
        .bind(scenario_id)
        .bind(principal_cents)
        .bind(rate_bps)
        .bind(term_months)
        .bind(disbursement_date)
        .execute(p)
        .await
        .unwrap();
        for (index, (ttype, amount, desc, date)) in rows.iter().enumerate() {
            sqlx::query(
                "INSERT INTO \"transaction\" \
                 (id, type, amount, description, date, is_projection, scenario_id, loan_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
            )
            .bind(format!("{loan_id}-row-{index}"))
            .bind(ttype)
            .bind(amount)
            .bind(desc)
            .bind(date)
            .bind(scenario_id)
            .bind(loan_id)
            .execute(p)
            .await
            .unwrap();
        }
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

    #[tokio::test]
    async fn invalid_replacement_line_rolls_back_the_whole_override() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Uma ocorrência (datas 2099 p/ independer do relógio: a série filtra `>= mês corrente`).
        txn(&p, "aluguel-fut", "expense", 150_000, "2099-08-05").await;
        line_item(&p, "li-fut", "aluguel-fut", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        // `transfer` sem conta-destino faz `insert_scenario_transaction` falhar no meio da série.
        let result = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 100_000,
                description: Some("Aluguel novo".into()),
                txn_type: Some("transfer".into()),
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (overrides,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            overrides, 0,
            "linha de substituição inválida não deixa override órfão"
        );
        let (lines,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(lines, 0, "rollback não deixa nem série parcial");
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

    #[test]
    fn plan_scenario_loan_generates_principal_and_labeled_installments() {
        let loan = ScenarioLoan::try_from(ScenarioLoanInput {
            scenario_id: "scenario-1".into(),
            principal_cents: 100_000,
            term_months: 3,
            rate_bps: 200,
            disbursement_date: "2030-01-15".into(),
            first_installment_date: "2030-02-15".into(),
            description: "Empréstimo".into(),
        })
        .unwrap();

        let lines = plan_scenario_loan(&loan).unwrap();

        assert_eq!(
            lines,
            vec![
                ScenarioLoanLine {
                    kind: ScenarioLoanLineKind::Principal,
                    amount: MoneyCents(100_000),
                    description: "Empréstimo".into(),
                    date: d("2030-01-15"),
                },
                ScenarioLoanLine {
                    kind: ScenarioLoanLineKind::Installment,
                    amount: MoneyCents(34_675),
                    description: "Empréstimo parcela 1/3".into(),
                    date: d("2030-02-15"),
                },
                ScenarioLoanLine {
                    kind: ScenarioLoanLineKind::Installment,
                    amount: MoneyCents(34_675),
                    description: "Empréstimo parcela 2/3".into(),
                    date: d("2030-03-15"),
                },
                ScenarioLoanLine {
                    kind: ScenarioLoanLineKind::Installment,
                    amount: MoneyCents(34_675),
                    description: "Empréstimo parcela 3/3".into(),
                    date: d("2030-04-15"),
                },
            ]
        );
    }

    #[test]
    fn plan_scenario_loan_rejects_an_installment_date_overflow() {
        let loan = ScenarioLoan {
            scenario_id: ScenarioId("scenario-1".into()),
            principal: MoneyCents(100_000),
            term: TermMonths(3),
            rate: RateBps(200),
            disbursement_date: d("2030-01-15"),
            first_installment_date: NaiveDate::MAX.checked_sub_months(Months::new(1)).unwrap(),
            description: "Empréstimo".into(),
        };

        let result = plan_scenario_loan(&loan);

        assert_eq!(
            result.unwrap_err(),
            "data de parcela fora do intervalo suportado"
        );
    }

    fn scenario_loan_input(scenario_id: &str, first_installment_date: String) -> ScenarioLoanInput {
        ScenarioLoanInput {
            scenario_id: scenario_id.into(),
            principal_cents: 100_000,
            term_months: 3,
            rate_bps: 200,
            disbursement_date: chrono::Local::now()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string(),
            first_installment_date,
            description: "Empréstimo".into(),
        }
    }

    #[tokio::test]
    async fn create_scenario_loan_persists_the_complete_plan() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let first_date = chrono::Local::now()
            .date_naive()
            .checked_add_months(Months::new(1))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        create_scenario_loan(&p, scenario_loan_input(&sc.id, first_date))
            .await
            .unwrap();

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(count, 4);
    }

    #[tokio::test]
    async fn create_scenario_loan_rejects_invalid_payloads_before_writing() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let mut invalid_inputs = Vec::new();

        let mut invalid_principal = scenario_loan_input(&sc.id, "2030-02-15".into());
        invalid_principal.principal_cents = 0;
        invalid_inputs.push(invalid_principal);

        let mut invalid_term = scenario_loan_input(&sc.id, "2030-02-15".into());
        invalid_term.term_months = 481;
        invalid_inputs.push(invalid_term);

        let mut invalid_rate = scenario_loan_input(&sc.id, "2030-02-15".into());
        invalid_rate.rate_bps = -1;
        invalid_inputs.push(invalid_rate);

        invalid_inputs.push(scenario_loan_input(&sc.id, "data-inválida".into()));

        for input in invalid_inputs {
            assert!(create_scenario_loan(&p, input).await.is_err());
        }
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn create_scenario_loan_rolls_back_every_row_when_a_later_insert_fails() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let first_date = chrono::Local::now()
            .date_naive()
            .checked_add_months(Months::new(1))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        sqlx::query(
            "CREATE TRIGGER fail_second_installment BEFORE INSERT ON \"transaction\" \
             WHEN NEW.description LIKE '%parcela 2/3%' \
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END",
        )
        .execute(&p)
        .await
        .unwrap();

        let result = create_scenario_loan(&p, scenario_loan_input(&sc.id, first_date)).await;

        assert!(result.is_err());
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            count, 0,
            "uma falha intermediária desfaz principal e parcelas"
        );
        let (loan_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scenario_loan")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(loan_count, 0, "a entidade também é desfeita no rollback");
    }

    // --- Ciclo de vida da entidade scenario_loan ---

    fn future_date(months_ahead: u32) -> String {
        chrono::Local::now()
            .date_naive()
            .checked_add_months(Months::new(months_ahead))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string()
    }

    #[tokio::test]
    async fn create_scenario_loan_persists_entity_and_linked_rows() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();

        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc.id, future_date(1)))
            .await
            .unwrap();

        let loans = list_scenario_loans(&p, &sc.id).await.unwrap();
        assert_eq!(loans.len(), 1);
        let loan = &loans[0];
        assert_eq!(loan.id, loan_id);
        assert_eq!(loan.principal_cents, 100_000);
        assert_eq!(loan.rate_bps, 200);
        assert_eq!(loan.term_months, 3);
        assert_eq!(loan.description, "Empréstimo");

        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(rows.len(), 4);
        assert!(
            rows.iter().all(|r| r.loan_id.as_deref() == Some(&*loan_id)),
            "todas as linhas apontam para a entidade"
        );
        assert!(
            rows.iter().all(|r| !r.description.contains("#loan:")),
            "nenhuma descrição carrega o marcador legado"
        );
    }

    #[tokio::test]
    async fn update_scenario_loan_regenerates_the_series_under_the_same_identity() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc.id, future_date(1)))
            .await
            .unwrap();
        // Ajuste fino à mão: remove uma parcela antes de editar.
        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        let removed = rows.iter().find(|r| r.r#type == "expense").unwrap();
        delete_scenario_transaction(&p, &sc.id, &removed.id)
            .await
            .unwrap();

        let mut input = scenario_loan_input(&sc.id, future_date(2));
        input.principal_cents = 200_000;
        input.term_months = 2;
        update_scenario_loan(&p, &loan_id, input).await.unwrap();

        let loans = list_scenario_loans(&p, &sc.id).await.unwrap();
        assert_eq!(loans.len(), 1, "mesma identidade, nenhum grupo novo");
        assert_eq!(loans[0].id, loan_id);
        assert_eq!(loans[0].principal_cents, 200_000);
        assert_eq!(loans[0].term_months, 2);

        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(
            rows.len(),
            3,
            "série regenerada por completo (principal + 2 parcelas) — o ajuste fino não sobrevive"
        );
        assert!(rows.iter().all(|r| r.loan_id.as_deref() == Some(&*loan_id)));
        assert!(
            rows.iter()
                .any(|r| r.r#type == "income" && r.amount == 200_000),
            "o principal novo entrou"
        );
    }

    #[tokio::test]
    async fn update_scenario_loan_rejects_a_loan_from_another_scenario() {
        let p = pool().await;
        let sc_a = create_scenario(&p, "A").await.unwrap();
        let sc_b = create_scenario(&p, "B").await.unwrap();
        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc_a.id, future_date(1)))
            .await
            .unwrap();

        let result =
            update_scenario_loan(&p, &loan_id, scenario_loan_input(&sc_b.id, future_date(1))).await;

        assert!(
            result.is_err(),
            "empréstimo de outro cenário não é editável"
        );
    }

    #[tokio::test]
    async fn update_scenario_loan_rolls_back_entity_and_rows_on_failure() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc.id, future_date(1)))
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_edit BEFORE INSERT ON \"transaction\" \
             WHEN NEW.description LIKE '%parcela 2/2%' \
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END",
        )
        .execute(&p)
        .await
        .unwrap();

        let mut input = scenario_loan_input(&sc.id, future_date(2));
        input.term_months = 2;
        let result = update_scenario_loan(&p, &loan_id, input).await;

        assert!(result.is_err());
        let loans = list_scenario_loans(&p, &sc.id).await.unwrap();
        assert_eq!(
            loans[0].term_months, 3,
            "os parâmetros antigos sobrevivem ao rollback"
        );
        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(
            rows.len(),
            4,
            "a série antiga sobrevive intacta — nenhum estado intermediário"
        );
    }

    #[tokio::test]
    async fn delete_scenario_loan_removes_entity_and_every_row() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc.id, future_date(1)))
            .await
            .unwrap();

        delete_scenario_loan(&p, &sc.id, &loan_id).await.unwrap();

        assert!(list_scenario_loans(&p, &sc.id).await.unwrap().is_empty());
        assert!(
            list_scenario_transactions(&p, &sc.id)
                .await
                .unwrap()
                .is_empty(),
            "o CASCADE leva principal + parcelas junto"
        );
    }

    #[tokio::test]
    async fn delete_scenario_loan_rejects_another_scenarios_loan() {
        let p = pool().await;
        let sc_a = create_scenario(&p, "A").await.unwrap();
        let sc_b = create_scenario(&p, "B").await.unwrap();
        let loan_id = create_scenario_loan(&p, scenario_loan_input(&sc_a.id, future_date(1)))
            .await
            .unwrap();

        assert!(delete_scenario_loan(&p, &sc_b.id, &loan_id).await.is_err());
        assert_eq!(list_scenario_loans(&p, &sc_a.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deleting_the_last_loan_row_removes_the_loan_record() {
        let p = pool().await;
        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        let mut input = scenario_loan_input(&sc.id, future_date(1));
        input.term_months = 1;
        let _loan_id = create_scenario_loan(&p, input).await.unwrap();

        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(rows.len(), 2);
        delete_scenario_transaction(&p, &sc.id, &rows[0].id)
            .await
            .unwrap();
        assert_eq!(
            list_scenario_loans(&p, &sc.id).await.unwrap().len(),
            1,
            "com uma linha restante o empréstimo ainda existe"
        );

        delete_scenario_transaction(&p, &sc.id, &rows[1].id)
            .await
            .unwrap();
        assert!(
            list_scenario_loans(&p, &sc.id).await.unwrap().is_empty(),
            "apagar a última linha apaga o registro — sem fantasma"
        );
    }

    // --- Backfill do legado `#loan:` ---

    /// Linha legada crua: marcador na descrição, `loan_id` NULL.
    async fn legacy_row(
        p: &SqlitePool,
        id: &str,
        scenario_id: &str,
        ttype: &str,
        amount: i64,
        description: &str,
        date: &str,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, description, date, is_projection, scenario_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        )
        .bind(id)
        .bind(ttype)
        .bind(amount)
        .bind(description)
        .bind(date)
        .bind(scenario_id)
        .execute(p)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn backfill_converts_a_clean_legacy_group_into_an_entity() {
        let p = pool().await;
        let sc = create_scenario(&p, "Legado").await.unwrap();
        legacy_row(
            &p,
            "l-0",
            &sc.id,
            "income",
            100_000,
            "Empréstimo #loan:g1:200",
            "2026-08-05",
        )
        .await;
        for (id, i, date) in [
            ("l-1", 1, "2026-09-05"),
            ("l-2", 2, "2026-10-05"),
            ("l-3", 3, "2026-11-05"),
        ] {
            legacy_row(
                &p,
                id,
                &sc.id,
                "expense",
                34_675,
                &format!("Empréstimo parcela {i}/3 #loan:g1:200"),
                date,
            )
            .await;
        }

        backfill_scenario_loans(&p).await.unwrap();

        let loans = list_scenario_loans(&p, &sc.id).await.unwrap();
        assert_eq!(loans.len(), 1);
        let loan = &loans[0];
        assert_eq!(loan.principal_cents, 100_000);
        assert_eq!(loan.rate_bps, 200);
        assert_eq!(loan.term_months, 3);
        assert_eq!(loan.disbursement_date, "2026-08-05");
        assert_eq!(loan.first_installment_date, "2026-09-05");
        assert_eq!(loan.description, "Empréstimo");

        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert!(
            rows.iter().all(|r| r.loan_id.as_deref() == Some(&*loan.id)),
            "todas as linhas do grupo apontam para a entidade"
        );
        assert!(
            rows.iter().all(|r| !r.description.contains("#loan:")),
            "os sufixos foram removidos"
        );

        // Idempotência: re-rodar não cria nada novo.
        backfill_scenario_loans(&p).await.unwrap();
        assert_eq!(list_scenario_loans(&p, &sc.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn backfill_recovers_first_installment_date_from_a_partial_group() {
        let p = pool().await;
        let sc = create_scenario(&p, "Legado").await.unwrap();
        legacy_row(
            &p,
            "l-0",
            &sc.id,
            "income",
            100_000,
            "Empréstimo #loan:g1:200",
            "2026-08-05",
        )
        .await;
        // Parcela 1 removida à mão no legado: a 1ª data deriva da parcela 2, recuada 1 mês.
        for (id, i, date) in [("l-2", 2, "2026-10-05"), ("l-3", 3, "2026-11-05")] {
            legacy_row(
                &p,
                id,
                &sc.id,
                "expense",
                34_675,
                &format!("Empréstimo parcela {i}/3 #loan:g1:200"),
                date,
            )
            .await;
        }

        backfill_scenario_loans(&p).await.unwrap();

        let loans = list_scenario_loans(&p, &sc.id).await.unwrap();
        assert_eq!(loans.len(), 1);
        assert_eq!(loans[0].term_months, 3, "prazo vem do rótulo i/N");
        assert_eq!(
            loans[0].first_installment_date, "2026-09-05",
            "data da 1ª parcela recuada i−1 meses a partir da parcela de menor i"
        );
        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(rows.len(), 3, "as linhas removidas à mão não renascem");
    }

    #[tokio::test]
    async fn backfill_turns_an_underivable_group_into_loose_rows() {
        let p = pool().await;
        let sc = create_scenario(&p, "Legado").await.unwrap();
        // Sem linha de principal (income): não há derivação limpa.
        legacy_row(
            &p,
            "l-1",
            &sc.id,
            "expense",
            34_675,
            "Empréstimo parcela 1/3 #loan:g1:200",
            "2026-09-05",
        )
        .await;
        // Fora do grupo: linha solta sem marcador fica intacta.
        legacy_row(&p, "l-x", &sc.id, "expense", 5_000, "Cinema", "2026-09-06").await;

        backfill_scenario_loans(&p).await.unwrap();

        assert!(
            list_scenario_loans(&p, &sc.id).await.unwrap().is_empty(),
            "grupo sem derivação limpa não vira entidade"
        );
        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        assert_eq!(rows.len(), 2, "nada é apagado");
        let orphan = rows.iter().find(|r| r.id == "l-1").unwrap();
        assert_eq!(
            orphan.description, "Empréstimo parcela 1/3",
            "o sufixo sai mesmo sem entidade — vira linha solta comum"
        );
        assert!(orphan.loan_id.is_none());
        let untouched = rows.iter().find(|r| r.id == "l-x").unwrap();
        assert_eq!(untouched.description, "Cinema");
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

    // A UI classifica Custo de vida ("Dentro da renda"/"Acima da renda")
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
        seed_loan(
            &p,
            &sc.id,
            "loan-carro",
            150,
            1_000_000,
            1,
            "2026-08-05",
            &[
                ("income", 1_000_000, "Empréstimo desembolso", "2026-08-05"),
                ("expense", 90_000, "Empréstimo parcela 1/1", "2026-08-10"),
            ],
        )
        .await;

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

    // --- Invariantes de detecção e comparação ---

    // Detecção robusta: a detecção do empréstimo usa TODAS as linhas hipotéticas (sem janela de
    // data), então o principal desembolsado no próprio `today` nunca some do grupo — o custo total
    // não pode ser superestimado pelo principal inteiro, em silêncio.
    #[tokio::test]
    async fn loan_detects_principal_disbursed_today() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-02").await;

        let sc = create_scenario(&p, "Financiamento").await.unwrap();
        seed_loan(
            &p,
            &sc.id,
            "loan-moto",
            100,
            1_000_000,
            1,
            "2026-08-01", // desembolso == today injetado abaixo
            &[
                ("income", 1_000_000, "Desembolso", "2026-08-01"),
                ("expense", 90_000, "Desembolso parcela 1/1", "2026-08-10"),
            ],
        )
        .await;

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

    // Um evento HIPOTÉTICO datado do próprio `today` (ex.: o principal do empréstimo,
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

    // A mesma fronteira vale para saída: uma despesa hipotética de hoje
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

    // Um hipotético de AMANHÃ precisa contribuir exatamente uma vez para o encadeamento. Métricas e
    // encadeamento são pipelines separados; o evento futuro não pode somar em dobro.
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

    // MAJOR 2: um SEGUNDO empréstimo não vira `loan` (só o primeiro, ordem data,id), mas as
    // linhas dele PRECISAM aparecer em `changes` como "add" — nunca somem do DTO.
    #[tokio::test]
    async fn second_loan_group_rows_surface_in_changes() {
        let p = pool().await;
        txn(&p, "inc-1", "income", 500_000, "2026-08-01").await;

        let sc = create_scenario(&p, "Dois financiamentos").await.unwrap();
        // Grupo A (primeiro por data): reportado via `loan`.
        seed_loan(
            &p,
            &sc.id,
            "loan-carro",
            150,
            1_000_000,
            1,
            "2026-08-05",
            &[
                ("income", 1_000_000, "Desembolso carro", "2026-08-05"),
                ("expense", 90_000, "Carro parcela 1/1", "2026-08-10"),
            ],
        )
        .await;
        // Grupo B (depois): deve aparecer em `changes`.
        seed_loan(
            &p,
            &sc.id,
            "loan-moto",
            200,
            500_000,
            1,
            "2026-09-05",
            &[
                ("income", 500_000, "Desembolso moto", "2026-09-05"),
                ("expense", 55_000, "Moto parcela 1/1", "2026-09-10"),
            ],
        )
        .await;

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
            .filter(|c| c.description.contains("moto") || c.description.contains("Moto"))
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
                .any(|c| c.description.contains("carro") || c.description.contains("Carro")),
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

    #[tokio::test]
    async fn schema_rejects_duplicate_override_target() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let obligation_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO scenario_override \
               (id, scenario_id, op, from_date, obligation_id) \
             VALUES ('first', ?1, 'suppress', '2030-01-01', ?2)",
        )
        .bind(&sc.id)
        .bind(&obligation_id)
        .execute(&p)
        .await
        .unwrap();

        let error = sqlx::query(
            "INSERT INTO scenario_override \
               (id, scenario_id, op, from_date, obligation_id) \
             VALUES ('second', ?1, 'suppress', '2030-02-01', ?2)",
        )
        .bind(&sc.id)
        .bind(&obligation_id)
        .execute(&p)
        .await
        .expect_err("o schema deve rejeitar um segundo override para o mesmo alvo");

        assert!(error.as_database_error().unwrap().is_unique_violation());
    }

    #[tokio::test]
    async fn concurrent_overrides_for_the_same_target_leave_exactly_one_row() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let obligation_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let first = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-08-01",
            Some(&obligation_id),
            None,
            None,
        );
        let second = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2026-09-01",
            Some(&obligation_id),
            None,
            None,
        );
        let results = tokio::join!(first, second);

        let successes = [results.0.as_ref(), results.1.as_ref()]
            .into_iter()
            .filter(|result| result.is_ok())
            .count();
        let errors: Vec<&str> = [results.0.as_ref(), results.1.as_ref()]
            .into_iter()
            .filter_map(|result| result.err().map(String::as_str))
            .collect();
        assert_eq!(successes, 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("já existe uma alteração para esta obrigação"));

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM scenario_override \
             WHERE scenario_id = ?1 AND obligation_id = ?2",
        )
        .bind(&sc.id)
        .bind(&obligation_id)
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(count, 1);
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

    #[tokio::test]
    async fn unique_target_migration_deduplicates_existing_overrides() {
        let p = pool().await;
        sqlx::query("DROP INDEX ux_scenario_override_scenario_obligation")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query("DROP INDEX ux_scenario_override_scenario_recurrence")
            .execute(&p)
            .await
            .unwrap();

        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let obligation_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
             VALUES ('rec-1', 'mensal', 0, 2, '2026-08-05')",
        )
        .execute(&p)
        .await
        .unwrap();

        for id in ["ob-oldest", "ob-newest"] {
            sqlx::query(
                "INSERT INTO scenario_override \
                   (id, scenario_id, op, from_date, obligation_id) \
                 VALUES (?1, ?2, 'suppress', '2026-08-01', ?3)",
            )
            .bind(id)
            .bind(&sc.id)
            .bind(&obligation_id)
            .execute(&p)
            .await
            .unwrap();
        }
        for id in ["rec-oldest", "rec-newest"] {
            sqlx::query(
                "INSERT INTO scenario_override \
                   (id, scenario_id, op, from_date, recurrence_id) \
                 VALUES (?1, ?2, 'suppress', '2026-08-01', 'rec-1')",
            )
            .bind(id)
            .bind(&sc.id)
            .execute(&p)
            .await
            .unwrap();
        }

        sqlx::raw_sql(include_str!(
            "../migrations/20260712000001_scenario_override_unique_targets.sql"
        ))
        .execute(&p)
        .await
        .unwrap();

        let ids: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM scenario_override WHERE scenario_id = ?1 ORDER BY id")
                .bind(&sc.id)
                .fetch_all(&p)
                .await
                .unwrap();
        assert_eq!(ids, vec![("ob-oldest".into(),), ("rec-oldest".into(),)]);

        sqlx::migrate!("./migrations").run(&p).await.unwrap();
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
        // Sem campo `date`: a série usa as datas dos itens da obrigação (2026-08-05/06).
        let repl = |amount: i64, label: &str| ReplacementInput {
            amount_cents: amount,
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
            Some(repl(200_000, "Aluguel novo")),
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
            Some(repl(10_000, "Internet nova")),
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

    // --- #167: série de substituição, fronteira, FK e backfill ---
    // Datas 2099 tornam a geração de série independente do relógio (ela filtra `>= mês corrente`).

    #[tokio::test]
    async fn replace_over_obligation_generates_one_line_per_occurrence() {
        let p = pool().await;
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        for (i, date) in ["2099-08-05", "2099-09-05", "2099-10-05"]
            .iter()
            .enumerate()
        {
            txn(&p, &format!("aluguel-{i}"), "expense", 150_000, date).await;
            line_item(
                &p,
                &format!("li-{i}"),
                &format!("aluguel-{i}"),
                150_000,
                "Aluguel",
                None,
            )
            .await;
        }
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 180_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();

        let rows = list_scenario_transactions(&p, &sc.id).await.unwrap();
        let series: Vec<_> = rows
            .iter()
            .filter(|r| r.override_id.as_deref() == Some(ov_id.as_str()))
            .collect();
        assert_eq!(
            series.len(),
            3,
            "uma linha de substituição por ocorrência suprimida"
        );
        assert!(
            series.iter().all(|r| r.amount == 180_000),
            "cada linha da série vale o NOVO valor"
        );
        let mut dates: Vec<&str> = series.iter().map(|r| r.date.as_str()).collect();
        dates.sort_unstable();
        assert_eq!(dates, ["2099-08-05", "2099-09-05", "2099-10-05"]);
    }

    #[tokio::test]
    async fn deleting_obligation_cascades_override_and_replacement_series() {
        let p = pool().await;
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        txn(&p, "aluguel-0", "expense", 150_000, "2099-08-05").await;
        line_item(&p, "li-0", "aluguel-0", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 180_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();
        assert!(
            list_scenario_transactions(&p, &sc.id)
                .await
                .unwrap()
                .iter()
                .any(|r| r.override_id.as_deref() == Some(ov_id.as_str())),
            "série existe antes de apagar a obrigação"
        );

        obligations::delete_obligation(&p, &ob_id).await.unwrap();

        let (ov_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE id = ?1")
                .bind(&ov_id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(ov_count, 0, "apagar a obrigação mata o override (cascade)");
        assert!(
            list_scenario_transactions(&p, &sc.id)
                .await
                .unwrap()
                .iter()
                .all(|r| r.override_id.is_none()),
            "a série morre junto — nunca vira linha órfã ('manter X e adicionar Y')"
        );
    }

    #[tokio::test]
    async fn override_blocked_when_suppression_would_drop_a_live_sibling() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Sobre-explicada: total 100.000, itens Aluguel 100.000 + Internet 50.000 (soma 150 > 100).
        txn(&p, "cell", "expense", 100_000, "2099-08-05").await;
        line_item(&p, "li-al", "cell", 100_000, "Aluguel", None).await;
        line_item(&p, "li-in", "cell", 50_000, "Internet", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let result = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2099-08-01",
            Some(&ob_id),
            None,
            None,
        )
        .await;
        assert!(
            result.is_err(),
            "suprimir Aluguel zeraria a célula e apagaria a Internet viva"
        );
        assert!(result.unwrap_err().contains("zeraria a célula"));
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(n, 0, "recusa antes de persistir qualquer coisa");
    }

    #[tokio::test]
    async fn override_allowed_for_underexplained_cell_and_lone_item() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Sub-explicada: total 100.000, item Aluguel 60.000 (residual 40.000 não documentado).
        txn(&p, "cell-under", "expense", 100_000, "2099-08-05").await;
        line_item(&p, "li-under", "cell-under", 60_000, "Aluguel", None).await;
        // Item único que É a célula: total 80.000, item Luz 80.000 (sem irmão a perder).
        txn(&p, "cell-lone", "expense", 80_000, "2099-08-06").await;
        line_item(&p, "li-lone", "cell-lone", 80_000, "Luz", None).await;
        let ob_alu = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ob_luz = obligations::create_obligation(&p, "Luz", "Luz", None)
            .await
            .unwrap();

        assert!(
            set_scenario_override(
                &p,
                &sc.id,
                "suppress",
                "2099-08-01",
                Some(&ob_alu),
                None,
                None
            )
            .await
            .is_ok(),
            "sub-explicada passa — a subtração preserva o residual"
        );
        assert!(
            set_scenario_override(
                &p,
                &sc.id,
                "suppress",
                "2099-08-01",
                Some(&ob_luz),
                None,
                None
            )
            .await
            .is_ok(),
            "item único que É a célula passa — zerar está correto, sem irmão"
        );
    }

    #[tokio::test]
    async fn cumulative_suppression_across_overrides_blocks_the_second_that_drops_a_sibling() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Total 100.000; itens Aluguel 60.000 + Internet 50.000 + Água 30.000 (soma 140 > 100).
        txn(&p, "cell", "expense", 100_000, "2099-08-05").await;
        line_item(&p, "li-al", "cell", 60_000, "Aluguel", None).await;
        line_item(&p, "li-in", "cell", 50_000, "Internet", None).await;
        line_item(&p, "li-ag", "cell", 30_000, "Agua", None).await;
        let ob_al = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ob_in = obligations::create_obligation(&p, "Internet", "Internet", None)
            .await
            .unwrap();

        // 1º (Aluguel 60): 60 < 100 → passa (residual 40).
        set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2099-08-01",
            Some(&ob_al),
            None,
            None,
        )
        .await
        .unwrap();
        // 2º (Internet 50): cumulativo 60+50 = 110 ≥ 100 → zeraria a célula, mas a Água (30) vive.
        let second = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2099-08-01",
            Some(&ob_in),
            None,
            None,
        )
        .await;
        assert!(
            second.is_err(),
            "a supressão cumulativa dos dois overrides zeraria a célula e apagaria a Água"
        );
        assert!(second.unwrap_err().contains("zeraria a célula"));
    }

    #[tokio::test]
    async fn replace_change_reports_per_occurrence_old_and_weighs_every_month() {
        let p = pool().await;
        txn(&p, "inc", "income", 1_000_000, "2099-08-01").await;
        txn(&p, "al-ago", "expense", 150_000, "2099-08-05").await;
        line_item(&p, "li-ago", "al-ago", 150_000, "Aluguel", None).await;
        txn(&p, "al-set", "expense", 150_000, "2099-09-05").await;
        line_item(&p, "li-set", "al-set", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 200_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2099-07-31"))
            .await
            .unwrap();
        let replace = compare.changes.iter().find(|c| c.op == "replace").unwrap();
        // Par MENSAL: 150.000 → 200.000 (não o total do horizonte 300.000 → 200.000).
        assert_eq!(
            replace.old_amount_cents,
            Some(150_000),
            "old é a ocorrência representativa (mensal), não o total suprimido"
        );
        assert_eq!(replace.new_amount_cents, Some(200_000));
        // A série repõe TODO mês: cada mês cai 50.000 a mais (o delta acumula 50k → 100k).
        let ago = compare
            .month_end
            .iter()
            .find(|m| m.year == 2099 && m.month == 8)
            .unwrap();
        assert_eq!(ago.delta_cents, -50_000, "agosto: novo 200 vs velho 150");
        let set = compare
            .month_end
            .iter()
            .find(|m| m.year == 2099 && m.month == 9)
            .unwrap();
        assert_eq!(
            set.delta_cents, -100_000,
            "setembro acumula outro −50.000 — a série não fica otimista"
        );
    }

    #[tokio::test]
    async fn replace_over_recurrence_generates_series_and_lists_targets() {
        let p = pool().await;
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        sqlx::query(
            "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
             VALUES ('rec-1','mensal',0,2,'2099-08-10')",
        )
        .execute(&p)
        .await
        .unwrap();
        for (id, date) in [("r-ago", "2099-08-10"), ("r-set", "2099-09-10")] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, \
                 recurrence_id) VALUES (?1,'expense',30000,'Academia',?2,1,'rec-1')",
            )
            .bind(id)
            .bind(date)
            .execute(&p)
            .await
            .unwrap();
        }

        let targets = list_recurrence_targets(&p).await.unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "rec-1");
        assert_eq!(targets[0].description, "Academia");
        assert_eq!(targets[0].frequency, "mensal");
        assert_eq!(recurrence_occurrences(&p, "rec-1").await.unwrap().len(), 2);

        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            None,
            Some("rec-1"),
            Some(ReplacementInput {
                amount_cents: 45_000,
                description: Some("Academia nova".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();
        let series: Vec<_> = list_scenario_transactions(&p, &sc.id)
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.override_id.as_deref() == Some(ov_id.as_str()))
            .collect();
        assert_eq!(
            series.len(),
            2,
            "uma linha por ocorrência real da recorrência"
        );
        assert!(series.iter().all(|r| r.amount == 45_000));
    }

    #[tokio::test]
    async fn replace_representative_old_is_deterministic_on_same_date_cells() {
        let p = pool().await;
        txn(&p, "inc", "income", 1_000_000, "2099-08-01").await;
        // Duas células na MESMA data, casadas pela mesma obrigação, com valores distintos.
        txn(&p, "a-cell", "expense", 100_000, "2099-08-05").await;
        line_item(&p, "li-a", "a-cell", 100_000, "Aluguel", None).await;
        txn(&p, "z-cell", "expense", 80_000, "2099-08-05").await;
        line_item(&p, "li-z", "z-cell", 80_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 200_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();
        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2099-07-31"))
            .await
            .unwrap();
        let replace = compare.changes.iter().find(|c| c.op == "replace").unwrap();
        // Desempate estável por (data, transaction_id): "a-cell" < "z-cell" → 100.000 sempre, nunca
        // 80.000 conforme o seed de hash do processo (era o bug do HashMap::values().min_by).
        assert_eq!(
            replace.old_amount_cents,
            Some(100_000),
            "representante da ocorrência é determinístico no empate de data"
        );
    }

    #[tokio::test]
    async fn historical_overexplained_cell_does_not_block_override() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Célula PASSADA (antes do mês corrente) sobre-explicada com irmão vivo. A projeção nunca
        // a toca (load_real_rows começa em month_start), então a fronteira NÃO pode bloquear.
        txn(&p, "cell-hist", "expense", 100_000, "2020-01-05").await;
        line_item(&p, "li-al", "cell-hist", 100_000, "Aluguel", None).await;
        line_item(&p, "li-in", "cell-hist", 50_000, "Internet", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        let result = set_scenario_override(
            &p,
            &sc.id,
            "suppress",
            "2020-01-01",
            Some(&ob_id),
            None,
            None,
        )
        .await;
        assert!(
            result.is_ok(),
            "célula histórica inerte na projeção não dispara a fronteira (falso-positivo evitado)"
        );
    }

    #[tokio::test]
    async fn replace_also_triggers_the_sibling_boundary() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        // Sobre-explicada futura: total 100.000, Aluguel 100.000 + Internet 50.000 (irmão vivo).
        txn(&p, "cell", "expense", 100_000, "2099-08-05").await;
        line_item(&p, "li-al", "cell", 100_000, "Aluguel", None).await;
        line_item(&p, "li-in", "cell", 50_000, "Internet", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();

        // `replace` também suprime o alvo → passa pela MESMA fronteira do `suppress`.
        let result = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 120_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await;
        assert!(
            result.is_err(),
            "replace destrutivo é recusado igual ao suppress"
        );
        assert!(result.unwrap_err().contains("zeraria a célula"));
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE scenario_id = ?1")
                .bind(&sc.id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(n, 0, "recusa antes de gerar qualquer linha da série");
    }

    #[tokio::test]
    async fn deleting_recurrence_cascades_override_and_replacement_series() {
        let p = pool().await;
        let sc = create_scenario(&p, "Aumento").await.unwrap();
        sqlx::query(
            "INSERT INTO recurrence (id, frequency, infinite, repetitions, start_date) \
             VALUES ('rec-1','mensal',0,2,'2099-08-10')",
        )
        .execute(&p)
        .await
        .unwrap();
        for (id, date) in [("r-ago", "2099-08-10"), ("r-set", "2099-09-10")] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, \
                 recurrence_id) VALUES (?1,'expense',30000,'Academia',?2,1,'rec-1')",
            )
            .bind(id)
            .bind(date)
            .execute(&p)
            .await
            .unwrap();
        }
        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            None,
            Some("rec-1"),
            Some(ReplacementInput {
                amount_cents: 45_000,
                description: Some("Academia nova".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            list_scenario_transactions(&p, &sc.id)
                .await
                .unwrap()
                .iter()
                .filter(|r| r.override_id.as_deref() == Some(ov_id.as_str()))
                .count(),
            2
        );

        // `transaction.recurrence_id` não cascateia, então as linhas reais saem primeiro; apagar o
        // REGISTRO de recorrência então cascateia scenario_override.recurrence_id → e a série
        // (transaction.override_id) morre junto.
        sqlx::query(
            "DELETE FROM \"transaction\" WHERE recurrence_id = 'rec-1' AND scenario_id IS NULL",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query("DELETE FROM recurrence WHERE id = 'rec-1'")
            .execute(&p)
            .await
            .unwrap();

        let (ov_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM scenario_override WHERE id = ?1")
                .bind(&ov_id)
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(ov_count, 0, "override morre com a recorrência (cascade FK)");
        assert!(
            list_scenario_transactions(&p, &sc.id)
                .await
                .unwrap()
                .iter()
                .all(|r| r.override_id.is_none()),
            "a série da recorrência morre junto — sem linha órfã"
        );
    }

    #[tokio::test]
    async fn replace_series_works_on_single_connection_pool() {
        // Regressão: produção usa `max_connections(1)`. As datas da série devem ser derivadas
        // ANTES de abrir a transação — derivá-las com a transação aberta pediria uma 2ª conexão
        // que nunca vem (deadlock). Um `acquire_timeout` curto transforma o deadlock em erro
        // rápido, então este teste falha em ~2s (não trava) se a regressão voltar.
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        txn(&p, "al", "expense", 150_000, "2099-08-05").await;
        line_item(&p, "li", "al", 150_000, "Aluguel", None).await;
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let sc = create_scenario(&p, "Cenário").await.unwrap();

        let ov_id = set_scenario_override(
            &p,
            &sc.id,
            "replace",
            "2099-08-01",
            Some(&ob_id),
            None,
            Some(ReplacementInput {
                amount_cents: 180_000,
                description: Some("Aluguel novo".into()),
                txn_type: None,
                payment_method: None,
                is_fixed: None,
            }),
        )
        .await
        .expect("set_scenario_override não pode esperar uma 2ª conexão (deadlock)");
        let series: Vec<_> = list_scenario_transactions(&p, &sc.id)
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.override_id.as_deref() == Some(ov_id.as_str()))
            .collect();
        assert_eq!(
            series.len(),
            1,
            "a série é gerada normalmente no pool de 1 conexão"
        );
    }

    #[tokio::test]
    async fn backfill_replacements_converts_marker_to_fk_and_is_idempotent() {
        let p = pool().await;
        let sc = create_scenario(&p, "Cenário").await.unwrap();
        let ob_id = obligations::create_obligation(&p, "Aluguel", "Aluguel", None)
            .await
            .unwrap();
        let ov_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO scenario_override (id, scenario_id, op, from_date, obligation_id) \
             VALUES (?1, ?2, 'replace', '2099-08-01', ?3)",
        )
        .bind(&ov_id)
        .bind(&sc.id)
        .bind(&ob_id)
        .execute(&p)
        .await
        .unwrap();
        // Linha legada marcada + uma órfã (override inexistente).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, \
             scenario_id) VALUES ('repl-1','expense',180000,?1,'2099-08-05',1,?2)",
        )
        .bind(format!("Aluguel novo #repl:{ov_id}"))
        .bind(&sc.id)
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, \
             scenario_id) VALUES ('repl-orphan','expense',9999,'Fantasma #repl:morto','2099-08-06',1,?1)",
        )
        .bind(&sc.id)
        .execute(&p)
        .await
        .unwrap();

        backfill_scenario_override_replacements(&p).await.unwrap();

        let (desc, oid): (String, Option<String>) = sqlx::query_as(
            "SELECT description, override_id FROM \"transaction\" WHERE id = 'repl-1'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(desc, "Aluguel novo", "sufixo removido");
        assert_eq!(oid.as_deref(), Some(ov_id.as_str()), "marca vira FK");
        let (odesc, ooid): (String, Option<String>) = sqlx::query_as(
            "SELECT description, override_id FROM \"transaction\" WHERE id = 'repl-orphan'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(odesc, "Fantasma", "órfã perde o sufixo");
        assert_eq!(ooid, None, "órfã vira adição comum (sem FK)");

        // Idempotente: re-rodar não altera (nenhuma linha carrega mais o marcador).
        backfill_scenario_override_replacements(&p).await.unwrap();
        let (desc2, oid2): (String, Option<String>) = sqlx::query_as(
            "SELECT description, override_id FROM \"transaction\" WHERE id = 'repl-1'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(desc2, "Aluguel novo");
        assert_eq!(oid2.as_deref(), Some(ov_id.as_str()));
    }

    // --- Régua "Reserva após financiar" (canônica: reserva ÷ (mediana + parcela)) ---

    /// Conta com liquidez explícita, para os testes da régua de reserva.
    async fn account_with_liquidity(p: &SqlitePool, id: &str, balance_cents: i64, liquidity: &str) {
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES (?1, ?1, 'bank', 'pe-res', ?2, ?3)",
        )
        .bind(id)
        .bind(balance_cents)
        .bind(liquidity)
        .execute(p)
        .await
        .unwrap();
    }

    /// Fixture da régua: perfil + dois meses completos de custo de vida (mediana 200_000) +
    /// cenário com um empréstimo de parcela 50_000. Reserva fica a cargo de cada teste.
    async fn seed_reserve_ruler_fixture(p: &SqlitePool) -> String {
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-res', 'Eu')")
            .execute(p)
            .await
            .unwrap();
        // Meses COMPLETOS na janela de 6 meses de um `today` em 2026-05: mediana = 200_000.
        txn(p, "res-cost-mar", "expense", 200_000, "2026-03-10").await;
        txn(p, "res-cost-apr", "expense", 200_000, "2026-04-10").await;
        let sc = create_scenario(p, "E se eu financiar").await.unwrap();
        seed_loan(
            p,
            &sc.id,
            "loan-res",
            250,
            1_000_000,
            2,
            "2026-05-10",
            &[
                ("income", 1_000_000, "Empréstimo", "2026-05-10"),
                ("expense", 50_000, "Empréstimo parcela 1/2", "2026-06-10"),
                ("expense", 50_000, "Empréstimo parcela 2/2", "2026-07-10"),
            ],
        )
        .await;
        sc.id
    }

    fn assert_close(actual: Option<f64>, expected: f64, label: &str) {
        let v = actual.unwrap_or_else(|| panic!("{label}: esperado Some({expected}), veio None"));
        assert!(
            (v - expected).abs() < 1e-9,
            "{label}: esperado {expected}, veio {v}"
        );
    }

    /// A régua é a canônica do método (reserva ÷ mediana de meses completos), então NÃO pode
    /// variar com o dia do mês — a antiga (custo de vida realizado do mês corrente) inflava no
    /// início do mês, exatamente no gate "posso assumir esta parcela?".
    #[tokio::test]
    async fn reserve_ruler_is_canonical_and_day_independent() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;

        let early = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let late = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-28"))
            .await
            .unwrap();

        for (day, compare) in [("dia 2", &early), ("dia 28", &late)] {
            let loan = compare.loan.as_ref().expect("empréstimo detectado");
            // Antes: 1_200_000 ÷ 200_000 = 6 meses — idêntico ao `reserve_months` do dashboard.
            assert_close(
                loan.reserve_months_before_financing,
                6.0,
                &format!("before ({day})"),
            );
            // Depois: 1_200_000 ÷ (200_000 + 50_000) = 4,8 — só a parcela entra como
            // compromisso novo, então after < before sempre que installment > 0.
            assert_close(
                loan.reserve_months_after_financing,
                4.8,
                &format!("after ({day})"),
            );
        }
    }

    /// Sem mês completo realizado não há custo de vida típico: a régua fica oculta (None),
    /// nunca um número inventado.
    #[tokio::test]
    async fn reserve_ruler_hidden_when_baseline_is_zero() {
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-res', 'Eu')")
            .execute(&p)
            .await
            .unwrap();
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;
        let sc = create_scenario(&p, "E se eu financiar").await.unwrap();
        seed_loan(
            &p,
            &sc.id,
            "loan-res",
            250,
            1_000_000,
            1,
            "2026-05-10",
            &[
                ("income", 1_000_000, "Empréstimo", "2026-05-10"),
                ("expense", 50_000, "Empréstimo parcela 1/1", "2026-06-10"),
            ],
        )
        .await;

        let compare = get_scenario_forecast_inner(&p, &sc.id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        assert_eq!(loan.reserve_months_before_financing, None);
        assert_eq!(loan.reserve_months_after_financing, None);
    }

    /// Reserva zerada é informação (o gate reprova com 0,0 meses) — não se esconde a linha.
    #[tokio::test]
    async fn reserve_ruler_zero_reserve_is_zero_not_hidden() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        // Nenhuma conta de reserva: numerador = 0.

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        assert_close(loan.reserve_months_before_financing, 0.0, "before");
        assert_close(loan.reserve_months_after_financing, 0.0, "after");
    }

    /// O numerador é o saldo das contas `liquidity='reserve'` — conta corrente não conta,
    /// espelhando a régua do dashboard.
    #[tokio::test]
    async fn reserve_ruler_numerator_counts_only_reserve_accounts() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 600_000, "reserve").await;
        account_with_liquidity(&p, "acc-corrente", 9_999_999, "liquid").await;

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        // 600_000 ÷ 200_000 = 3 · 600_000 ÷ 250_000 = 2,4 — o saldo líquido fica de fora.
        assert_close(loan.reserve_months_before_financing, 3.0, "before");
        assert_close(loan.reserve_months_after_financing, 2.4, "after");
    }

    // --- Régua "Economia após parcela" (2ª perna do gate: piso 20% + regra da metade) ---

    /// Transfer para uma conta de reserva já criada — classificado como Economia pela
    /// liquidez do destino, igual ao lançamento real de "guardar dinheiro".
    async fn economia_transfer(p: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES (?1, 'transfer', ?2, ?3, 'acc-reserva', 0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(p)
        .await
        .unwrap();
    }

    /// before/after/mediana derivam das medianas de renda e economia dos meses ativos: a régua
    /// gêmea da reserva descreve o MESMO "mês típico" (mesma janela, mesmo estimador).
    #[tokio::test]
    async fn savings_ruler_derives_from_medians_and_subtracts_installment() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;
        txn(&p, "inc-mar", "income", 300_000, "2026-03-05").await;
        txn(&p, "inc-apr", "income", 300_000, "2026-04-05").await;
        economia_transfer(&p, "eco-mar", 90_000, "2026-03-12").await;
        economia_transfer(&p, "eco-apr", 90_000, "2026-04-12").await;

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        // Antes: 90_000 ÷ 300_000 = 30% = 3000 bps.
        assert_eq!(loan.savings_rate_before_bps, Some(3000));
        // Depois: (90_000 − 50_000) ÷ 300_000 = 13,33% = 1333 bps — a parcela desconta, então
        // after < before sempre que installment > 0.
        assert_eq!(loan.savings_rate_after_bps, Some(1333));
        // A mediana da economia alimenta a regra da metade e a frase em R$ do popover.
        assert_eq!(loan.economia_median_cents, 90_000);
    }

    /// Sem renda na janela não há percentual possível: os dois campos ficam None e a linha
    /// some — nunca uma divisão por zero disfarçada.
    #[tokio::test]
    async fn savings_ruler_hidden_when_income_median_is_zero() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;
        // Meses ativos existem (custo de vida da fixture), mas nenhum tem renda.

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        assert_eq!(loan.savings_rate_before_bps, None);
        assert_eq!(loan.savings_rate_after_bps, None);
    }

    /// Parcela maior que a economia típica ⇒ `after` NEGATIVO, preservado bruto — o clamp em 0%
    /// é só de exibição (frontend); o estado da escada julga o valor real.
    #[tokio::test]
    async fn savings_ruler_preserves_negative_after_raw() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;
        txn(&p, "inc-mar", "income", 200_000, "2026-03-05").await;
        txn(&p, "inc-apr", "income", 200_000, "2026-04-05").await;
        economia_transfer(&p, "eco-mar", 30_000, "2026-03-12").await;
        economia_transfer(&p, "eco-apr", 30_000, "2026-04-12").await;

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        // Antes: 30_000 ÷ 200_000 = 15% = 1500 bps.
        assert_eq!(loan.savings_rate_before_bps, Some(1500));
        // Depois: (30_000 − 50_000) ÷ 200_000 = −10% = −1000 bps, bruto.
        assert_eq!(loan.savings_rate_after_bps, Some(-1000));
    }

    /// O backend expõe o bruto sem arredondar por cima: 20,00% exato chega como 2000 bps e o
    /// piso (julgado no frontend) considera 2000 como "passa" — fronteira inclusiva.
    #[tokio::test]
    async fn savings_ruler_exposes_exact_floor_boundary() {
        let p = pool().await;
        let sc_id = seed_reserve_ruler_fixture(&p).await;
        account_with_liquidity(&p, "acc-reserva", 1_200_000, "reserve").await;
        txn(&p, "inc-mar", "income", 300_000, "2026-03-05").await;
        txn(&p, "inc-apr", "income", 300_000, "2026-04-05").await;
        // (110_000 − 50_000) ÷ 300_000 = 20,00% exato.
        economia_transfer(&p, "eco-mar", 110_000, "2026-03-12").await;
        economia_transfer(&p, "eco-apr", 110_000, "2026-04-12").await;

        let compare = get_scenario_forecast_inner(&p, &sc_id, d("2026-05-02"))
            .await
            .unwrap();
        let loan = compare.loan.as_ref().expect("empréstimo detectado");
        assert_eq!(loan.savings_rate_after_bps, Some(2000));
    }

    #[tokio::test]
    async fn scenario_forecast_keeps_baseline_invoices_with_a_loan() {
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('person-card', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id) \
             VALUES ('card', 'Cartão', 'credit_card', 'person-card')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice', 'card', '2026-08', '2026-07-20', '2026-08-10', 85000)",
        )
        .execute(&p)
        .await
        .unwrap();
        let scenario = create_scenario(&p, "Empréstimo").await.unwrap();
        seed_loan(
            &p,
            &scenario.id,
            "loan",
            150,
            100_000,
            1,
            "2026-09-05",
            &[
                ("income", 100_000, "Desembolso", "2026-09-05"),
                ("expense", 55_000, "Parcela 1/1", "2026-09-10"),
            ],
        )
        .await;

        let compare = get_scenario_forecast_inner(&p, &scenario.id, d("2026-08-01"))
            .await
            .unwrap();
        assert_eq!(compare.real_cost_of_living_cents, 85_000);
        assert_eq!(compare.scenario_cost_of_living_cents, 85_000);
        assert!(compare.loan.is_some());
    }
}
