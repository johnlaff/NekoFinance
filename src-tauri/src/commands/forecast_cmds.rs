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
    // Mesmo filtro `exclude_from_totals` ("Ignorar") de `load_year_events`/`annual_metrics`: uma
    // linha marcada cai fora da MÉTRICA, então também não pode entrar no net de poupança realizada,
    // senão o guardrail e o painel de métricas divergem.
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN t.type='income' THEN t.amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN t.type='expense' THEN t.amount ELSE 0 END), 0) \
         FROM \"transaction\" t WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type IN ('income','expense') AND t.scenario_id IS NULL \
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
    .map_err(|e| format!("realized annual: {e}"))?;
    Ok((row.0, row.0 - row.1)) // (renda, poupança=net) dos meses completos
}

/// Economia REGISTRADA do ano até hoje (meses completos). É o numerador do "Economizado" do
/// método (Economia/Entradas), DISTINTO do net superávit de `realized_annual_savings` (que é o
/// "colchão" do Neko). Existir os dois lado a lado sem se confundir foi um achado da review.
///
/// Espelha o motor MENSAL: por mês, `max(derivado, anotação da aba)`, onde o
/// derivado = itens de nota ECONOMIA + transfers manuais → conta RESERVA. Transfer para conta
/// ILÍQUIDA é previdência/patrimônio — fora do Economizado%, como no mensal.
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

    // Derivado por mês ("YYYY-MM"): itens de nota ECONOMIA (pacote K) + transfers→reserva MANUAIS
    // (plano 003). Espelha `load_metric_db_events` + `forecast::classify`: 'illiquid' é
    // previdência/PATRIMÔNIO, não economia — fora do Economizado% (o anual
    // somava 'illiquid' indevidamente e ignorava os itens de nota).
    let mut derived: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    let item_rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT t.date, li.amount_cents, li.description, li.section \
         FROM line_item li \
         JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.type = 'expense' AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&lower)
    .bind(&upper)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("annual economia line items: {e}"))?;
    for (date, cents, description, section) in item_rows {
        if import::classify_line_item(section.as_deref(), &description)
            == import::ItemKind::Economia
            && let Some(ym) = date.get(0..7)
        {
            *derived.entry(ym.to_string()).or_insert(0) += cents.abs();
        }
    }

    // A janela de meses COMPLETOS decide, não o flag `is_projection` (que fica congelado
    // quando a data passa — mesma regra de staleness do savings, ver commands::tests).
    let transfer_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT substr(t.date, 1, 7), COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 \
           AND t.type='transfer' AND a.liquidity = 'reserve' AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           ) \
         GROUP BY substr(t.date, 1, 7)",
    )
    .bind(&lower)
    .bind(&upper)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("realized economia transfers: {e}"))?;
    for (ym, cents) in transfer_rows {
        *derived.entry(ym).or_insert(0) += cents;
    }

    // Anotação da aba Economia (`economia_annotation`, plano 052) por mês, na MESMA janela de meses
    // COMPLETOS (com o deslocamento de JANEIRO para DEZEMBRO, simétrico a `realized_annual_savings`).
    let annotation_rows: Vec<(i64, i64, i64)> = if is_january {
        sqlx::query_as(
            "SELECT year, month, COALESCE(SUM(amount_cents), 0) FROM economia_annotation \
             WHERE year = ?1 AND month = 12 GROUP BY year, month",
        )
        .bind(today_naive.year() as i64 - 1)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("annotation economia (jan): {e}"))?
    } else {
        sqlx::query_as(
            "SELECT year, month, COALESCE(SUM(amount_cents), 0) FROM economia_annotation \
             WHERE year = ?1 AND month < ?2 GROUP BY year, month",
        )
        .bind(today_naive.year() as i64)
        .bind(today_naive.month() as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("annotation economia: {e}"))?
    };
    let mut annotation_by: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    for (y, m, cents) in annotation_rows {
        annotation_by.insert(format!("{y:04}-{m:02}"), cents);
    }

    // Regra do MÁXIMO por mês (mesma de `month_metrics`): após o
    // round-trip do write-back (plano 062) a anotação espelha o derivado — somar dobraria. O mês
    // vale o maior dos dois; mês só-planilha usa a anotação, excedente digitado à mão ainda conta.
    let mut months: std::collections::HashSet<&String> = derived.keys().collect();
    months.extend(annotation_by.keys());
    let total: i64 = months
        .into_iter()
        .map(|ym| {
            let d = derived.get(ym).copied().unwrap_or(0);
            let a = annotation_by.get(ym).copied().unwrap_or(0);
            d.max(a)
        })
        .sum();
    Ok(total)
}

