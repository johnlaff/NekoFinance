//! A bancada de evals da conversa: o catálogo de casos contra o loop real.
//!
//! O instrumento que mede a conversa é o mesmo código que a serve: o binário `mia-bench` monta o
//! MESMO loop, a MESMA fachada e o MESMO adapter que a aplicação usa — uma bancada reimplementada
//! mediria outra coisa e divergiria em silêncio do que roda de verdade.
//!
//! Rodar custa dinheiro de verdade. Por isso a trava de gasto é dupla — teto de custo acumulado
//! no runner e chave dedicada com limite no painel do provedor — e por isso a bancada nunca roda
//! em CI: custo e segredo não pertencem a pipeline. Cada execução versiona um relatório datado
//! com modelo, provedor e resultados, para que qualquer mudança de fachada, prompt ou modelo seja
//! reavaliada pelo mesmo critério.

pub(crate) mod case;
pub(crate) mod cli;
pub(crate) mod fixtures;
pub(crate) mod grade;
pub(crate) mod report;

#[cfg(test)]
mod tests;

use crate::mia::method_tools::MethodPack;
use crate::mia::provider::pins::ModelPin;
use crate::mia::run::{
    AnswerProvenance, CancelToken, ProviderAdapter, Round, RunEvent, RunLimits, Runner, StopReason,
};
use crate::mia::{Context, consent, prompt};
use case::Case;
use serde_json::Value;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(crate) struct BenchConfig {
    pub pin: &'static ModelPin,
    /// A trava de gasto do runner, em milionésimos de dólar sobre o custo declarado pelo
    /// provedor. Ela fecha ANTES de abrir a próxima repetição: uma rodada em andamento termina, e
    /// o estouro máximo é o teto de UMA rodada — que os limites por rodada já prendem.
    pub max_spend_micro_usd: i64,
    /// Onde está o pack curado do método. Ausente, a bancada roda com o prefixo degradado — a
    /// conversa diz que não tem o método de cor, como na máquina de quem não o instalou.
    pub pack_root: Option<PathBuf>,
    pub limits: RunLimits,
}

/// O que uma repetição deixou para o relatório.
#[derive(Debug)]
pub(crate) struct RepetitionOutcome {
    pub verdict: grade::Verdict,
    pub stop: StopReason,
    pub provenance: Option<AnswerProvenance>,
    /// A resposta publicada fica no relatório inteira: é dela que o julgamento cego da didática
    /// precisa, e é ela a evidência quando um caso mecânico reprova.
    pub answer: Option<String>,
    pub tools_called: Vec<String>,
    pub cost_micro_usd: i64,
    /// Falso quando algum turno terminou sem custo declarado pelo provedor — a repetição custou
    /// dinheiro que o contador não viu, e a trava do runner deixa de ser confiável.
    pub cost_declared: bool,
    pub turns: u32,
    pub attempts: u32,
}

#[derive(Debug)]
pub(crate) struct CaseRun {
    pub case: Case,
    pub outcomes: Vec<RepetitionOutcome>,
    /// A trava de gasto fechou antes de este caso terminar as repetições. Abortado é estado
    /// declarado, nunca linha que some: o relatório diz o que NÃO foi medido.
    pub aborted: bool,
}

#[derive(Debug)]
pub(crate) struct BenchRun {
    pub pin: &'static ModelPin,
    /// O prefixo tinha o núcleo do método? O relatório carrega a resposta porque os casos de
    /// didática só valem julgamento quando o núcleo estava montado.
    pub method_core: bool,
    pub cases: Vec<CaseRun>,
    pub total_cost_micro_usd: i64,
    /// A trava vigente na execução, gravada no relatório: um total baixo com trava baixa e um
    /// total baixo porque tudo passou são histórias diferentes.
    pub max_spend_micro_usd: i64,
    pub spend_lock_hit: bool,
    /// O provedor deixou de declarar custo em alguma repetição. A bancada fecha na hora — sem
    /// custo declarado a trava do runner é cega, e "custo zero" no relatório significaria "não
    /// medi", nunca "foi de graça".
    pub cost_gap: bool,
}

/// As ferramentas que a rodada chamou, na ordem, lidas do transcript — inclusive as recusadas
/// pela validação: o gesto de chamar é o que se mede, não o sucesso da chamada.
pub(crate) fn tools_called(transcript: &[Value]) -> Vec<String> {
    transcript
        .iter()
        .filter(|message| message["role"] == "assistant")
        .flat_map(|message| message["tool_calls"].as_array().into_iter().flatten())
        .filter_map(|call| call["function"]["name"].as_str().map(str::to_string))
        .collect()
}

