use super::*;

// --- App info ---

#[derive(serde::Serialize)]
pub struct AppInfo {
    pub version: String,
    pub db_path: String,
}

#[tauri::command]
pub async fn get_app_info(app_dir: State<'_, AppDataDir>) -> Result<AppInfo, String> {
    Ok(app_info_for_dir(&app_dir.0))
}

/// Pure helper so the command stays a thin adapter (testable without Tauri `State`).
pub(crate) fn app_info_for_dir(app_dir: &std::path::Path) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        db_path: app_dir.join("neko-finance.db").display().to_string(),
    }
}

// --- Forecast projection (spec 005) ---

/// Sum of liquid cash accounts — the projection seed (spec 003 US2).
/// Spec 007: only `liquidity = 'liquid'` pockets are cash; reserve/restricted/illiquid
/// money must not inflate the projected balance.
pub(crate) async fn liquid_seed(pool: &SqlitePool) -> Result<i64, String> {
    let seed: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'liquid'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;
    Ok(seed.0)
}

/// Semente da projeção — o saldo de partida do qual a engine encadeia o futuro.
///
/// Método da planilha: a coluna `Saldo` É o saldo que "bate com o banco". Quando há série
/// importada, a semente = `Saldo` do dia mais recente ≤ hoje; quaisquer lançamentos realizados
/// ENTRE esse dia e hoje são somados (cobre o caso de a planilha ainda não ter hoje preenchido),
/// de modo que o carregador de eventos pode seguir usando `date > today` sem perder o intervalo.
/// Sem planilha importada, cai nos Bolsos líquidos (spec 007). Precedência: planilha > bolsos —
/// quem importa a planilha quer que a projeção continue a própria linha dela.
pub(crate) async fn projection_seed(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    let latest: Option<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date <= ?1 ORDER BY date DESC LIMIT 1",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("seed query: {e}"))?;

    let Some((seed_date, balance)) = latest else {
        return liquid_seed(pool).await;
    };

    // `transfer` (Economia) é INCLUÍDO: reduz o saldo líquido tanto quanto uma despesa (o dinheiro
    // sai da conta de gastos para a reserva). Todo `transfer` gravado tem destino reserve/illiquid
    // (validado no lançamento manual e no import), então é sempre uma saída do líquido. O CASE
    // abaixo já o trata como saída (−amount); excluí-lo superestimava a semente pelas Economias
    // ocorridas entre o último Saldo da planilha e hoje.
    let gap: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
         FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense','transfer')",
    )
    .bind(&seed_date)
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("seed gap: {e}"))?;

    Ok(balance + gap.0)
}

/// Meta de poupança do método para o guardrail ANUAL "pode gastar": **25% (2500 bps)** — a MÉDIA
/// da faixa canônica 20–30% (MÉDIA ANUAL: o ano todo deve ficar na faixa, os meses variam). É uma
/// barra DELIBERADAMENTE mais alta que o piso de 20% do método: o gate anual decide quanto se pode
/// gastar HOJE, então mira no alvo médio, não no piso.
///
/// O piso mínimo de 20% (2000 bps) é o `SAVINGS_MIN_BPS` do frontend (`src/screens/totaisStatus.ts`),
/// usado nos indicadores/visuais MENSAIS e ANUAIS (badge "Dentro do ideal", cor da visão anual,
/// gate da fase "operar"), que são lenientes a variações de um mês. Ambos os limiares ficam dentro
/// da faixa canônica 20–30%; não os unifique sem decisão de método (unificar afrouxaria o gate anual).
pub(crate) const SAVINGS_TARGET_BPS: i64 = 2500;

/// Limiar de cobertura: um mês futuro com menos de 60% do gasto típico já lançado é tratado como
/// INCOMPLETO (projeção otimista demais — o "chá revelação" do método). Margem ampla porque o
/// método aceita variação mês a mês; abaixo disso é quase certo que falta fatura/variável.
pub(crate) const COVERAGE_COMPLETE_BPS: i64 = 6_000;

/// Meses de reserva mínimos do método (fase "operar"): o mesmo limiar que o frontend usa em
/// `colchaoPhase.ts` (RESERVE_MIN_MONTHS). Mantidos em sync manualmente (a lógica de fase é
/// puramente frontend; se mudar, atualizar os dois).
pub(crate) const RESERVE_MIN_MONTHS: i64 = 6;

/// Renda e net REALIZADOS do ano corrente até hoje (`is_projection = 0`): a poupança é o net
/// `renda − saída` realizado dos meses completos. Retorna `(renda, net)` — o `net` superávit
/// alimenta `AnnualSavingsDto.realized_savings_cents` (o "colchão" exibido); a Economia registrada
/// que o guardrail de poupança usa vem de `realized_annual_economia` (transfers→reserva).
///
/// Conta só **meses COMPLETOS** do ano (`substr(date) < mês corrente`), nunca o mês em andamento.
/// No meio do mês as contas fixas já podem ter entrado mas o salário ainda não, o que daria um
/// net negativo de timing e um "pode gastar R$ 0" de falso pânico.
///
/// NÃO filtra `is_projection`: ele é congelado no import (data vs hoje DAQUELE dia) e fica STALE
/// quando o dono não re-importa por dias/meses. Um mês completo já passou — é realizado por
/// definição —, então a janela de DATA é a fonte de verdade, não o flag congelado.
pub(crate) async fn realized_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let is_january = cur_ym == format!("{}-01", today_naive.year());
    // Janela = só meses COMPLETOS do ano corrente: `[ano-01-01, 1º dia do mês corrente)`.
    // Em 1º de JANEIRO essa janela é `[YYYY-01-01, YYYY-01-01)` → VAZIA, o que zerava o guardrail
    // justo na virada do ano (falso "sem restrição"). Nesse caso deslocamos a janela para DEZEMBRO
    // do ano anterior — o último período COMPLETO de poupança realizada —, `[YYYY-1-12-01,
    // YYYY-01-01)`, mantendo o guardrail ATIVO. Sem dado de dezembro a query devolve 0 (mesmo
    // fallback seguro de antes, mas agora o chamador distingue "sem dado" de "janela vazia").
    let (lower, upper) = if is_january {
        (
            format!("{}-12-01", today_naive.year() - 1),
            format!("{}-01-01", today_naive.year()),
        )
    } else {
        // `date < 'YYYY-MM-01'` ≡ `substr(date,1,7) < 'YYYY-MM'` p/ ISO.
        (
            format!("{}-01-01", today_naive.year()),
            format!("{cur_ym}-01"),
        )
    };
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date < ?2 \
           AND type IN ('income','expense')",
    )
    .bind(&lower)
    .bind(&upper)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized annual: {e}"))?;
    Ok((row.0, row.0 - row.1)) // (renda, poupança=net) dos meses completos
}