/// Anotação da aba Economia (`economia_annotation`, plano 052) para os ANOS informados, indexada por
/// `(ano, mês)` em centavos. Alimenta `month_metrics_for`/`project_with_metrics` como parcela ADITIVA
/// do Economizado% — disjunta dos transfers de reserva REAIS (que chegam via eventos de caixa).
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
    // malformada que começa com o ano mas não é ISO válida (review P2).
    let start = format!("{}-01-01", today_naive.year());
    let end = format!("{}-12-31", today_naive.year());
    // Mesmo filtro `exclude_from_totals` de `realized_annual_savings`/`load_year_events`: linhas
    // "Ignorar" ficam fora da projeção de poupança como ficam fora das métricas.
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN t.type='income' THEN t.amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN t.type='expense' THEN t.amount ELSE 0 END), 0) \
         FROM \"transaction\" t WHERE t.date >= ?1 AND t.date <= ?2 \
           AND t.type IN ('income','expense') AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("projected annual: {e}"))?;
    Ok((row.0, row.0 - row.1))
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
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    // O loader compartilhado já aplica ABS por item/transação e o filtro de tags excluídas.
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1).unwrap();
    let window_start = month_start - chrono::Months::new(6);
    let events = load_metric_db_events(pool, window_start, month_start).await?;

    let mut by_month: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for e in events {
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
    let mut vals: Vec<i64> = by_month.into_values().collect();
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
    // `SUM(ABS(amount))` por linha (não `ABS(SUM(amount))`): despesas IMPORTADAS chegam negativas
    // (`-amount_out`) e manuais positivas; num mês de sinal misto o ABS externo da soma assinada
    // cancelaria parcialmente, sub-reportando o Diário médio. Invariante de agregação de despesa em
    // TODOS os sites de query (month_grid / realized_monthly_baseline / daily_spend_today).
    let sum: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(amount)), 0) FROM \"transaction\" \
         WHERE type='expense' AND is_fixed=0 AND is_projection=0 \
           AND (payment_method IS NULL OR payment_method <> 'credit') \
           AND substr(date,1,7) = ?1 AND scenario_id IS NULL",
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
/// Plano 047: TODOS os passos (deprecar antigos + inserir o novo total + limpar/inserir categorias)
/// rodam numa ÚNICA `sqlx::Transaction` — atômico de ponta a ponta. Antes, o total ia pelo
/// `upsert_daily_budget_inner` (commit imediato no pool) e as categorias iam numa transação SEPARADA;
/// um crash entre os dois deixava um orçamento ativo SEM categorias, ou categorias velhas do
/// orçamento anterior. O `upsert_daily_budget_inner` permanece intacto para o caminho simples.
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
) -> Result<(), String> {
    // Valida ANTES de abrir a transação (atomicidade lógica: ou tudo válido, ou nada muda).
    for c in categories {
        if c.amount_cents <= 0 {
            return Err("cada categoria deve ter valor positivo (magnitude)".into());
        }
    }

    // Obtém o person_id do primeiro perfil (padrão single-user) — igual ao `upsert_daily_budget_inner`.
    let person: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("upsert_daily_budget (person): {e}"))?;
    let Some((person_id,)) = person else {
        // Nenhum perfil ainda — silencioso (usuário novo sem import).
        return Ok(());
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("upsert daily budget (begin): {e}"))?;

    // Depreca os registros ativos anteriores (todos, não só o primeiro).
    sqlx::query("UPDATE daily_budget SET status='deprecated' WHERE status='active'")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert_daily_budget (deprecate): {e}"))?;

    if amount_cents > 0 {
        let budget_id = uuid::Uuid::new_v4().to_string();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status) \
             VALUES (?1, ?2, ?3, ?4, 'active')",
        )
        .bind(&budget_id)
        .bind(&person_id)
        .bind(amount_cents)
        .bind(&today)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert_daily_budget (insert): {e}"))?;

        // Só anexa categorias quando há um teto explícito ativo E uma quebra informada. Usa o
        // `budget_id` recém-inserido (sem SELECT extra) → não há janela entre inserir e categorizar.
        if !categories.is_empty() {
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
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("upsert daily budget (commit): {e}"))?;
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
}

