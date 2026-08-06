//! A carga dos insumos — a única fronteira de SQL da rota de forecast.
//!
//! Casca imperativa de um par functional-core/imperative-shell: aqui mora todo o IO, e a
//! composição recebe [`ForecastInputs`] por referência, sem pool e sem relógio.
//! Não existe atalho de "só um numerozinho": um insumo que a regra precisa entra como campo do
//! inventário, carregado aqui.

use super::inputs::*;
use crate::commands::forecast_cmds as fc;
use crate::forecast;
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;

#[cfg(test)]
tokio::task_local! {
    /// Contagem de cargas, só em teste — a prova de que uma chamada de ferramenta com N
    /// inclusões opcionais dispara UMA composição, nunca uma por inclusão. Escopada por TAREFA
    /// assíncrona (não por thread nem por processo): o runtime multi-thread do Tokio migra a
    /// tarefa entre threads, e um contador global compartilhado contaria também as cargas de
    /// outros testes rodando em paralelo no mesmo binário.
    pub(crate) static LOAD_INPUTS_CALLS: std::cell::Cell<usize>;
}

/// Carrega, de uma vez, tudo que a leitura do dia precisa.
pub(crate) async fn load_inputs(
    pool: &SqlitePool,
    today: NaiveDate,
) -> Result<ForecastInputs, String> {
    #[cfg(test)]
    let _ = LOAD_INPUTS_CALLS.try_with(|c| c.set(c.get() + 1));

    let horizon_end = fc::forecast_horizon_end(pool, today).await?;
    let seed_cents = fc::projection_seed(pool, today).await?;
    let cash_events = fc::load_cashflow_events(pool, today, horizon_end).await?;
    let metric_events = fc::load_metric_events(pool, today, horizon_end).await?;

    // Anos da anotação: do primeiro ano que alguma régua olha (a janela do guardrail desloca para
    // dezembro do ano anterior em janeiro) até o fim do horizonte.
    let registered_window = forecast::registered_window(today);
    let first_year = registered_window
        .first()
        .map_or(today.year(), |&(year, _)| year)
        .min(today.year());
    let years: Vec<i32> = (first_year..=horizon_end.year()).collect();
    let economia_annotation = fc::load_economia_annotation(pool, &years).await?;

    let ceiling_reading = fc::daily_ceiling_reading(pool, today).await?;
    let ceiling = CeilingInputs {
        per_day_cents: fc::effective_daily_ceiling(pool, today).await?,
        source: ceiling_reading.source,
        estimate_basis: ceiling_reading.estimate_basis,
        projection_per_day_cents: fc::projection_daily_ceiling(pool, today).await?,
        proposal_pending: fc::has_pending_ceiling_proposal(pool).await?,
    };

    // A janela de meses COMPLETOS do guardrail é lida uma vez e serve as três figuras que a
    // habitam — renda-base, Economia e Patrimônio. O recorte VIVIDO da régua anual (que inclui o
    // mês em curso) sai de `year_metrics`, na composição: são janelas diferentes, com campos
    // diferentes, e é essa separação que impede a régua da tela e o guardrail de divergirem.
    let guardrail_metrics = fc::registered_window_metrics(pool, today).await?;
    let (registered_income_cents, registered_net_cents) =
        fc::realized_annual_savings(pool, today).await?;
    let (projected_income_cents, projected_net_cents) =
        fc::projected_annual_savings(pool, today).await?;
    let annual = AnnualInputs {
        year_metrics: fc::annual_month_metrics(pool, today.year(), today).await?,
        registered_income_cents,
        registered_net_cents,
        registered_economia_cents: guardrail_metrics.iter().map(|m| m.economia_cents).sum(),
        registered_patrimonio_cents: guardrail_metrics.iter().map(|m| m.patrimonio_cents).sum(),
        projected_income_cents,
        projected_net_cents,
    };

    let (monthly_cents, months) = fc::realized_monthly_baseline_detail(pool, today).await?;
    let (typical_income_cents, typical_economia_cents) =
        fc::realized_savings_baseline(pool, today).await?;
    let baseline = BaselineInputs {
        monthly_cents,
        months,
        typical_income_cents,
        typical_economia_cents,
    };

    let reserve = load_reserve_inputs(pool).await?;

    let mode = fc::spending_mode_summary(pool, today).await?;
    let spending_mode = SpendingModeInputs {
        samples: mode.samples,
        cartao_month_cents: mode.cartao_month_cents,
        next_fatura: mode.next_fatura.and_then(|(date, amount_cents)| {
            NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, amount_cents))
        }),
    };

    // Faturas a vencer sem limite superior: o dia lê a próxima, e a cobertura dos meses futuros
    // precisa das que vencem depois do mês corrente.
    let (has_card, active_invoices) =
        fc::load_card_invoice_events(pool, today, today, None).await?;
    let cards = CardInputs {
        has_card,
        active_invoices,
        invoiced_cycles: fc::load_invoiced_cycles(pool).await?,
        alias_index: fc::load_card_alias_index(pool).await?,
    };

    let today_spend = DailySpendInputs {
        daily_avg_cents: fc::daily_spend_today(pool, today).await?,
        card_cents: fc::card_spend_today(pool, today).await?,
    };

    let (transaction_count, last_real_tx_date) = fc::ledger_counts(pool, today).await?;

    Ok(ForecastInputs {
        today,
        horizon_end,
        seed_cents,
        cash_events,
        metric_events,
        economia_annotation,
        ceiling,
        annual,
        baseline,
        reserve,
        spending_mode,
        cards,
        today_spend,
        ledger: LedgerInputs {
            transaction_count,
            last_real_tx_date,
        },
    })
}

