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

// --- Forecast projection ---

/// Sum of liquid cash accounts — the projection seed.
/// Only `liquidity = 'liquid'` pockets are cash; reserve/restricted/illiquid
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
/// Sem planilha importada, cai nos Bolsos líquidos. Precedência: planilha > bolsos —
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
         FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense','transfer') \
           AND scenario_id IS NULL",
    )
    .bind(&seed_date)
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("seed gap: {e}"))?;

    Ok(balance + gap.0)
}

/// A faixa do método e o piso da reserva vivem no motor, com as réguas que os aplicam; a casca
/// os reexporta para que os DTOs publiquem os mesmos limiares que julgam.
pub(crate) use crate::forecast::{
    RESERVE_MIN_MONTHS, SAVINGS_CEILING_BPS, SAVINGS_FLOOR_BPS, SAVINGS_TARGET_BPS,
};

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
    // Renda-base do guardrail 20–30%: soma de `income_cents` (view Economia) dos meses que
    // `guardrail_window` devolve — o mesmo motor que já sustenta Economia/Patrimônio anuais
    // (`realized_annual_economia`/`realized_annual_patrimonio`). Nenhuma janela própria aqui.
    let window_metrics = guardrail_window_metrics(pool, today_naive).await?;
    let income_savings: i64 = window_metrics.iter().map(|m| m.income_cents).sum();

    // O net/colchão (view Performance) ainda lê a `transaction` crua — a régua de Performance
    // não é MonthMetric-agregável do mesmo jeito por rodar com a projeção do dia. A janela de
    // datas, porém, vem só dos meses que `guardrail_window` já elegeu — sem redefinir "mês
    // completo" nem o deslocamento de janeiro aqui.
    let window = forecast::guardrail_window(today_naive);
    let (&(first_year, first_month), &(last_year, last_month)) =
        match (window.first(), window.last()) {
            (Some(f), Some(l)) => (f, l),
            _ => return Ok((income_savings, 0)),
        };
    let lower = NaiveDate::from_ymd_opt(first_year, first_month, 1)
        .expect("valid month")
        .format("%Y-%m-%d")
        .to_string();
    let upper = (NaiveDate::from_ymd_opt(last_year, last_month, 1).expect("valid month")
        + chrono::Months::new(1))
    .format("%Y-%m-%d")
    .to_string();

    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN t.type='income' \
             AND NOT EXISTS (SELECT 1 FROM transaction_tag tt2 JOIN tag tg ON tg.id = tt2.tag_id \
                 WHERE tt2.transaction_id = t.id AND tg.exclude_from_performance = 1) \
             THEN t.amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN t.type='expense' \
             AND NOT EXISTS (SELECT 1 FROM transaction_tag tt2 JOIN tag tg ON tg.id = tt2.tag_id \
                 WHERE tt2.transaction_id = t.id AND tg.exclude_from_performance = 1) \
             THEN t.amount ELSE 0 END), 0) \
         FROM \"transaction\" t WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type IN ('income','expense') AND t.scenario_id IS NULL",
    )
    .bind(&lower)
    .bind(&upper)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized annual: {e}"))?;
    let (income_perf, expense_perf) = row;
    Ok((income_savings, income_perf - expense_perf)) // (renda-base, net colchão) dos meses completos
}

/// Os meses `MonthMetric` que compõem a janela do guardrail: `annual_month_metrics` do ano certo
/// (o corrente, ou o anterior quando `guardrail_window` desloca para dezembro em janeiro),
/// filtrado pela lista que a janela devolve. Única leitura por trás das duas figuras anuais
/// irmãs — Economia e Patrimônio já vêm classificadas, mascaradas por tag e reconciliadas com a
/// anotação da aba, porque `annual_month_metrics` é o mesmo motor que a tela do Ano usa.
async fn guardrail_window_metrics(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<Vec<forecast::MonthMetric>, String> {
    let window = forecast::guardrail_window(today_naive);
    let Some(&(year, _)) = window.first() else {
        return Ok(Vec::new());
    };
    let metrics = annual_month_metrics(pool, year, today_naive).await?;
    Ok(metrics
        .into_iter()
        .filter(|m| window.contains(&(m.year, m.month)))
        .collect())
}

/// Economia REGISTRADA do ano até hoje (meses completos que `guardrail_window` devolve). É o
/// numerador do "Economizado" do método (Economia/Entradas), DISTINTO do net superávit de
/// `realized_annual_savings` (que é o "colchão" do Neko). Existir os dois lado a lado sem se
/// confundir foi um achado da review.
pub(crate) async fn realized_annual_economia(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let metrics = guardrail_window_metrics(pool, today_naive).await?;
    Ok(metrics.iter().map(|m| m.economia_cents).sum())
}

/// Patrimônio REALIZADO do ano, na MESMA janela de meses completos de `realized_annual_economia`.
/// Publicado ao lado da régua, nunca somado a ela: patrimônio é classificação, não Economia, em
/// qualquer cobertura de reserva.
pub(crate) async fn realized_annual_patrimonio(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let metrics = guardrail_window_metrics(pool, today_naive).await?;
    Ok(metrics.iter().map(|m| m.patrimonio_cents).sum())
}

/// Sinais do modo de gasto + insumos do re-roteamento do dia no modo cartão. A janela é a da
/// detecção (2 meses completos + corrente); os campos de fatura olham do mês corrente ao fim do
/// mês SEGUINTE (o próximo vencimento pode cair na virada).
pub(crate) struct SpendingModeSummary {
    pub mode: forecast::SpendingMode,
    /// `false` quando o modo é o default de dado insuficiente, não uma leitura da janela.
    pub detected: bool,
    /// Cartão do mês corrente (realizado + projetado), magnitude.
    pub cartao_month_cents: i64,
    /// Próximo dia de fatura (evento Cartão) a partir de hoje: (data ISO, total do dia).
    pub next_fatura: Option<(String, i64)>,
}

pub(crate) async fn spending_mode_summary(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<SpendingModeSummary, String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1).unwrap();
    let window_start = month_start - chrono::Months::new(2);
    let next_month_end = month_start + chrono::Months::new(2);
    let events = load_metric_db_events(pool, today_naive, window_start, next_month_end).await?;

    let window_keys = [
        window_start.format("%Y-%m").to_string(),
        (month_start - chrono::Months::new(1))
            .format("%Y-%m")
            .to_string(),
        month_start.format("%Y-%m").to_string(),
    ];
    let cur_ym = window_keys[2].clone();

    let mut samples = [forecast::MonthSpendSample {
        daily_days: 0,
        daily_total_cents: 0,
        cartao_present: false,
    }; 3];
    let mut daily_dates: [std::collections::HashSet<NaiveDate>; 3] = Default::default();
    let mut cartao_month_cents = 0i64;
    let mut next_fatura_by_date: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();

    // Detecção de modo é pergunta de FORMA-do-gasto → view Custo de vida: um lançamento excluído
    // dessa régua não conta como diário/cartão para o modo (preserva o comportamento do backfill).
    for me in events.iter().filter(|me| me.mask.cost_of_living) {
        let e = &me.event;
        let ym = e.date.format("%Y-%m").to_string();
        let slot = window_keys.iter().position(|k| *k == ym);
        match e.kind {
            forecast::EventKind::Daily => {
                // Realização decidida pela DATA (≤ hoje), não pelo `is_projection` congelado.
                if let Some(i) = slot
                    && e.date <= today_naive
                {
                    samples[i].daily_total_cents += e.amount_cents.abs();
                    daily_dates[i].insert(e.date);
                }
            }
            forecast::EventKind::Cartao => {
                if let Some(i) = slot {
                    samples[i].cartao_present = true;
                }
                if ym == cur_ym {
                    cartao_month_cents += e.amount_cents.abs();
                }
                if e.date >= today_naive {
                    *next_fatura_by_date
                        .entry(e.date.format("%Y-%m-%d").to_string())
                        .or_insert(0) += e.amount_cents.abs();
                }
            }
            _ => {}
        }
    }
    for (i, dates) in daily_dates.iter().enumerate() {
        samples[i].daily_days = dates.len() as u32;
    }

    // O sinal de cartão vem da planilha, não do evento já resolvido: sem isso, quem gasta tudo no
    // crédito e ainda não cadastrou o cartão cai no fallback e é lido como usuário de débito.
    let declared =
        declared_card_months(pool, window_start, month_start + chrono::Months::new(1)).await?;
    for (i, key) in window_keys.iter().enumerate() {
        samples[i].cartao_present |= declared.contains(key);
    }

    Ok(SpendingModeSummary {
        mode: forecast::detect_spending_mode(&samples),
        detected: forecast::spending_mode_is_detected(&samples),
        cartao_month_cents,
        next_fatura: next_fatura_by_date.into_iter().next(),
    })
}

/// Anotação da aba Economia (`economia_annotation`) para os ANOS informados, indexada por
/// `(ano, mês)` em centavos. Alimenta `month_metrics_for`/`project_with_metrics`, que reconcilia a
/// anotação com a Economia derivada usando o maior valor do mês para evitar dupla contagem.
pub(crate) async fn load_economia_annotation(
    pool: &SqlitePool,
    years: &[i32],
) -> Result<std::collections::HashMap<(i32, u32), i64>, String> {
    let mut out = std::collections::HashMap::new();
    for &year in years {
        let rows: Vec<(i64, i64)> =
            sqlx::query_as("SELECT month, amount_cents FROM economia_annotation WHERE year = ?1")
                .bind(year as i64)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("annotation for year {year}: {e}"))?;
        for (m, cents) in rows {
            if (1..=12).contains(&m) {
                out.insert((year, m as u32), cents);
            }
        }
    }
    Ok(out)
}

/// Renda e net do ANO INTEIRO projetado (todas as linhas do ano). Exibido só como contraste com
/// o realizado — é OTIMISTA quando os meses futuros estão incompletos (não usar no guardrail).
pub(crate) async fn projected_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    // Range explícito (não `LIKE 'YYYY%'`) — consistente com o realizado e rejeita data
    // malformada que começa com o ano mas não é ISO válida.
    let start = format!("{}-01-01", today_naive.year());
    let end = format!("{}-12-31", today_naive.year());
    // Réguas por perna iguais às de `realized_annual_savings`: renda-base → Economia
    // (`exclude_from_savings`); net → Performance (`exclude_from_performance`).
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN t.type='income' \
             AND NOT EXISTS (SELECT 1 FROM transaction_tag tt2 JOIN tag tg ON tg.id = tt2.tag_id \
                 WHERE tt2.transaction_id = t.id AND tg.exclude_from_savings = 1) \
             THEN t.amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN t.type='income' \
             AND NOT EXISTS (SELECT 1 FROM transaction_tag tt2 JOIN tag tg ON tg.id = tt2.tag_id \
                 WHERE tt2.transaction_id = t.id AND tg.exclude_from_performance = 1) \
             THEN t.amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN t.type='expense' \
             AND NOT EXISTS (SELECT 1 FROM transaction_tag tt2 JOIN tag tg ON tg.id = tt2.tag_id \
                 WHERE tt2.transaction_id = t.id AND tg.exclude_from_performance = 1) \
             THEN t.amount ELSE 0 END), 0) \
         FROM \"transaction\" t WHERE t.date >= ?1 AND t.date <= ?2 \
           AND t.type IN ('income','expense') AND t.scenario_id IS NULL",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("projected annual: {e}"))?;
    let (income_savings, income_perf, expense_perf) = row;
    Ok((income_savings, income_perf - expense_perf))
}

/// Gasto típico de um mês = MEDIANA do CUSTO DE VIDA (fixas + diário + cartão, classificado por
/// item de nota) dos meses realizados COMPLETOS, na janela dos **últimos 6 meses de calendário**
/// (recentes representam melhor o padrão atual que meses antigos — review ui-vs-planilha).
/// Mediana para ser robusta a um mês atípico. Itens ECONOMIA/INVESTIMENTO aninhados numa célula
/// de Saída ficam FORA: são poupança, não custo — senão o piso de reserva, a cobertura de meses
/// futuros e o `reserve_months` do dashboard inflam com dinheiro guardado.
pub(crate) async fn realized_monthly_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    Ok(realized_monthly_baseline_detail(pool, today_naive).await?.0)
}

/// Como [`realized_monthly_baseline`], mas devolve também QUANTOS meses sustentam a mediana —
/// o que separa o veredito (janela cheia de 6) do "retrato vivo" (1–5 meses, estimativa
/// marcada) e do sem-registro (0).
pub(crate) async fn realized_monthly_baseline_detail(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    // O loader compartilhado já aplica ABS por item/transação e o filtro de tags excluídas.
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1).unwrap();
    let window_start = month_start - chrono::Months::new(6);
    let events = load_metric_db_events(pool, today_naive, window_start, month_start).await?;

    // Mediana do CUSTO DE VIDA → view Custo de vida: só entram os eventos cuja máscara conta nessa
    // régua (uma tag `exclude_from_cost_of_living` tira a linha do gasto típico, e só dela).
    let mut by_month: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for me in events.iter().filter(|me| me.mask.cost_of_living) {
        let e = &me.event;
        match e.kind {
            forecast::EventKind::FixedOut
            | forecast::EventKind::Daily
            | forecast::EventKind::Cartao => {
                *by_month
                    .entry(e.date.format("%Y-%m").to_string())
                    .or_insert(0) += e.amount_cents;
            }
            _ => {}
        }
    }
    let months = by_month.len() as i64;
    let vals: Vec<i64> = by_month.into_values().collect();
    if vals.is_empty() {
        return Ok((0, 0));
    }
    Ok((forecast::median_cents(vals), months))
}

/// Rendas e economia de um mês típico: `(mediana(renda), mediana(economia))` sobre os últimos
/// 6 meses de calendário COMPLETOS — mesma janela e estimador do `realized_monthly_baseline`,
/// para as duas pernas do gate de financiamento descreverem o MESMO "mês típico".
///
/// O universo são os meses ATIVOS (ao menos um evento de qualquer tipo na janela): um mês ativo
/// sem economia registrada entra como economia 0 — é sinal real de "não poupou", não ausência
/// de dado; descartá-lo inflaria a mediana. A economia mensal reconcilia derivado × anotação da
/// aba Economia com `max` por mês, a mesma regra do motor mensal (evita dupla contagem após o
/// round-trip da planilha). Janela sem mês ativo ⇒ `(0, 0)`.
pub(crate) async fn realized_savings_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1).unwrap();
    let window_start = month_start - chrono::Months::new(6);
    let events = load_metric_db_events(pool, today_naive, window_start, month_start).await?;

    let mut income_by: std::collections::HashMap<(i32, u32), i64> =
        std::collections::HashMap::new();
    let mut economia_by: std::collections::HashMap<(i32, u32), i64> =
        std::collections::HashMap::new();
    let mut active: std::collections::BTreeSet<(i32, u32)> = std::collections::BTreeSet::new();
    // Medianas de renda e economia → view Economia: a régua de gate de financiamento enxerga só
    // os eventos que contam nela (uma tag `exclude_from_savings` tira renda/economia do mês típico).
    for me in events.iter().filter(|me| me.mask.savings) {
        let e = &me.event;
        let key = (e.date.year(), e.date.month());
        active.insert(key);
        match e.kind {
            forecast::EventKind::Income => *income_by.entry(key).or_insert(0) += e.amount_cents,
            forecast::EventKind::Economia => *economia_by.entry(key).or_insert(0) += e.amount_cents,
            _ => {}
        }
    }
    if active.is_empty() {
        return Ok((0, 0));
    }

    let years: Vec<i32> = active
        .iter()
        .map(|(y, _)| *y)
        .collect::<std::collections::BTreeSet<i32>>()
        .into_iter()
        .collect();
    let annotation = load_economia_annotation(pool, &years).await?;

    let mut incomes = Vec::with_capacity(active.len());
    let mut economias = Vec::with_capacity(active.len());
    for key in &active {
        incomes.push(income_by.get(key).copied().unwrap_or(0));
        let derived = economia_by.get(key).copied().unwrap_or(0);
        let annotated = annotation.get(key).copied().unwrap_or(0);
        economias.push(derived.max(annotated));
    }
    Ok((
        forecast::median_cents(incomes),
        forecast::median_cents(economias),
    ))
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
    if let Some(amount) = active_daily_budget(pool).await? {
        return Ok(amount);
    }
    Ok(prev_month_daily_avg(pool, today_naive)
        .await?
        .map_or(0, |basis| basis.per_day_cents))
}

/// Orçamento diário explícito ativo (> 0), se houver — o único teto que é VEREDITO (escolhido).
pub(crate) async fn active_daily_budget(pool: &SqlitePool) -> Result<Option<i64>, String> {
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT amount FROM daily_budget WHERE status='active' AND amount > 0 \
         ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("daily ceiling (budget): {e}"))?;
    Ok(active.map(|(amount,)| amount))
}

/// Diário médio do último mês COMPLETO (Σ diário realizado ÷ dias do mês) — a base do teto
/// ESTIMADO quando o dono não estipulou nada.
/// Base da estimativa do teto: os operandos que produzem a média, não só o resultado. A tela
/// IMPRIME esta conta — uma frase que a descrevesse poderia divergir do que o SQL faz.
#[derive(Debug, Clone)]
pub(crate) struct CeilingEstimateBasis {
    /// Mês da base, `YYYY-MM`.
    pub month: String,
    /// Gasto variável somado do mês anterior (magnitude).
    pub variable_cents: i64,
    /// Dias do mês anterior — o divisor.
    pub days: i64,
    pub per_day_cents: i64,
}

async fn prev_month_daily_avg(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<Option<CeilingEstimateBasis>, String> {
    // Mês anterior completo: primeiro dia do mês corrente − 1 dia.
    let first_this = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data inválida")?;
    let last_prev = match first_this.pred_opt() {
        Some(d) => d,
        None => return Ok(None),
    };
    let prev_ym = last_prev.format("%Y-%m").to_string();
    let days_prev = last_prev.day() as i64;
    if days_prev <= 0 {
        return Ok(None);
    }
    let prev_month_start =
        NaiveDate::from_ymd_opt(last_prev.year(), last_prev.month(), 1).ok_or("data inválida")?;
    // Mesma fonte que a régua Diário médio mensal/anual (`month_metrics_for`): eventos de métrica
    // com a máscara `daily_avg`, não SQL cru sobre `"transaction"` sem o JOIN em
    // `transaction_tag`/`tag` — item por item, uma linha excluída da régua some das duas contas
    // igual. Sem filtro `is_projection` (congelado/stale): o mês anterior já FECHOU pela data — uma
    // projeção importada que virou passado é gasto do mês, mesmo sem re-import (mesma regra de
    // staleness da detecção de modo e do baseline).
    let events = load_metric_db_events(pool, today_naive, prev_month_start, first_this).await?;
    // `SUM(ABS(amount))` por linha (não `ABS(SUM(amount))`): despesas IMPORTADAS chegam negativas
    // (`-amount_out`) e manuais positivas; num mês de sinal misto o ABS externo da soma assinada
    // cancelaria parcialmente, sub-reportando o Diário médio. Invariante de agregação de despesa em
    // TODOS os sites de query (month_grid / realized_monthly_baseline / daily_spend_today).
    let variable_cents: i64 = events
        .iter()
        .filter(|me| me.mask.daily_avg && me.event.kind == forecast::EventKind::Daily)
        .map(|me| me.event.amount_cents)
        .sum();
    Ok(Some(CeilingEstimateBasis {
        month: prev_ym,
        variable_cents,
        days: days_prev,
        per_day_cents: variable_cents / days_prev,
    }))
}

/// Procedência do teto exibido. `chosen` é o único veredito; `estimate` é a média do mês
/// anterior COM selo (o fallback silencioso morre na exibição — o motor de projeção continua
/// usando `effective_daily_ceiling`); `none` = travessão + CTA da cerimônia. A proposta pendente
/// da cerimônia é um OVERLAY (banner de confirmação), nunca a procedência do número exibido — o
/// valor proposto não entra em progresso/projeção antes do aceite explícito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingSource {
    Chosen,
    Estimate,
    None,
}

impl CeilingSource {
    pub fn as_str(self) -> &'static str {
        match self {
            CeilingSource::Chosen => "chosen",
            CeilingSource::Estimate => "estimate",
            CeilingSource::None => "none",
        }
    }
}

/// Leitura do teto para exibição: valor + procedência explícita.
pub(crate) struct CeilingReading {
    pub per_day_cents: i64,
    pub source: CeilingSource,
    /// Presente só quando `source` é `Estimate`: o teto escolhido é decisão do dono, e
    /// número digitado não tem conta a mostrar.
    pub estimate_basis: Option<CeilingEstimateBasis>,
}