struct MetricLineItemRow {
    amount_cents: i64,
    description: String,
    section: Option<String>,
}

fn event_kind_for_item_kind(kind: import::ItemKind) -> forecast::EventKind {
    match kind {
        import::ItemKind::Saida | import::ItemKind::Ajuste => forecast::EventKind::FixedOut,
        import::ItemKind::Diario => forecast::EventKind::Daily,
        import::ItemKind::Cartao => forecast::EventKind::Cartao,
        import::ItemKind::Economia => forecast::EventKind::Economia,
        import::ItemKind::Patrimonio => forecast::EventKind::Patrimonio,
    }
}

async fn load_metric_db_events(
    pool: &SqlitePool,
    start_inclusive: NaiveDate,
    end_exclusive: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let start = start_inclusive.format("%Y-%m-%d").to_string();
    let end = end_exclusive.format("%Y-%m-%d").to_string();

    let txn_rows: Vec<MetricTxnRow> = sqlx::query_as(
        "SELECT t.id, t.type AS ttype, t.amount, t.date, \
                COALESCE(t.payment_method,'') AS payment_method, \
                t.is_fixed, t.is_projection, COALESCE(a.liquidity,'') AS to_liquidity \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           )",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query metric transactions: {e}"))?;

    let item_rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT li.transaction_id, li.amount_cents, li.description, li.section \
         FROM line_item li \
         JOIN \"transaction\" t ON t.id = li.transaction_id \
         WHERE t.date >= ?1 AND t.date < ?2 AND t.scenario_id IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM transaction_tag tt2 \
               JOIN tag tg ON tg.id = tt2.tag_id \
               WHERE tt2.transaction_id = t.id AND tg.exclude_from_totals = 1 \
           ) \
         ORDER BY li.transaction_id, li.position",
    )
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
                events.push(CashflowEvent {
                    date,
                    kind: event_kind_for_item_kind(kind),
                    amount_cents: item.amount_cents.abs(),
                    realized: row.is_projection == 0,
                });
            }
            // A célula é a dona do TOTAL: se as partes da nota não somam o pai, o resíduo
            // (célula − Σ|partes|) entra como Saída fixa COM SINAL — a convenção AJUSTES
            // "Diferença" da planilha aplicada na leitura, sem item sintético persistido.
            // Um resíduo negativo (partes > célula) REDUZ fixed_out para os baldes fecharem
            // com o total; por isso este evento é a exceção documentada à convenção
            // "amount_cents sempre positivo" do CashflowEvent (só métrica, nunca na cadeia).
            let parts_sum: i64 = items.iter().map(|i| i.amount_cents.abs()).sum();
            let residual = row.amount.abs() - parts_sum;
            if residual != 0 {
                events.push(CashflowEvent {
                    date,
                    kind: forecast::EventKind::FixedOut,
                    amount_cents: residual,
                    realized: row.is_projection == 0,
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
            events.push(event);
        }
    }

    Ok(events)
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
         WHERE t.date > ?1 AND t.date <= ?2 AND t.scenario_id IS NULL",
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
    let end_exclusive = today_naive
        .succ_opt()
        .ok_or("data de hoje inválida para intervalo de métricas")?;
    load_metric_db_events(pool, month_start, end_exclusive).await
}

