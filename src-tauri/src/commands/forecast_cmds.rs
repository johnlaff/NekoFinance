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

/// Meta de poupança do método: piso de 25% (faixa 20–30%, MÉDIA ANUAL — o ano todo deve ficar
/// na faixa, os meses variam). Régua do guardrail ANUAL "pode gastar".
/// O badge MENSAL "Dentro do ideal" (src/screens/TotaisScreen.tsx) usa 20% (piso da faixa), por ser
/// leniente a variações de um mês; ambos ficam dentro da faixa canônica 20–30%.
pub(crate) const SAVINGS_TARGET_BPS: i64 = 2500;

/// Limiar de cobertura: um mês futuro com menos de 60% do gasto típico já lançado é tratado como
/// INCOMPLETO (projeção otimista demais — o "chá revelação" do método). Margem ampla porque o
/// método aceita variação mês a mês; abaixo disso é quase certo que falta fatura/variável.
pub(crate) const COVERAGE_COMPLETE_BPS: i64 = 6_000;

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
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date < ?2 \
           AND type IN ('income','expense')",
    )
    .bind(&year_start)
    .bind(format!("{cur_ym}-01")) // 1º dia do mês corrente: `date < 'YYYY-MM-01'` ≡ `substr(date,1,7) < 'YYYY-MM'` p/ ISO
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
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid')",
    )
    .bind(&year_start)
    .bind(format!("{cur_ym}-01")) // 1º dia do mês corrente: range ≡ `substr(date,1,7) < 'YYYY-MM'` p/ ISO
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

/// Piso de reserva = colchão intocável que a folga de caixa não pode comer. Por ora = soma dos
/// Bolsos marcados como reserva (spec 007, `liquidity = 'reserve'`); esses NÃO entram na semente
/// líquida, então subtraí-los aqui não dobra. O ideal metodológico (custo de vida × 12) fica
/// para quando a reserva for modelada como meta — ver limitações na spec 010. Hoje, sem reserva
/// configurada, retorna 0 e a régua de poupança é a que morde.
pub(crate) async fn reserve_floor(pool: &SqlitePool) -> Result<i64, String> {
    let floor: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor: {e}"))?;
    Ok(floor.0)
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
         WHERE t.date >= ?1 AND t.date <= ?2",
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

    let reserve_floor_cents = reserve_floor(pool).await?;
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
         WHERE t.date >= ?1 AND t.date < ?2",
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
        "SELECT date, \
                COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN amount ELSE 0 END), 0) \
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