pub(crate) async fn daily_ceiling_reading(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<CeilingReading, String> {
    if let Some(amount) = active_daily_budget(pool).await? {
        return Ok(CeilingReading {
            per_day_cents: amount,
            source: CeilingSource::Chosen,
            estimate_basis: None,
        });
    }
    if let Some(basis) = prev_month_daily_avg(pool, today_naive).await?
        && basis.per_day_cents > 0
    {
        return Ok(CeilingReading {
            per_day_cents: basis.per_day_cents,
            source: CeilingSource::Estimate,
            estimate_basis: Some(basis),
        });
    }
    Ok(CeilingReading {
        per_day_cents: 0,
        source: CeilingSource::None,
        estimate_basis: None,
    })
}

/// Teto/dia usado como DRIVER de projeção (re-roteado pelo modo de gasto). No modo cartão o
/// gasto variável vive nas faturas — que já entram como eventos Cartão (realizados ou
/// pré-lançados) — então injetar também o Diário típico dobraria a saída projetada. O teto
/// estipulado segue existindo como referência de exibição; só o driver desliga.
pub(crate) async fn projection_daily_ceiling(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let mode = spending_mode_summary(pool, today_naive).await?.mode;
    if matches!(mode, forecast::SpendingMode::Card) {
        return Ok(0);
    }
    effective_daily_ceiling(pool, today_naive).await
}

/// Existe proposta de cerimônia aguardando o dono?
pub(crate) async fn has_pending_ceiling_proposal(pool: &SqlitePool) -> Result<bool, String> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM ceiling_proposal WHERE status = 'pending' LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("ceiling proposal (pending): {e}"))?;
    Ok(row.is_some())
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

/// Reserva em MESES de custo de vida, com o estado epistêmico que a acompanha.
///
/// A tabela `reserve.current_months` não tem writer de produção, então a cobertura é derivada ao
/// vivo: saldo das contas de reserva ÷ custo de vida mensal (mediana das saídas dos meses
/// completos). O método não exige histórico mínimo — 1–5 meses valem como "retrato vivo"
/// (estimativa marcada) e 6 meses completos viram veredito.
pub(crate) struct ReserveReading {
    pub balance_cents: i64,
    /// Custo de vida mensal que serve de divisor. `0` = sem histórico para dividir.
    pub baseline_cents: i64,
    /// Meses completos que sustentam o divisor — o que separa veredito de retrato vivo.
    pub basis_months: i64,
    pub months: f64,
    /// `verdict` · `estimate` · `zero` (contas mapeadas e zeradas) · `no_record`.
    pub state: &'static str,
    pub trend: String,
    /// Alvo do método em dinheiro: custo de vida × meses mínimos. `0` sem base para calcular.
    pub target_cents: i64,
    /// Quanto passa do alvo — a pergunta que o método faz depois que a reserva está de pé
    /// ("tenho 70, preciso de 40, tenho 30 de excedente; o que faz sentido pra minha vida
    /// agora?"). `None` enquanto a reserva ainda está sendo construída ou não há base.
    pub surplus_cents: Option<i64>,
}

pub(crate) async fn reserve_reading(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<ReserveReading, String> {
    let balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query reserve balance: {e}"))?;
    let has_accounts: (i64,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM account WHERE liquidity = 'reserve')")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query reserve accounts: {e}"))?;
    let (baseline, basis_months) = realized_monthly_baseline_detail(pool, today_naive).await?;
    let months = if baseline > 0 {
        balance.0 as f64 / baseline as f64
    } else {
        0.0
    };
    // Sem contas de reserva mapeadas ou sem baseline nenhum, não há número honesto (sem
    // registro); contas mapeadas zeradas são o alerta legítimo (zero-diagnóstico).
    let state = if has_accounts.0 == 0 || baseline <= 0 {
        "no_record"
    } else if balance.0 == 0 {
        "zero"
    } else if basis_months >= 6 {
        "verdict"
    } else {
        "estimate"
    };
    let trend: (String,) = sqlx::query_as(
        "SELECT COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve trend: {e}"))?
    .unwrap_or(("flat".to_string(),));

    // O alvo do método é custo de vida × meses; o excedente só existe depois de alcançado. Sem
    // base de custo de vida não há alvo honesto — e sem alvo não há excedente a declarar.
    let target_cents = baseline * forecast::RESERVE_MIN_MONTHS;
    let surplus_cents =
        (target_cents > 0 && balance.0 > target_cents).then(|| balance.0 - target_cents);

    Ok(ReserveReading {
        balance_cents: balance.0,
        baseline_cents: baseline,
        basis_months,
        months,
        state,
        trend: trend.0,
        target_cents,
        surplus_cents,
    })
}

/// A régua de economia que JULGA: a Economia REGISTRADA do ano, e só ela. Patrimônio
/// (previdência, ilíquido) é saída de longo prazo — vive fora do numerador, como outra linha da
/// vida financeira, e é publicado ao lado para leitura, nunca somado à régua.
///
/// Uma régua só, sem bifurcar semântica: alimenta o guardrail de poupança e a perna de economia
/// do gate do modo cartão.
pub(crate) struct EconomiaRulerReading {
    /// O numerador que julga: Economia registrada dos meses completos.
    pub registered_cents: i64,
    /// Patrimônio realizado do ano — leitura vizinha, fora da régua.
    pub patrimonio_cents: i64,
    /// `verdict` (régua viva) · `no_record` (nada registrado — a superfície mostra a sobra
    /// derivada como estimativa marcada).
    pub state: &'static str,
}

pub(crate) async fn economia_ruler_reading(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<EconomiaRulerReading, String> {
    let registered = realized_annual_economia(pool, today_naive).await?;
    let patrimonio = realized_annual_patrimonio(pool, today_naive).await?;
    Ok(EconomiaRulerReading {
        registered_cents: registered,
        patrimonio_cents: patrimonio,
        state: if registered > 0 {
            "verdict"
        } else {
            "no_record"
        },
    })
}

/// A régua anual de um ano com as métricas que a sustentam, montadas uma vez só: carrega os doze
/// meses e aplica a régua do motor. Quem precisa do Economizado%, do recorte vivido ou do
/// veredito da faixa lê daqui — a montagem repetida por call site é o que deixa um deles para
/// trás quando a régua ganha um parâmetro. As métricas vêm junto porque quem lista o ano mês a
/// mês precisa das duas leituras, e carregá-las de novo leria o ano duas vezes.
pub(crate) async fn annual_ruler_reading(
    pool: &SqlitePool,
    year: i32,
    today_naive: NaiveDate,
) -> Result<(Vec<forecast::MonthMetric>, forecast::AnnualRuler), String> {
    let metrics = annual_month_metrics(pool, year, today_naive).await?;
    let ruler = forecast::annual_ruler(&metrics, year, today_naive);
    Ok((metrics, ruler))
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
    let max_txn: (Option<String>,) = sqlx::query_as(
        "SELECT MAX(date) FROM \"transaction\" WHERE date >= ?1 AND scenario_id IS NULL",
    )
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
    let max_invoice: (Option<String>,) = sqlx::query_as(
        "SELECT MAX(i.due_date) FROM invoice i \
         JOIN account a ON a.id = i.account_id \
         WHERE a.type = 'credit_card' AND i.due_date >= ?1",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("horizon invoice: {e}"))?;

    let mut horizon = calendar::last_day_of_month(today_naive.year(), today_naive.month());
    for (candidate,) in [max_txn, max_bal, max_invoice] {
        if let Some(date_str) = candidate
            && let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            && d > horizon
        {
            horizon = d;
        }
    }
    Ok(horizon)
}

#[derive(sqlx::FromRow)]
struct MetricTxnRow {
    id: String,
    ttype: String,
    amount: i64,
    date: String,
    payment_method: String,
    is_fixed: i64,
    is_projection: i64,
    to_liquidity: String,
    invoice_id: Option<String>,
}

struct MetricLineItemRow {
    amount_cents: i64,
    description: String,
    section: Option<String>,
}

type CardInvoiceRow = (
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    i64,
    i64,
    i64,
);

#[derive(Debug, Clone)]
pub(crate) struct CardInvoiceEvent {
    pub account_id: String,
    pub card_name: String,
    pub owner_name: String,
    pub closing_date: NaiveDate,
    pub due_date: NaiveDate,
    pub amount_cents: i64,
    /// Existe Entrada vinculada (`refund_invoice_id`) — a expectativa de reembolso da fatura.
    pub has_refund_expectation: bool,
    pub refund_expected_cents: i64,
}

pub(crate) async fn load_card_invoice_events(
    pool: &SqlitePool,
    today: NaiveDate,
    start_inclusive: NaiveDate,
    end_exclusive: Option<NaiveDate>,
) -> Result<(bool, Vec<CardInvoiceEvent>), String> {
    let has_card: (i64,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM account WHERE type = 'credit_card')")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("cartões: {e}"))?;
    if has_card.0 == 0 {
        return Ok((false, Vec::new()));
    }

    let lower = today.max(start_inclusive).format("%Y-%m-%d").to_string();
    let upper = end_exclusive.map(|end| end.format("%Y-%m-%d").to_string());
    // Reembolso é Entrada, por definição do domínio.
    let rows: Vec<CardInvoiceRow> = sqlx::query_as(
        "SELECT i.account_id, a.name, COALESCE(p.name, ''), i.closing_date, i.due_date, \
                i.stated_total_cents, COALESCE(SUM(ABS(t.amount)), 0), \
                EXISTS(SELECT 1 FROM \"transaction\" r WHERE r.refund_invoice_id = i.id \
                       AND r.type = 'income' AND r.scenario_id IS NULL), \
                COALESCE((SELECT SUM(ABS(r.amount)) FROM \"transaction\" r \
                          WHERE r.refund_invoice_id = i.id AND r.type = 'income' \
                            AND r.scenario_id IS NULL), 0) \
         FROM invoice i \
         JOIN account a ON a.id = i.account_id \
         LEFT JOIN person p ON p.id = a.owner_person_id \
         LEFT JOIN \"transaction\" t ON t.invoice_id = i.id AND t.type = 'expense' \
             AND t.scenario_id IS NULL \
         WHERE a.type = 'credit_card' AND i.due_date >= ?1 \
           AND (?2 IS NULL OR i.due_date < ?2) \
         GROUP BY i.id, i.account_id, a.name, p.name, i.closing_date, i.due_date, \
                  i.stated_total_cents \
         ORDER BY i.due_date, a.name, i.id",
    )
    .bind(&lower)
    .bind(upper)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("faturas: {e}"))?;

    let invoices = rows
        .into_iter()
        .map(
            |(
                account_id,
                card_name,
                owner_name,
                closing_date,
                due_date,
                stated_total_cents,
                purchases_sum_cents,
                has_refund_expectation,
                refund_expected_cents,
            )| {
                let amount_cents =
                    crate::cards::effective_total_cents(stated_total_cents, purchases_sum_cents);
                Ok(CardInvoiceEvent {
                    account_id,
                    card_name,
                    owner_name,
                    closing_date: NaiveDate::parse_from_str(&closing_date, "%Y-%m-%d")
                        .map_err(|_| format!("data de fechamento inválida: {closing_date}"))?,
                    due_date: NaiveDate::parse_from_str(&due_date, "%Y-%m-%d")
                        .map_err(|_| format!("data de vencimento inválida: {due_date}"))?,
                    amount_cents,
                    has_refund_expectation: has_refund_expectation != 0,
                    // O reembolso não ultrapassa o compromisso que compensa, mesmo em Entrada mista.
                    refund_expected_cents: refund_expected_cents.min(amount_cents).max(0),
                })
            },
        )
        .collect::<Result<Vec<_>, String>>()?;
    Ok((true, invoices))
}

/// A semente da projeção (`projection_seed`) já embute tudo até HOJE inclusive (Saldo mais
/// recente ≤ hoje + gap de transações reais até hoje). Por isso a injeção da fatura — e a
/// supressão do evento cru que ela substitui — usam o MESMO limite estritamente futuro
/// (`> today`, nunca `>= today`): uma fatura vencendo hoje já está contada pela semente, e
/// reinjetá-la aqui abateria o mesmo dinheiro duas vezes.
/// Genérica no envelope do evento (caixa `CashflowEvent` ou métrica `MetricEvent`) para o
/// invariante da precedência viver num lugar só: `is_future_cartao` marca o Cartão cru futuro a
/// descartar; `make_lump` constrói o evento sintético da fatura no vencimento.
pub(crate) fn apply_card_invoice_precedence<T>(
    today: NaiveDate,
    has_card: bool,
    raw_events: Vec<T>,
    invoices: &[CardInvoiceEvent],
    is_future_cartao: impl Fn(&T) -> bool,
    make_lump: impl Fn(&CardInvoiceEvent) -> T,
) -> Vec<T> {
    if !has_card {
        return raw_events;
    }

    let mut events: Vec<T> = raw_events
        .into_iter()
        .filter(|event| !is_future_cartao(event))
        .collect();
    events.extend(
        invoices
            .iter()
            .filter(|invoice| invoice.due_date > today)
            .map(make_lump),
    );
    events
}

pub(crate) async fn finalize_card_events(
    pool: &SqlitePool,
    today: NaiveDate,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
    raw_events: Vec<CashflowEvent>,
) -> Result<Vec<CashflowEvent>, String> {
    let (has_card, invoices) =
        load_card_invoice_events(pool, today, start_inclusive, Some(end_exclusive)).await?;
    Ok(apply_card_invoice_precedence(
        today,
        has_card,
        raw_events,
        &invoices,
        |event: &CashflowEvent| event.kind == forecast::EventKind::Cartao && event.date > today,
        |invoice| CashflowEvent {
            date: invoice.due_date,
            kind: forecast::EventKind::Cartao,
            amount_cents: invoice.amount_cents,
            realized: false,
        },
    ))
}

/// Como [`finalize_card_events`], mas no stream de MÉTRICAS. A fatura materializada é um evento
/// SINTÉTICO: recebe `RulerMask::ALL` — o exclude de uma compra interna não vive no lump (o total
/// da fatura sai da conta de qualquer forma, e o valor suprimido de uma compra realizada já cai
/// pela máscara dela antes de virar lump). Preserva o comportamento do flag único: uma tag 4×
/// desligada dentro de uma fatura não reduz o lump, exatamente como antes.
pub(crate) async fn finalize_card_metric_events(
    pool: &SqlitePool,
    today: NaiveDate,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
    raw_events: Vec<forecast::MetricEvent>,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let (has_card, invoices) =
        load_card_invoice_events(pool, today, start_inclusive, Some(end_exclusive)).await?;
    Ok(apply_card_invoice_precedence(
        today,
        has_card,
        raw_events,
        &invoices,
        |me: &forecast::MetricEvent| {
            me.event.kind == forecast::EventKind::Cartao && me.event.date > today
        },
        |invoice| forecast::MetricEvent {
            event: CashflowEvent {
                date: invoice.due_date,
                kind: forecast::EventKind::Cartao,
                amount_cents: invoice.amount_cents,
                realized: false,
            },
            mask: forecast::RulerMask::ALL,
        },
    ))
}

/// Classifica um item de nota `Cartao`: a CHAVE é "existe fatura para (conta, ciclo) da linha",
/// nunca "o alias é conhecido" sozinho. Uma proposta recém-aceita cria conta+alias mas não
/// materializa a fatura observada até o próximo import — nessa janela o item precisa continuar
/// Saída fixa (visível no forecast), porque a fatura da precedência (`apply_card_invoice_
/// precedence`) não existe para repor o valor suprimido. Regra compartilhada por caixa, métricas
/// (via `load_db_events`) e cenário (`scenarios.rs`) — nenhum dos três reclassifica sozinho.
pub(crate) fn event_kind_for_item_kind(
    kind: import::ItemKind,
    description: &str,
    date: NaiveDate,
    cards: &crate::cards::CardLexicon<String>,
    invoiced_cycles: &std::collections::HashSet<(String, String)>,
) -> forecast::EventKind {
    match kind {
        // A seção é a autoridade da classificação; quando ela falta, a linha ainda pode NOMEAR um
        // cartão já declarado. Sem isso a fatura de uma nota sem cabeçalho vira conta a pagar e
        // ainda escapa da precedência do lump, debitando duas vezes o mesmo vencimento.
        import::ItemKind::Saida | import::ItemKind::Cartao => {
            if invoice_backed_card(description, date, cards, invoiced_cycles) {
                forecast::EventKind::Cartao
            } else {
                forecast::EventKind::FixedOut
            }
        }
        import::ItemKind::Ajuste => forecast::EventKind::FixedOut,
        import::ItemKind::Diario => forecast::EventKind::Daily,
        import::ItemKind::Economia => forecast::EventKind::Economia,
        import::ItemKind::Patrimonio => forecast::EventKind::Patrimonio,
    }
}

/// `true` quando a linha nomeia um cartão do léxico QUE TEM fatura persistida para o ciclo da
/// data. As duas pernas são necessárias: sem fatura não há lump para repor o valor suprimido.
fn invoice_backed_card(
    description: &str,
    date: NaiveDate,
    cards: &crate::cards::CardLexicon<String>,
    invoiced_cycles: &std::collections::HashSet<(String, String)>,
) -> bool {
    cards.resolve(description).is_some_and(|account_id| {
        invoiced_cycles.contains(&(account_id, crate::cards::cycle_month_of(date)))
    })
}

/// Léxico das contas de cartão (nome da conta + `card_alias`) → `account_id`. Resolver a CONTA
/// (não só "o alias existe") é o que permite checar depois se ELA tem fatura para o ciclo da
/// linha — ver `event_kind_for_item_kind`.
pub(crate) async fn load_card_alias_index(
    pool: &SqlitePool,
) -> Result<crate::cards::CardLexicon<String>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT a.id, a.name FROM account a WHERE a.type = 'credit_card' \
         UNION ALL \
         SELECT a.id, ca.alias FROM card_alias ca \
         JOIN account a ON a.id = ca.account_id \
         WHERE a.type = 'credit_card'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("load card aliases for forecast: {e}"))?;

    let mut seen = std::collections::HashMap::new();
    for (account_id, alias) in rows {
        let normalized = crate::cards::normalize_alias(&alias);
        if !normalized.is_empty() {
            seen.entry(normalized).or_insert(account_id);
        }
    }
    Ok(crate::cards::CardLexicon::from_entries(seen))
}

/// Conjunto de (`account_id`, `cycle_month`) que TÊM fatura persistida — carregado em UMA query
/// antes do loop de classificação, para que cada item de nota apenas consulte o conjunto em vez
/// de bater no banco por linha.
pub(crate) async fn load_invoiced_cycles(
    pool: &SqlitePool,
) -> Result<std::collections::HashSet<(String, String)>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT account_id, cycle_month FROM invoice")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load invoiced cycles for forecast: {e}"))?;
    Ok(rows.into_iter().collect())
}

/// Um evento de método + o `transaction_id` do lançamento-pai. O caminho de MÉTRICAS usa o id
/// para herdar a máscara de réguas das tags (itens de nota e resíduo da célula herdam a máscara
/// do pai); o caminho de CAIXA descarta o id.
pub(crate) struct RawDbEvent {
    pub(crate) event: CashflowEvent,
    pub(crate) transaction_id: String,
}