/// Roda UMA repetição de um caso num pool já semeado, atravessando o loop real.
pub(crate) async fn run_repetition<A: ProviderAdapter>(
    pool: &SqlitePool,
    ctx: &Context,
    adapter: &A,
    pin: &'static ModelPin,
    limits: RunLimits,
    system: &str,
    case: &Case,
) -> RepetitionOutcome {
    let (events, mut receiver) = mpsc::channel(64);
    // A bancada lê o resultado consolidado, não o stream; o dreno existe para que a publicação
    // de eventos nunca prenda a rodada esperando por uma interface que não está aqui. No
    // caminho, ele guarda o único fato que o resultado consolidado apaga: se algum turno chegou
    // sem custo declarado.
    let drain = tokio::spawn(async move {
        let mut usage_without_cost = false;
        while let Some(event) = receiver.recv().await {
            if let RunEvent::Usage(usage) = &event
                && usage.cost_micro_usd.is_none()
            {
                usage_without_cost = true;
            }
        }
        usage_without_cost
    });

    let runner = Runner {
        pool,
        ctx,
        adapter,
        pin,
        limits,
        cancel: CancelToken::new(),
        events,
    };
    let outcome = runner
        .run(Round {
            system,
            history: &[],
            question: &case.question,
        })
        .await;
    drop(runner);
    // Dreno perdido conta como lacuna: na dúvida entre "não vi custo" e "não houve custo", a
    // trava de gasto precisa do lado fechado.
    let usage_without_cost = drain.await.unwrap_or(true);
    let cost_declared = !usage_without_cost && (outcome.turns == 0 || outcome.cost_micro_usd > 0);

    let observed = grade::Observed {
        stop: outcome.stop,
        answer: outcome.answer.clone(),
        provenance: outcome.provenance,
        tools_called: tools_called(&outcome.transcript),
    };
    let verdict = grade::grade(&case.expected, &observed);

    RepetitionOutcome {
        verdict,
        stop: outcome.stop,
        provenance: outcome.provenance,
        answer: outcome.answer,
        tools_called: observed.tools_called,
        cost_micro_usd: outcome.cost_micro_usd,
        cost_declared,
        turns: outcome.turns,
        attempts: outcome.attempts,
    }
}

/// Roda o catálogo inteiro: um pool novo, semeado e consentido POR repetição — cada rodada mede
/// o modelo sobre o mesmo mundo, nunca sobre o rastro da rodada anterior.
pub(crate) async fn run_catalog<A: ProviderAdapter>(
    adapter: &A,
    cases: Vec<Case>,
    config: &BenchConfig,
) -> Result<BenchRun, String> {
    // Didática sem o núcleo do método não mede ensino — mede a recusa de capacidade da camada
    // ausente, e o julgamento cego receberia respostas que nunca tiveram como ensinar. Melhor
    // recusar a bancada inteira do que pagar por uma família que não vale julgamento.
    if config.pack_root.is_none()
        && cases
            .iter()
            .any(|case| case.family == case::Family::Didatica)
    {
        return Err(
            "Os casos de didática exigem o pack curado do método: rode com --pack, ou deixe-os \
             de fora com --only."
                .to_string(),
        );
    }

    // O prefixo é montado uma vez, como na aplicação: ele é o mesmo para toda rodada, e é sobre
    // o texto MONTADO que o gate de privacidade do pack já passou. A ausência de pack usa um
    // caminho com sufixo aleatório, nunca um nome previsível: um diretório fixo em /tmp deixaria
    // qualquer processo local plantar um núcleo — e o prefixo de sistema da rodada, junto do
    // method_core do relatório público, passaria a ser dele.
    let pack_root = config.pack_root.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "neko-finance-mia-bench-sem-pack-{}",
            uuid::Uuid::new_v4()
        ))
    });
    let system = prompt::system_prompt(&MethodPack::at(&pack_root))
        .await
        .map_err(|error| format!("{} {}", error.message, error.fix))?;

    let mut runs: Vec<CaseRun> = Vec::with_capacity(cases.len());
    let mut total_cost_micro_usd = 0_i64;
    let mut spend_lock_hit = false;
    let mut cost_gap = false;

    for case in cases {
        let mut outcomes = Vec::new();
        let mut aborted = false;

        for _ in 0..case.repetitions {
            if total_cost_micro_usd >= config.max_spend_micro_usd {
                spend_lock_hit = true;
                aborted = true;
                break;
            }
            if cost_gap {
                aborted = true;
                break;
            }

            let pool = SqlitePoolOptions::new()
                // Uma conexão, como a de produção: pool folgado esconderia deadlock de transação.
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .map_err(|error| format!("O pool da bancada não abriu: {error}."))?;
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .map_err(|error| format!("As migrações da bancada falharam: {error}."))?;
            fixtures::seed(&pool, &case.fixture).await?;
            // O consentimento é semeado porque quem roda a bancada JÁ consentiu — a chave
            // dedicada é dela — e o loop recusa rodada sem registro, na bancada como no app.
            consent::grant(&pool, config.pin, &fixtures::bench_clock().as_of())
                .await
                .map_err(|error| format!("O consentimento da bancada não gravou: {error}."))?;

            let ctx = Context {
                clock: fixtures::bench_clock(),
                pack: MethodPack::at(&pack_root),
            };
            let outcome = run_repetition(
                &pool,
                &ctx,
                adapter,
                config.pin,
                config.limits.clone(),
                &system.text,
                &case,
            )
            .await;
            total_cost_micro_usd = total_cost_micro_usd.saturating_add(outcome.cost_micro_usd);
            if !outcome.cost_declared {
                cost_gap = true;
            }
            outcomes.push(outcome);
        }

        runs.push(CaseRun {
            case,
            outcomes,
            aborted,
        });
    }

    Ok(BenchRun {
        pin: config.pin,
        method_core: system.method_core,
        cases: runs,
        total_cost_micro_usd,
        max_spend_micro_usd: config.max_spend_micro_usd,
        spend_lock_hit,
        cost_gap,
    })
}