/// A reserva crua. O saldo e a existência de conta mapeada são leituras distintas de propósito:
/// contas mapeadas e zeradas são o alerta legítimo, ausência de conta é falta de registro.
async fn load_reserve_inputs(pool: &SqlitePool) -> Result<ReserveInputs, String> {
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
    let trend: (String,) = sqlx::query_as(
        "SELECT COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve trend: {e}"))?
    .unwrap_or(("flat".to_string(),));

    Ok(ReserveInputs {
        balance_cents: balance.0,
        has_accounts: has_accounts.0 != 0,
        trend: trend.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pool de UMA conexão, a convenção do repositório: a carga precisa provar que atravessa o
    /// mesmo aperto do runtime de produção, onde uma segunda conexão simplesmente não existe.
    async fn pool() -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    async fn seed_owner(p: &SqlitePool) {
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-1', 'Tester')")
            .execute(p)
            .await
            .unwrap();
    }

    async fn insert_account(p: &SqlitePool, id: &str, liquidity: &str, balance: i64) {
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES (?1, ?1, 'bank', 'pe-1', ?2, ?3)",
        )
        .bind(id)
        .bind(balance)
        .bind(liquidity)
        .execute(p)
        .await
        .unwrap();
    }

    async fn insert_txn(
        p: &SqlitePool,
        id: &str,
        ttype: &str,
        amount: i64,
        date: &str,
        is_fixed: i64,
    ) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        )
        .bind(id)
        .bind(ttype)
        .bind(amount)
        .bind(date)
        .bind(is_fixed)
        .execute(p)
        .await
        .unwrap();
    }

    /// Economia = transferência para uma conta de reserva.
    async fn insert_economia(p: &SqlitePool, id: &str, amount: i64, date: &str) {
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) \
             VALUES (?1, 'transfer', ?2, ?3, 'acc-res', 0)",
        )
        .bind(id)
        .bind(amount)
        .bind(date)
        .execute(p)
        .await
        .unwrap();
    }

    /// Diário = despesa variável (não fixa, não crédito).
    async fn insert_daily(p: &SqlitePool, id: &str, amount: i64, date: &str) {
        insert_txn(p, id, "expense", amount, date, 0).await;
    }

    /// Tag que desliga SÓ a régua do Diário médio — o resto das réguas continua contando a linha.
    async fn tag_out_of_daily_avg(p: &SqlitePool, txn_id: &str) {
        sqlx::query(
            "INSERT INTO tag (id, name, exclude_from_daily_avg) \
             VALUES ('tg-daily', 'Fora do Diário', 1)",
        )
        .execute(p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, 'tg-daily')")
            .bind(txn_id)
            .execute(p)
            .await
            .unwrap();
    }

    // --- Janela por janela ---

    // A semente é o Saldo importado mais recente ≤ hoje somado ao que se realizou depois dele; o
    // horizonte vai até o último dado pré-lançado, nunca só até o fim do mês.
    #[tokio::test]
    async fn seed_and_horizon_read_the_sheet_series_and_the_last_prelaunched_day() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        sqlx::query(
            "INSERT INTO sheet_daily_balance (sheet_name, date, balance_cents) \
             VALUES ('2026', '2026-06-10', 500_000)",
        )
        .execute(&p)
        .await
        .unwrap();
        insert_daily(&p, "gap", 20_000, "2026-06-12").await;
        insert_txn(&p, "futura", "expense", 30_000, "2026-09-20", 1).await;

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(
            inputs.seed_cents, 480_000,
            "semente = Saldo de 10/06 menos o gasto realizado entre ele e hoje"
        );
        assert_eq!(
            inputs.horizon_end,
            NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
            "o horizonte alcança o último lançamento pré-lançado"
        );
        assert_eq!(inputs.today, today);
    }

    // A economia anual da RÉGUA e a do GUARDRAIL vêm de janelas diferentes, e as duas precisam
    // estar certas: o guardrail conta só meses COMPLETOS (uma decisão não se toma com denominador
    // em formação), a régua conta o recorte VIVIDO, com o mês em curso incluído. As duas moram no
    // inventário como campos distintos — nunca como a mesma consulta lida duas vezes.
    #[tokio::test]
    async fn annual_ruler_savings_and_guardrail_savings_come_from_different_windows() {
        let p = pool().await;
        seed_owner(&p).await;
        insert_account(&p, "acc-res", "reserve", 0).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Maio (mês completo): renda 100.000 e Economia 30.000 → entra nas duas janelas.
        insert_txn(&p, "in-mai", "income", 100_000, "2026-05-05", 0).await;
        insert_economia(&p, "ec-mai", 30_000, "2026-05-06").await;
        // Junho (mês EM CURSO): renda 200.000 e Economia 50.000 → só no recorte vivido.
        insert_txn(&p, "in-jun", "income", 200_000, "2026-06-05", 0).await;
        insert_economia(&p, "ec-jun", 50_000, "2026-06-06").await;

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(
            inputs.annual.registered_income_cents, 100_000,
            "a renda-base do guardrail para em maio"
        );
        assert_eq!(
            inputs.annual.registered_economia_cents, 30_000,
            "a Economia do guardrail para em maio"
        );

        let ruler = forecast::annual_ruler(&inputs.annual.year_metrics, today.year(), today);
        assert_eq!(
            ruler.income_lived_cents, 300_000,
            "o recorte vivido da régua inclui o mês em curso"
        );
        assert_eq!(
            ruler.economia_lived_cents, 80_000,
            "a Economia vivida inclui o mês em curso"
        );
    }

    // Em 1º de janeiro a janela do guardrail desloca para dezembro do ano anterior — e a anotação
    // da aba Economia precisa cobrir esse ano, senão a régua deslocada leria uma janela sem a
    // parcela anotada.
    #[tokio::test]
    async fn january_guardrail_reads_the_prior_december_window() {
        let p = pool().await;
        seed_owner(&p).await;
        insert_account(&p, "acc-res", "reserve", 0).await;
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        insert_txn(&p, "in-dez", "income", 400_000, "2025-12-05", 0).await;
        insert_economia(&p, "ec-dez", 90_000, "2025-12-20").await;

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(inputs.annual.registered_income_cents, 400_000);
        assert_eq!(inputs.annual.registered_economia_cents, 90_000);
        assert!(
            inputs
                .economia_annotation
                .keys()
                .all(|(year, _)| *year >= 2025),
            "a anotação cobre desde o ano da janela deslocada"
        );
    }

    // Os eventos de método chegam com a máscara de réguas APLICADA NA ORIGEM — inclusive o gasto
    // do dia, que é o insumo onde a contabilidade paralela nasceu. Uma linha fora da régua do
    // Diário some da conta do dia e continua pesando no caixa.
    #[tokio::test]
    async fn metric_events_and_today_spend_carry_the_ruler_mask_from_the_source() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        insert_daily(&p, "hoje-conta", 4_000, "2026-06-15").await;
        insert_daily(&p, "hoje-fora", 9_000, "2026-06-15").await;
        tag_out_of_daily_avg(&p, "hoje-fora").await;

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(
            inputs.today_spend.daily_avg_cents, 4_000,
            "o gasto do dia obedece à máscara da régua do Diário"
        );
        let masked = inputs
            .metric_events
            .iter()
            .find(|me| me.event.amount_cents == 9_000)
            .expect("a linha excluída continua no stream de métricas, mascarada");
        assert!(
            !masked.mask.daily_avg && masked.mask.cost_of_living,
            "a máscara desliga só a régua que a tag excluiu"
        );
    }

    // O teto entra com valor, procedência e operandos; e o DRIVER da projeção é outro campo,
    // porque no modo cartão ele é zero enquanto o teto exibido continua existindo.
    #[tokio::test]
    async fn ceiling_carries_source_operands_and_a_separate_projection_driver() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        // Maio (30 dias após o dia 1) com 60.000 de Diário → média de 2.000/dia.
        for (i, day) in (1..=3).enumerate() {
            insert_daily(&p, &format!("mai-{i}"), 20_000, &format!("2026-05-0{day}")).await;
        }

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(inputs.ceiling.source, fc::CeilingSource::Estimate);
        let basis = inputs
            .ceiling
            .estimate_basis
            .as_ref()
            .expect("a estimativa publica os operandos que a produzem");
        assert_eq!(basis.month, "2026-05");
        assert_eq!(basis.variable_cents, 60_000);
        assert_eq!(basis.days, 31);
        assert_eq!(inputs.ceiling.per_day_cents, 60_000 / 31);
        assert_eq!(
            inputs.ceiling.projection_per_day_cents, inputs.ceiling.per_day_cents,
            "sem sinal de cartão, o driver é o próprio teto"
        );
        assert!(!inputs.ceiling.proposal_pending);
    }

    // Reserva, mês típico e livro-razão: cada figura na sua janela, cruas — o estado epistêmico é
    // decisão da composição, não da carga.
    #[tokio::test]
    async fn reserve_baseline_and_ledger_read_their_own_windows() {
        let p = pool().await;
        seed_owner(&p).await;
        insert_account(&p, "acc-res", "reserve", 250_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Três meses completos de custo de vida idêntico → mediana estável.
        for (i, month) in [3, 4, 5].iter().enumerate() {
            insert_txn(
                &p,
                &format!("fix-{i}"),
                "expense",
                50_000,
                &format!("2026-0{month}-10"),
                1,
            )
            .await;
        }
        insert_daily(&p, "hoje", 1_000, "2026-06-15").await;

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(inputs.reserve.balance_cents, 250_000);
        assert!(inputs.reserve.has_accounts);
        assert_eq!(inputs.reserve.trend, "flat");
        assert_eq!(inputs.baseline.monthly_cents, 50_000);
        assert_eq!(
            inputs.baseline.months, 3,
            "três meses sustentam a mediana — retrato vivo, não veredito"
        );
        assert_eq!(inputs.ledger.transaction_count, 4);
        assert_eq!(
            inputs.ledger.last_real_tx_date.as_deref(),
            Some("2026-06-15")
        );
    }

    // O insumo de cartão traz as faturas a vencer, os ciclos já faturados e o índice de apelidos —
    // as três peças que decidem se uma linha da nota é fatura ou conta a pagar.
    #[tokio::test]
    async fn card_inputs_carry_invoices_cycles_and_the_alias_index() {
        let p = pool().await;
        seed_owner(&p).await;
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES ('acc-card', 'Bradesco', 'credit_card', 'pe-1', 0, 'liquid')",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO invoice (id, account_id, cycle_month, closing_date, due_date, \
                                  stated_total_cents) \
             VALUES ('inv-1', 'acc-card', '2026-06', '2026-06-20', '2026-06-28', 120_000)",
        )
        .execute(&p)
        .await
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        let inputs = load_inputs(&p, today).await.unwrap();

        assert!(inputs.cards.has_card);
        assert_eq!(inputs.cards.active_invoices.len(), 1);
        assert_eq!(inputs.cards.active_invoices[0].amount_cents, 120_000);
        assert_eq!(
            inputs.cards.active_invoices[0].due_date,
            NaiveDate::from_ymd_opt(2026, 6, 28).unwrap()
        );
        assert!(
            inputs
                .cards
                .invoiced_cycles
                .contains(&("acc-card".to_string(), "2026-06".to_string()))
        );
        assert_eq!(
            inputs.cards.alias_index.resolve("Fatura Bradesco"),
            Some("acc-card".to_string()),
            "o índice resolve a conta a partir do apelido da linha"
        );
    }

    // A leitura composta e as rotas de produção publicam os MESMOS números: é a prova de que
    // migrar um consumidor para a leitura não muda nada do que o usuário vê. O saldo do fim do mês
    // fecha apesar de o dashboard projetar só o caixa, porque as duas projeções encadeiam a mesma
    // série de caixa — a diferença está nas métricas, que não tocam o saldo.
    #[tokio::test]
    async fn the_composed_reading_publishes_the_same_numbers_as_the_production_routes() {
        let p = pool().await;
        seed_owner(&p).await;
        insert_account(&p, "acc-res", "reserve", 900_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        for month in [3, 4, 5] {
            insert_txn(
                &p,
                &format!("inc-{month}"),
                "income",
                400_000,
                &format!("2026-0{month}-05"),
                0,
            )
            .await;
            insert_txn(
                &p,
                &format!("fix-{month}"),
                "expense",
                150_000,
                &format!("2026-0{month}-10"),
                1,
            )
            .await;
            insert_economia(
                &p,
                &format!("ec-{month}"),
                100_000,
                &format!("2026-0{month}-20"),
            )
            .await;
        }
        insert_txn(&p, "inc-jun", "income", 400_000, "2026-06-05", 0).await;
        insert_daily(&p, "dia-hoje", 3_000, "2026-06-15").await;
        insert_txn(&p, "fut-jul", "expense", 150_000, "2026-07-10", 1).await;

        let reading = super::super::compose::compose(&load_inputs(&p, today).await.unwrap());
        let dash = fc::dashboard_summary(&p, today).await.unwrap();
        let dto = fc::forecast_dto(&p, today).await.unwrap();

        assert_eq!(reading.projected_month_end_cents, dash.balance);
        assert_eq!(reading.today_spend.daily_avg_cents, dash.daily_spend_today);
        assert_eq!(reading.reserve.months, dash.reserve_months);
        assert_eq!(reading.reserve.state, dash.reserve_state);
        assert_eq!(reading.cards.gate.as_str(), dash.card_gate);
        assert_eq!(reading.cards.gate_economy_bps, dash.card_gate_economy_bps);
        assert_eq!(reading.ceiling.per_day_cents, dash.daily_budget);
        assert_eq!(reading.spending_mode.mode.as_str(), dash.spending_mode);

        assert_eq!(
            reading.safe_to_spend.amount_cents,
            dto.safe_to_spend_today_cents
        );
        assert_eq!(
            reading.safe_to_spend.binding.as_str(),
            dto.binding_guardrail
        );
        assert_eq!(
            reading.annual.economia_bps.unwrap_or(0),
            dto.annual_savings.economia_ruler_rate_bps
        );
        assert_eq!(
            reading.annual.registered_economia_cents,
            dto.annual_savings.registered_economia_cents
        );
        assert_eq!(
            reading.annual.economia_state,
            dto.annual_savings.economia_state
        );
        assert_eq!(
            reading.coverage.baseline_outflow_cents,
            dto.baseline_outflow_cents
        );
        assert_eq!(
            reading.coverage.trusted_through_month,
            dto.trusted_through_month
        );
        assert_eq!(
            reading.coverage.total_missing_cents,
            dto.total_missing_cents
        );
        assert_eq!(reading.forecast.daily.len(), dto.daily.len());
    }

    // Bordas: base vazia (ano sem renda, mês corrente sem lastro, reserva sem conta mapeada,
    // horizonte de um dia — o piso do fim do mês — e teto sem registro) carrega sem erro, com
    // cada campo no estado epistêmico correto em vez de um número fabricado.
    #[tokio::test]
    async fn an_empty_base_loads_with_every_input_in_its_absent_state() {
        let p = pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

        let inputs = load_inputs(&p, today).await.unwrap();

        assert_eq!(inputs.seed_cents, 0);
        assert_eq!(
            inputs.horizon_end, today,
            "sem dado pré-lançado, o horizonte é o piso do fim do mês corrente — um dia"
        );
        assert!(inputs.cash_events.is_empty());
        assert!(inputs.metric_events.is_empty());
        assert!(inputs.economia_annotation.is_empty());
        assert_eq!(inputs.ceiling.source, fc::CeilingSource::None);
        assert_eq!(inputs.ceiling.per_day_cents, 0);
        assert!(inputs.ceiling.estimate_basis.is_none());
        assert_eq!(inputs.annual.registered_income_cents, 0);
        assert_eq!(inputs.annual.registered_economia_cents, 0);
        assert_eq!(inputs.annual.year_metrics.len(), 12);
        assert_eq!(inputs.baseline.monthly_cents, 0);
        assert_eq!(inputs.baseline.months, 0);
        assert!(
            !inputs.reserve.has_accounts,
            "sem conta mapeada não há cobertura a afirmar"
        );
        assert!(!inputs.cards.has_card);
        assert!(inputs.cards.active_invoices.is_empty());
        assert_eq!(inputs.today_spend.daily_avg_cents, 0);
        assert_eq!(inputs.ledger.transaction_count, 0);
        assert!(inputs.ledger.last_real_tx_date.is_none());
    }
}