/// Decompõe TODAS as transações da janela `[start, end)` em eventos de método, sem filtro de tag
/// — a exclusão por tag virou máscara por régua, aplicada depois de carregar. Fonte única da
/// decomposição linha→evento (itens de nota, resíduo da célula, relabel de crédito órfão),
/// compartilhada pelos caminhos de caixa (descarta a máscara) e de métricas (a herda).
pub(crate) async fn load_raw_db_events(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
    orphan_credit_from: Option<NaiveDate>,
) -> Result<Vec<RawDbEvent>, String> {
    let start = start_inclusive.format("%Y-%m-%d").to_string();
    let end = end_exclusive.format("%Y-%m-%d").to_string();
    let alias_to_account = load_card_alias_index(pool).await?;
    let invoiced_cycles = load_invoiced_cycles(pool).await?;
    let has_card = if orphan_credit_from.is_some() {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM account WHERE type = 'credit_card')",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| format!("cartões: {e}"))?
            != 0
    } else {
        false
    };

    const TRANSACTIONS: &str = "SELECT t.id, t.type AS ttype, t.amount, t.date, \
                COALESCE(t.payment_method,'') AS payment_method, \
                t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity, t.invoice_id \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL";
    let txn_rows: Vec<MetricTxnRow> = sqlx::query_as(TRANSACTIONS)
        .bind(&start)
        .bind(&end)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query metric transactions: {e}"))?;

    const ITEMS: &str = "SELECT li.transaction_id, li.amount_cents, li.description, li.section \
         FROM line_item li \
         JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL \
         ORDER BY li.transaction_id, li.position";
    let item_rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(ITEMS)
        .bind(&start)
        .bind(&end)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query metric line items: {e}"))?;

    let mut items_by_txn: std::collections::HashMap<String, Vec<MetricLineItemRow>> =
        std::collections::HashMap::new();
    for (transaction_id, amount_cents, description, section) in item_rows {
        items_by_txn
            .entry(transaction_id)
            .or_default()
            .push(MetricLineItemRow {
                amount_cents,
                description,
                section,
            });
    }

    let mut events = Vec::new();
    for row in txn_rows {
        if row.ttype == "expense"
            && let Some(items) = items_by_txn.get(&row.id)
            && !items.is_empty()
        {
            let Some(date) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").ok() else {
                continue;
            };
            for item in items {
                let kind =
                    import::classify_line_item(item.section.as_deref(), item.description.as_str());
                events.push(RawDbEvent {
                    event: CashflowEvent {
                        date,
                        kind: event_kind_for_item_kind(
                            kind,
                            &item.description,
                            date,
                            &alias_to_account,
                            &invoiced_cycles,
                        ),
                        amount_cents: item.amount_cents.abs(),
                        realized: row.is_projection == 0,
                    },
                    transaction_id: row.id.clone(),
                });
            }
            // A célula é a dona do TOTAL: se as partes da nota não somam o pai, o resíduo
            // (célula − Σ|partes|) entra como Saída fixa COM SINAL — a convenção AJUSTES
            // "Diferença" da planilha aplicada na leitura, sem item sintético persistido.
            // Um resíduo negativo (partes > célula) REDUZ fixed_out para os baldes fecharem
            // com o total; por isso este evento é a exceção documentada à convenção
            // "amount_cents sempre positivo" do CashflowEvent. O mesmo resíduo precisa atravessar
            // caixa e métricas: os dois caminhos devem conservar o total da célula.
            let parts_sum: i64 = items.iter().map(|i| i.amount_cents.abs()).sum();
            let residual = row.amount.abs() - parts_sum;
            if residual != 0 {
                events.push(RawDbEvent {
                    event: CashflowEvent {
                        date,
                        kind: forecast::EventKind::FixedOut,
                        amount_cents: residual,
                        realized: row.is_projection == 0,
                    },
                    transaction_id: row.id.clone(),
                });
            }
            continue;
        }

        if let Some(event) = map_cashflow_row((
            row.ttype,
            row.amount,
            row.date,
            row.payment_method,
            row.is_fixed,
            row.is_projection,
            row.to_liquidity,
        )) {
            events.push(RawDbEvent {
                event: relabel_orphan_credit_event(
                    event,
                    row.invoice_id.as_deref(),
                    has_card,
                    orphan_credit_from,
                ),
                transaction_id: row.id,
            });
        }
    }

    Ok(events)
}

/// Eventos de CAIXA da janela: todo dinheiro que entra/sai, sem máscara (o Saldo sempre conta).
async fn load_db_events(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
    orphan_credit_from: Option<NaiveDate>,
) -> Result<Vec<CashflowEvent>, String> {
    Ok(
        load_raw_db_events(pool, start_inclusive, end_exclusive, orphan_credit_from)
            .await?
            .into_iter()
            .map(|raw| raw.event)
            .collect(),
    )
}

/// Eventos de MÉTRICA da janela: cada evento carrega a máscara de réguas herdada das tags do
/// lançamento-pai. Lançamento sem tag conta em todas as réguas (`RulerMask::ALL`).
async fn load_db_metric_events(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
    orphan_credit_from: Option<NaiveDate>,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let raw = load_raw_db_events(pool, start_inclusive, end_exclusive, orphan_credit_from).await?;
    let mask_by_txn = load_ruler_mask_map(pool, start_inclusive, end_exclusive).await?;
    Ok(raw
        .into_iter()
        .map(|raw| forecast::MetricEvent {
            event: raw.event,
            mask: mask_by_txn
                .get(&raw.transaction_id)
                .copied()
                .unwrap_or(forecast::RulerMask::ALL),
        })
        .collect())
}

/// Meses (`YYYY-MM`) em que a planilha DECLARA movimento de cartão, antes de qualquer resolução
/// de conta ou fatura.
///
/// O modo de gasto é uma pergunta sobre a FORMA do gasto, e quem a responde é a planilha: a seção
/// de cartões da nota, ou uma linha que nomeia um cartão já declarado. Derivar esse sinal do
/// `EventKind::Cartao` fazia a resposta depender de o dono já ter cadastrado a conta — quem gasta
/// tudo no crédito e ainda não cadastrou nada era lido como se gastasse no débito.
///
/// A máscara de régua vale aqui pelo mesmo motivo que vale no resto da detecção: um lançamento
/// fora do custo de vida não descreve a forma do gasto corrente.
pub(crate) async fn declared_card_months(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
) -> Result<std::collections::HashSet<String>, String> {
    let start = start_inclusive.format("%Y-%m-%d").to_string();
    let end = end_exclusive.format("%Y-%m-%d").to_string();
    // Só despesa: na coluna Entrada, "Fatura <cartão>" é o REEMBOLSO de quem divide a fatura —
    // dinheiro voltando, não gasto no cartão. Contá-lo diria "modo cartão" a quem só recebe.
    let rows: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT t.id, t.date, li.section, li.description \
         FROM line_item li JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL \
           AND t.type = 'expense'",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("declared card months: {e}"))?;
    if rows.is_empty() {
        return Ok(std::collections::HashSet::new());
    }

    let lexicon = load_recognized_card_lexicon(pool).await?;
    let mask_by_txn = load_ruler_mask_map(pool, start_inclusive, end_exclusive).await?;
    let mut months = std::collections::HashSet::new();
    for (transaction_id, date, section, description) in rows {
        let counts_for_cost_of_living = mask_by_txn
            .get(&transaction_id)
            .copied()
            .unwrap_or(forecast::RulerMask::ALL)
            .cost_of_living;
        if !counts_for_cost_of_living {
            continue;
        }
        let declared = import::classify_line_item(section.as_deref(), &description)
            == import::ItemKind::Cartao
            || lexicon.resolve(&description).is_some();
        if declared && date.len() >= 7 {
            months.insert(date[..7].to_string());
        }
    }
    Ok(months)
}

/// Léxico de RECONHECIMENTO — o que o domínio já sabe ser cartão: contas cadastradas e propostas
/// ainda pendentes. Distinto de [`load_card_alias_index`], que resolve a CONTA para checar a
/// fatura do ciclo; aqui a pergunta é só "esta linha nomeia um cartão?". Uma proposta dispensada
/// não entra: o dono já disse que aquilo não é cartão.
pub(crate) async fn load_recognized_card_lexicon(
    pool: &SqlitePool,
) -> Result<crate::cards::CardLexicon<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM account WHERE type = 'credit_card' \
         UNION ALL \
         SELECT ca.alias FROM card_alias ca JOIN account a ON a.id = ca.account_id \
         WHERE a.type = 'credit_card' \
         UNION ALL \
         SELECT alias FROM card_proposal WHERE status = 'pending'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("recognized card lexicon: {e}"))?;
    Ok(crate::cards::CardLexicon::from_entries(
        rows.into_iter().filter_map(|(alias,)| {
            let normalized = crate::cards::normalize_alias(&alias);
            (!normalized.is_empty()).then(|| (normalized.clone(), normalized))
        }),
    ))
}

/// Máscara de réguas por transação na janela `[start, end)`: a máscara de um lançamento é a
/// INTERSEÇÃO (`RulerMask::and`) das suas tags — conta numa régua só se NENHUMA tag o excluir
/// dela. Escopo por data via JOIN em "transaction" para não varrer a base inteira; lançamento
/// sem tag não aparece aqui e herda `RulerMask::ALL` no chamador.
pub(crate) async fn load_ruler_mask_map(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
) -> Result<std::collections::HashMap<String, forecast::RulerMask>, String> {
    let start = start_inclusive.format("%Y-%m-%d").to_string();
    let end = end_exclusive.format("%Y-%m-%d").to_string();
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT tt.transaction_id, \
                tg.exclude_from_performance, tg.exclude_from_cost_of_living, \
                tg.exclude_from_savings, tg.exclude_from_daily_avg \
         FROM transaction_tag tt \
         JOIN tag tg ON tg.id = tt.tag_id \
         JOIN \"transaction\" t ON t.id = tt.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("ruler mask map: {e}"))?;

    let mut mask_by_txn: std::collections::HashMap<String, forecast::RulerMask> =
        std::collections::HashMap::new();
    for (transaction_id, perf, cost, savings, daily) in rows {
        // Um flag = 1 exclui a régua; a máscara da TAG conta na régua quando o flag é 0.
        let tag_mask = forecast::RulerMask {
            performance: perf == 0,
            cost_of_living: cost == 0,
            savings: savings == 0,
            daily_avg: daily == 0,
        };
        mask_by_txn
            .entry(transaction_id)
            .and_modify(|m| *m = m.and(tag_mask))
            .or_insert(tag_mask);
    }
    Ok(mask_by_txn)
}

/// Só o crédito FUTURO de uma base com cartão é substituído pelo lump no vencimento. Sem vínculo
/// de fatura, a compra crua (legada ou hipotética de cenário) permanece uma saída de caixa;
/// passado e bases sem cartão conservam a classificação crua. Compartilhada entre o motor
/// principal (`load_db_events`) e o ramo de cenário — nenhum dos dois reclassifica sozinho.
pub(crate) fn relabel_orphan_credit_event(
    mut event: CashflowEvent,
    invoice_id: Option<&str>,
    has_card: bool,
    orphan_credit_from: Option<NaiveDate>,
) -> CashflowEvent {
    if event.kind == forecast::EventKind::Cartao
        && invoice_id.is_none()
        && has_card
        && orphan_credit_from.is_some_and(|today| event.date >= today)
    {
        event.kind = forecast::EventKind::FixedOut;
    }
    event
}

async fn load_metric_db_events(
    pool: &SqlitePool,
    today: NaiveDate,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let raw_events =
        load_db_metric_events(pool, start_inclusive, end_exclusive, Some(today)).await?;
    finalize_card_metric_events(pool, today, start_inclusive, end_exclusive, raw_events).await
}

/// Loads forward cashflow events for the projection window: future transactions (date > today,
/// avoiding double-counting today's already-realized spending baked into the balance snapshot)
/// plus faturas on their due dates. Single source of row→event mapping, shared by
/// `dashboard_summary` and `forecast_dto`.
pub(crate) async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let raw_start = today_naive
        .succ_opt()
        .ok_or("data de hoje inválida para intervalo de caixa")?;
    let end_exclusive = horizon_end
        .succ_opt()
        .ok_or("horizonte inválido para intervalo de caixa")?;
    // Sem máscara de réguas: a visão de caixa continua contabilizando todo dinheiro que sai,
    // mesmo que um lançamento esteja excluído de alguma régua de método.
    let raw_events = load_db_events(pool, raw_start, end_exclusive, Some(today_naive)).await?;
    finalize_card_events(pool, today_naive, today_naive, end_exclusive, raw_events).await
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
    let daily_ceiling = projection_daily_ceiling(pool, today_naive).await?;
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
/// deles — senão o mês corrente aparece pela metade. Só transações; os
/// lumps de fatura realizados deste mês já estão na coluna Saída da planilha como transação.
pub(crate) async fn load_realized_month_events(
    pool: &SqlitePool,
    month_start: NaiveDate,
    today_naive: NaiveDate,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let end_exclusive = today_naive
        .succ_opt()
        .ok_or("data de hoje inválida para intervalo de métricas")?;
    load_metric_db_events(pool, today_naive, month_start, end_exclusive).await
}

/// Eventos para as MÉTRICAS por mês = futuros (encadeamento) + realizados do mês corrente.
/// Cobre o mês inteiro de hoje (realizado + projetado); meses à frente já são todos futuros.
pub(crate) async fn load_metric_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data de hoje inválida")?;
    let end_exclusive = horizon_end
        .succ_opt()
        .ok_or("horizonte inválido para intervalo de métricas")?;
    let mut metric = load_realized_month_events(pool, month_start, today_naive).await?;
    let future_start = today_naive
        .succ_opt()
        .ok_or("data de hoje inválida para intervalo futuro de métricas")?;
    metric.extend(load_metric_db_events(pool, today_naive, future_start, end_exclusive).await?);
    let daily_ceiling = projection_daily_ceiling(pool, today_naive).await?;
    // Cobertura de dias do teto = fato COMPORTAMENTAL (o dia teve registro de Diário), sem máscara:
    // um dia coberto por gasto excluído de alguma régua não recebe dupla projeção do teto.
    let days_with_daily: std::collections::HashSet<NaiveDate> = metric
        .iter()
        .filter(|me| me.event.kind == forecast::EventKind::Daily)
        .map(|me| me.event.date)
        .collect();
    // Teto projetado = evento SINTÉTICO → conta em todas as réguas (`RulerMask::ALL`).
    metric.extend(forecast::lift_all(&forecast::project_daily_ceiling(
        daily_ceiling,
        today_naive,
        horizon_end,
        &days_with_daily,
    )));
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct DayPointDto {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MonthEndDto {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthMetricDto {
    pub year: i32,
    pub month: u32,
    /// Renda na view ECONOMIA — a "sua renda" do método, denominador do Economizado%.
    pub income_cents: i64,
    /// Renda na view PERFORMANCE (perna positiva de `performance_cents`).
    pub income_performance_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas fixas realizadas (coluna Saída sem cartão/economia/patrimônio).
    pub fixed_out_cents: i64,
    /// Diário realizado (coluna Diário, view CUSTO DE VIDA).
    pub daily_out_cents: i64,
    /// Diário realizado na view DIÁRIO MÉDIO — numerador de `real_daily_avg_cents`.
    pub daily_avg_out_cents: i64,
    /// Previsão de diário do mês (teto dos dias futuros + pré-lançados); desconta a Performance.
    pub daily_projected_cents: i64,
    /// Cartão realizado, bucket próprio dentro do custo de vida.
    pub cartao_cents: i64,
    /// Diário médio do mês = Σ diário realizado ÷ dias decorridos (D/N).
    pub real_daily_avg_cents: i64,
    /// Economia lançada no mês (numerador do Economizado%).
    pub economia_cents: i64,
    /// Patrimônio/long-term/illiquid, fora de custo de vida e Economia% acessível.
    pub patrimonio_cents: i64,
    pub savings_rate_bps: i64,
}

/// Poupança do ano: realizada (honesta) vs projetada (otimista quando o futuro está incompleto).
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" exibido no Neko; `registered_economia_cents` é a Economia
/// REGISTRADA do método (transfers→reserva), numerador do Economizado%. O guardrail de poupança
/// usa a Economia registrada; o net só aparece como exibição do colchão.
#[derive(serde::Serialize)]
pub struct AnnualSavingsDto {
    /// Entradas dos meses VIVIDOS do ano, o corrente incluído — o denominador da régua anual.
    pub realized_income_cents: i64,
    /// Sobra dos meses vividos (o colchão), na mesma janela do denominador.
    pub realized_savings_cents: i64,
    pub realized_rate_bps: i64,
    /// Economia REGISTRADA do ano (transfers→reserva), meses completos. Distinta do net.
    pub registered_economia_cents: i64,
    /// Patrimônio realizado do ano (previdência/ilíquido) — a outra leitura do popover.
    pub patrimonio_cents: i64,
    /// Numerador da régua que julga: a Economia lançada nos meses vividos. Patrimônio fica de
    /// fora — é saída de longo prazo, não Economia.
    pub economia_ruler_cents: i64,
    pub economia_ruler_rate_bps: i64,
    /// Estado epistêmico da régua de economia: `verdict` (economia registrada viva) ·
    /// `no_record` (nada registrado — a UI exibe a sobra derivada como estimativa marcada).
    pub economia_state: String,
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
    let metric_events = load_metric_events(pool, today_naive, horizon_end).await?;
    // Anotação da aba Economia para os anos cobertos pelo horizonte — parcela aditiva do
    // Economizado% por mês, disjunta dos transfers de reserva reais (que já chegam nos eventos).
    let years: Vec<i32> = (today_naive.year()..=horizon_end.year()).collect();
    let annotation = load_economia_annotation(pool, &years).await?;
    let fc = forecast::project_with_metrics(
        seed,
        today_naive,
        &events,
        // O loader de métricas já carrega a máscara de réguas por lançamento em cada evento.
        &metric_events,
        horizon_end,
        &annotation,
    );

    // Renda-base do GUARDRAIL: só meses COMPLETOS. No meio do mês as fixas já entraram e o
    // salário pode não ter, e um denominador em formação viraria "pode gastar R$ 0" de falso
    // pânico. A régua que a tela publica é outra janela — decisão, não exibição.
    let (guardrail_income_cents, _) = realized_annual_savings(pool, today_naive).await?;
    let economia = economia_ruler_reading(pool, today_naive).await?;
    let annual_economia = economia.registered_cents;
    let annual_patrimonio = economia.patrimonio_cents;
    let sts = forecast::safe_to_spend_today(
        &fc,
        guardrail_income_cents,
        annual_economia,
        SAVINGS_TARGET_BPS,
    );
    let binding_guardrail = sts.binding.as_str().to_string();

    // Previsibilidade: poupança realizada vs projetada + cobertura dos meses futuros.
    let (proj_income, proj_savings) = projected_annual_savings(pool, today_naive).await?;
    // Taxa em bps para EXIBIÇÃO (round half-up, não trunca — senão 25,00% vira 2499/abaixo da
    // meta). Nunca usada em decisão (o guardrail compara centavos diretos).
    let rate_bps = |save: i64, inc: i64| {
        if inc > 0 {
            (save * 10_000 + inc / 2) / inc
        } else {
            0
        }
    };
    // A régua anual é ÚNICA e mora no motor: Economizado% = Economia ÷ entradas sobre os meses
    // VIVIDOS, o corrente incluído. Ler daqui é o que faz a conversa, a tela do ano e este DTO
    // publicarem o mesmo percentual — uma segunda derivação aqui abriria duas verdades.
    let (_, annual_ruler) = annual_ruler_reading(pool, today_naive.year(), today_naive).await?;
    let ruler_income_cents = annual_ruler.income_lived_cents;

    let annual_savings = AnnualSavingsDto {
        realized_income_cents: ruler_income_cents,
        realized_savings_cents: annual_ruler.surplus_lived_cents,
        realized_rate_bps: rate_bps(annual_ruler.surplus_lived_cents, ruler_income_cents),
        registered_economia_cents: annual_economia,
        patrimonio_cents: annual_patrimonio,
        economia_ruler_cents: annual_ruler.economia_lived_cents,
        // O percentual sai truncado do motor: exibir 21% de 21,9% nunca promete o que a régua
        // não mediu. Sem renda vivida não há o que dividir — a régua não fabrica zero.
        economia_ruler_rate_bps: annual_ruler.lived_bps.unwrap_or(0),
        economia_state: economia.state.to_string(),
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
            forecast::EventKind::FixedOut | forecast::EventKind::Cartao => {
                entry.1 += e.amount_cents
            }
            forecast::EventKind::Daily => entry.2 += e.amount_cents,
            forecast::EventKind::Economia => entry.3 += e.amount_cents,
            // ForecastDayDto is a legacy day-flow shape; monthly DTOs expose Patrimônio.
            forecast::EventKind::Patrimonio => {}
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
                income_performance_cents: m.income_performance_cents,
                performance_cents: m.performance_cents,
                cost_of_living_cents: m.cost_of_living_cents,
                fixed_out_cents: m.fixed_out_cents,
                daily_out_cents: m.daily_out_cents,
                daily_avg_out_cents: m.daily_avg_out_cents,
                daily_projected_cents: m.daily_projected_cents,
                cartao_cents: m.cartao_cents,
                real_daily_avg_cents: m.real_daily_avg_cents,
                economia_cents: m.economia_cents,
                patrimonio_cents: m.patrimonio_cents,
                savings_rate_bps: m.savings_rate_bps,
            })
            .collect(),
    })
}

// --- Visão anual por mês ---