/// Eventos para as MÉTRICAS por mês = futuros (encadeamento) + realizados do mês corrente.
/// Cobre o mês inteiro de hoje (realizado + projetado); meses à frente já são todos futuros.
pub(crate) async fn load_metric_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data de hoje inválida")?;
    let end_exclusive = horizon_end
        .succ_opt()
        .ok_or("horizonte inválido para intervalo de métricas")?;
    let mut metric = load_realized_month_events(pool, month_start, today_naive).await?;
    let future_start = today_naive
        .succ_opt()
        .ok_or("data de hoje inválida para intervalo futuro de métricas")?;
    metric.extend(load_metric_db_events(pool, future_start, end_exclusive).await?);
    let daily_ceiling = effective_daily_ceiling(pool, today_naive).await?;
    let days_with_daily: std::collections::HashSet<NaiveDate> = metric
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    metric.extend(forecast::project_daily_ceiling(
        daily_ceiling,
        today_naive,
        horizon_end,
        &days_with_daily,
    ));
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
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas fixas realizadas (coluna Saída sem cartão/economia/patrimônio).
    pub fixed_out_cents: i64,
    /// Diário realizado (coluna Diário).
    pub daily_out_cents: i64,
    /// Previsão de diário do mês (teto dos dias futuros + pré-lançados); desconta a Performance.
    pub daily_projected_cents: i64,
    /// Cartão realizado, bucket próprio dentro do custo de vida.
    pub cartao_cents: i64,
    /// Diário médio do mês = Σ diário realizado ÷ dias decorridos (D/N). Antes morria no DTO.
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
    let metric_events = load_metric_events(pool, today_naive, horizon_end).await?;
    // Anotação da aba Economia (plano 052) para os anos cobertos pelo horizonte — parcela aditiva do
    // Economizado% por mês, disjunta dos transfers de reserva reais (que já chegam nos eventos).
    let years: Vec<i32> = (today_naive.year()..=horizon_end.year()).collect();
    let annotation = load_economia_annotation(pool, &years).await?;
    let fc = forecast::project_with_metrics(
        seed,
        today_naive,
        &events,
        &metric_events,
        horizon_end,
        &annotation,
    );

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
                performance_cents: m.performance_cents,
                cost_of_living_cents: m.cost_of_living_cents,
                fixed_out_cents: m.fixed_out_cents,
                daily_out_cents: m.daily_out_cents,
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

// --- Visão anual (spec 019 month-views) ---