/// Economia REGISTRADA do ano até hoje (meses completos): transfers cujo destino é conta
/// reserva/ilíquida — mesma classificação de `forecast::classify`. É o numerador do "Economizado"
/// do método (Economia/Entradas), DISTINTO do net superávit de `realized_annual_savings` (que é o
/// "colchão" do Neko). Existir os dois lado a lado sem se confundir foi um achado da review.
pub(crate) async fn realized_annual_economia(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Mesma janela de "meses COMPLETOS" de `realized_annual_savings` — MANTER SIMÉTRICAS: o guardrail
    // de poupança compara a renda (daquela função) contra a Economia (desta). Em 1º de JANEIRO a
    // janela `[ano-01-01, mês-corrente-01)` é VAZIA, o que zerava a Economia justo na virada do ano
    // enquanto a renda usava a janela deslocada para DEZEMBRO → guardrail incoerente ("pode gastar"
    // enganoso). Aqui replicamos o mesmo deslocamento para DEZEMBRO do ano anterior.
    let is_january = cur_ym == format!("{}-01", today_naive.year());
    let (lower, upper) = if is_january {
        (
            format!("{}-12-01", today_naive.year() - 1),
            format!("{}-01-01", today_naive.year()),
        )
    } else {
        // `date < 'YYYY-MM-01'` ≡ `substr(date,1,7) < 'YYYY-MM'` p/ ISO.
        (
            format!("{}-01-01", today_naive.year()),
            format!("{cur_ym}-01"),
        )
    };
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid') \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&lower)
    .bind(&upper)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized economia: {e}"))?;
    Ok(row.0)
}

/// Renda e net do ANO INTEIRO projetado (todas as linhas do ano). Exibido só como contraste com
/// o realizado — é OTIMISTA quando os meses futuros estão incompletos (não usar no guardrail).
pub(crate) async fn projected_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    // Range explícito (não `LIKE 'YYYY%'`) — consistente com o realizado e rejeita data
    // malformada que começa com o ano mas não é ISO válida (review P2).
    let start = format!("{}-01-01", today_naive.year());
    let end = format!("{}-12-31", today_naive.year());
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date <= ?2 AND type IN ('income','expense')",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("projected annual: {e}"))?;
    Ok((row.0, row.0 - row.1))
}

/// Gasto típico de um mês = MEDIANA da saída dos meses realizados COMPLETOS (anteriores ao mês
/// corrente), dos **últimos 6 meses** (recentes representam melhor o padrão atual que meses
/// antigos de anos anteriores — review ui-vs-planilha). Mediana para ser robusta a um mês atípico.
pub(crate) async fn realized_monthly_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT SUM(amount) FROM \"transaction\" \
         WHERE type='expense' AND date < ?1 \
         GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6",
    )
    .bind(format!("{cur_ym}-01")) // WHERE vira range (usa o índice); GROUP/ORDER por mês ficam
    .fetch_all(pool)
    .await
    .map_err(|e| format!("baseline: {e}"))?;
    let mut vals: Vec<i64> = rows.into_iter().map(|(s,)| s).collect();
    if vals.is_empty() {
        return Ok(0);
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    let median = if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2
    };
    Ok(median)
}

/// Teto de diário "típico" por dia. Fonte única para (a) projetar o gasto diário nos dias futuros do
/// mês corrente (driver do forecast — senão o saldo nasce otimista assumindo zero gasto) e (b) a
/// referência exibida no tile "Diário de hoje" (`de R$X`). Regra:
/// 1. se houver orçamento diário explícito ativo (> 0), ele vence (o dono definiu um teto);
/// 2. senão, o Diário médio do último mês COMPLETO = Σ diário realizado (despesa não-fixa, não-crédito)
///    ÷ dias do mês. Espelha o `real_daily_avg_cents` do método ("Diário médio") sobre o mês anterior.
///
/// Sem mês anterior com diário, retorna 0 (usuário novo — nada a assumir).
pub(crate) async fn effective_daily_ceiling(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT amount FROM daily_budget WHERE status='active' AND amount > 0 \
         ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("daily ceiling (budget): {e}"))?;
    if let Some((amount,)) = active {
        return Ok(amount);
    }
    // Mês anterior completo: primeiro dia do mês corrente − 1 dia.
    let first_this = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data inválida")?;
    let last_prev = match first_this.pred_opt() {
        Some(d) => d,
        None => return Ok(0),
    };
    let prev_ym = last_prev.format("%Y-%m").to_string();
    let days_prev = last_prev.day() as i64;
    let sum: (i64,) = sqlx::query_as(
        "SELECT ABS(COALESCE(SUM(amount), 0)) FROM \"transaction\" \
         WHERE type='expense' AND is_fixed=0 AND is_projection=0 \
           AND (payment_method IS NULL OR payment_method <> 'credit') \
           AND substr(date,1,7) = ?1",
    )
    .bind(&prev_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("daily ceiling (avg): {e}"))?;
    Ok(if days_prev > 0 { sum.0 / days_prev } else { 0 })
}

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

// --- Plano 045: quebra por categoria do orçamento Diário ---

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