/// A porta do binário. Vive no lib para que a bancada inteira seja exercitável pela suíte; o
/// `main` do binário é só a casca que a chama.
///
/// O código de saída fala da EXECUÇÃO, não do veredito: uma bancada que rodou até o fim sai com
/// sucesso mesmo com casos reprovados — o veredito mora no relatório, e é o gate de ligar quem o
/// lê. Falha de saída é operacional: argumento inválido, recusa de ambiente, catálogo quebrado,
/// provedor inalcançável.
pub fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match cli::parse_args(&args) {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let ci = std::env::var("CI").ok();
    let key = std::env::var("NEKO_MIA_BENCH_KEY").ok();
    if let Some(reason) = cli::refuse_reason(ci.as_deref(), key.as_deref()) {
        eprintln!("{reason}");
        return std::process::ExitCode::FAILURE;
    }
    let key = key.expect("a recusa de ambiente cobre a chave ausente");

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("O runtime da bancada não subiu: {error}.");
            return std::process::ExitCode::FAILURE;
        }
    };
    match runtime.block_on(execute(cli, key)) {
        Ok(summary) => {
            println!("{summary}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn execute(cli: cli::CliArgs, key: String) -> Result<String, String> {
    let mut cases = case::load_catalog(&cli.cases_dir).map_err(|error| error.to_string())?;
    // O filtro corta DEPOIS da carga: o catálogo inteiro continua obrigado a ser válido e a
    // cobrir as seis famílias, mesmo quando só um caso vai rodar.
    if let Some(only) = &cli.only {
        cases.retain(|case| case.id.contains(only.as_str()));
        if cases.is_empty() {
            return Err(format!("Nenhum caso do catálogo casa com \"{only}\"."));
        }
    }

    let pin = match &cli.model {
        Some(model) => crate::mia::provider::pins::pin(model).ok_or_else(|| {
            format!(
                "O modelo \"{model}\" não está na matriz de pins. Use um destes: {}.",
                crate::mia::provider::pins::PINS
                    .iter()
                    .map(|pin| pin.model)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?,
        None => crate::mia::provider::pins::default_pin(),
    };

    // O destino do relatório é validado ANTES de qualquer rodada: descobrir o diretório
    // impossível depois do catálogo pago jogaria a bancada fora.
    std::fs::create_dir_all(&cli.reports_dir).map_err(|error| {
        format!(
            "O diretório de relatórios {} não pôde ser criado: {error}.",
            cli.reports_dir.display()
        )
    })?;
    let probe = cli.reports_dir.join(".sonda-de-escrita");
    std::fs::write(&probe, b"")
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|error| {
            format!(
                "O diretório de relatórios {} não aceita escrita: {error}.",
                cli.reports_dir.display()
            )
        })?;

    let adapter = crate::mia::provider::http::HttpAdapter::new(key)?;
    let config = BenchConfig {
        pin,
        max_spend_micro_usd: cli.max_spend_micro_usd,
        pack_root: cli.pack_root.clone(),
        limits: RunLimits::default(),
    };
    let run = run_catalog(&adapter, cases, &config).await?;

    let ran_at = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    let pack = cli.pack_root.as_ref().map(MethodPack::at);
    let path = report::write(&cli.reports_dir, &run, &ran_at, pack.as_ref()).await?;

    let outcomes = || run.cases.iter().flat_map(|case| case.outcomes.iter());
    Ok(format!(
        "{} casos, {} repetições: {} aprovadas, {} reprovadas, {} pendentes de julgamento, {} casos abortados pela trava.\nCusto declarado: {} micro-USD (trava em {}).\nRelatório: {}",
        run.cases.len(),
        outcomes().count(),
        outcomes()
            .filter(|outcome| matches!(outcome.verdict, grade::Verdict::Passed))
            .count(),
        outcomes()
            .filter(|outcome| matches!(outcome.verdict, grade::Verdict::Failed { .. }))
            .count(),
        outcomes()
            .filter(|outcome| matches!(outcome.verdict, grade::Verdict::PendingJudgment))
            .count(),
        run.cases.iter().filter(|case| case.aborted).count(),
        run.total_cost_micro_usd,
        run.max_spend_micro_usd,
        path.display(),
    ))
}