/// Todos os eventos do ANO (realizado + projetado), classificados. O teto de diário do mês
/// corrente é injetado pelo chamador (`annual_metrics`), espelhando o forecast — a Performance
/// do mesmo mês precisa ser idêntica nas duas visões. Para a visão anual das 4 métricas.
pub(crate) async fn load_year_events(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<CashflowEvent>, String> {
    let start = NaiveDate::from_ymd_opt(year, 1, 1).ok_or("ano inválido")?;
    let end = NaiveDate::from_ymd_opt(year + 1, 1, 1).ok_or("ano inválido")?;
    load_metric_db_events(pool, start, end).await
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
    let mut events = load_year_events(pool, year).await?;
    // Mesmo teto de diário do forecast para o MÊS CORRENTE: sem ele, a Performance do mesmo mês
    // divergia entre a visão anual e o Totais (o teto só existe no caminho do forecast). O
    // project_daily_ceiling já se limita ao fim do mês corrente.
    if year == today.year() {
        let daily_ceiling = effective_daily_ceiling(pool, today).await?;
        let days_with_daily: std::collections::HashSet<NaiveDate> = events
            .iter()
            .filter(|e| e.kind == forecast::EventKind::Daily)
            .map(|e| e.date)
            .collect();
        let month_end = forecast::last_day_of_month(today.year(), today.month());
        events.extend(forecast::project_daily_ceiling(
            daily_ceiling,
            today,
            month_end,
            &days_with_daily,
        ));
    }
    let months: Vec<(i32, u32)> = (1..=12).map(|m| (year, m)).collect();
    let annotation = load_economia_annotation(pool, &[year]).await?;
    let metrics = forecast::month_metrics_for(today, &events, &months, &annotation);
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
         FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 AND scenario_id IS NULL GROUP BY date",
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
    //   `create_transaction` exige `> 0`); o sinal vem do `type`. Usamos `SUM(ABS(amount))` para
    //   somar a MAGNITUDE de cada linha — robusto caso algum writer grave com sinal (despesas
    //   importadas chegam negativas, lançamentos manuais positivos): num dia misto, `ABS(SUM(...))`
    //   cancelaria parcialmente antes do ABS. Mesmo padrão de `realized_monthly_baseline`/`month_grid`.
    // - Fonte única: o gasto do dia vem das transações Diário (despesa variável não-crédito);
    //   sem nenhuma transação no dia, a soma é 0. O ritual diário é uma transação Diário comum.
    let daily_spend: (i64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(ABS(amount)) FROM \"transaction\" \
                          WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                            AND (payment_method IS NULL OR payment_method <> 'credit') \
                            AND scenario_id IS NULL), 0)",
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

    // Bug 2 (plano 052): o Economizado% (savings_rate_bps) reflete a ANOTAÇÃO da aba Economia mesmo
    // quando o dono poupa só via Saída no grid (sem transfer de reserva → nenhum EventKind::Economia).
    // Antes, `economia = 0` → savings_rate_bps = 0, divergindo da planilha.
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
            std::slice::from_ref(&income),
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

    // Plano 060: uma Saída itemizada é atribuída por seção, sem contar o pai de novo.
    #[tokio::test]
    async fn annual_metrics_attributes_line_items_by_section_without_double_counting_parent() {
        let p = pool().await;

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

    // Bug 3 (plano 052): `realized_annual_economia` (numerador do guardrail safe_to_spend) soma a
    // ANOTAÇÃO da aba Economia + os transfers de reserva REAIS (plano 003). Sem isso, quem poupa só
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

        // Transfer de reserva MANUAL (plano 003) em março: conta reserva + transação transfer.
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
        // não uma parcela aditiva. Total: 5 × 10.000 = 50.000 (antes somava 58.000, dobrando o
        // dinheiro que o write-back da aba deriva dos próprios lançamentos).
        assert_eq!(economia, 50_000);
    }

    // O numerador ANUAL do Economizado espelha o MENSAL —
    // (a) itens de nota ECONOMIA contam; (b) transfer para conta ILÍQUIDA é previdência/patrimônio,
    // NÃO economia (o anual somava 'illiquid' indevidamente, inflando o Economizado% e afrouxando o
    // guardrail); (c) anotação igual ao derivado (round-trip do write-back 062) não duplica.
    #[tokio::test]
    async fn annual_economia_mirrors_monthly_classification() {
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

        // (a) Janeiro: Saída itemizada com item de seção ECONOMIA (pacote K) de 300,00.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES ('t-jan', 'expense', 30000, '2026-01-10', 1, 0)",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO line_item (id, transaction_id, amount_cents, description, position, is_user_edited, section) \
             VALUES ('li:t-jan:0', 't-jan', 30000, 'Poupança', 0, 0, 'ECONOMIA')",
        )
        .execute(&p)
        .await
        .unwrap();
        // Round-trip do write-back: a aba Economia já espelha o derivado de janeiro → conta UMA vez.
        crate::commands::write_back_cmds::store_economia_entries(&p, &[(2026, 1, 30000)])
            .await
            .unwrap();

        // (b) Fevereiro: transfer manual → reserva (economia líquida) de 200,00.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-fev', 'transfer', 20000, '2026-02-15', 'acc-res', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // (c) Março: transfer → ilíquido (previdência) de 500,00 — patrimônio, fora do Economizado.
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-mar', 'transfer', 50000, '2026-03-15', 'acc-prev', 0)",
        )
        .execute(&p)
        .await
        .unwrap();

        // (d) Agosto (FUTURO): ocorrência projetada de série → reserva — a janela de meses
        // completos a deixa fora (o flag is_projection NÃO é o guarda; ver
        // economia_ignores_stale_is_projection_flag).
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES ('t-ago-proj', 'transfer', 70000, '2026-08-15', 'acc-res', 1)",
        )
        .execute(&p)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let economia = realized_annual_economia(&p, today).await.unwrap();
        // jan max(30.000, 30.000) + fev max(20.000, 0) = 50.000; previdência e futuro fora.
        assert_eq!(economia, 50_000);
    }

    // Célula 100,00 com nota somando 120,00 — o resíduo −20,00
    // entra como Saída fixa COM SINAL para os baldes fecharem com o total da célula (a
    // convenção AJUSTES "Diferença" da planilha aplicada na leitura, sem item sintético).
    #[tokio::test]
    async fn metric_events_reconcile_item_residual_with_sign() {
        let p = pool().await;
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
        let events = load_metric_db_events(&p, start, end).await.unwrap();

        let cartao: i64 = events
            .iter()
            .filter(|e| e.kind == forecast::EventKind::Cartao)
            .map(|e| e.amount_cents)
            .sum();
        let fixed: i64 = events
            .iter()
            .filter(|e| e.kind == forecast::EventKind::FixedOut)
            .map(|e| e.amount_cents)
            .sum();
        assert_eq!(cartao, 12_000);
        assert_eq!(fixed, -2_000, "resíduo com sinal reduz a Saída fixa");
        assert_eq!(
            cartao + fixed,
            10_000,
            "os baldes fecham com o total da célula"
        );
    }

    // Bugs 1+3 (plano 052): a anotação e os transfers reais são DISJUNTOS — `store_economia_entries`
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

    // Plano 049: mesma família de bug do `month_grid` acima, agora em `realized_monthly_baseline`.
    // Despesas importadas são negativas, manuais positivas; um `SUM(amount)` de sinal misto se
    // cancela e produz uma mediana errada (possivelmente negativa), corrompendo o reserve floor.
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

        // Só um mês na janela → mediana = total do mês.
        // Correto: ABS(-90000) + 60000 = 150_000.
        // Errado (antes do fix): -90000 + 60000 = -30_000.
        let baseline = realized_monthly_baseline(&p, today).await.unwrap();
        assert_eq!(
            baseline, 150_000,
            "realized_monthly_baseline must sum magnitudes (ABS), not signed amounts"
        );

        // reserve_floor = baseline × RESERVE_MIN_MONTHS (6). Verifica que o piso é positivo e
        // coerente (não negativo, como aconteceria com a baseline corrompida).
        let floor = reserve_floor(&p, today).await.unwrap();
        assert!(
            floor >= 150_000 * RESERVE_MIN_MONTHS,
            "reserve_floor must be at least baseline × RESERVE_MIN_MONTHS"
        );
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
    async fn upsert_daily_budget_with_categories_is_atomic() {
        // Plano 047 (P2): total + categorias gravam numa ÚNICA transação. Caminho feliz: ambos
        // confirmam juntos; nenhum orçamento ATIVO fica sem suas categorias.
        let p = pool().await;
        seed_person(&p).await;

        upsert_daily_budget_with_categories_inner(
            &p,
            10000,
            &[cat("Alpha", 6000, 0), cat("Beta", 4000, 1)],
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
        upsert_daily_budget_with_categories_inner(&p, 0, &[])
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
        assert_eq!(monthly_to_daily_rate(3100, 31), 100);
        assert_eq!(monthly_to_daily_rate(125000, 31), 4032); // 4032,25 truncado
        assert_eq!(monthly_to_daily_rate(100, 0), 0, "dias=0 não causa panic");
    }

    // Bug 1 (plano 053): `daily_spend_today` precisa somar a MAGNITUDE de cada linha do dia
    // (`SUM(ABS(amount))`), não o ABS da soma assinada. Despesas importadas chegam negativas e
    // lançamentos manuais positivos: num dia misto, `ABS(SUM(...))` cancelaria parcialmente antes
    // do ABS, sub-reportando o "Diário de hoje". Guarda contra a regressão para o padrão antigo.
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

        // Soma das magnitudes: 5000 + 3000 = 8000. O bug antigo (`ABS(SUM(amount))`) daria
        // ABS(-5000 + 3000) = 2000.
        assert_eq!(
            summary.daily_spend_today, 8000,
            "Diário de hoje soma magnitudes (SUM(ABS)), não o ABS da soma assinada"
        );
    }

    // Plano 054: o fallback de média do mês anterior em `effective_daily_ceiling` precisa somar a
    // MAGNITUDE de cada linha (`SUM(ABS(amount))`), não o ABS da soma assinada. Despesas importadas
    // chegam negativas e manuais positivas; num mês de sinal misto o ABS externo da soma cancelaria
    // parcialmente, sub-reportando o teto diário. Último site que estava no padrão antigo.
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
        // Soma das magnitudes = 6200 + 3100 = 9300; ÷ 31 dias de maio = 300. O bug antigo
        // (`ABS(SUM(amount))`) daria ABS(-6200 + 3100) = 3100 ÷ 31 = 100.
        assert_eq!(
            ceiling, 300,
            "o teto diário soma magnitudes (SUM(ABS)), não o ABS da soma assinada"
        );
    }

    // Plano 054: `realized_annual_savings` precisa aplicar o MESMO filtro `exclude_from_totals`
    // ("Ignorar") de `load_year_events`/`annual_metrics`. Uma linha marcada cai fora da métrica;
    // se entrasse no net de poupança realizada, o guardrail e o painel de métricas divergiriam.
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
}