/// Puro: valor MENSAL → teto por DIA do mês informado. O teto diário do método é o orçamento
/// mensal dividido pelos dias do mês (ex.: 3100 ÷ 31 = 100/dia). `days_in_month = 0` → 0 (sem panic).
/// Espelha a mesma intenção do `effective_daily_ceiling` (a tela computa a exibição; o engine
/// continua lendo o `daily_budget.amount` como o teto/dia escrito pelo dono).
///
/// A derivação do teto/dia exibido vive HOJE na UI (`DiarioCategorySection`, em TypeScript) para não
/// pagar um round-trip; este núcleo puro existe como fonte-da-verdade testável da fórmula, pronto
/// para um futuro caller backend (ex.: auto-parse de notas) — daí `allow(dead_code)` deliberado.
#[allow(dead_code)]
pub(crate) fn monthly_to_daily_rate(amount_cents: i64, days_in_month: u32) -> i64 {
    if days_in_month == 0 {
        return 0;
    }
    amount_cents / days_in_month as i64
}

/// Núcleo puro: grava o teto total do Diário + uma quebra opcional por categoria.
///
/// 1. Reaproveita `upsert_daily_budget_inner` para escrever/deprecar a linha de TOTAL — assim o
///    engine (`effective_daily_ceiling`, que lê `daily_budget WHERE status='active' AND amount>0`)
///    segue funcionando sem mudança: a quebra por categoria é só drill-down de UI.
/// 2. Se `amount_cents > 0` e `categories` não-vazio: localiza o id do orçamento recém-ativado e
///    substitui (DELETE + INSERT) as categorias dele numa ÚNICA transação SQLite.
/// 3. `categories` vazio: nada a fazer na tabela de categorias (o total-only continua válido).
///
/// Validação: cada `category.amount_cents > 0`; senão retorna Err sem tocar no banco.
pub(crate) async fn upsert_daily_budget_with_categories_inner(
    pool: &SqlitePool,
    amount_cents: i64,
    categories: &[CategoryInput],
) -> Result<(), String> {
    // Valida ANTES de escrever (atomicidade lógica: ou tudo válido, ou nada muda).
    for c in categories {
        if c.amount_cents <= 0 {
            return Err("cada categoria deve ter valor positivo (magnitude)".into());
        }
    }

    // Passo 1: escreve/depreca o TOTAL pelo mesmo caminho do teto simples (engine inalterado).
    upsert_daily_budget_inner(pool, amount_cents).await?;

    // Passo 2: só anexa categorias quando há um teto explícito ativo E uma quebra informada.
    if amount_cents > 0 && !categories.is_empty() {
        let budget: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM daily_budget WHERE status='active' ORDER BY start_date DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("upsert categories (budget id): {e}"))?;
        let Some((budget_id,)) = budget else {
            // Sem perfil (upsert_daily_budget_inner foi no-op): nada a anexar.
            return Ok(());
        };

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("upsert categories (begin): {e}"))?;
        sqlx::query("DELETE FROM daily_budget_category WHERE budget_id = ?1")
            .bind(&budget_id)
            .execute(&mut *tx)
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
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("upsert categories (insert): {e}"))?;
        }
        tx.commit()
            .await
            .map_err(|e| format!("upsert categories (commit): {e}"))?;
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
) -> Result<(), String> {
    upsert_daily_budget_with_categories_inner(pool.inner(), amount_cents, &categories).await
}

/// Lê as categorias do orçamento Diário ativo (vazio = sem quebra). Adapter fino.
#[tauri::command]
pub async fn get_daily_budget_categories_cmd(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<DailyBudgetCategoryRow>, String> {
    get_daily_budget_categories_inner(pool.inner()).await
}

/// Piso de reserva = colchão intocável que a folga de caixa não pode comer.
///
/// Lógica em duas camadas:
/// 1. Saldo dos Bolsos de reserva configurados (`liquidity = 'reserve'`). Esses Bolsos NÃO
///    entram na semente líquida, então subtraí-los aqui não os dobra.
/// 2. Piso mínimo do método: `custo de vida mensal × RESERVE_MIN_MONTHS`. O custo de vida mensal é
///    o `realized_monthly_baseline` (mediana das saídas dos meses completos = fixas + diário +
///    cartão). Se não há Bolso de reserva configurado (ou o saldo está abaixo do piso), usa o
///    piso calculado — assim o guardrail não fica completamente desmontado para quem ainda não
///    criou um Bolso de reserva.
///
/// Sem histórico de custo de vida (baseline = 0, usuário novo), o piso calculado é 0 e o resultado
/// cai no saldo de reserva (também 0 nesse caso) — não bloqueia quem está começando. Sem Bolso de
/// reserva mas com histórico, retorna o piso calculado.
pub(crate) async fn reserve_floor(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let reserve_balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor (balance): {e}"))?;
    let baseline = realized_monthly_baseline(pool, today_naive).await?;
    let computed_floor = baseline * RESERVE_MIN_MONTHS;
    Ok(reserve_balance.0.max(computed_floor))
}

/// Fim do horizonte da projeção = o último dia com dado pré-lançado (transação futura ou Saldo
/// importado) ≥ hoje. A planilha do método já lança o ano inteiro à frente, então varremos ATÉ
/// O FIM DOS DADOS, não só o mês corrente — senão o "pode gastar" fica cego ao buraco do futuro
/// e às faturas dos meses seguintes (decisão do dono). Piso: fim do mês corrente.
pub(crate) async fn forecast_horizon_end(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<NaiveDate, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let max_txn: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM \"transaction\" WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon txn: {e}"))?;
    let max_bal: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM sheet_daily_balance WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon bal: {e}"))?;

    let mut horizon = forecast::last_day_of_month(today_naive.year(), today_naive.month());
    for (candidate,) in [max_txn, max_bal] {
        if let Some(date_str) = candidate
            && let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            && d > horizon
        {
            horizon = d;
        }
    }
    Ok(horizon)
}