/// Todos os eventos do ANO (realizado + projetado), classificados. O teto de diário do mês
/// corrente é injetado pelo chamador (`annual_metrics`), espelhando o forecast — a Performance
/// do mesmo mês precisa ser idêntica nas duas visões. Para a visão anual das 4 métricas.
pub(crate) async fn load_year_events(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<Vec<forecast::MetricEvent>, String> {
    let start = NaiveDate::from_ymd_opt(year, 1, 1).ok_or("ano inválido")?;
    let end = NaiveDate::from_ymd_opt(year + 1, 1, 1).ok_or("ano inválido")?;
    load_metric_db_events(pool, today, start, end).await
}

#[derive(serde::Serialize)]
pub struct AnnualMetricsDto {
    pub year: i32,
    pub months: Vec<MonthMetricDto>,
}

/// As doze métricas do ano no tipo do MOTOR — a forma que as réguas puras consomem, antes de
/// virar DTO de fio. Quem responde a uma tela usa [`annual_metrics`]; quem aplica uma régua do
/// método (a faixa anual, por exemplo) lê daqui, sem passar pela serialização.
pub(crate) async fn annual_month_metrics(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<Vec<forecast::MonthMetric>, String> {
    let mut events = load_year_events(pool, year, today).await?;
    // Mesmo teto de diário do forecast para o MÊS CORRENTE: sem ele, a Performance do mesmo mês
    // divergia entre a visão anual e o Totais (o teto só existe no caminho do forecast). O
    // project_daily_ceiling já se limita ao fim do mês corrente.
    if year == today.year() {
        let daily_ceiling = projection_daily_ceiling(pool, today).await?;
        // Cobertura de dias do teto = fato COMPORTAMENTAL (o dia teve Diário), sem máscara.
        let days_with_daily: std::collections::HashSet<NaiveDate> = events
            .iter()
            .filter(|me| me.event.kind == forecast::EventKind::Daily)
            .map(|me| me.event.date)
            .collect();
        let month_end = calendar::last_day_of_month(today.year(), today.month());
        // Teto projetado = evento SINTÉTICO → conta em todas as réguas (`RulerMask::ALL`).
        events.extend(forecast::lift_all(&forecast::project_daily_ceiling(
            daily_ceiling,
            today,
            month_end,
            &days_with_daily,
        )));
    }
    let months: Vec<(i32, u32)> = (1..=12).map(|m| (year, m)).collect();
    let annotation = load_economia_annotation(pool, &[year]).await?;
    // O loader de métricas já carrega a máscara de réguas por lançamento em cada evento.
    Ok(forecast::month_metrics_for(
        today,
        &events,
        &months,
        &annotation,
    ))
}

/// Saldos de fim de mês do ano, `(mês, saldo)` em ordem. Os meses já vividos leem a corrente da
/// planilha; do mês corrente em diante manda a projeção, que é quem conhece o que ainda não
/// aconteceu. Ano fechado não consulta a projeção: ela só olha para frente.
pub(crate) async fn annual_month_end(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<Vec<(u32, i64)>, String> {
    let mut by_month: std::collections::BTreeMap<u32, i64> = sheet_month_end_balances(pool, year)
        .await?
        .into_iter()
        .collect();
    if year >= today.year() {
        let fc = forecast_dto(pool, today).await?;
        for m in fc.month_end.iter().filter(|m| m.year == year) {
            by_month.insert(m.month, m.balance_cents);
        }
    }
    Ok(by_month.into_iter().collect())
}

/// Último Saldo importado de cada mês do ano — a mesma leitura que a grade do mês publica,
/// direto da série da planilha.
pub(crate) async fn sheet_month_end_balances(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<(u32, i64)>, String> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance \
         WHERE date >= ?1 AND date <= ?2 ORDER BY date",
    )
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{year:04}-12-31"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("saldos do ano: {e}"))?;

    let mut by_month: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
    for (date, balance) in rows {
        if let Some(month) = date.get(5..7).and_then(|m| m.parse::<u32>().ok()) {
            by_month.insert(month, balance);
        }
    }
    Ok(by_month.into_iter().collect())
}

/// Um mês do ano na ótica do método — o que saiu, se já foi vivido, se sustenta o veredito e
/// quanto faltaria lançar para custar o típico.
#[derive(serde::Serialize)]
pub struct AnnualRulerMonthDto {
    pub month: u32,
    pub outflow_cents: i64,
    pub lived: bool,
    pub suspect: bool,
    pub missing_cents: i64,
}

/// Onde o ano termina, com o cenário em que os meses sem lastro custassem o típico.
#[derive(serde::Serialize)]
pub struct YearEndDto {
    pub end_month: Option<u32>,
    pub end_balance_cents: Option<i64>,
    pub end_balance_typical_cents: Option<i64>,
}

/// A régua anual do método pronta para consumo: o Economizado% que julga, o recorte que o
/// sustenta, o veredito da faixa e o fim do ano. Sai inteira do motor (`forecast::annual_ruler`)
/// — é a MESMA leitura que a conversa responde em `get_year_analysis`, sem segunda composição.
#[derive(serde::Serialize)]
pub struct AnnualRulerDto {
    pub year: i32,
    pub lived_months: u32,
    pub future_months: u32,
    /// Mediana das saídas dos meses vividos: o mês típico contra o qual o futuro é medido.
    pub typical_spend_cents: i64,
    pub income_lived_cents: i64,
    pub economia_lived_cents: i64,
    /// A sobra dos meses vividos — o colchão, distinto da Economia registrada.
    pub surplus_lived_cents: i64,
    pub income_year_cents: i64,
    pub economia_year_cents: i64,
    /// Meses vividos com renda registrada e a média por mês deles — a leitura que compara um ano
    /// com outro sem que o calendário finja uma queda de renda.
    pub recorded_months: u32,
    pub avg_income_cents: i64,
    pub lived_bps: Option<i64>,
    pub projected_bps: Option<i64>,
    /// O percentual que JULGA: o vivido enquanto houver mês sem lastro, senão o do ano.
    pub bps: Option<i64>,
    /// A régua fala do recorte vivido (e não do ano fechado).
    pub scope_lived: bool,
    pub has_data: bool,
    /// Quanto falta guardar para o piso de 20%, nos dois recortes. Negativo = o piso já passou.
    pub shortfall_lived_cents: i64,
    pub shortfall_year_cents: i64,
    pub per_month_shortfall_cents: Option<i64>,
    /// `no_record` · `zero_by_choice` · `below_band` · `in_band` · `above_band`.
    pub verdict: &'static str,
    pub band: BandDto,
    pub months: Vec<AnnualRulerMonthDto>,
    /// Saldo de fim de cada mês do ano (corrente da planilha até o vivido, projeção à frente).
    pub month_end: Vec<MonthEndDto>,
    pub year_end: YearEndDto,
}

/// A faixa do método que o veredito aplica, publicada junto do número que ela julga.
#[derive(serde::Serialize)]
pub struct BandDto {
    pub floor_bps: i64,
    pub target_bps: i64,
    pub ceiling_bps: i64,
}

pub(crate) async fn annual_ruler_dto(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<AnnualRulerDto, String> {
    let metrics = annual_month_metrics(pool, year, today).await?;
    let ruler = forecast::annual_ruler(&metrics, year, today);
    let month_end = annual_month_end(pool, year, today).await?;
    let year_end = forecast::year_end_scenario(&ruler, &month_end);
    // Sem reserva conhecida o veredito não pode ler economia zerada como escolha — é o "não
    // guardou nada" honesto, e é assim que a conversa também a lê.
    let reserve = reserve_reading(pool, today).await?;
    let reserve_months = (reserve.state != "no_record").then_some(reserve.months);

    Ok(AnnualRulerDto {
        year,
        lived_months: ruler.lived_months,
        future_months: ruler.future_months,
        typical_spend_cents: ruler.typical_spend_cents,
        income_lived_cents: ruler.income_lived_cents,
        economia_lived_cents: ruler.economia_lived_cents,
        surplus_lived_cents: ruler.surplus_lived_cents,
        income_year_cents: ruler.income_year_cents,
        economia_year_cents: ruler.economia_year_cents,
        recorded_months: ruler.recorded_months,
        avg_income_cents: ruler.avg_income_cents,
        lived_bps: ruler.lived_bps,
        projected_bps: ruler.projected_bps,
        bps: ruler.bps,
        scope_lived: ruler.scope_lived,
        has_data: ruler.has_data,
        shortfall_lived_cents: ruler.shortfall_lived_cents,
        shortfall_year_cents: ruler.shortfall_year_cents,
        per_month_shortfall_cents: ruler.per_month_shortfall_cents,
        verdict: forecast::band_verdict(&ruler, reserve_months).as_str(),
        band: BandDto {
            floor_bps: SAVINGS_FLOOR_BPS,
            target_bps: SAVINGS_TARGET_BPS,
            ceiling_bps: SAVINGS_CEILING_BPS,
        },
        months: ruler
            .months
            .iter()
            .map(|m| AnnualRulerMonthDto {
                month: m.month,
                outflow_cents: m.outflow_cents,
                lived: m.lived,
                suspect: m.suspect,
                missing_cents: m.missing_cents,
            })
            .collect(),
        month_end: month_end
            .into_iter()
            .map(|(month, balance_cents)| MonthEndDto {
                year,
                month,
                balance_cents,
            })
            .collect(),
        year_end: YearEndDto {
            end_month: year_end.end_month,
            end_balance_cents: year_end.end_balance_cents,
            end_balance_typical_cents: year_end.end_balance_typical_cents,
        },
    })
}

pub(crate) async fn annual_metrics(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<AnnualMetricsDto, String> {
    let months = annual_month_metrics(pool, year, today)
        .await?
        .iter()
        .map(|m| MonthMetricDto {
            year: m.year,
            month: m.month,
            income_cents: m.income_cents,
            income_performance_cents: m.income_performance_cents,
            performance_cents: m.performance_cents,
            cost_of_living_cents: m.cost_of_living_cents,
            fixed_out_cents: m.fixed_out_cents,
            daily_out_cents: m.daily_out_cents,
            daily_avg_out_cents: m.daily_avg_out_cents,
            daily_projected_cents: m.daily_projected_cents,
            cartao_cents: m.cartao_cents,
            real_daily_avg_cents: m.real_daily_avg_cents,
            economia_cents: m.economia_cents,
            patrimonio_cents: m.patrimonio_cents,
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
    month_grid_at(pool, year, month, chrono::Local::now().date_naive()).await
}

pub(crate) async fn month_grid_at(
    pool: &SqlitePool,
    year: i32,
    month: u32,
    today: NaiveDate,
) -> Result<Vec<MonthGridDayDto>, String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or("mês inválido")?;
    let last = calendar::last_day_of_month(year, month);
    let first_s = first.format("%Y-%m-%d").to_string();
    let last_s = last.format("%Y-%m-%d").to_string();
    let end_exclusive = last
        .succ_opt()
        .ok_or("mês inválido para intervalo da grade")?;

    // A grade reaproveita a mesma construção final de eventos do forecast: uma nota itemizada
    // entra pelos seus itens, e a fatura substitui qualquer Cartão cru futuro no vencimento.
    let raw_events = load_db_events(pool, first, end_exclusive, Some(today)).await?;
    let events = finalize_card_events(pool, today, first, end_exclusive, raw_events).await?;
    let mut flows_by_date: std::collections::HashMap<String, (i64, i64, i64)> =
        std::collections::HashMap::new();
    for event in events {
        let entry = flows_by_date
            .entry(event.date.format("%Y-%m-%d").to_string())
            .or_default();
        match event.kind {
            forecast::EventKind::Income => entry.0 += event.amount_cents,
            forecast::EventKind::FixedOut | forecast::EventKind::Cartao => {
                entry.1 += event.amount_cents
            }
            forecast::EventKind::Daily => entry.2 += event.amount_cents,
            forecast::EventKind::Economia | forecast::EventKind::Patrimonio => {}
        }
    }

    // Saldo da planilha por dia.
    let balances: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date BETWEEN ?1 AND ?2",
    )
    .bind(&first_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("month balances: {e}"))?;

    let balance_of = |d: &str| balances.iter().find(|b| b.0 == d).map(|b| b.1);

    let n_days = (last - first).num_days() + 1;
    let mut grid = Vec::with_capacity(n_days as usize);
    for offset in 0..n_days {
        let date = first + chrono::Duration::days(offset);
        let date_s = date.format("%Y-%m-%d").to_string();
        let (income, fixed_out, daily_out) =
            flows_by_date.get(&date_s).copied().unwrap_or_default();
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

#[tauri::command]
pub async fn get_annual_ruler(
    pool: State<'_, SqlitePool>,
    year: i32,
) -> Result<AnnualRulerDto, String> {
    annual_ruler_dto(pool.inner(), year, chrono::Local::now().date_naive()).await
}

// --- Dashboard query commands ---

#[derive(serde::Serialize)]
pub struct UpcomingInvoiceDto {
    pub account_id: String,
    pub card_name: String,
    pub due_date: String,
    pub amount_cents: i64,
    pub status: String,
    pub owner_name: String,
    /// Existe Entrada vinculada à fatura (`refund_invoice_id`) — etiqueta "Reembolso" na Hoje.
    pub has_refund_expectation: bool,
    pub refund_expected_cents: i64,
}

#[derive(serde::Serialize)]
pub struct CeilingEstimateJson {
    /// Gasto variável somado do mês anterior (magnitude).
    pub variable_cents: i64,
    /// Dias do mês anterior — o divisor da média.
    pub days: i64,
    /// Mês da base, `YYYY-MM`.
    pub month: String,
}

#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    /// Procedência do teto exibido: `chosen` (veredito) · `estimate` (média do mês anterior,
    /// com selo) · `none` (sem registro).
    pub daily_ceiling_source: String,
    /// Operandos da estimativa do teto, para a tela IMPRIMIR a conta em vez de descrevê-la.
    /// `None` quando o teto é escolhido: número digitado não tem conta a mostrar.
    pub daily_ceiling_estimate: Option<CeilingEstimateJson>,
    /// Overlay: existe proposta da cerimônia do teto aguardando confirmação do dono.
    pub ceiling_proposal_pending: bool,
    pub daily_spend_today: i64,
    /// Compras de cartão realizadas HOJE (magnitude) — o total do bloco do dia no modo cartão.
    pub card_spend_today_cents: i64,
    pub reserve_months: f64,
    /// Estado epistêmico da reserva: `verdict` (mediana de 6 meses completos) · `estimate`
    /// ("retrato vivo", 1–5 meses) · `zero` (contas de reserva zeradas) · `no_record`.
    pub reserve_state: String,
    /// Meses completos que sustentam o custo de vida da régua (base do retrato vivo).
    pub reserve_basis_months: i64,
    /// Alvo da reserva em dinheiro (custo de vida × meses mínimos do método). Leitura
    /// patrimonial: a reserva socorre o saldo negativo, não trava o teto do dia.
    pub reserve_target_cents: i64,
    /// Quanto a reserva passa do alvo — o excedente que o método manda olhar para decidir o
    /// próximo movimento. `null` enquanto ela ainda está sendo construída.
    pub reserve_surplus_cents: Option<i64>,
    pub reserve_trend: String,
    /// Modo de gasto: `debit` · `card`.
    pub spending_mode: String,
    /// `false` quando o modo é o default de dado insuficiente. A tela precisa da procedência para
    /// não apresentar como leitura dos dados o valor que sobra quando o motor não sabe.
    pub spending_mode_detected: bool,
    /// Gate composto de legitimidade do modo cartão: economia anual e reserva precisam estar vivas.
    pub card_gate: String,
    /// Perna de economia do gate de legitimidade do modo cartão.
    pub card_gate_economy: String,
    /// Percentual bruto (bps) por trás da perna de economia — a matemática que a tela Cartões
    /// mostra ("14%, falta 6 p/ 20%"), não só o veredito. `None` só quando a perna é `unknown`
    /// (sem renda anual para dividir — nunca um número fabricado).
    pub card_gate_economy_bps: Option<i64>,
    /// Perna de reserva do gate de legitimidade do modo cartão.
    pub card_gate_reserve: String,
    /// Cartão do mês corrente (realizado + projetado), magnitude — o que o dia lê no modo cartão.
    pub cartao_month_cents: i64,
    /// Próximo dia de fatura a partir de hoje (`YYYY-MM-DD`), quando existe.
    pub next_fatura_date: Option<String>,
    pub next_fatura_amount_cents: i64,
    /// Próxima fatura de cada cartão, ordenada por vencimento e nome do cartão.
    pub upcoming_invoices: Vec<UpcomingInvoiceDto>,
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

    // `balance` is the projected end-of-current-month figure (the method's hero),
    // not the raw current account sum.
    let fc = forecast::project(seed, today_naive, &all_events, horizon_end);
    let projected_balance = fc
        .month_end
        .iter()
        .find(|m| m.year == today_naive.year() && m.month == today_naive.month())
        .map(|m| m.balance_cents)
        .or_else(|| fc.daily.last().map(|p| p.balance_cents))
        .unwrap_or(seed);

    // Teto do diário exibido no tile "Diário de hoje" (`de R$X`), com PROCEDÊNCIA explícita:
    // escolhido · estimado pela média do mês anterior · sem registro. Mesma fonte do driver de
    // projeção (`effective_daily_ceiling` = escolhido → média). A proposta da cerimônia é um
    // overlay de confirmação — nunca o número.
    let ceiling = daily_ceiling_reading(pool, today_naive).await?;
    let daily_budget = ceiling.per_day_cents;
    let ceiling_proposal_pending = has_pending_ceiling_proposal(pool).await?;
    let mode = spending_mode_summary(pool, today_naive).await?;
    let (has_card, active_invoices) =
        load_card_invoice_events(pool, today_naive, today_naive, None).await?;

    // Diário de HOJE como MAGNITUDE positiva (o card faz `teto - gasto` e `gasto/teto`).
    // - Mesma fonte que a régua Diário médio (`effective_daily_ceiling`/`prev_month_daily_avg`):
    //   eventos de métrica com a máscara `daily_avg`, item por item — uma linha excluída da régua
    //   (`exclude_from_daily_avg`) tira o dia de contar duas vezes o mesmo jeito que a visão
    //   mensal/anual já tira.
    // - `amount_cents` já chega como MAGNITUDE (`load_raw_db_events` aplica `.abs()`), então a
    //   soma nunca cancela parcialmente num dia de sinal misto — mesma invariante de
    //   `realized_monthly_baseline`/`month_grid`, herdada da fonte única de eventos de métrica.
    // - Fonte única: o gasto do dia vem das transações Diário (despesa variável não-crédito);
    //   sem nenhuma transação no dia, a soma é 0. O ritual diário é uma transação Diário comum.
    let daily_spend_cents: i64 = load_metric_db_events(
        pool,
        today_naive,
        today_naive,
        today_naive.succ_opt().ok_or("data de hoje inválida")?,
    )
    .await?
    .into_iter()
    .filter(|me| {
        me.mask.daily_avg && me.event.kind == forecast::EventKind::Daily && me.event.realized
    })
    .map(|me| me.event.amount_cents)
    .sum();
    let daily_spend: (i64,) = (daily_spend_cents,);

    // Espelho do `daily_spend` para o modo cartão: compras de cartão realizadas HOJE — manual
    // (`payment_method='credit'`) ou vinculada a fatura (`invoice_id`). O lump importado do
    // vencimento (célula da planilha) tem `payment_method` NULL e nunca vira compra, então o
    // pagamento da fatura não conta como gasto do dia.
    let card_spend: (i64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(ABS(amount)) FROM \"transaction\" \
                          WHERE type='expense' AND is_projection=0 AND date = ?1 \
                            AND (payment_method = 'credit' OR invoice_id IS NOT NULL) \
                            AND scenario_id IS NULL), 0)",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query card spend: {e}"))?;

    // Reserva em MESES de custo de vida (método) — espelha os R$ que o PocketsCard mostra.
    let reserve = reserve_reading(pool, today_naive).await?;

    // Transações já realizadas: por DATA (≤ hoje), não pelo `is_projection` congelado (stale
    // quando o dono não re-importa por dias — auditoria de robustez a edições).
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM \"transaction\" WHERE date <= ?1 AND scenario_id IS NULL",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    // Data do lançamento REAL mais recente (não-projeção, ≤ hoje) — alimenta o aviso "lançou
    // pela última vez há X dias" do dashboard. NULL quando ainda não há lançamentos reais.
    let last_real: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT MAX(date) FROM \"transaction\" WHERE is_projection = 0 AND date <= ?1 AND scenario_id IS NULL",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query last_real_tx_date: {e}"))?;
    let last_real_tx_date = last_real.and_then(|(d,)| d);

    // Gate de legitimidade do modo cartão: a economia 20–30% precisa estar VIVA. A perna lê a
    // régua anual do motor — a mesma que a conversa e a tela do ano publicam —, para que o gate
    // nunca declare viva uma economia que a pessoa está vendo abaixo da faixa.
    let (_, annual_ruler) = annual_ruler_reading(pool, today_naive.year(), today_naive).await?;
    let card_gate_economy_bps = annual_ruler.lived_bps;
    let card_gate_reserve_months = (reserve.state != "no_record").then_some(reserve.months);
    let card_gate = crate::cards::card_gate(card_gate_economy_bps, card_gate_reserve_months);
    let card_gate_economy = crate::cards::economy_gate_leg(card_gate_economy_bps);
    let card_gate_reserve = crate::cards::reserve_gate_leg(card_gate_reserve_months);

    let mut seen_accounts = std::collections::HashSet::new();
    let upcoming_invoices: Vec<UpcomingInvoiceDto> = active_invoices
        .iter()
        // Fatura zerada preserva a estrutura mensal, não um compromisso: não ocupa a vaga da real.
        .filter(|invoice| invoice.amount_cents != 0)
        .filter(|invoice| seen_accounts.insert(invoice.account_id.clone()))
        .map(|invoice| UpcomingInvoiceDto {
            account_id: invoice.account_id.clone(),
            card_name: invoice.card_name.clone(),
            due_date: invoice.due_date.format("%Y-%m-%d").to_string(),
            amount_cents: invoice.amount_cents,
            status: crate::cards::invoice_status(
                today_naive,
                invoice.closing_date,
                invoice.due_date,
            )
            .as_str()
            .to_string(),
            owner_name: invoice.owner_name.clone(),
            has_refund_expectation: invoice.has_refund_expectation,
            refund_expected_cents: invoice.refund_expected_cents,
        })
        .collect();
    let next_fatura = if has_card {
        active_invoices
            .iter()
            .find(|invoice| invoice.amount_cents != 0)
            .map(|first| {
                let amount_cents = active_invoices
                    .iter()
                    .filter(|invoice| {
                        invoice.due_date == first.due_date && invoice.amount_cents != 0
                    })
                    .map(|invoice| invoice.amount_cents)
                    .sum();
                (first.due_date, amount_cents)
            })
    } else {
        mode.next_fatura.as_ref().and_then(|(date, amount_cents)| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, *amount_cents))
        })
    };

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        daily_ceiling_source: ceiling.source.as_str().to_string(),
        daily_ceiling_estimate: ceiling
            .estimate_basis
            .as_ref()
            .map(|b| CeilingEstimateJson {
                variable_cents: b.variable_cents,
                days: b.days,
                month: b.month.clone(),
            }),
        ceiling_proposal_pending,
        daily_spend_today: daily_spend.0,
        card_spend_today_cents: card_spend.0,
        reserve_months: reserve.months,
        reserve_state: reserve.state.to_string(),
        reserve_basis_months: reserve.basis_months,
        reserve_target_cents: reserve.target_cents,
        reserve_surplus_cents: reserve.surplus_cents,
        reserve_trend: reserve.trend,
        spending_mode: mode.mode.as_str().to_string(),
        spending_mode_detected: mode.detected,
        card_gate: card_gate.as_str().to_string(),
        card_gate_economy: card_gate_economy.as_str().to_string(),
        card_gate_economy_bps,
        card_gate_reserve: card_gate_reserve.as_str().to_string(),
        cartao_month_cents: mode.cartao_month_cents,
        next_fatura_date: next_fatura
            .as_ref()
            .map(|(date, _)| date.format("%Y-%m-%d").to_string()),
        next_fatura_amount_cents: next_fatura
            .map(|(_, amount_cents)| amount_cents)
            .unwrap_or(0),
        upcoming_invoices,
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

    /// Liga uma tag "Ignorar" (4 réguas desligadas = semântica do flag único antigo) a um lançamento.
    async fn tag_as_excluded(pool: &SqlitePool, txn_id: &str) {
        sqlx::query(
            "INSERT INTO tag (id, name, exclude_from_performance, exclude_from_cost_of_living, \
                              exclude_from_savings, exclude_from_daily_avg) \
             VALUES ('tg-ignore', 'Ignorar', 1, 1, 1, 1)",
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

    // Uma despesa FUTURA marcada com tag "Ignorar" (4 réguas desligadas) continua pesando no Saldo
    // PROJETADO — o dinheiro vai sair da conta de qualquer forma. A tag só mascara as MÉTRICAS
    // (Performance/Custo de vida), nunca a visão de CAIXA. Por isso `load_cashflow_events` não
    // carrega máscara de réguas. Este teste guarda os DOIS lados: caixa inclui, métrica zera.
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

        // Lado MÉTRICA: uma despesa REALIZADA "Ignorar" (4 réguas desligadas) do mês corrente NÃO
        // some do stream — entra com a máscara 4× desligada, então o motor a zera em todas as
        // réguas. A exclusão virou máscara por evento, não filtro no SQL do loader.
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
        assert_eq!(
            metric.len(),
            1,
            "a linha 'Ignorar' entra no stream de métricas"
        );
        let masked = &metric[0];
        assert_eq!(masked.event.amount_cents, 7000);
        assert!(
            !masked.mask.performance
                && !masked.mask.cost_of_living
                && !masked.mask.savings
                && !masked.mask.daily_avg,
            "a máscara 4× desligada tira a linha de todas as réguas"
        );
    }

    // Em 1º de JANEIRO, `realized_annual_economia` precisa da MESMA janela
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

    // A renda-base do guardrail (`realized_annual_savings().0`) é a MESMA soma que
    // `guardrail_window_metrics` devolve para `income_cents` — não uma query paralela com sua
    // própria janela. Cobre meio de ano (múltiplos meses completos) e a virada de janeiro
    // (janela deslocada para dezembro), para travar que as duas leituras nunca divergem.
    #[tokio::test]
    async fn annual_income_matches_guardrail_window_metrics_sum_mid_year() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();

        insert_income(&p, "inc-jan", 100_000, "2026-01-10").await;
        insert_income(&p, "inc-feb", 50_000, "2026-02-10").await;
        insert_income(&p, "inc-mar", 70_000, "2026-03-10").await;
        // Mês corrente (abril) não é completo — não pode entrar na renda-base.
        insert_income(&p, "inc-abr", 999_000, "2026-04-05").await;

        let (income, _) = realized_annual_savings(&p, today).await.unwrap();
        let expected: i64 = guardrail_window_metrics(&p, today)
            .await
            .unwrap()
            .iter()
            .map(|m| m.income_cents)
            .sum();

        assert_eq!(income, 220_000, "só jan+fev+mar (meses completos) contam");
        assert_eq!(
            income, expected,
            "renda-base do guardrail é a mesma soma de MonthMetric via guardrail_window"
        );
    }

    #[tokio::test]
    async fn annual_income_matches_guardrail_window_metrics_sum_january() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        insert_income(&p, "inc-dez", 80_000, "2025-12-15").await;

        let (income, _) = realized_annual_savings(&p, today).await.unwrap();
        let expected: i64 = guardrail_window_metrics(&p, today)
            .await
            .unwrap()
            .iter()
            .map(|m| m.income_cents)
            .sum();

        assert_eq!(income, 80_000, "1º/jan enxerga dezembro do ano anterior");
        assert_eq!(
            income, expected,
            "mesma paridade também na virada de janeiro"
        );
    }

    // O Economizado% (`savings_rate_bps`) reflete a ANOTAÇÃO da aba Economia mesmo
    // quando o dono poupa só via Saída no grid (sem transfer de reserva → nenhum
    // `EventKind::Economia`); ignorar a anotação deixaria `economia` e `savings_rate_bps` zerados.
    #[tokio::test]
    async fn savings_rate_reflects_annotation() {
        let income = forecast::CashflowEvent {
            date: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
            kind: forecast::EventKind::Income,
            amount_cents: 100_000,
            realized: true,
        };
        let mut annotation = std::collections::HashMap::new();
        annotation.insert((2026, 3u32), 25_000i64); // anotou 250,00 de economia em março

        let today = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let metrics = forecast::month_metrics_for(
            today,
            &forecast::lift_all(std::slice::from_ref(&income)),
            &[(2026, 3)],
            &annotation,
        );

        assert_eq!(metrics.len(), 1);
        assert_eq!(
            metrics[0].economia_cents, 25_000,
            "economia_cents = anotação"
        );
        // Economizado% = 25.000 / 100.000 = 25% = 2500 bps.
        assert_eq!(metrics[0].savings_rate_bps, 2500);
    }

    // Uma Saída itemizada é atribuída por seção, sem contar o pai de novo.
    #[tokio::test]
    async fn annual_metrics_attributes_line_items_by_section_without_double_counting_parent() {
        let p = pool().await;

        sqlx::query("INSERT INTO person (id, name) VALUES ('person-card', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id) \
             VALUES ('card-known', 'cartao', 'credit_card', 'person-card')",
        )
        .execute(&p)
        .await
        .unwrap();
        // A fatura do ciclo é o que legitima a linha CARTÕES: como Cartao (não só o alias
        // conhecido) — sem ela o item cairia em Saída fixa, que é o comportamento correto para
        // uma proposta aceita sem fatura ainda, mas não é o que este teste quer exercitar (a
        // atribuição por seção das 5 categorias).
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('invoice-card-known', 'card-known', '2026-03', '2026-02-20', '2026-03-15')",
        )
        .execute(&p)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('income-060', 'income', 1000000, '2026-03-01', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('expense-060', 'expense', 800000, '2026-03-15', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        for (position, section, amount, description) in [
            (0, "CONTAS:", 300_000, "fixo"),
            (1, "DIÁRIO:", 200_000, "variavel"),
            (2, "CARTÕES:", 150_000, "cartao"),
            (3, "ECONOMIA:", 100_000, "reserva"),
            (4, "INVESTIMENTO:", 50_000, "longo prazo"),
        ] {
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
                 VALUES (?1, 'expense-060', ?2, ?3, ?4, ?5)",
            )
            .bind(format!("li-060-{position}"))
            .bind(amount)
            .bind(description)
            .bind(position)
            .bind(section)
            .execute(&p)
            .await
            .unwrap();
        }

        let annual = annual_metrics(&p, 2026, NaiveDate::from_ymd_opt(2026, 3, 31).unwrap())
            .await
            .unwrap();
        let mar = annual.months.iter().find(|m| m.month == 3).unwrap();

        assert_eq!(mar.fixed_out_cents, 300_000);
        assert_eq!(mar.daily_out_cents, 200_000);
        assert_eq!(mar.cartao_cents, 150_000);
        assert_eq!(mar.economia_cents, 100_000);
        assert_eq!(mar.patrimonio_cents, 50_000);
        assert_eq!(mar.cost_of_living_cents, 650_000); // 300 + 200 + 150
        assert_eq!(mar.savings_rate_bps, 1_000); // 100 / 1000 = 10%
        assert_eq!(
            mar.performance_cents, 200_000,
            "performance conta os itens uma vez: 1000 - 800"
        );
    }

    // `realized_annual_economia` (numerador do guardrail `safe_to_spend`) soma a
    // ANOTAÇÃO da aba Economia + os transfers de reserva REAIS. Sem isso, quem poupa só
    // via Saída no grid via guardrail com numerador 0 e era restringido indevidamente.
    #[tokio::test]
    async fn realized_annual_economia_includes_annotation() {
        let p = pool().await;

        // Anotação jan–mai/2026 = 5 × 100,00.
        for m in 1..=5i64 {
            sqlx::query(
                "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
                 VALUES ('', 2026, ?1, 10000, '2026-06-01T00:00:00Z')",
            )
            .bind(m)
            .execute(&p)
            .await
            .unwrap();
        }

        // Transfer de reserva MANUAL em março: conta reserva + transação transfer.
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
             VALUES ('econ-mar', 'transfer', 8000, '2026-03-20', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let economia = realized_annual_economia(&p, today).await.unwrap();
        // Regra do MÁXIMO por mês (espelha o mensal). Março vale
        // max(transfer 8.000, anotação 10.000) = 10.000 — a aba é o registro consolidado do mês,
        // não uma parcela aditiva. Total: 5 × 10.000 = 50.000; somar os dois sinais produziria
        // 58.000 e contaria duas vezes o dinheiro que o write-back deriva dos lançamentos.
        assert_eq!(economia, 50_000);
    }

    // Economia e Patrimônio anuais do guardrail somam `MonthMetric` (o mesmo motor da tela do Ano),
    // filtrado por `guardrail_window`: um mês FUTURO fica fora, o mês CORRENTE (incompleto) fica
    // fora, e só os meses completos do ano contam. A classificação em si (item de nota, transfer de
    // reserva/ilíquido, máscara de tag, regra do máximo) é coberta pelos testes do motor mensal —
    // este teste prova só a JANELA, não reprova a classificação.
    #[tokio::test]
    async fn annual_economia_and_patrimonio_respect_guardrail_window() {
        let p = pool().await;

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
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-prev', 'Previdência', 'pension', 'pe-1', 0, 'illiquid')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Fevereiro (mês COMPLETO): transfer → reserva (Economia) de 200,00 — dentro da janela.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-fev-econ', 'transfer', 20000, '2026-02-15', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Março (mês COMPLETO): transfer → ilíquido (Patrimônio) de 500,00 — dentro da janela.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-mar-patr', 'transfer', 50000, '2026-03-15', 'acc-prev', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Junho (mês CORRENTE, incompleto): mesmos dois movimentos — devem ficar de fora.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-jun-econ', 'transfer', 999900, '2026-06-05', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-jun-patr', 'transfer', 888800, '2026-06-05', 'acc-prev', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Agosto (FUTURO): mesmos dois movimentos — devem ficar de fora.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-ago-econ', 'transfer', 70000, '2026-08-15', 'acc-res', 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-ago-patr', 'transfer', 60000, '2026-08-15', 'acc-prev', 1)",
        )
        .execute(&p)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let economia = realized_annual_economia(&p, today).await.unwrap();
        let patrimonio = realized_annual_patrimonio(&p, today).await.unwrap();
        assert_eq!(economia, 20_000, "só fevereiro (mês completo) conta");
        assert_eq!(patrimonio, 50_000, "só março (mês completo) conta");
    }

    // Célula 100,00 com nota somando 120,00 — o resíduo −20,00
    // entra como Saída fixa COM SINAL para os baldes fecharem com o total da célula (a
    // convenção AJUSTES "Diferença" da planilha aplicada na leitura, sem item sintético).
    #[tokio::test]
    async fn metric_events_reconcile_item_residual_with_sign() {
        let p = pool().await;
        sqlx::query("INSERT INTO person (id, name) VALUES ('person-card', 'Pessoa')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id) \
             VALUES ('card-known', 'Banco A', 'credit_card', 'person-card')",
        )
        .execute(&p)
        .await
        .unwrap();
        // A fatura do ciclo é o que legitima a linha como Cartao (chave: fatura existente, não só
        // alias conhecido) — este teste exercita o resíduo com sinal, não o domínio de fatura.
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date) \
             VALUES ('invoice-card-known', 'card-known', '2026-03', '2026-02-20', '2026-03-10')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('t-res', 'expense', 10000, '2026-03-10', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('li:t-res:1', 't-res', 12000, 'Banco A', 1, 0, 'CARTÕES')",
        )
        .execute(&p)
        .await
        .unwrap();

        let start = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let events = load_metric_db_events(&p, today, start, end).await.unwrap();

        let cartao: i64 = events
            .iter()
            .filter(|me| me.event.kind == forecast::EventKind::Cartao)
            .map(|me| me.event.amount_cents)
            .sum();
        let fixed: i64 = events
            .iter()
            .filter(|me| me.event.kind == forecast::EventKind::FixedOut)
            .map(|me| me.event.amount_cents)
            .sum();
        assert_eq!(cartao, 12_000);
        assert_eq!(fixed, -2_000, "resíduo com sinal reduz a Saída fixa");
        assert_eq!(
            cartao + fixed,
            10_000,
            "os baldes fecham com o total da célula"
        );
    }

    // A anotação e os transfers reais são DISJUNTOS: `store_economia_entries`
    // grava só em `economia_annotation`, jamais um transfer fantasma em `transaction`. Logo
    // `realized_annual_economia` conta a anotação UMA vez (sem dupla contagem) e o lado-transfer fica 0.
    #[tokio::test]
    async fn annotation_and_transfer_no_double_count() {
        let p = pool().await;

        let written =
            crate::commands::write_back_cmds::store_economia_entries(&p, &[(2026, 3, 20000)])
                .await
                .unwrap();
        assert_eq!(written, 1);

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let economia = realized_annual_economia(&p, today).await.unwrap();
        assert_eq!(
            economia, 20_000,
            "só a anotação conta (sem transfer fantasma)"
        );

        let (transfers,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE type='transfer'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            transfers, 0,
            "a anotação não gera nenhum transfer em `transaction`"
        );
    }

    #[tokio::test]
    async fn month_grid_expense_total_is_magnitude_regardless_of_sign() {
        // Imported expenses are stored negative (`-amount_out`), while manual expenses are
        // positive. `month_grid` must sum magnitudes so both sources add up correctly.
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

    // `realized_monthly_baseline` precisa somar magnitudes: despesas importadas são negativas e
    // manuais positivas; `SUM(amount)` mistura sinais, cancela valores e corrompe o reserve floor.
    #[tokio::test]
    async fn realized_monthly_baseline_sums_magnitudes_not_signed_amounts() {
        let p = pool().await;
        // `today` num mês posterior → o mês de teste já está completo (antes de cur_ym-01).
        let today = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        // Despesa importada: valor negativo, simulando -amount_out do import.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('imp-bl', 'expense', -90000, '2026-04-10', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Despesa manual: valor positivo.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('man-bl', 'expense', 60000, '2026-04-20', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Só um mês na janela → mediana = total do mês. A soma das magnitudes é
        // ABS(-90000) + 60000 = 150_000; a soma assinada seria -30_000.
        let baseline = realized_monthly_baseline(&p, today).await.unwrap();
        assert_eq!(
            baseline, 150_000,
            "realized_monthly_baseline must sum magnitudes (ABS), not signed amounts"
        );

        // O alvo da reserva deriva da mesma baseline: positivo e coerente (não negativo, como
        // aconteceria com a baseline corrompida).
        let reserve = reserve_reading(&p, today).await.unwrap();
        assert_eq!(reserve.target_cents, 150_000 * forecast::RESERVE_MIN_MONTHS);
    }

    // --- Quebra por categoria do orçamento Diário ---

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

    // `daily_spend_today` precisa somar a MAGNITUDE de cada linha do dia
    // (`SUM(ABS(amount))`), não o ABS da soma assinada. Despesas importadas chegam negativas e
    // lançamentos manuais positivos: num dia misto, `ABS(SUM(...))` cancelaria parcialmente antes
    // do ABS e sub-reportaria o "Diário de hoje".
    #[tokio::test]
    async fn daily_spend_today_sums_magnitudes_not_signed_amounts() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let today_str = today.format("%Y-%m-%d").to_string();

        // Conta com saldo: dá ao `dashboard_summary` um seed de projeção válido.
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-1', 'Corrente', 'bank', 'pe-1', 100000, 'liquid')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Despesa IMPORTADA do dia, gravada NEGATIVA (simula `-amount_out` da planilha).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('imp-1', 'expense', -5000, ?1, 0, 0)",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();
        // Despesa MANUAL do dia, gravada POSITIVA (magnitude).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('man-1', 'expense', 3000, ?1, 0, 0)",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();

        // Soma das magnitudes: 5000 + 3000 = 8000. `ABS(SUM(amount))` daria
        // ABS(-5000 + 3000) = 2000.
        assert_eq!(
            summary.daily_spend_today, 8000,
            "Diário de hoje soma magnitudes (SUM(ABS)), não o ABS da soma assinada"
        );
    }

    // O fallback de média do mês anterior em `effective_daily_ceiling` precisa somar a
    // MAGNITUDE de cada linha (`SUM(ABS(amount))`), não o ABS da soma assinada. Despesas importadas
    // chegam negativas e manuais positivas; num mês de sinal misto o ABS externo da soma cancelaria
    // parcialmente e sub-reportaria o teto diário.
    #[tokio::test]
    async fn effective_daily_ceiling_sums_magnitudes_not_signed_amounts() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Mês anterior COMPLETO = maio/2026 (31 dias). Sem orçamento explícito → cai no fallback.
        // Despesa de diário IMPORTADA (negativa) + MANUAL (positiva), ambas não-fixas, não-crédito.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('prev-imp', 'expense', -6200, '2026-05-10', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('prev-man', 'expense', 3100, '2026-05-20', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let ceiling = effective_daily_ceiling(&p, today).await.unwrap();
        // Soma das magnitudes = 6200 + 3100 = 9300; ÷ 31 dias de maio = 300.
        // `ABS(SUM(amount))` daria ABS(-6200 + 3100) = 3100 ÷ 31 = 100.
        assert_eq!(
            ceiling, 300,
            "o teto diário soma magnitudes (SUM(ABS)), não o ABS da soma assinada"
        );
    }

    // `prev_month_daily_avg` (Conta A, teto do dia) e `month_metrics_for` (Conta B, decompositor +
    // máscara por trás da visão mensal/anual) leem o MESMO lançamento e precisam concordar: uma
    // célula de Diário ITEMIZADA cujo pai carrega `exclude_from_daily_avg` some do numerador em
    // AMBAS as contas, porque o item herda a máscara do `transaction_id` do pai
    // (`load_ruler_mask_map` / `load_db_metric_events`) — a mesma fonte alimenta as duas.
    #[tokio::test]
    async fn daily_ceiling_and_daily_avg_ruler_agree_on_excluded_tag() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Célula de Diário ITEMIZADA de maio/2026 (mês anterior COMPLETO, 31 dias): um item na
        // seção "Diário" sob um lançamento marcado com uma tag que desliga só a régua de diário
        // médio (as outras 3 réguas seguem ligadas — não é a tag "Ignorar" de 4 flags).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('itm-parent', 'expense', 10000, '2026-05-10', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('itm-1', 'itm-parent', 10000, 'Mercado', 0, 'Diário')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tag (id, name, exclude_from_performance, exclude_from_cost_of_living, \
                              exclude_from_savings, exclude_from_daily_avg) \
             VALUES ('tg-daily-excl', 'Fora do diário médio', 0, 0, 0, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES ('itm-parent', 'tg-daily-excl')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Conta A: fallback do teto do dia.
        let basis = prev_month_daily_avg(&p, today).await.unwrap().unwrap();

        // Conta B: mesma janela (maio/2026) pelo decompositor + máscara de tags, como a visão
        // anual lê (`month_metrics_for`).
        let start = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let metric_events = load_db_metric_events(&p, start, end, None).await.unwrap();
        let months = forecast::month_metrics_for(
            today,
            &metric_events,
            &[(2026, 5)],
            &std::collections::HashMap::new(),
        );
        let month_b = &months[0];

        assert_eq!(
            basis.per_day_cents, month_b.real_daily_avg_cents,
            "teto do dia (Conta A, R${}) e diário médio do mês (Conta B, R${}) devem concordar \
             sobre o mesmo item excluído da régua de diário médio",
            basis.per_day_cents, month_b.real_daily_avg_cents
        );
        assert_eq!(
            basis.variable_cents, month_b.daily_avg_out_cents,
            "o numerador de maio também deve concordar: o item com a tag não conta em nenhuma \
             das duas contas"
        );
    }

    // Mesma invariante do teste acima, mas sobre uma célula SIMPLES (sem itemização): a tag
    // `exclude_from_daily_avg` vive direto no lançamento, sem `line_item`/`section` no caminho.
    #[tokio::test]
    async fn daily_ceiling_excludes_simple_cell_with_exclude_tag() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('simple-excl', 'expense', 8000, '2026-05-10', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tag (id, name, exclude_from_performance, exclude_from_cost_of_living, \
                              exclude_from_savings, exclude_from_daily_avg) \
             VALUES ('tg-daily-excl', 'Fora do diário médio', 0, 0, 0, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES ('simple-excl', 'tg-daily-excl')",
        )
        .execute(&p)
        .await
        .unwrap();

        let basis = prev_month_daily_avg(&p, today).await.unwrap().unwrap();
        assert_eq!(
            basis.variable_cents, 0,
            "a célula simples sob a tag some do numerador, como a célula itemizada"
        );
    }

    // Espelha a invariante do teto no numerador de "Diário de hoje": uma despesa Diário lançada
    // HOJE sob `exclude_from_daily_avg` não deve contar no gasto do dia — a mesma tag que já tira
    // a linha do teto do mês tira também o que ela mede hoje.
    #[tokio::test]
    async fn dashboard_daily_spend_today_excludes_daily_avg_tag() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        insert_expense(&p, "today-excl", 4000, "2026-06-15").await;
        sqlx::query("UPDATE \"transaction\" SET is_projection = 0 WHERE id = 'today-excl'")
            .execute(&p)
            .await
            .unwrap();
        tag_as_excluded(&p, "today-excl").await;

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(
            summary.daily_spend_today, 0,
            "gasto Diário de hoje sob a tag de exclusão não conta no numerador"
        );
    }

    // Item de seção Economia/Patrimônio dentro de uma célula itemizada nunca conta como gasto do
    // Diário no teto — mesma regra de `realized_monthly_baseline` (custo de vida exclui Economia).
    #[tokio::test]
    async fn prev_month_daily_avg_excludes_economia_item_in_itemized_cell() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('mixed-parent', 'expense', 15000, '2026-05-10', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('mixed-diario', 'mixed-parent', 10000, 'Mercado', 0, 'Diário')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('mixed-economia', 'mixed-parent', 5000, 'Reserva', 1, 'Economia')",
        )
        .execute(&p)
        .await
        .unwrap();

        let basis = prev_month_daily_avg(&p, today).await.unwrap().unwrap();
        assert_eq!(
            basis.variable_cents, 10000,
            "só o item Diário conta no teto — o item Economia nunca é gasto do dia"
        );
    }

    // `realized_annual_savings` exclui as linhas de uma tag "Ignorar" (4 réguas desligadas), em
    // paridade com `load_year_events`/`annual_metrics`. Uma linha marcada cai fora da métrica; se
    // entrasse no net de poupança realizada, o guardrail e o painel de métricas divergiriam.
    #[tokio::test]
    async fn realized_annual_savings_excludes_ignorar_tagged_rows() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Janela = meses completos de 2026: [2026-01-01, 2026-06-01). Renda + despesa em maio.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('inc-keep', 'income', 100000, '2026-05-05', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('exp-keep', 'expense', 40000, '2026-05-06', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // Baseline sem tags: renda 100000, net = 100000 − 40000 = 60000.
        let (income, savings) = realized_annual_savings(&p, today).await.unwrap();
        assert_eq!((income, savings), (100000, 60000));

        // Linha extra de renda E de despesa, ambas marcadas "Ignorar" — não podem contar.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('inc-ign', 'income', 50000, '2026-05-07', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        tag_as_excluded(&p, "inc-ign").await; // cria a tag 'tg-ignore' e marca a renda
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES ('exp-ign', 'expense', 70000, '2026-05-08', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES ('exp-ign','tg-ignore')",
        )
        .execute(&p)
        .await
        .unwrap();

        // Os totais NÃO mudam: as linhas "Ignorar" são filtradas como em `load_year_events`.
        let (income2, savings2) = realized_annual_savings(&p, today).await.unwrap();
        assert_eq!(
            (income2, savings2),
            (100000, 60000),
            "linhas 'Ignorar' não entram na poupança realizada (paridade com load_year_events)"
        );
    }

    // --- realized_savings_baseline: medianas de renda e economia dos meses ativos ---

    async fn insert_income(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) \
             VALUES (?1, 'income', ?2, ?3, 0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Transfer para a conta de reserva `acc-res` (classificada como `EventKind::Economia`);
    /// cria pessoa + conta na primeira chamada.
    async fn insert_economia_transfer(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query("INSERT OR IGNORE INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR IGNORE INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-res', 'Reserva', 'savings', 'pe-1', 0, 'reserve')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES (?1, 'transfer', ?2, ?3, 'acc-res', 0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn annotate_economia(pool: &SqlitePool, year: i32, month: u32, cents: i64) {
        sqlx::query(
            "INSERT INTO economia_annotation (profile_id, year, month, amount_cents, updated_at) \
             VALUES ('', ?1, ?2, ?3, '2026-01-01T00:00:00Z')",
        )
        .bind(year as i64)
        .bind(month as i64)
        .bind(cents)
        .execute(pool)
        .await
        .unwrap();
    }

    // A economia mensal usa `max(derivado, anotação)` por mês — a mesma regra do motor mensal.
    // Um mês cuja anotação da aba supera os transfers não pode subcontar na mediana.
    #[tokio::test]
    async fn savings_baseline_economia_uses_max_of_derived_and_annotation() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();

        // Março: transfer 30_000, anotação 50_000 → mês vale 50_000 (max).
        insert_economia_transfer(&p, "eco-mar", 30_000, "2026-03-10").await;
        annotate_economia(&p, 2026, 3, 50_000).await;
        // Abril: transfer 20_000 sem anotação → mês vale 20_000.
        insert_economia_transfer(&p, "eco-apr", 20_000, "2026-04-10").await;
        insert_income(&p, "inc-mar", 100_000, "2026-03-05").await;
        insert_income(&p, "inc-apr", 100_000, "2026-04-05").await;

        let (income_median, economia_median) = realized_savings_baseline(&p, today).await.unwrap();
        assert_eq!(income_median, 100_000);
        // Mediana de {50_000, 20_000} = 35_000 — se o max não fosse aplicado seria 25_000.
        assert_eq!(economia_median, 35_000);
    }

    // Mês ATIVO (tem eventos) sem economia registrada conta como economia 0 — é sinal real de
    // "não poupou", não ausência de dado; descartá-lo inflaria a mediana.
    #[tokio::test]
    async fn savings_baseline_active_month_without_economia_counts_as_zero() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();

        insert_income(&p, "inc-feb", 100_000, "2026-02-05").await;
        insert_income(&p, "inc-mar", 100_000, "2026-03-05").await;
        insert_income(&p, "inc-apr", 100_000, "2026-04-05").await;
        // Só março poupou.
        insert_economia_transfer(&p, "eco-mar", 60_000, "2026-03-10").await;

        let (income_median, economia_median) = realized_savings_baseline(&p, today).await.unwrap();
        assert_eq!(income_median, 100_000);
        // Mediana de {0, 60_000, 0} = 0 — fevereiro e abril entram como 0, não somem.
        assert_eq!(economia_median, 0);
    }

    // A janela são os últimos 6 meses de calendário COMPLETOS: o mês corrente e meses além de
    // 6 meses atrás ficam fora (mesma janela do `realized_monthly_baseline` — perna 1).
    #[tokio::test]
    async fn savings_baseline_window_excludes_current_and_older_months() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();

        // Fora da janela [2025-11-01, 2026-05-01): mês corrente e outubro/2025.
        insert_income(&p, "inc-may", 999_999, "2026-05-02").await;
        insert_income(&p, "inc-oct", 999_999, "2025-10-20").await;
        // Dentro: novembro/2025 (fronteira inferior inclusiva) e abril/2026.
        insert_income(&p, "inc-nov", 80_000, "2025-11-05").await;
        insert_income(&p, "inc-apr", 120_000, "2026-04-05").await;

        let (income_median, economia_median) = realized_savings_baseline(&p, today).await.unwrap();
        // Mediana de {80_000, 120_000} = 100_000 — os meses fora da janela não contaminam.
        assert_eq!(income_median, 100_000);
        assert_eq!(economia_median, 0);
    }

    // Janela sem nenhum mês ativo ⇒ (0, 0) — o chamador oculta a régua, nunca inventa número.
    #[tokio::test]
    async fn savings_baseline_empty_window_returns_zeros() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        let (income_median, economia_median) = realized_savings_baseline(&p, today).await.unwrap();
        assert_eq!((income_median, economia_median), (0, 0));
    }

    // --- Estados epistêmicos: procedência do teto, reserva, modo de gasto, previdência ---

    /// Transfer para conta ILÍQUIDA `acc-prev` (previdência → `EventKind::Patrimonio`).
    async fn insert_patrimonio_transfer(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query("INSERT OR IGNORE INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR IGNORE INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-prev', 'Previdência', 'pension', 'pe-1', 0, 'illiquid')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES (?1, 'transfer', ?2, ?3, 'acc-prev', 0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_reserve_account(pool: &SqlitePool, balance: i64) {
        sqlx::query("INSERT OR IGNORE INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR IGNORE INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-res', 'Reserva', 'savings', 'pe-1', ?1, 'reserve')",
        )
        .bind(balance)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Diário realizado (despesa variável não-crédito) em uma data.
    async fn insert_daily(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, payment_method) \
             VALUES (?1, 'expense', ?2, ?3, 0, 0, 'debit')",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Despesa de cartão (crédito) — vira `EventKind::Cartao`.
    async fn insert_credit(pool: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, payment_method) \
             VALUES (?1, 'expense', ?2, ?3, 1, 0, 'credit')",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    struct CardInvoiceFixture<'a> {
        account_id: &'a str,
        card_name: &'a str,
        owner_name: &'a str,
        invoice_id: &'a str,
        closing_date: &'a str,
        due_date: &'a str,
        stated_total_cents: Option<i64>,
    }

    async fn insert_card_invoice(pool: &SqlitePool, fixture: CardInvoiceFixture<'_>) {
        let owner_id = format!("owner-{}", fixture.account_id);
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&owner_id)
            .bind(fixture.owner_name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id) \
             VALUES (?1, ?2, 'credit_card', ?3)",
        )
        .bind(fixture.account_id)
        .bind(fixture.card_name)
        .bind(&owner_id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES (?1, ?2, substr(?3, 1, 7), ?4, ?3, ?5)",
        )
        .bind(fixture.invoice_id)
        .bind(fixture.account_id)
        .bind(fixture.due_date)
        .bind(fixture.closing_date)
        .bind(fixture.stated_total_cents)
        .execute(pool)
        .await
        .unwrap();
    }

    // Procedência do teto: escolhido vence; sem escolhido a média vira ESTIMATIVA; sem nada,
    // sem registro — o número nunca chega sem marca.
    #[tokio::test]
    async fn daily_ceiling_reading_reports_chosen_estimate_and_none() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        let none = daily_ceiling_reading(&p, today).await.unwrap();
        assert_eq!(none.source.as_str(), "none");
        assert_eq!(none.per_day_cents, 0);

        // Diário de maio (31 dias): 3.100,00 → média 100,00/dia = estimativa.
        insert_daily(&p, "d1", 310_000, "2026-05-10").await;
        let est = daily_ceiling_reading(&p, today).await.unwrap();
        assert_eq!(est.source.as_str(), "estimate");
        assert_eq!(est.per_day_cents, 10_000);

        // Orçamento explícito ativo vence com procedência de veredito.
        seed_person(&p).await;
        upsert_daily_budget_inner(&p, 15_000).await.unwrap();
        let chosen = daily_ceiling_reading(&p, today).await.unwrap();
        assert_eq!(chosen.source.as_str(), "chosen");
        assert_eq!(chosen.per_day_cents, 15_000);
    }

    // Perfil cartão (Diário morto + faturas vivas): modo cartão, Cartão do mês somado e próximo
    // vencimento a partir de hoje — os insumos do re-roteamento do dia.
    #[tokio::test]
    async fn spending_mode_summary_detects_card_profile_and_faturas() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_credit(&p, "c1", 250_000, "2026-04-12").await;
        insert_credit(&p, "c2", 260_000, "2026-05-12").await;
        insert_credit(&p, "c3", 120_000, "2026-06-05").await; // mês corrente, passado
        insert_credit(&p, "c4", 140_000, "2026-06-20").await; // mês corrente, futuro
        let s = spending_mode_summary(&p, today).await.unwrap();
        assert!(matches!(s.mode, forecast::SpendingMode::Card));
        assert_eq!(s.cartao_month_cents, 260_000); // 120k + 140k do mês corrente
        assert_eq!(s.next_fatura, Some(("2026-06-20".to_string(), 140_000)));
    }

    // O modo é pergunta sobre a FORMA do gasto, e a planilha já a responde: uma seção de cartões
    // viva com o Diário zerado é perfil cartão AINDA QUE nenhum cartão tenha sido cadastrado.
    // Fazer o modo depender de conta cadastrada devolvia "débito" para quem só gasta no crédito.
    #[tokio::test]
    async fn spending_mode_summary_reads_the_cards_section_before_any_card_is_registered() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        for (id, date, cents) in [
            ("f-abr", "2026-04-12", 250_000),
            ("f-mai", "2026-05-12", 260_000),
            ("f-jun", "2026-06-12", 140_000),
        ] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
                 VALUES (?1, 'expense', ?2, ?3, 1, 0)",
            )
            .bind(id)
            .bind(cents)
            .bind(date)
            .execute(&p)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
                 VALUES (?1, ?2, ?3, 'Bradesco', 0, 'CARTÕES:')",
            )
            .bind(format!("{id}-item"))
            .bind(id)
            .bind(cents)
            .execute(&p)
            .await
            .unwrap();
        }

        let cards: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account WHERE type='credit_card'")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(cards, 0, "nenhum cartão cadastrado — é esse o cenário");

        let s = spending_mode_summary(&p, today).await.unwrap();
        assert!(
            matches!(s.mode, forecast::SpendingMode::Card),
            "seção de cartões viva + Diário zerado é perfil cartão"
        );
    }

    /// Quem divide a fatura registra o reembolso na coluna Entrada, com o mesmo nome do cartão
    /// ("Fatura Bradesco Gio"). É dinheiro voltando: não descreve a forma do gasto, e sozinho
    /// não pode declarar modo cartão.
    #[tokio::test]
    async fn spending_mode_summary_ignores_a_card_named_refund_on_the_income_side() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('reembolso', 'income', 59_137, '2026-06-09', 0, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
             VALUES ('reembolso-item', 'reembolso', 59_137, 'Fatura Bradesco Gio', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO card_proposal (id,alias,display_name,source_month,status) \
             VALUES ('p1','bradesco gio','Bradesco Gio','2025-10','pending')",
        )
        .execute(&p)
        .await
        .unwrap();

        let s = spending_mode_summary(&p, today).await.unwrap();
        assert!(
            matches!(s.mode, forecast::SpendingMode::Debit),
            "reembolso na Entrada não declara gasto no cartão"
        );
    }

    // Constância de débito no mês corrente devolve o modo débito mesmo com faturas vivas.
    #[tokio::test]
    async fn spending_mode_summary_debit_when_daily_constant() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_credit(&p, "c1", 250_000, "2026-05-12").await;
        for (i, day) in [1, 3, 5, 8, 10].iter().enumerate() {
            insert_daily(&p, &format!("d{i}"), 3_000, &format!("2026-06-{day:02}")).await;
        }
        let s = spending_mode_summary(&p, today).await.unwrap();
        assert!(matches!(s.mode, forecast::SpendingMode::Debit));
    }

    // Escada de estados da reserva: sem conta mapeada → sem registro; poucos meses → retrato
    // vivo (estimativa); janela cheia → veredito; conta zerada → zero-diagnóstico.
    #[tokio::test]
    async fn dashboard_summary_reserve_state_ladder() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        let p = pool().await;
        insert_daily(&p, "d1", 100_000, "2026-05-10").await;
        let s = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(s.reserve_state, "no_record"); // sem conta de reserva mapeada

        let p = pool().await;
        insert_daily(&p, "d1", 100_000, "2026-05-10").await;
        insert_daily(&p, "d2", 100_000, "2026-04-10").await;
        insert_reserve_account(&p, 600_000).await;
        let s = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(s.reserve_state, "estimate"); // 2 meses = retrato vivo
        assert_eq!(s.reserve_basis_months, 2);

        let p = pool().await;
        for m in 1..=6 {
            insert_daily(&p, &format!("d{m}"), 100_000, &format!("2026-{m:02}-10")).await;
        }
        insert_reserve_account(&p, 600_000).await;
        let s = dashboard_summary(&p, NaiveDate::from_ymd_opt(2026, 7, 15).unwrap())
            .await
            .unwrap();
        assert_eq!(s.reserve_state, "verdict");
        assert_eq!(s.reserve_basis_months, 6);

        let p = pool().await;
        insert_daily(&p, "d1", 100_000, "2026-05-10").await;
        insert_reserve_account(&p, 0).await;
        let s = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(s.reserve_state, "zero"); // conta mapeada zerada é alerta legítimo
    }

    // Previdência é PATRIMÔNIO, nunca Economia: ela é publicada ao lado como leitura própria e
    // fica fora do numerador da régua, qualquer que seja a cobertura da reserva.
    #[tokio::test]
    async fn previdencia_nunca_entra_na_regua_de_economia() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Reserva folgada (6 × custo de vida de 1.000,00).
        let p = pool().await;
        insert_income(&p, "i1", 500_000, "2026-03-05").await;
        insert_daily(&p, "d1", 100_000, "2026-05-10").await;
        insert_patrimonio_transfer(&p, "pv1", 30_000, "2026-04-08").await;
        insert_reserve_account(&p, 600_000).await;
        let f = forecast_dto(&p, today).await.unwrap();
        assert_eq!(f.annual_savings.patrimonio_cents, 30_000);
        assert_eq!(f.annual_savings.economia_ruler_cents, 0);
        assert_eq!(f.annual_savings.economia_state, "no_record");

        // Reserva curta (< 6 meses): mesmo veredito, a cobertura não move a régua.
        let p = pool().await;
        insert_income(&p, "i1", 500_000, "2026-03-05").await;
        insert_daily(&p, "d1", 100_000, "2026-05-10").await;
        insert_patrimonio_transfer(&p, "pv1", 30_000, "2026-04-08").await;
        insert_reserve_account(&p, 500_000).await;
        let f = forecast_dto(&p, today).await.unwrap();
        assert_eq!(f.annual_savings.patrimonio_cents, 30_000);
        assert_eq!(f.annual_savings.economia_ruler_cents, 0);
        assert_eq!(f.annual_savings.economia_state, "no_record");
    }

    // Aceite explícito da proposta: orçamento (valor/dia + divisor + itens) e status no mesmo
    // tudo-ou-nada; dispensa marca sem escrever orçamento.
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

    // O resumo do dashboard expõe a procedência do teto e o overlay de proposta pendente.
    #[tokio::test]
    async fn dashboard_summary_reports_ceiling_source_and_proposal_overlay() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        sqlx::query(
            "INSERT INTO ceiling_proposal (id, note_hash, per_day_cents, divisor_days, items_json, source_month, status) \
             VALUES ('cp-1', 'h1', 4033, 31, '[]', '2026-05', 'pending')",
        )
        .execute(&p)
        .await
        .unwrap();
        let s = dashboard_summary(&p, today).await.unwrap();
        // A proposta é overlay: a procedência segue "none" e o número não é fabricado.
        assert_eq!(s.daily_ceiling_source, "none");
        assert_eq!(s.daily_budget, 0);
        assert!(s.ceiling_proposal_pending);
        // Sem estimativa não há conta a imprimir — a tela não pode inventar operandos.
        assert!(s.daily_ceiling_estimate.is_none());
    }

    // A tela IMPRIME a estimativa do teto, então os operandos precisam FECHAR na divisão: uma
    // conta que não bate é pior que uma frase vaga, porque tem cara de prova.
    #[tokio::test]
    async fn ceiling_estimate_basis_divides_to_the_shown_per_day() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        for (day, cents) in [("2026-05-03", 30_000_i64), ("2026-05-20", 32_000)] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed) \
                 VALUES (?1, 'expense', ?3, ?2, 0)",
            )
            .bind(format!("t-{day}"))
            .bind(day)
            .bind(cents)
            .execute(&p)
            .await
            .unwrap();
        }

        let s = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(s.daily_ceiling_source, "estimate");
        let basis = s.daily_ceiling_estimate.expect("estimativa tem base");
        assert_eq!(basis.month, "2026-05");
        assert_eq!(basis.variable_cents, 62_000);
        assert_eq!(
            basis.days, 31,
            "maio tem 31 dias — o divisor é o mês da base"
        );
        assert_eq!(basis.variable_cents / basis.days, s.daily_budget);
    }

    // --- Fixes da revisão adversarial: driver por modo, staleness da média, supersede ---

    // No modo cartão o driver de projeção desliga (as faturas já são eventos Cartão; injetar o
    // Diário típico dobraria a saída), mas o teto de exibição segue existindo.
    #[tokio::test]
    async fn projection_daily_ceiling_is_zero_in_card_mode() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        seed_person(&p).await;
        upsert_daily_budget_inner(&p, 4_000).await.unwrap();
        insert_credit(&p, "c1", 250_000, "2026-04-12").await;
        insert_credit(&p, "c2", 260_000, "2026-05-12").await;

        assert_eq!(effective_daily_ceiling(&p, today).await.unwrap(), 4_000);
        assert_eq!(projection_daily_ceiling(&p, today).await.unwrap(), 0);

        // Com constância de débito o driver volta a ser o teto.
        for (i, day) in [1, 3, 5, 8, 10].iter().enumerate() {
            insert_daily(&p, &format!("pd{i}"), 3_000, &format!("2026-06-{day:02}")).await;
        }
        assert_eq!(projection_daily_ceiling(&p, today).await.unwrap(), 4_000);
    }

    // A média do mês anterior decide pela DATA: uma projeção importada que virou passado conta,
    // mesmo com o `is_projection` congelado em 1 (staleness sem re-import).
    #[tokio::test]
    async fn ceiling_estimate_counts_stale_projections_by_date() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, payment_method) \
             VALUES ('sp1', 'expense', 310000, '2026-05-10', 0, 1, 'debit')",
        )
        .execute(&p)
        .await
        .unwrap();
        let reading = daily_ceiling_reading(&p, today).await.unwrap();
        assert_eq!(reading.source.as_str(), "estimate");
        assert_eq!(reading.per_day_cents, 10_000); // 3.100,00 ÷ 31 dias
    }

    #[tokio::test]
    async fn future_invoice_is_the_projected_card_outflow_on_its_due_date() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão principal",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-20",
                due_date: "2026-07-10",
                stated_total_cents: Some(85_000),
            },
        )
        .await;

        let forecast = forecast_dto(&p, today).await.unwrap();
        let due_day = forecast
            .daily
            .iter()
            .find(|day| day.date == "2026-07-10")
            .expect("a fatura futura entra na série diária");
        assert_eq!(due_day.fixed_out_cents, 85_000);
        assert_eq!(due_day.balance_cents, -85_000);
        let july = forecast
            .months
            .iter()
            .find(|month| (month.year, month.month) == (2026, 7))
            .expect("métrica de julho");
        assert_eq!(july.cartao_cents, 85_000);
    }

    /// A semente já embute tudo até hoje (`projection_seed`: Saldo ≤ hoje + gap de transações
    /// reais até hoje inclusive); reinjetar uma fatura que VENCE hoje por cima abateria o mesmo
    /// dinheiro duas vezes. A injeção (e a supressão do evento cru correspondente) precisam ficar
    /// estritamente `> hoje` para não sobrepor o que a semente já contou.
    #[tokio::test]
    async fn invoice_due_today_is_not_double_counted_between_seed_and_injection() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let amount_cents = 85_000;

        // Sem fatura: nenhum evento, saldo de hoje nasce só da semente vazia.
        let baseline_pool = pool().await;
        let baseline = forecast_dto(&baseline_pool, today).await.unwrap();
        let baseline_balance = baseline
            .daily
            .iter()
            .find(|d| d.date == "2026-06-15")
            .expect("dia de hoje na série")
            .balance_cents;

        // Com fatura vencendo HOJE: a planilha já registrou a Saída de hoje (semente via
        // sheet_daily_balance + gap) e a fatura persistida existe para o MESMO vencimento — as
        // duas pernas descrevem o mesmo dinheiro, não dois.
        let p = pool().await;
        sqlx::query(
            "INSERT INTO sheet_daily_balance (sheet_name, date, balance_cents) \
             VALUES ('2026', '2026-06-14', 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('due-today', 'expense', ?1, '2026-06-15', 1, 1)",
        )
        .bind(amount_cents)
        .execute(&p)
        .await
        .unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-05-20",
                due_date: "2026-06-15",
                stated_total_cents: Some(amount_cents),
            },
        )
        .await;

        let forecast = forecast_dto(&p, today).await.unwrap();
        let with_invoice_balance = forecast
            .daily
            .iter()
            .find(|d| d.date == "2026-06-15")
            .expect("dia de hoje na série")
            .balance_cents;

        assert_eq!(
            baseline_balance - with_invoice_balance,
            amount_cents,
            "a fatura vencendo hoje abate o saldo uma única vez, não em dobro"
        );
    }

    #[tokio::test]
    async fn future_card_line_item_and_its_invoice_count_once_in_month_metrics() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(85_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('sheet-card', 'expense', 85000, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('sheet-card-item', 'sheet-card', 85000, 'Cartão', 0, 'CARTÕES:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let events = load_metric_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        let card_events: Vec<_> = events
            .iter()
            .filter(|me| me.event.kind == forecast::EventKind::Cartao)
            .collect();
        assert_eq!(card_events.len(), 1);
        assert_eq!(
            card_events[0].event.date,
            NaiveDate::from_ymd_opt(2026, 6, 20).unwrap()
        );
        assert_eq!(card_events[0].event.amount_cents, 85_000);
    }

    #[tokio::test]
    async fn unknown_card_note_alias_remains_a_future_fixed_outflow_when_cards_are_configured() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        crate::commands::card_cmds::create_card_account_inner(
            &p,
            "Visa",
            None,
            Some(20),
            Some(10),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('future-unknown-card', 'expense', 10_000, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('future-unknown-card-item', 'future-unknown-card', 10_000, 'Nubank', 0, 'CARTÕES:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        assert!(events.iter().any(|event| {
            event.date == NaiveDate::from_ymd_opt(2026, 6, 20).unwrap()
                && event.kind == forecast::EventKind::FixedOut
                && event.amount_cents == 10_000
        }));
    }

    #[tokio::test]
    async fn known_card_note_alias_is_replaced_once_by_its_invoice() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "visa-account",
                card_name: "Visa",
                owner_name: "Pessoa",
                invoice_id: "visa-invoice",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('future-known-card', 'expense', 10_000, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('future-known-card-item', 'future-known-card', 10_000, 'Visa', 0, 'CARTÕES:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        let card_events: Vec<_> = events
            .iter()
            .filter(|event| event.kind == forecast::EventKind::Cartao)
            .collect();
        assert_eq!(card_events.len(), 1);
        assert_eq!(card_events[0].amount_cents, 10_000);
    }

    /// A gramática da planilha declara a fatura de duas maneiras, e as duas são dado do dono: sob
    /// o cabeçalho de seção, e — quando o cabeçalho falta — como uma linha que nomeia o cartão.
    /// Perder a segunda fazia a fatura virar conta a pagar e o mês inteiro sair do balde Cartão.
    #[tokio::test]
    async fn invoice_line_written_outside_the_cards_section_is_still_a_card_event() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "visa-account",
                card_name: "Visa",
                owner_name: "Pessoa",
                invoice_id: "visa-invoice",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('sectionless', 'expense', 10_000, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        // Sem `section`: exatamente a nota de janeiro que não trazia o cabeçalho CARTÕES.
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
             VALUES ('sectionless-item', 'sectionless', 10_000, 'Fatura Visa', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let due = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        let card_events: Vec<_> = events
            .iter()
            .filter(|event| event.kind == forecast::EventKind::Cartao)
            .collect();
        assert_eq!(
            card_events.len(),
            1,
            "a linha de fatura fora da seção pertence ao balde Cartão"
        );
        assert_eq!(card_events[0].amount_cents, 10_000);
        // A prova do dano: classificada como Saída fixa, a linha escapa da precedência da fatura
        // e o vencimento passa a debitar duas vezes o mesmo dinheiro.
        let due_day_total: i64 = events
            .iter()
            .filter(|event| event.date == due)
            .map(|event| event.amount_cents)
            .sum();
        assert_eq!(
            due_day_total, 10_000,
            "a fatura é a voz única do vencimento — a linha não soma por fora"
        );
    }

    /// A contrapartida da regra acima: fora da seção, só a linha que se APRESENTA como fatura
    /// nomeia um cartão. Mencionar o emissor é coincidência de texto, não movimento de cartão.
    #[tokio::test]
    async fn a_line_that_merely_mentions_the_card_outside_the_section_stays_a_fixed_outflow() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "visa-account",
                card_name: "Visa",
                owner_name: "Pessoa",
                invoice_id: "visa-invoice",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('mention', 'expense', 4_500, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position) \
             VALUES ('mention-item', 'mention', 4_500, 'Seguro Visa', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        assert!(
            events.iter().any(|event| {
                event.kind == forecast::EventKind::FixedOut && event.amount_cents == 4_500
            }),
            "seguro não vira fatura por citar o nome do cartão"
        );
    }

    /// A chave unificadora do domínio do cartão: o discriminador é "existe fatura para (conta,
    /// ciclo) da linha", não "o alias é conhecido". Uma proposta recém-aceita cria conta+alias mas
    /// não materializa a fatura observada até o próximo import — o item tem de continuar Saída fixa
    /// (visível no forecast), nunca suprimido sem fatura para repor o valor.
    #[tokio::test]
    async fn known_card_alias_without_an_invoice_for_its_cycle_stays_a_fixed_outflow() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        crate::commands::card_cmds::create_card_account_inner(
            &p,
            "Visa",
            None,
            Some(20),
            Some(10),
            None,
            None,
            None,
            &[],
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('future-known-card', 'expense', 10_000, '2026-06-20', 1, 1)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('future-known-card-item', 'future-known-card', 10_000, 'Visa', 0, 'CARTÕES:')",
        )
        .execute(&p)
        .await
        .unwrap();

        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        assert!(
            events.iter().any(|event| {
                event.date == NaiveDate::from_ymd_opt(2026, 6, 20).unwrap()
                    && event.kind == forecast::EventKind::FixedOut
                    && event.amount_cents == 10_000
            }),
            "sem fatura pro ciclo, o valor não pode sumir — fica Saída fixa"
        );
        let card_events: Vec<_> = events
            .iter()
            .filter(|event| event.kind == forecast::EventKind::Cartao)
            .collect();
        assert!(
            card_events.is_empty(),
            "nada suprime o item quando não há fatura para repor"
        );
    }

    #[tokio::test]
    async fn orphan_future_credit_purchase_remains_a_fixed_outflow_while_linked_purchase_uses_invoice_once()
     {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "visa-account",
                card_name: "Visa",
                owner_name: "Pessoa",
                invoice_id: "visa-invoice",
                closing_date: "2026-06-10",
                due_date: "2026-06-25",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('linked-credit', 'expense', 10000, '2026-06-20', 'credit', 1, 1, 'visa-invoice'), \
                    ('orphan-credit', 'expense', 10000, '2026-07-21', 'credit', 1, 1, NULL)",
        )
        .execute(&p)
        .await
        .unwrap();

        assert_eq!(
            forecast_horizon_end(&p, today).await.unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            "a compra órfã estende o horizonte de caixa"
        );
        let events = load_cashflow_events(&p, today, NaiveDate::from_ymd_opt(2026, 7, 31).unwrap())
            .await
            .unwrap();
        assert!(events.iter().any(|event| {
            event.date == NaiveDate::from_ymd_opt(2026, 7, 21).unwrap()
                && event.kind == forecast::EventKind::FixedOut
                && event.amount_cents == 10_000
        }));
        let invoice_events: Vec<_> = events
            .iter()
            .filter(|event| event.kind == forecast::EventKind::Cartao)
            .collect();
        assert_eq!(
            invoice_events.len(),
            1,
            "a compra vinculada não dobra a fatura"
        );
        assert_eq!(invoice_events[0].amount_cents, 10_000);
        assert_eq!(
            events.iter().map(|event| event.amount_cents).sum::<i64>(),
            20_000
        );
    }

    #[tokio::test]
    async fn metric_events_keep_today_and_future_orphan_credit_as_fixed_outflows() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "visa-account",
                card_name: "Visa",
                owner_name: "Pessoa",
                invoice_id: "visa-invoice",
                closing_date: "2026-06-10",
                due_date: "2026-06-25",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" \
             (id, type, amount, date, payment_method, is_fixed, is_projection, invoice_id) \
             VALUES ('linked-credit', 'expense', 10_000, '2026-06-20', 'credit', 1, 1, 'visa-invoice'), \
                    ('orphan-today', 'expense', 2_000, '2026-06-15', 'credit', 1, 0, NULL), \
                    ('orphan-tomorrow', 'expense', 3_000, '2026-06-16', 'credit', 1, 1, NULL)",
        )
        .execute(&p)
        .await
        .unwrap();

        let annual = annual_metrics(&p, 2026, today).await.unwrap();
        let june = annual
            .months
            .iter()
            .find(|month| month.month == 6)
            .expect("métricas de junho");

        assert_eq!(june.fixed_out_cents, 5_000);
        assert_eq!(
            june.cartao_cents, 10_000,
            "a compra vinculada entra só pela fatura"
        );
        assert_eq!(june.cost_of_living_cents, 15_000);
    }

    #[tokio::test]
    async fn materialized_card_series_raises_the_future_invoice_effective_total() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        let card_id = crate::commands::card_cmds::create_card_account_inner(
            &p,
            "Cartão",
            None,
            Some(20),
            Some(10),
            None,
            Some("Pessoa"),
            None,
            &[],
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) \
             VALUES ('invoice-series', ?1, '2026-09', '2026-08-20', '2026-09-10', 85000)",
        )
        .bind(&card_id)
        .execute(&p)
        .await
        .unwrap();
        crate::commands::card_cmds::create_card_series_inner(
            &p,
            &card_id,
            "Parcela",
            20_000,
            Some(1),
            "2026-08-01",
        )
        .await
        .unwrap();

        let forecast = forecast_dto(&p, today).await.unwrap();
        let september = forecast
            .months
            .iter()
            .find(|month| (month.year, month.month) == (2026, 9))
            .expect("métrica de setembro");
        assert_eq!(september.cartao_cents, 105_000);
    }

    #[tokio::test]
    async fn past_card_events_stay_authoritative_when_an_invoice_exists() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_credit(&p, "card-past", 85_000, "2026-03-10").await;
        let before = annual_metrics(&p, 2026, today).await.unwrap();
        let before_march = before.months.iter().find(|month| month.month == 3).unwrap();
        assert_eq!(before_march.cartao_cents, 85_000);

        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-02-20",
                due_date: "2026-03-10",
                stated_total_cents: Some(85_000),
            },
        )
        .await;
        let after = annual_metrics(&p, 2026, today).await.unwrap();
        let after_march = after.months.iter().find(|month| month.month == 3).unwrap();
        assert_eq!(after_march.cartao_cents, 85_000);
    }

    #[tokio::test]
    async fn forecast_keeps_the_existing_credit_behavior_without_card_accounts() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_credit(&p, "legacy-credit", 42_000, "2026-06-20").await;

        let events = load_forecast_events(&p, today, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, forecast::EventKind::Cartao);
        assert_eq!(events[0].amount_cents, 42_000);
    }

    #[tokio::test]
    async fn dashboard_lists_each_card_next_invoice_in_due_date_order() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "holder",
                card_name: "Azul",
                owner_name: "Ana",
                invoice_id: "invoice-holder",
                closing_date: "2026-06-10",
                due_date: "2026-07-05",
                stated_total_cents: Some(90_000),
            },
        )
        .await;
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "additional",
                card_name: "Verde",
                owner_name: "Bia",
                invoice_id: "invoice-additional",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(85_000),
            },
        )
        .await;
        sqlx::query("UPDATE account SET linked_account_id = 'holder' WHERE id = 'additional'")
            .execute(&p)
            .await
            .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(summary.upcoming_invoices.len(), 2);
        assert_eq!(summary.upcoming_invoices[0].account_id, "additional");
        assert_eq!(summary.upcoming_invoices[0].card_name, "Verde");
        assert_eq!(summary.upcoming_invoices[0].owner_name, "Bia");
        assert_eq!(summary.upcoming_invoices[0].status, "fechada");
        assert_eq!(summary.upcoming_invoices[1].account_id, "holder");
        assert_eq!(summary.upcoming_invoices[1].owner_name, "Ana");
    }

    #[tokio::test]
    async fn dashboard_reads_the_first_nonzero_invoice_for_each_card() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 8, 21).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-august-zero",
                closing_date: "2026-07-20",
                due_date: "2026-08-26",
                stated_total_cents: Some(0),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) VALUES ('invoice-september', 'card-1', '2026-09', '2026-08-20', '2026-09-26', 10_000)",
        )
        .execute(&p)
        .await
        .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();

        assert_eq!(summary.upcoming_invoices.len(), 1);
        assert_eq!(summary.upcoming_invoices[0].account_id, "card-1");
        assert_eq!(summary.upcoming_invoices[0].due_date, "2026-09-26");
        assert_eq!(summary.upcoming_invoices[0].amount_cents, 10_000);
        assert_eq!(summary.next_fatura_date.as_deref(), Some("2026-09-26"));
        assert_eq!(summary.next_fatura_amount_cents, 10_000);
    }

    #[tokio::test]
    async fn dashboard_omits_cards_with_only_zero_value_invoices() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 8, 21).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-august-zero",
                closing_date: "2026-07-20",
                due_date: "2026-08-26",
                stated_total_cents: Some(0),
            },
        )
        .await;

        let summary = dashboard_summary(&p, today).await.unwrap();

        assert!(summary.upcoming_invoices.is_empty());
        assert_eq!(summary.next_fatura_date, None);
        assert_eq!(summary.next_fatura_amount_cents, 0);
    }

    #[tokio::test]
    async fn zero_value_invoices_do_not_change_the_projected_balance_or_safe_to_spend() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 8, 21).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-september",
                closing_date: "2026-08-20",
                due_date: "2026-09-26",
                stated_total_cents: Some(10_000),
            },
        )
        .await;
        let summary_without_zero = dashboard_summary(&p, today).await.unwrap();
        let forecast_without_zero = forecast_dto(&p, today).await.unwrap();

        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, stated_total_cents) VALUES ('invoice-august-zero', 'card-1', '2026-08', '2026-07-20', '2026-08-26', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        let summary_with_zero = dashboard_summary(&p, today).await.unwrap();
        let forecast_with_zero = forecast_dto(&p, today).await.unwrap();

        assert_eq!(summary_with_zero.balance, summary_without_zero.balance);
        assert_eq!(
            forecast_with_zero.safe_to_spend_today_cents,
            forecast_without_zero.safe_to_spend_today_cents
        );
    }

    // O bloco do dia no modo cartão mostra "quanto somou nas faturas HOJE" — a soma é das
    // COMPRAS de cartão do dia (magnitude, mesmo contrato do `daily_spend_today`): manual
    // (`payment_method='credit'`) ou vinculada a fatura (`invoice_id`). O lump importado do
    // vencimento (célula da planilha, `payment_method` NULL e sem vínculo de compra) é o
    // PAGAMENTO da fatura, nunca gasto do dia — não pode entrar.
    #[tokio::test]
    async fn card_spend_today_sums_only_todays_card_purchases() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let today_str = today.format("%Y-%m-%d").to_string();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-20",
                due_date: "2026-07-10",
                stated_total_cents: None,
            },
        )
        .await;

        // Compra manual de hoje, vinculada à fatura (o caminho do `register_card_purchase`).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
             payment_method, invoice_id) VALUES ('buy-1', 'expense', 4_500, ?1, 0, 0, 'credit', 'invoice-1')",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();
        // Compra legada de hoje sem vínculo (só `payment_method`), gravada NEGATIVA (importada):
        // magnitude soma, sinal não cancela.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
             payment_method) VALUES ('buy-2', 'expense', -2_000, ?1, 0, 0, 'credit')",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();
        // Ocorrência PROJETADA de série hoje: futuro, não gasto realizado.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
             payment_method, invoice_id) VALUES ('proj-1', 'expense', 9_900, ?1, 0, 1, 'credit', 'invoice-1')",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();
        // Lump importado do vencimento hoje (pagamento de fatura): `payment_method` NULL, sem vínculo.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('lump-1', 'expense', 85_000, ?1, 0, 0)",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();
        // Diário de hoje: pertence ao outro modo, nunca à soma do cartão.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('daily-1', 'expense', 3_000, ?1, 0, 0)",
        )
        .bind(&today_str)
        .execute(&p)
        .await
        .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(
            summary.card_spend_today_cents, 6_500,
            "compras de cartão de hoje: 4.500 + 2.000; projeção, lump e Diário fora"
        );
    }

    // A linha da fatura na Hoje etiqueta "Reembolso" quando existe Entrada vinculada
    // (`refund_invoice_id`) — expectativa do protocolo do adicional. O flag viaja no DTO
    // para a tela não pagar um `get_invoice` por cartão.
    #[tokio::test]
    async fn upcoming_invoices_flag_refund_expectations() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-shared",
                card_name: "Compartilhado",
                owner_name: "Gio",
                invoice_id: "invoice-shared",
                closing_date: "2026-06-20",
                due_date: "2026-07-12",
                stated_total_cents: Some(98_770),
            },
        )
        .await;
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-solo",
                card_name: "Próprio",
                owner_name: "Eu",
                invoice_id: "invoice-solo",
                closing_date: "2026-06-20",
                due_date: "2026-07-10",
                stated_total_cents: Some(163_172),
            },
        )
        .await;
        // Entrada projetada no vencimento com o vínculo de reembolso da sub-fatura.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
             refund_invoice_id) VALUES ('refund-1', 'income', 98_770, '2026-07-12', 0, 1, 'invoice-shared')",
        )
        .execute(&p)
        .await
        .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();
        let shared = summary
            .upcoming_invoices
            .iter()
            .find(|i| i.account_id == "card-shared")
            .unwrap();
        let solo = summary
            .upcoming_invoices
            .iter()
            .find(|i| i.account_id == "card-solo")
            .unwrap();
        assert!(
            shared.has_refund_expectation,
            "fatura com Entrada vinculada"
        );
        assert!(
            !solo.has_refund_expectation,
            "fatura sem expectativa de reembolso"
        );
    }

    #[tokio::test]
    async fn upcoming_invoices_sum_refund_expectations() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-shared",
                card_name: "Compartilhado",
                owner_name: "Gio",
                invoice_id: "invoice-shared",
                closing_date: "2026-06-20",
                due_date: "2026-07-12",
                stated_total_cents: Some(100_000),
            },
        )
        .await;
        for (id, amount) in [("refund-1", 25_000), ("refund-2", 30_000)] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
                 refund_invoice_id) VALUES (?1, 'income', ?2, '2026-07-12', 0, 1, 'invoice-shared')",
            )
            .bind(id)
            .bind(amount)
            .execute(&p)
            .await
            .unwrap();
        }

        let summary = dashboard_summary(&p, today).await.unwrap();
        let shared = summary
            .upcoming_invoices
            .iter()
            .find(|i| i.account_id == "card-shared")
            .unwrap();
        assert_eq!(shared.refund_expected_cents, 55_000);
    }

    #[tokio::test]
    async fn upcoming_invoices_ignore_an_expense_linked_as_a_refund() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-shared",
                card_name: "Compartilhado",
                owner_name: "Gio",
                invoice_id: "invoice-shared",
                closing_date: "2026-06-20",
                due_date: "2026-07-12",
                stated_total_cents: Some(100_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
             refund_invoice_id) VALUES ('not-refund', 'expense', 50_000, '2026-07-12', 0, 1, 'invoice-shared')",
        )
        .execute(&p)
        .await
        .unwrap();

        let summary = dashboard_summary(&p, today).await.unwrap();
        let shared = summary
            .upcoming_invoices
            .iter()
            .find(|i| i.account_id == "card-shared")
            .unwrap();

        assert!(!shared.has_refund_expectation);
        assert_eq!(shared.refund_expected_cents, 0);
    }

    #[tokio::test]
    async fn upcoming_invoices_cap_refund_expectations_to_the_effective_invoice_total() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-shared",
                card_name: "Compartilhado",
                owner_name: "Gio",
                invoice_id: "invoice-shared",
                closing_date: "2026-06-20",
                due_date: "2026-07-12",
                stated_total_cents: Some(100_000),
            },
        )
        .await;
        for (id, amount) in [("refund-1", 80_000), ("refund-2", 50_000)] {
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection, \
                 refund_invoice_id) VALUES (?1, 'income', ?2, '2026-07-12', 0, 1, 'invoice-shared')",
            )
            .bind(id)
            .bind(amount)
            .execute(&p)
            .await
            .unwrap();
        }

        let summary = dashboard_summary(&p, today).await.unwrap();
        let shared = summary
            .upcoming_invoices
            .iter()
            .find(|i| i.account_id == "card-shared")
            .unwrap();
        assert_eq!(shared.refund_expected_cents, 100_000);
    }

    #[tokio::test]
    async fn refund_links_do_not_change_the_projected_balance_or_guardrail() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        seed_person(&p).await;
        upsert_daily_budget_inner(&p, 4_000).await.unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-shared",
                card_name: "Compartilhado",
                owner_name: "Gio",
                invoice_id: "invoice-shared",
                closing_date: "2026-06-20",
                due_date: "2026-07-12",
                stated_total_cents: Some(100_000),
            },
        )
        .await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('refund-1', 'income', 55_000, '2026-07-12', 0, 1)",
        )
        .execute(&p)
        .await
        .unwrap();

        let unlinked_summary = dashboard_summary(&p, today).await.unwrap();
        let unlinked_forecast = forecast_dto(&p, today).await.unwrap();
        sqlx::query(
            "UPDATE \"transaction\" SET refund_invoice_id = 'invoice-shared' WHERE id = 'refund-1'",
        )
        .execute(&p)
        .await
        .unwrap();
        let linked_summary = dashboard_summary(&p, today).await.unwrap();
        let linked_forecast = forecast_dto(&p, today).await.unwrap();

        assert_eq!(
            linked_forecast.safe_to_spend_today_cents,
            unlinked_forecast.safe_to_spend_today_cents
        );
        assert_eq!(
            linked_forecast.cash_headroom_cents,
            unlinked_forecast.cash_headroom_cents
        );
        assert_eq!(linked_forecast.month_end, unlinked_forecast.month_end);
        assert_eq!(linked_summary.balance, unlinked_summary.balance);
        assert_eq!(linked_summary.daily_budget, unlinked_summary.daily_budget);
    }

    #[tokio::test]
    async fn dashboard_card_month_and_next_due_come_from_invoices_without_sheet_rows() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(85_000),
            },
        )
        .await;

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(summary.cartao_month_cents, 85_000);
        assert_eq!(summary.next_fatura_date.as_deref(), Some("2026-06-20"));
        assert_eq!(summary.next_fatura_amount_cents, 85_000);
        assert_eq!(summary.spending_mode, "card");
    }

    #[tokio::test]
    async fn dashboard_card_month_combines_realized_card_events_and_current_invoices_once() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(85_000),
            },
        )
        .await;
        insert_credit(&p, "realized-card", 15_000, "2026-06-05").await;

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(summary.cartao_month_cents, 100_000);
    }

    #[tokio::test]
    async fn dashboard_aggregates_cards_with_the_same_next_due_date() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        for (account_id, card_name, invoice_id, amount_cents) in [
            ("card-1", "Azul", "invoice-1", 85_000),
            ("card-2", "Verde", "invoice-2", 15_000),
        ] {
            insert_card_invoice(
                &p,
                CardInvoiceFixture {
                    account_id,
                    card_name,
                    owner_name: "Pessoa",
                    invoice_id,
                    closing_date: "2026-06-10",
                    due_date: "2026-06-20",
                    stated_total_cents: Some(amount_cents),
                },
            )
            .await;
        }

        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(summary.next_fatura_date.as_deref(), Some("2026-06-20"));
        assert_eq!(summary.next_fatura_amount_cents, 100_000);
    }

    #[tokio::test]
    async fn dashboard_card_gate_composes_reserve_evidence_without_recomputing_it() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        let no_record = pool().await;
        insert_income(&no_record, "income", 100_000, "2026-05-05").await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('economia', 'expense', 20000, '2026-05-10', 1, 0)",
        )
        .execute(&no_record)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, section) \
             VALUES ('economia-item', 'economia', 20000, 'Reserva', 0, 'ECONOMIA:')",
        )
        .execute(&no_record)
        .await
        .unwrap();
        let summary = dashboard_summary(&no_record, today).await.unwrap();
        assert_eq!(summary.card_gate_economy, "alive");
        assert_eq!(summary.card_gate_reserve, "unknown");
        assert_eq!(summary.card_gate, "unknown");

        let exact_six = pool().await;
        insert_income(&exact_six, "income", 100_000, "2026-05-05").await;
        insert_daily(&exact_six, "daily", 100_000, "2026-05-10").await;
        insert_reserve_account(&exact_six, 600_000).await;
        insert_economia_transfer(&exact_six, "economia", 20_000, "2026-05-12").await;
        let summary = dashboard_summary(&exact_six, today).await.unwrap();
        assert_eq!(summary.reserve_months, 6.0);
        assert_eq!(summary.card_gate_economy, "alive");
        assert_eq!(summary.card_gate_reserve, "alive");
        assert_eq!(summary.card_gate, "alive");
    }

    /// A tela Cartões precisa mostrar a matemática do gate ("14%, falta 6 p/ 20%"), não só o
    /// veredito — o percentual bruto (bps) tem que sair do `DashboardSummary` para a UI não
    /// re-derivar a régua de economia sozinha.
    #[tokio::test]
    async fn dashboard_summary_exposes_the_economy_gate_percentage() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        let p = pool().await;
        insert_income(&p, "income", 100_000, "2026-05-05").await;
        insert_economia_transfer(&p, "economia", 14_000, "2026-05-12").await;
        let summary = dashboard_summary(&p, today).await.unwrap();
        assert_eq!(summary.card_gate_economy, "below");
        assert_eq!(
            summary.card_gate_economy_bps,
            Some(1_400),
            "14.000 de economia sobre 100.000 de renda é 14% (1.400 bps) — a métrica que falta pro piso de 20%"
        );

        let no_income = pool().await;
        let summary_no_income = dashboard_summary(&no_income, today).await.unwrap();
        assert_eq!(summary_no_income.card_gate_economy, "unknown");
        assert_eq!(
            summary_no_income.card_gate_economy_bps, None,
            "sem renda anual, a régua é incomputável — nunca um número fabricado"
        );
    }

    #[tokio::test]
    async fn annual_metrics_and_month_grid_use_the_same_future_invoice_event() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_card_invoice(
            &p,
            CardInvoiceFixture {
                account_id: "card-1",
                card_name: "Cartão",
                owner_name: "Pessoa",
                invoice_id: "invoice-1",
                closing_date: "2026-06-10",
                due_date: "2026-06-20",
                stated_total_cents: Some(85_000),
            },
        )
        .await;

        let annual = annual_metrics(&p, 2026, today).await.unwrap();
        let june = annual.months.iter().find(|month| month.month == 6).unwrap();
        assert_eq!(june.cartao_cents, 85_000);

        let grid = month_grid_at(&p, 2026, 6, today).await.unwrap();
        let due = grid.iter().find(|day| day.day == 20).unwrap();
        assert_eq!(due.fixed_out_cents, 85_000);
    }

    // === Onda Tags: máscara de réguas real (banco → motor) ===

    /// Renda/despesa REALIZADAS (`is_projection = 0`); `is_fixed = 1` ⇒ Saída fixa (custo de vida).
    async fn insert_realized_fixed(
        pool: &SqlitePool,
        id: &str,
        ttype: &str,
        amount: i64,
        date: &str,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1, ?2, ?3, ?4, 1, 0)",
        )
        .bind(id)
        .bind(ttype)
        .bind(amount)
        .bind(date)
        .execute(pool)
        .await
        .unwrap();
    }

    // (a) Golden de equivalência: uma tag com as 4 réguas desligadas reproduz o flag único antigo —
    // as MÉTRICAS (mensal, anual e mediana de custo) excluem a linha; o CAIXA a inclui.
    #[tokio::test]
    async fn golden_four_rulers_off_excludes_from_metrics_keeps_in_cash() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Março COMPLETO (hoje = junho): renda 500k + duas saídas fixas de 100k.
        insert_realized_fixed(&p, "inc", "income", 500_000, "2026-03-05").await;
        insert_realized_fixed(&p, "exp-a", "expense", 100_000, "2026-03-10").await;
        insert_realized_fixed(&p, "exp-b", "expense", 100_000, "2026-03-12").await;

        // Baseline sem tag: as duas saídas contam.
        let base = annual_metrics(&p, 2026, today).await.unwrap();
        let mar = base.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.cost_of_living_cents, 200_000);
        assert_eq!(mar.performance_cents, 300_000);
        assert_eq!(realized_monthly_baseline(&p, today).await.unwrap(), 200_000);

        // Tag exp-a com as 4 réguas desligadas.
        let tag = crate::tags::create_tag(&p, "Ignorar", "var(--cat-jade)", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_rulers(&p, &tag, true, true, true, true)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&p, "exp-a", std::slice::from_ref(&tag))
            .await
            .unwrap();

        // MÉTRICA: idêntico a um mundo sem exp-a.
        let after = annual_metrics(&p, 2026, today).await.unwrap();
        let mar = after.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(
            mar.cost_of_living_cents, 100_000,
            "custo de vida perde exp-a"
        );
        assert_eq!(mar.performance_cents, 400_000, "performance = 500 − 100");
        assert_eq!(
            realized_monthly_baseline(&p, today).await.unwrap(),
            100_000,
            "a mediana de custo perde exp-a"
        );

        // CAIXA: o grid do mês conta as DUAS saídas (o Saldo sempre conta).
        let grid = month_grid_at(&p, 2026, 3, today).await.unwrap();
        let out: i64 = grid
            .iter()
            .map(|d| d.fixed_out_cents + d.daily_out_cents)
            .sum();
        assert_eq!(out, 200_000, "o caixa inclui a linha excluída das métricas");
    }

    // (b) Divergência POR RÉGUA atravessando a fronteira SQL/máscara: desligar SÓ a Economia de um
    // transfer→reserva muda `realized_annual_economia` (guardrail) mas não a mediana de custo.
    #[tokio::test]
    async fn savings_ruler_off_changes_economia_not_cost_baseline() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_reserve_account(&p, 0).await;
        insert_realized_fixed(&p, "exp", "expense", 40_000, "2026-03-10").await;
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('tr', 'transfer', 30_000, '2026-03-20', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        assert_eq!(realized_annual_economia(&p, today).await.unwrap(), 30_000);
        assert_eq!(realized_monthly_baseline(&p, today).await.unwrap(), 40_000);

        // Só a régua de Economia desligada, no transfer.
        let tag = crate::tags::create_tag(&p, "Fora da economia", "c", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_rulers(&p, &tag, false, false, true, false)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&p, "tr", std::slice::from_ref(&tag))
            .await
            .unwrap();

        assert_eq!(
            realized_annual_economia(&p, today).await.unwrap(),
            0,
            "savings-off tira o transfer da Economia"
        );
        assert_eq!(
            realized_monthly_baseline(&p, today).await.unwrap(),
            40_000,
            "a régua de custo de vida fica intacta"
        );
    }

    // (b) Simétrico: desligar SÓ o Custo de vida de uma despesa muda a mediana de custo mas não a
    // renda anual (base do guardrail 20–30%).
    #[tokio::test]
    async fn cost_ruler_off_changes_baseline_not_annual_income() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized_fixed(&p, "inc", "income", 100_000, "2026-03-05").await;
        insert_realized_fixed(&p, "exp", "expense", 40_000, "2026-03-10").await;

        assert_eq!(realized_monthly_baseline(&p, today).await.unwrap(), 40_000);
        assert_eq!(realized_annual_savings(&p, today).await.unwrap().0, 100_000);

        // Só a régua de Custo de vida desligada, na despesa.
        let tag = crate::tags::create_tag(&p, "Fora do custo", "c", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_rulers(&p, &tag, false, true, false, false)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&p, "exp", std::slice::from_ref(&tag))
            .await
            .unwrap();

        assert_eq!(
            realized_monthly_baseline(&p, today).await.unwrap(),
            0,
            "cost-off tira a despesa do custo típico"
        );
        assert_eq!(
            realized_annual_savings(&p, today).await.unwrap().0,
            100_000,
            "a renda anual não muda com a régua de custo"
        );
    }

    // (c) Pool de UMA conexão (deadlock conhecido do repo): a query nova de máscara de réguas
    // compõe com as queries de evento do loader de métricas numa conexão só, sem "pool timed out"
    // — e a máscara reflete o flag por régua. `pool()` já usa `max_connections(1)`.
    #[tokio::test]
    async fn ruler_mask_map_survives_single_connection_pool() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized_fixed(&p, "exp", "expense", 40_000, "2026-03-10").await;
        let tag = crate::tags::create_tag(&p, "T", "c", None, false)
            .await
            .unwrap();
        crate::tags::update_tag_rulers(&p, &tag, false, true, false, false)
            .await
            .unwrap();
        crate::tags::set_transaction_tags(&p, "exp", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let start = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let events = load_metric_db_events(&p, today, start, end).await.unwrap();
        let masked = events
            .iter()
            .find(|me| me.event.amount_cents == 40_000)
            .expect("a despesa entra no stream de métricas");
        assert!(
            !masked.mask.cost_of_living && masked.mask.performance && masked.mask.savings,
            "a máscara reflete SÓ o custo de vida desligado, numa conexão só"
        );
    }
}