/// Loads forward cashflow events for the projection window: future transactions (date > today,
/// avoiding double-counting today's already-realized spending baked into the balance snapshot).
/// Credit bills are already carried as a single outflow on the due date by these transaction rows.
/// Single source of row→event mapping, shared by `dashboard_summary` and `forecast_dto`.
pub(crate) async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let horizon = horizon_end.format("%Y-%m-%d").to_string();

    // Liquidez da conta-destino entra no SELECT para classificar `transfer` → Economia (guardar
    // num bolso não-líquido) vs net-zero (entre contas líquidas).
    //
    // SEM filtro por tag-excluída: esta é a visão de CAIXA (encadeamento do Saldo projetado,
    // déficit mais profundo, Horizonte). Tags "Ignorar" suprimem só as MÉTRICAS (Performance/Custo
    // de vida) — o dinheiro de um gasto futuro marcado "Ignorar" AINDA sai da conta, então tem que
    // continuar pesando no Saldo. O filtro `exclude_from_totals` vive nas funções de MÉTRICA
    // (`load_realized_month_events`, `load_year_events`, `realized_annual_economia`), não aqui.
    let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?1 AND t.date <= ?2",
    )
    .bind(&today)
    .bind(&horizon)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    let all_events: Vec<CashflowEvent> =
        txn_rows.into_iter().filter_map(map_cashflow_row).collect();

    Ok(all_events)
}

/// Eventos que alimentam projeções de caixa: lançamentos reais/futuros + Diário típico futuro do
/// mês corrente. Usado por `forecast_dto` e `dashboard_summary` para manter o saldo projetado
/// idêntico em todas as telas.
pub(crate) async fn load_forecast_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let mut events = load_cashflow_events(pool, today_naive, horizon_end).await?;
    // Previsão de diário como DRIVER: injeta o teto/dia nos dias futuros do mês corrente, para o
    // saldo projetado e a Performance não nascerem otimistas (assumem o gasto típico até o fim do mês).
    let daily_ceiling = effective_daily_ceiling(pool, today_naive).await?;
    let days_with_daily: std::collections::HashSet<NaiveDate> = events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    events.extend(forecast::project_daily_ceiling(
        daily_ceiling,
        today_naive,
        horizon_end,
        &days_with_daily,
    ));
    Ok(events)
}

/// Eventos JÁ REALIZADOS do mês corrente (`month_start..=today`), classificados como os futuros.
/// O encadeamento de caixa não os usa (a semente já os embute), mas a performance do mês precisa
/// deles — senão o mês corrente aparece pela metade (review adversarial P0). Só transações; os
/// lumps de fatura realizados deste mês já estão na coluna Saída da planilha como transação.
pub(crate) async fn load_realized_month_events(
    pool: &SqlitePool,
    month_start: NaiveDate,
    today_naive: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let start = month_start.format("%Y-%m-%d").to_string();
    let today = today_naive.format("%Y-%m-%d").to_string();

    let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date <= ?2 \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&start)
    .bind(&today)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query realized month: {e}"))?;

    Ok(rows.into_iter().filter_map(map_cashflow_row).collect())
}

/// Eventos para as MÉTRICAS por mês = futuros (encadeamento) + realizados do mês corrente.
/// Cobre o mês inteiro de hoje (realizado + projetado); meses à frente já são todos futuros.
pub(crate) async fn load_metric_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    future_events: &[CashflowEvent],
) -> Result<Vec<CashflowEvent>, String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data de hoje inválida")?;
    let mut metric = load_realized_month_events(pool, month_start, today_naive).await?;
    metric.extend_from_slice(future_events);
    Ok(metric)
}

#[derive(serde::Serialize)]
pub struct ForecastDayDto {
    pub date: String,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub economia_cents: i64,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct DayPointDto {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthEndDto {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthMetricDto {
    pub year: i32,
    pub month: u32,
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas fixas realizadas (coluna Saída; cartão entra como lump). Para o rodapé ENTRADAS|SAÍDAS|DIÁRIO.
    pub fixed_out_cents: i64,
    /// Diário realizado (coluna Diário). `cost_of_living = fixed_out + daily_out`.
    pub daily_out_cents: i64,
    /// Diário médio do mês = Σ diário realizado ÷ dias decorridos (D/N). Antes morria no DTO.
    pub real_daily_avg_cents: i64,
    /// Economia lançada no mês (numerador do Economizado%).
    pub economia_cents: i64,
    pub savings_rate_bps: i64,
}

/// Poupança do ano: realizada (honesta) vs projetada (otimista quando o futuro está incompleto).
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" exibido no Neko; `registered_economia_cents` é a Economia
/// REGISTRADA do método (transfers→reserva), numerador do Economizado%. O guardrail de poupança
/// usa a Economia registrada; o net só aparece como exibição do colchão.
#[derive(serde::Serialize)]
pub struct AnnualSavingsDto {
    pub realized_income_cents: i64,
    pub realized_savings_cents: i64,
    pub realized_rate_bps: i64,
    /// Economia REGISTRADA do ano (transfers→reserva/ilíquido), meses completos. Distinta do net.
    pub registered_economia_cents: i64,
    pub projected_income_cents: i64,
    pub projected_savings_cents: i64,
    pub projected_rate_bps: i64,
    pub target_bps: i64,
}

/// Cobertura de um mês futuro (quanto do gasto típico já está lançado).
#[derive(serde::Serialize)]
pub struct MonthCoverageDto {
    pub year: i32,
    pub month: u32,
    pub projected_outflow_cents: i64,
    pub baseline_outflow_cents: i64,
    pub coverage_bps: i64,
    pub is_complete: bool,
    pub estimated_missing_cents: i64,
}

#[derive(serde::Serialize)]
pub struct ForecastDto {
    pub today: String,
    pub horizon_end: String,
    /// Poupança do ano — realizada vs projetada (previsibilidade).
    pub annual_savings: AnnualSavingsDto,
    /// Cobertura por mês futuro (vazio se a projeção está completa).
    pub coverage: Vec<MonthCoverageDto>,
    /// Gasto típico/mês (mediana realizada). `0` = sem histórico → previsibilidade indeterminada.
    pub baseline_outflow_cents: i64,
    /// Último mês cuja projeção é confiável ("YYYY-MM"); `null` se não há baseline para avaliar.
    pub trusted_through_month: Option<String>,
    /// Soma do que falta lançar nos meses incompletos (fatura + variáveis).
    pub total_missing_cents: i64,
    /// "Pode gastar hoje" honesto: o MAIS APERTADO de caixa × poupança (guardrail duplo).
    pub safe_to_spend_today_cents: i64,
    /// Folga de caixa (menor saldo projetado no horizonte − piso de reserva).
    pub cash_headroom_cents: i64,
    /// Folga da meta de poupança do mês corrente (negativa = já abaixo da meta). `null` quando a
    /// régua de poupança está inativa (mês sem renda) → só o caixa decide.
    pub savings_headroom_cents: Option<i64>,
    /// Qual régua limita: "cash" ou "savings".
    pub binding_guardrail: String,
    /// Meta de poupança em basis points (2500 = 25%).
    pub savings_target_bps: i64,
    pub deepest_deficit: Option<DayPointDto>,
    pub daily: Vec<ForecastDayDto>,
    pub month_end: Vec<MonthEndDto>,
    /// Performance/poupança por mês (Caixa ≠ Performance; expõe meses futuros magros).
    pub months: Vec<MonthMetricDto>,
}

#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String> {
    forecast_dto(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation with an injected `today` (deterministic, integration-testable).
/// Maps the pure engine output to ISO-8601-string DTOs; the core stays serde-free.
pub(crate) async fn forecast_dto(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<ForecastDto, String> {
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let seed = projection_seed(pool, today_naive).await?;
    let events = load_forecast_events(pool, today_naive, horizon_end).await?;
    let metric_events = load_metric_events(pool, today_naive, &events).await?;
    let fc =
        forecast::project_with_metrics(seed, today_naive, &events, &metric_events, horizon_end);

    let reserve_floor_cents = reserve_floor(pool, today_naive).await?;
    // Poupança ANUAL realizada (não o mês isolado, não o ano projetado-incompleto).
    let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
    // Economia REGISTRADA do ano (transfers→reserva): numerador do Economizado% e entrada do
    // guardrail de poupança. O net `annual_savings_amt` é só exibição do colchão (não decide).
    let annual_economia = realized_annual_economia(pool, today_naive).await?;
    let sts = forecast::safe_to_spend_today(
        &fc,
        annual_income,
        annual_economia,
        SAVINGS_TARGET_BPS,
        reserve_floor_cents,
    );
    let binding_guardrail = match sts.binding {
        forecast::Guardrail::Cash => "cash",
        forecast::Guardrail::Savings => "savings",
    }
    .to_string();

    // Previsibilidade: poupança realizada vs projetada + cobertura dos meses futuros.
    let (proj_income, proj_savings) = projected_annual_savings(pool, today_naive).await?;
    // Taxa em bps para EXIBIÇÃO (round half-up, não trunca — senão 25,00% vira 2499/abaixo da
    // meta; review P3). Nunca usada em decisão (o guardrail compara centavos diretos).
    let rate_bps = |save: i64, inc: i64| {
        if inc > 0 {
            (save * 10_000 + inc / 2) / inc
        } else {
            0
        }
    };
    let annual_savings = AnnualSavingsDto {
        realized_income_cents: annual_income,
        realized_savings_cents: annual_savings_amt,
        realized_rate_bps: rate_bps(annual_savings_amt, annual_income),
        registered_economia_cents: annual_economia,
        projected_income_cents: proj_income,
        projected_savings_cents: proj_savings,
        projected_rate_bps: rate_bps(proj_savings, proj_income),
        target_bps: SAVINGS_TARGET_BPS,
    };

    let baseline = realized_monthly_baseline(pool, today_naive).await?;
    let coverage_raw =
        forecast::month_coverage(&fc.months, today_naive, baseline, COVERAGE_COMPLETE_BPS);
    // Sem baseline (nenhum mês realizado) não dá para afirmar "confiável até X" → `None`. Com
    // baseline, o mês corrente é sempre confiável (tem o realizado) e estende pelos meses futuros
    // completos até o primeiro incompleto.
    let trusted_through_month = if baseline <= 0 {
        None
    } else {
        let mut trusted = format!("{:04}-{:02}", today_naive.year(), today_naive.month());
        for c in coverage_raw.iter() {
            if c.is_complete {
                trusted = format!("{:04}-{:02}", c.year, c.month);
            } else {
                break;
            }
        }
        Some(trusted)
    };
    let total_missing_cents = coverage_raw
        .iter()
        .filter(|c| !c.is_complete)
        .map(|c| c.estimated_missing_cents)
        .sum();
    let coverage: Vec<MonthCoverageDto> = coverage_raw
        .iter()
        .map(|c| MonthCoverageDto {
            year: c.year,
            month: c.month,
            projected_outflow_cents: c.projected_outflow_cents,
            baseline_outflow_cents: c.baseline_outflow_cents,
            coverage_bps: c.coverage_bps,
            is_complete: c.is_complete,
            estimated_missing_cents: c.estimated_missing_cents,
        })
        .collect();

    // Per-day flow sums (income, fixed out, daily out), keyed by the same dates the engine emits.
    let mut flows: std::collections::HashMap<NaiveDate, (i64, i64, i64, i64)> =
        std::collections::HashMap::new();
    for e in &events {
        let entry = flows.entry(e.date).or_default();
        match e.kind {
            forecast::EventKind::Income => entry.0 += e.amount_cents,
            forecast::EventKind::FixedOut => entry.1 += e.amount_cents,
            forecast::EventKind::Daily => entry.2 += e.amount_cents,
            forecast::EventKind::Economia => entry.3 += e.amount_cents,
        }
    }

    let daily = fc
        .daily
        .iter()
        .map(|p| {
            let (income, fixed_out, daily_out, economia) =
                flows.get(&p.date).copied().unwrap_or_default();
            ForecastDayDto {
                date: p.date.format("%Y-%m-%d").to_string(),
                income_cents: income,
                fixed_out_cents: fixed_out,
                daily_out_cents: daily_out,
                economia_cents: economia,
                balance_cents: p.balance_cents,
            }
        })
        .collect();

    Ok(ForecastDto {
        today: today_naive.format("%Y-%m-%d").to_string(),
        horizon_end: horizon_end.format("%Y-%m-%d").to_string(),
        annual_savings,
        coverage,
        baseline_outflow_cents: baseline,
        trusted_through_month,
        total_missing_cents,
        safe_to_spend_today_cents: sts.amount_cents,
        cash_headroom_cents: sts.cash_headroom_cents,
        savings_headroom_cents: sts.savings_headroom_cents,
        binding_guardrail,
        savings_target_bps: SAVINGS_TARGET_BPS,
        deepest_deficit: fc.deepest_deficit.as_ref().map(|p| DayPointDto {
            date: p.date.format("%Y-%m-%d").to_string(),
            balance_cents: p.balance_cents,
        }),
        daily,
        month_end: fc
            .month_end
            .iter()
            .map(|m| MonthEndDto {
                year: m.year,
                month: m.month,
                balance_cents: m.balance_cents,
            })
            .collect(),
        months: fc
            .months
            .iter()
            .map(|m| MonthMetricDto {
                year: m.year,
                month: m.month,
                income_cents: m.income_cents,
                performance_cents: m.performance_cents,
                cost_of_living_cents: m.cost_of_living_cents,
                fixed_out_cents: m.fixed_out_cents,
                daily_out_cents: m.daily_out_cents,
                real_daily_avg_cents: m.real_daily_avg_cents,
                economia_cents: m.economia_cents,
                savings_rate_bps: m.savings_rate_bps,
            })
            .collect(),
    })
}

// --- Visão anual (spec 019 month-views) ---

/// Todos os eventos do ANO (realizado + projetado), classificados — sem o teto de diário (que só
/// vale para o mês corrente no forecast). Para a visão anual das 4 métricas.
pub(crate) async fn load_year_events(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<CashflowEvent>, String> {
    let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{}-01-01", year + 1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query year events: {e}"))?;

    Ok(rows.into_iter().filter_map(map_cashflow_row).collect())
}

#[derive(serde::Serialize)]
pub struct AnnualMetricsDto {
    pub year: i32,
    pub months: Vec<MonthMetricDto>,
}

pub(crate) async fn annual_metrics(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<AnnualMetricsDto, String> {
    let events = load_year_events(pool, year).await?;
    let months: Vec<(i32, u32)> = (1..=12).map(|m| (year, m)).collect();
    let metrics = forecast::month_metrics_for(today, &events, &months);
    let months = metrics
        .iter()
        .map(|m| MonthMetricDto {
            year: m.year,
            month: m.month,
            income_cents: m.income_cents,
            performance_cents: m.performance_cents,
            cost_of_living_cents: m.cost_of_living_cents,
            fixed_out_cents: m.fixed_out_cents,
            daily_out_cents: m.daily_out_cents,
            real_daily_avg_cents: m.real_daily_avg_cents,
            economia_cents: m.economia_cents,
            savings_rate_bps: m.savings_rate_bps,
        })
        .collect();
    Ok(AnnualMetricsDto { year, months })
}

// --- Grade do mês (visão fiel à planilha: Data | Entrada | Saída | Diário | Saldo) ---

/// Um dia da grade mensal. `balance_cents` é o Saldo encadeado da planilha (None se aquele dia não
/// foi importado). Os fluxos são agregados das transações do dia, separados por tipo.
#[derive(serde::Serialize)]
pub struct MonthGridDayDto {
    pub date: String,
    pub day: u32,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub balance_cents: Option<i64>,
}

/// Grade de TODOS os dias de um mês (1..último), com os fluxos realizados/pré-lançados agregados por
/// dia e o Saldo da planilha (`sheet_daily_balance`). É a visão Data|Entrada|Saída|Diário|Saldo que o
/// usuário tem na planilha, para QUALQUER mês (passado ou futuro) — diferente do `forecast.daily`,
/// que só vai de hoje para frente. Dias sem Saldo importado vêm com `balance_cents = None`.
pub(crate) async fn month_grid(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> Result<Vec<MonthGridDayDto>, String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or("mês inválido")?;
    let last = forecast::last_day_of_month(year, month);
    let first_s = first.format("%Y-%m-%d").to_string();
    let last_s = last.format("%Y-%m-%d").to_string();

    // Fluxos por dia, separados por tipo (Entrada / Saída fixa / Diário variável).
    let flows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        // Crédito entra em Saída como a fatura (lump no vencimento), não em Diário — espelha forecast::classify.
        // ABS() POR LINHA nas saídas: importadas são gravadas negativas (`-amount_out`), manuais
        // são positivas — ambas type='expense'. Somar cru dá total de sinal misto; `SUM(ABS(..))`
        // soma as MAGNITUDES, então as duas fontes no mesmo dia se acumulam corretamente (não se
        // cancelam). Entradas ficam fora (sempre positivas). Espelha o `amount.abs()` do forecast.
        "SELECT date, \
                COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN ABS(amount) ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN ABS(amount) ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 GROUP BY date",
    )
    .bind(&first_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("month flows: {e}"))?;

    // Saldo da planilha por dia.
    let balances: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date BETWEEN ?1 AND ?2",
    )
    .bind(&first_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("month balances: {e}"))?;

    let flow_of = |d: &str| flows.iter().find(|f| f.0 == d).map(|f| (f.1, f.2, f.3));
    let balance_of = |d: &str| balances.iter().find(|b| b.0 == d).map(|b| b.1);

    let n_days = (last - first).num_days() + 1;
    let mut grid = Vec::with_capacity(n_days as usize);
    for offset in 0..n_days {
        let date = first + chrono::Duration::days(offset);
        let date_s = date.format("%Y-%m-%d").to_string();
        let (income, fixed_out, daily_out) = flow_of(&date_s).unwrap_or((0, 0, 0));
        grid.push(MonthGridDayDto {
            day: date.day(),
            income_cents: income,
            fixed_out_cents: fixed_out,
            daily_out_cents: daily_out,
            balance_cents: balance_of(&date_s),
            date: date_s,
        });
    }
    Ok(grid)
}

/// Grade do mês `year-month` (visão fiel à planilha). Ver [`month_grid`].
#[tauri::command]
pub async fn get_month_grid(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<Vec<MonthGridDayDto>, String> {
    month_grid(pool.inner(), year, month).await
}

#[tauri::command]
pub async fn get_annual_metrics(
    pool: State<'_, SqlitePool>,
    year: i32,
) -> Result<AnnualMetricsDto, String> {
    annual_metrics(pool.inner(), year, chrono::Local::now().date_naive()).await
}

// --- Dashboard query commands ---

#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
    /// Most recent date (`YYYY-MM-DD`) of a non-projection transaction the user logged.
    /// `None` when no real transactions exist yet.
    pub last_real_tx_date: Option<String>,
}

#[tauri::command]
pub async fn get_dashboard_summary(
    pool: State<'_, SqlitePool>,
) -> Result<DashboardSummary, String> {
    dashboard_summary(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation: takes `&SqlitePool` and an injected `today`, so it is deterministic and
/// integration-testable without Tauri `State` or the ambient clock.
pub(crate) async fn dashboard_summary(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<DashboardSummary, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    // Seed + forward events: shared with `forecast_dto` (single source of event mapping).
    let seed = projection_seed(pool, today_naive).await?;
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let all_events = load_forecast_events(pool, today_naive, horizon_end).await?;

    // `balance` is the projected end-of-current-month figure (the method's hero, spec 003 US8),
    // not the raw current account sum.
    let fc = forecast::project(seed, today_naive, &all_events, horizon_end);
    let projected_balance = fc
        .month_end
        .iter()
        .find(|m| m.year == today_naive.year() && m.month == today_naive.month())
        .map(|m| m.balance_cents)
        .or_else(|| fc.daily.last().map(|p| p.balance_cents))
        .unwrap_or(seed);

    // Teto do diário exibido no tile "Diário de hoje" (`de R$X`): orçamento explícito ativo, senão
    // o Diário médio do mês anterior — mesma fonte do driver de projeção do forecast (consistência).
    let daily_budget = effective_daily_ceiling(pool, today_naive).await?;

    // Diário de HOJE como MAGNITUDE positiva (o card faz `teto - gasto` e `gasto/teto`).
    // - Sinal: por convenção, `amount` é gravado como magnitude positiva (import faz `.abs()`,
    //   `create_transaction` exige `> 0`); o sinal vem do `type`. `ABS()` é defesa-em-profundidade,
    //   espelhando o `amount.abs()` do forecast — robusto caso algum writer grave com sinal.
    // - Fonte única: o gasto do dia vem das transações Diário (despesa variável não-crédito);
    //   sem nenhuma transação no dia, a soma é 0. O ritual diário é uma transação Diário comum.
    let daily_spend: (i64,) = sqlx::query_as(
        "SELECT ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                              WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                                AND (payment_method IS NULL OR payment_method <> 'credit')), 0))",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query daily spend: {e}"))?;

    // Reserva em MESES de custo de vida (método): saldo das contas de reserva ÷ custo de vida mensal.
    // Custo de vida mensal = mediana das saídas dos últimos meses completos (realized_monthly_baseline
    // = fixas + diário + cartão). A tabela `reserve.current_months` não tem writer de produção (só seed
    // de teste), então derivamos ao vivo dos dados importados — espelha os R$ que o PocketsCard mostra.
    // `trend` permanece da tabela/snapshot (default 'flat' enquanto não há histórico).
    let reserve_balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query reserve balance: {e}"))?;
    let reserve_baseline = realized_monthly_baseline(pool, today_naive).await?;
    let reserve_months = if reserve_baseline > 0 {
        reserve_balance.0 as f64 / reserve_baseline as f64
    } else {
        0.0
    };
    let reserve_trend: (String,) = sqlx::query_as(
        "SELECT COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve trend: {e}"))?
    .unwrap_or(("flat".to_string(),));

    // Transações já realizadas: por DATA (≤ hoje), não pelo `is_projection` congelado (stale
    // quando o dono não re-importa por dias — auditoria de robustez a edições).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date <= ?1")
        .bind(&today)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("query: {e}"))?;

    // Data do lançamento REAL mais recente (não-projeção, ≤ hoje) — alimenta o aviso "lançou
    // pela última vez há X dias" do dashboard. NULL quando ainda não há lançamentos reais.
    let last_real: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT MAX(date) FROM \"transaction\" WHERE is_projection = 0 AND date <= ?1",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query last_real_tx_date: {e}"))?;
    let last_real_tx_date = last_real.and_then(|(d,)| d);

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        daily_spend_today: daily_spend.0,
        reserve_months,
        reserve_trend: reserve_trend.0,
        transaction_count: count.0,
        last_real_tx_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn insert_expense(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES (?1, 'expense', ?2, ?3, 1)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Liga uma tag `exclude_from_totals = 1` ("Ignorar") a um lançamento.
    async fn tag_as_excluded(pool: &SqlitePool, txn_id: &str) {
        sqlx::query(
            "INSERT INTO tag (id, name, exclude_from_totals) VALUES ('tg-ignore', 'Ignorar', 1)",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, 'tg-ignore')",
        )
        .bind(txn_id)
        .execute(pool)
        .await
        .unwrap();
    }

    // Bug 1 (plano 037): uma despesa FUTURA marcada com tag "Ignorar" (exclude_from_totals)
    // continua pesando no Saldo PROJETADO — o dinheiro vai sair da conta de qualquer forma. A tag
    // só suprime as MÉTRICAS (Performance/Custo de vida), nunca a visão de CAIXA. O bug do 034 era
    // o filtro `NOT EXISTS` em `load_cashflow_events` (a fonte do encadeamento do Saldo), que sumia
    // com o gasto futuro do Saldo projetado. Este teste guarda os DOIS lados: caixa inclui, métrica
    // exclui.
    #[tokio::test]
    async fn excluded_tag_expense_still_lowers_projected_balance() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let tomorrow = today.succ_opt().unwrap();
        let horizon = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();

        // Despesa FUTURA marcada "Ignorar".
        insert_expense(
            &p,
            "fut-ign",
            5000,
            &tomorrow.format("%Y-%m-%d").to_string(),
        )
        .await;
        tag_as_excluded(&p, "fut-ign").await;

        // Lado CAIXA: a despesa NÃO é filtrada — aparece na trajetória do Saldo projetado.
        let cash = load_cashflow_events(&p, today, horizon).await.unwrap();
        assert_eq!(cash.len(), 1, "o gasto futuro 'Ignorar' continua no caixa");
        assert_eq!(cash[0].kind, forecast::EventKind::Daily);
        assert_eq!(cash[0].amount_cents, 5000);

        // Lado MÉTRICA: uma despesa REALIZADA "Ignorar" do mês corrente É excluída (filtro
        // intencional preservado em `load_realized_month_events`). Guarda contra remover o filtro
        // da função errada.
        let month_start = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        insert_expense(&p, "past-ign", 7000, "2026-06-10").await;
        sqlx::query(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES ('past-ign', 'tg-ignore')",
        )
        .execute(&p)
        .await
        .unwrap();
        let metric = load_realized_month_events(&p, month_start, today)
            .await
            .unwrap();
        assert!(
            metric.is_empty(),
            "a despesa realizada 'Ignorar' some das MÉTRICAS"
        );
    }

    // Bug 3 (plano 037): em 1º de JANEIRO `realized_annual_economia` precisa da MESMA janela
    // deslocada para DEZEMBRO que `realized_annual_savings` usa — senão o guardrail compara renda de
    // dezembro contra Economia = 0 (janela vazia do ano novo). Mantém as duas funções SIMÉTRICAS.
    #[tokio::test]
    async fn jan1_economia_uses_prior_december_window() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        // Conta de reserva (destino da Economia) + renda de dezembro do ano anterior.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-res', 'Reserva', 'savings', 'pe-1', 0, 'reserve')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('econ-dez', 'transfer', 20000, '2025-12-15', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('renda-dez', 'income', 100000, '2025-12-10', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let economia = realized_annual_economia(&p, today).await.unwrap();
        assert_eq!(
            economia, 20000,
            "em 1º/jan a Economia usa a janela de dezembro (não 0)"
        );

        // Simetria: a poupança também enxerga dezembro (renda > 0), confirmando que ambas usam a
        // mesma janela na virada do ano.
        let (income, _savings) = realized_annual_savings(&p, today).await.unwrap();
        assert_eq!(
            income, 100000,
            "guardrail simétrico: renda de dezembro também é vista em 1º/jan"
        );
    }

    #[tokio::test]
    async fn month_grid_expense_total_is_magnitude_regardless_of_sign() {
        // Bug 4: imported expenses are stored negative (-amount_out); manual are positive.
        // month_grid must return the magnitude (ABS) so both sources add up correctly.
        let p = pool().await;

        // Simulate an imported expense (negative amount, is_fixed=1 = Saída).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES ('imp-exp', 'expense', -150000, '2026-03-15', 1, 0, '2026-03-15T00:00:00Z', '2026-03-15T00:00:00Z')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Simulate a manual expense (positive amount, is_fixed=1 = Saída).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, created_at, updated_at) \
             VALUES ('man-exp', 'expense', 80000, '2026-03-15', 1, 0, '2026-03-15T00:00:00Z', '2026-03-15T00:00:00Z')",
        )
        .execute(&p)
        .await
        .unwrap();

        let grid = month_grid(&p, 2026, 3).await.unwrap();
        let day15 = grid.iter().find(|d| d.date == "2026-03-15").unwrap();

        // fixed_out must be the sum of magnitudes: 150_000 + 80_000 = 230_000.
        // Before fix: -150_000 + 80_000 = -70_000 (wrong sign, wrong value).
        assert_eq!(
            day15.fixed_out_cents, 230_000,
            "month_grid fixed_out must be magnitude regardless of storage sign (Bug 4)"
        );
        assert_eq!(day15.daily_out_cents, 0);
        assert_eq!(day15.income_cents, 0);
    }

    // --- Plano 045: quebra por categoria do orçamento Diário ---

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
        upsert_daily_budget_with_categories_inner(&p, 125000, &cats)
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
        upsert_daily_budget_with_categories_inner(&p, 125000, &[cat("Shopping", 125000, 0)])
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
        upsert_daily_budget_with_categories_inner(&p, 60000, &[])
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
        upsert_daily_budget_with_categories_inner(&p, 100000, &[cat("Groceries", 100000, 0)])
            .await
            .unwrap();
        // Segunda chamada depreca o orçamento anterior e cria nova quebra no novo orçamento ativo.
        upsert_daily_budget_with_categories_inner(&p, 80000, &[cat("Transport", 80000, 0)])
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
        let err = upsert_daily_budget_with_categories_inner(&p, 50000, &[cat("Bad", 0, 0)])
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
    async fn get_daily_budget_categories_returns_empty_without_budget() {
        let p = pool().await;
        // Sem orçamento ativo → vetor vazio (não-panic).
        let rows = get_daily_budget_categories_inner(&p).await.unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn monthly_to_daily_rate_divides_correctly() {
        assert_eq!(monthly_to_daily_rate(3100, 31), 100);
        assert_eq!(monthly_to_daily_rate(125000, 31), 4032); // 4032,25 truncado
        assert_eq!(monthly_to_daily_rate(100, 0), 0, "dias=0 não causa panic");
    }
}
