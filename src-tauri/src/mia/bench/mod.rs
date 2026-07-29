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

pub(crate) mod bakeoff;
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
    /// Onde está o pack curado do método. Ausente, a bancada roda com o prefixo degradado — a
    /// conversa diz que não tem o método de cor, como na máquina de quem não o instalou.
    pub pack_root: Option<PathBuf>,
    pub repetitions: Repetitions,
    /// O prefixo de sistema JÁ montado, quando quem chama quer que todas as corridas leiam o
    /// mesmo pack. Ausente, cada corrida monta o seu — que é o certo para a corrida solta e
    /// errado para o bakeoff, onde um pack editado no meio daria prompts diferentes a candidatos
    /// que precisam ser comparáveis.
    pub system: Option<std::sync::Arc<prompt::SystemPrompt>>,
    pub limits: RunLimits,
}

/// Quantas vezes cada caso corre.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Repetitions {
    /// Como o arquivo do caso declara: a corrida solta respeita a autoria, porque quem escreveu o
    /// caso sabe se ele precisa de mais de uma amostra.
    AsAuthored,
    /// O mesmo número para todo caso. É assim que as fases do bakeoff comparam candidatos: sobre
    /// a mesma quantidade de evidência por caso, senão um caso repetido pesaria mais que outro na
    /// taxa que decide o default.
    Fixed(u32),
}

/// A trava de gasto do runner, em milionésimos de dólar sobre o custo declarado pelo provedor.
///
/// Ela é UMA e viaja por todas as corridas do bakeoff: uma trava por corrida deixaria o teto ser
/// gasto uma vez por candidato. Fecha ANTES de abrir a próxima repetição — a rodada em andamento
/// termina, e o estouro máximo é o teto de UMA rodada, que os limites por rodada já prendem.
///
/// O teto da FASE é o segundo botão: a peneira corre sob uma fatia do teto para que a final, que
/// é quem decide o default, encontre dinheiro sobrando quando chegar a vez dela.
#[derive(Debug)]
pub(crate) struct SpendLock {
    cap_micro_usd: i64,
    phase_cap_micro_usd: i64,
    spent_micro_usd: i64,
    cost_gap: bool,
}

impl SpendLock {
    pub(crate) fn new(cap_micro_usd: i64) -> Self {
        Self {
            cap_micro_usd,
            phase_cap_micro_usd: cap_micro_usd,
            spent_micro_usd: 0,
            cost_gap: false,
        }
    }

    /// Abre uma fase sob o teto pedido, nunca acima do teto total.
    pub(crate) fn open_phase(&mut self, cap_micro_usd: i64) {
        self.phase_cap_micro_usd = cap_micro_usd.min(self.cap_micro_usd);
    }

    /// Uma repetição pode nascer? Custo não declarado fecha tudo: sem o número do provedor a
    /// trava fica cega, e cega ela não é trava.
    pub(crate) fn may_start(&self) -> bool {
        !self.cost_gap && self.spent_micro_usd < self.phase_cap_micro_usd
    }

    /// Quanto ainda cabe, pelo MENOR dos dois tetos vigentes. É o que a rodada seguinte pode
    /// gastar no pior caso: a trava fecha entre repetições, então sem apertar o teto POR rodada a
    /// última repetição estouraria o teto pelo tamanho dela — e olhar só o teto total deixaria a
    /// última rodada da peneira comer a reserva da final.
    pub(crate) fn remaining_micro_usd(&self) -> i64 {
        (self.cap_micro_usd.min(self.phase_cap_micro_usd) - self.spent_micro_usd).max(0)
    }

    pub(crate) fn record(&mut self, cost_micro_usd: i64, cost_declared: bool) {
        self.spent_micro_usd = self.spent_micro_usd.saturating_add(cost_micro_usd);
        if !cost_declared {
            self.cost_gap = true;
        }
    }

    pub(crate) fn spent_micro_usd(&self) -> i64 {
        self.spent_micro_usd
    }

    pub(crate) fn cap_micro_usd(&self) -> i64 {
        self.cap_micro_usd
    }

    pub(crate) fn phase_cap_micro_usd(&self) -> i64 {
        self.phase_cap_micro_usd
    }

    pub(crate) fn cost_gap(&self) -> bool {
        self.cost_gap
    }
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
    /// A rodada foi cortada pelo teto que a TRAVA apertou, não pelo teto da conversa. É outra
    /// coisa que uma resposta ruim: o modelo não terminou de responder porque o dinheiro do
    /// bakeoff acabou, e contar isso como erro dele — ou a corrida como completa — inventaria um
    /// resultado que ninguém mediu.
    pub budget_truncated: bool,
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

impl CaseRun {
    /// O caso foi medido inteiro: nem faltaram repetições, nem alguma delas parou no meio porque
    /// o dinheiro acabou.
    pub(crate) fn measured(&self) -> bool {
        !self.aborted && !self.outcomes.iter().any(|outcome| outcome.budget_truncated)
    }
}

#[derive(Debug)]
pub(crate) struct BenchRun {
    pub pin: &'static ModelPin,
    /// O prefixo tinha o núcleo do método? O relatório carrega a resposta porque os casos de
    /// didática só valem julgamento quando o núcleo estava montado.
    pub method_core: bool,
    pub cases: Vec<CaseRun>,
    /// O custo DESTA corrida — é ele que compara candidatos. O acumulado do bakeoff vive na
    /// trava, que atravessa todas as corridas.
    pub total_cost_micro_usd: i64,
    /// A trava vigente na corrida, gravada no relatório: um total baixo com trava baixa e um
    /// total baixo porque tudo passou são histórias diferentes.
    pub max_spend_micro_usd: i64,
    pub spend_lock_hit: bool,
    /// A corrida parou por falha operacional — pool, migração, fixture, consentimento. O que ela
    /// já mediu (e já pagou) fica aqui: um erro que levasse embora os outcomes deixaria dinheiro
    /// gasto fora do relatório.
    pub failure: Option<String>,
    /// O provedor deixou de declarar custo em alguma repetição. A bancada fecha na hora — sem
    /// custo declarado a trava do runner é cega, e "custo zero" no relatório significaria "não
    /// medi", nunca "foi de graça".
    pub cost_gap: bool,
}

/// A configuração cobre o catálogo que vai correr?
///
/// Didática sem o núcleo do método não mede ensino — mede a recusa de capacidade da camada ausente,
/// e o julgamento cego receberia respostas que nunca tiveram como ensinar. Melhor recusar a bancada
/// inteira do que pagar por uma família que não vale julgamento. Vive fora do laço porque quem
/// chama precisa poder perguntar ANTES de qualquer rodada paga: descobrir isso depois da sonda
/// significa ter pago por ela à toa.
pub(crate) fn ensure_pack_covers(
    cases: &[Case],
    pack_root: Option<&std::path::Path>,
) -> Result<(), String> {
    if pack_root.is_none()
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
    Ok(())
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

/// O teto de custo vigente numa repetição, e de onde ele veio.
pub(crate) struct RoundBudget {
    pub limits: RunLimits,
    /// O teto foi reduzido pelo que sobrava na trava, e já não é o teto que a conversa usa no app.
    /// É o que distingue "o modelo gastou demais" de "acabou o dinheiro do bakeoff".
    pub tightened: bool,
}

/// Roda UMA repetição de um caso num pool já semeado, atravessando o loop real.
pub(crate) async fn run_repetition<A: ProviderAdapter>(
    pool: &SqlitePool,
    ctx: &Context,
    adapter: &A,
    pin: &'static ModelPin,
    budget: RoundBudget,
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
        limits: budget.limits,
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
    // Três perguntas, e uma lacuna em qualquer delas fecha: o stream publicou uso sem custo? a
    // rodada viu uso sem custo em ALGUMA tentativa, inclusive nas que falharam e não publicam
    // evento? houve turno sem um centavo contado?
    let cost_declared = !usage_without_cost
        && outcome.cost_declared
        && (outcome.turns == 0 || outcome.cost_micro_usd > 0);

    // Quem cortou a rodada: o teto da conversa ou o dinheiro que sobrava? Só o segundo desqualifica
    // a medição — o primeiro é um resultado legítimo sobre o modelo.
    let budget_truncated = outcome.stop == StopReason::CostCap && budget.tightened;

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
        budget_truncated,
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
    lock: &mut SpendLock,
) -> Result<BenchRun, String> {
    ensure_pack_covers(&cases, config.pack_root.as_deref())?;

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
    let system = match &config.system {
        Some(snapshot) => std::sync::Arc::clone(snapshot),
        None => std::sync::Arc::new(
            prompt::system_prompt(&MethodPack::at(&pack_root))
                .await
                .map_err(|error| format!("{} {}", error.message, error.fix))?,
        ),
    };

    let mut runs: Vec<CaseRun> = Vec::with_capacity(cases.len());
    let mut total_cost_micro_usd = 0_i64;
    let mut spend_lock_hit = false;
    let mut failure: Option<String> = None;

    for case in cases {
        let mut outcomes = Vec::new();
        let mut aborted = false;
        if failure.is_some() {
            runs.push(CaseRun {
                case,
                outcomes,
                aborted: true,
            });
            continue;
        }
        let repetitions = match config.repetitions {
            Repetitions::AsAuthored => case.repetitions,
            Repetitions::Fixed(fixed) => fixed,
        };

        for _ in 0..repetitions {
            if !lock.may_start() {
                spend_lock_hit = !lock.cost_gap();
                aborted = true;
                break;
            }

            // A preparação da repetição é falível, e falhar aqui NÃO pode descartar o que as
            // repetições anteriores já pagaram: a falha vira estado da corrida e o laço para.
            let prepared = prepare(config, &case.fixture).await;
            let pool = match prepared {
                Ok(pool) => pool,
                Err(error) => {
                    failure = Some(error);
                    aborted = true;
                    break;
                }
            };

            let ctx = Context {
                clock: fixtures::bench_clock(),
                pack: MethodPack::at(&pack_root),
            };
            // O teto da rodada é o menor entre o da conversa e o que sobra na trava: assim o teto
            // acumulado é respeitado pelo corte DENTRO da rodada, e não só pela decisão de não
            // abrir a próxima.
            let remaining = lock.remaining_micro_usd();
            let budget = RoundBudget {
                tightened: remaining < config.limits.max_cost_micro_usd,
                limits: RunLimits {
                    max_cost_micro_usd: config.limits.max_cost_micro_usd.min(remaining),
                    ..config.limits.clone()
                },
            };
            let outcome = run_repetition(
                &pool,
                &ctx,
                adapter,
                config.pin,
                budget,
                &system.text,
                &case,
            )
            .await;
            total_cost_micro_usd = total_cost_micro_usd.saturating_add(outcome.cost_micro_usd);
            lock.record(outcome.cost_micro_usd, outcome.cost_declared);
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
        max_spend_micro_usd: lock.phase_cap_micro_usd(),
        spend_lock_hit,
        cost_gap: lock.cost_gap(),
        failure,
    })
}

/// Monta o mundo de UMA repetição: pool novo, migrado, semeado e consentido. Cada repetição mede o
/// modelo sobre o mesmo mundo, nunca sobre o rastro da anterior.
async fn prepare(config: &BenchConfig, fixture: &str) -> Result<SqlitePool, String> {
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
    fixtures::seed(&pool, fixture).await?;
    // O consentimento é semeado porque quem roda a bancada JÁ consentiu — a chave dedicada é dela
    // — e o loop recusa rodada sem registro, na bancada como no app.
    consent::grant(&pool, config.pin, &fixtures::bench_clock().as_of())
        .await
        .map_err(|error| format!("O consentimento da bancada não gravou: {error}."))?;
    Ok(pool)
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

    // O julgamento é leitura e conta: não fala com o provedor, não gasta e não pede chave. As duas
    // recusas de ambiente valem para quem vai gastar dinheiro.
    let key = if cli.mode == cli::Mode::Judge {
        String::new()
    } else {
        let ci = std::env::var("CI").ok();
        let key = std::env::var("NEKO_MIA_BENCH_KEY").ok();
        if let Some(reason) = cli::refuse_reason(ci.as_deref(), key.as_deref()) {
            eprintln!("{reason}");
            return std::process::ExitCode::FAILURE;
        }
        key.expect("a recusa de ambiente cobre a chave ausente")
    };

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
    if cli.mode == cli::Mode::Judge {
        return judge(&cli).await;
    }

    let mut cases = case::load_catalog(&cli.cases_dir).map_err(|error| error.to_string())?;
    // O filtro corta DEPOIS da carga: o catálogo inteiro continua obrigado a ser válido e a
    // cobrir as seis famílias, mesmo quando só um caso vai rodar.
    if let Some(only) = &cli.only {
        cases.retain(|case| case.id.contains(only.as_str()));
        if cases.is_empty() {
            return Err(format!("Nenhum caso do catálogo casa com \"{only}\"."));
        }
    }

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
    let mut lock = SpendLock::new(cli.max_spend_micro_usd);
    let ran_at = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

    if cli.mode == cli::Mode::Bakeoff {
        let config = bakeoff::BakeoffConfig {
            cases,
            execution_id: uuid::Uuid::new_v4().to_string(),
            blind_sheet_path: std::cell::OnceCell::new(),
            pack_root: cli.pack_root.clone(),
            limits: RunLimits::default(),
            reports_dir: &cli.reports_dir,
            ran_at: &ran_at,
        };
        let (bakeoff, path) = bakeoff::run(&adapter, config, &mut lock).await?;
        // O caminho do caderno cego sai junto: é ele que se abre ANTES do relatório.
        let sheet = bakeoff.blind_sheet_path.clone();
        return Ok(bakeoff::summary(&bakeoff, &path, sheet.as_deref()));
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
    let config = BenchConfig {
        pin,
        pack_root: cli.pack_root.clone(),
        repetitions: Repetitions::AsAuthored,
        // A corrida solta monta o prefixo dela: é uma corrida só, e não há comparabilidade a
        // proteger entre candidatos.
        system: None,
        limits: RunLimits::default(),
    };
    let run = run_catalog(&adapter, cases, &config, &mut lock).await?;

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

/// Fecha o ciclo do julgamento cego: lê o caderno julgado, aplica os vereditos ao relatório e
/// grava a decisão final.
///
/// A decisão passa a existir no arquivo, não na cabeça de quem leu — e continua sendo adotada à
/// mão, porque trocar o pin é gesto deliberado.
async fn judge(cli: &cli::CliArgs) -> Result<String, String> {
    let report_path = cli.report.as_ref().expect("o parse exige --report");
    let verdicts_path = cli.verdicts.as_ref().expect("o parse exige --verdicts");

    let read = |path: &std::path::Path| -> Result<Value, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{} não pôde ser lido: {error}.", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("{} não parseia como JSON: {error}.", path.display()))
    };
    let mut report = read(report_path)?;
    let judged = read(verdicts_path)?;

    // Os bilhetes são determinísticos — caso e posição —, então o caderno de uma execução casaria
    // com o relatório de outra sem nada reclamar. É a identidade da execução que amarra os dois.
    let identity = |value: &Value, what: &str| -> Result<String, String> {
        value["execution_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{what} não traz execution_id: ele é de uma versão anterior."))
    };
    let report_id = identity(&report, "O relatório")?;
    let sheet_id = identity(&judged, "O caderno")?;
    if report_id != sheet_id {
        return Err(format!(
            "O caderno é da execução {sheet_id} e o relatório é da {report_id}. Julgue o caderno \
             que nasceu com este relatório."
        ));
    }

    // O veredito é sobre um TEXTO: um caderno com a resposta trocada faria a pessoa julgar uma
    // coisa e o comando aplicar o julgamento a outra.
    bakeoff::ensure_sheet_matches(&report, &judged)?;

    let entries = judged["entries"]
        .as_array()
        .ok_or_else(|| "O caderno julgado não traz uma lista entries.".to_string())?;
    let mut verdicts = std::collections::BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        let ticket = entry["ticket"]
            .as_str()
            .ok_or_else(|| "Um bilhete do caderno não tem identificador.".to_string())?;
        // A repetição é conferida ANTES do veredito: um bilhete em branco seguido do mesmo
        // bilhete preenchido passaria despercebido se a checagem viesse depois.
        if !seen.insert(ticket.to_string()) {
            return Err(format!(
                "O bilhete {ticket} aparece mais de uma vez no caderno. Deixe um veredito por \
                 bilhete."
            ));
        }
        // Sem veredito, o bilhete simplesmente não entra — e a cobertura reclama dele adiante,
        // nomeando o que falta ler em vez de decidir por omissão.
        let Some(verdict) = entry.get("verdict").and_then(Value::as_str) else {
            continue;
        };
        let verdict = match verdict {
            "aprovado" => bakeoff::Judgment::Approved,
            "reprovado" => bakeoff::Judgment::Rejected,
            other => {
                return Err(format!(
                    "O bilhete {ticket} traz o veredito \"{other}\": use \"aprovado\" ou \
                     \"reprovado\"."
                ));
            }
        };
        verdicts.insert(ticket.to_string(), verdict);
    }

    let decision = bakeoff::judged_decision(&report, &verdicts)?;
    report["decision"] = bakeoff::decision_json(&decision);
    report["judged_at"] =
        Value::String(chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false));

    let pack = cli.pack_root.as_ref().map(MethodPack::at);
    report::write_json(
        report_path.parent().unwrap_or(std::path::Path::new(".")),
        "",
        Some(report_path),
        &report,
        pack.as_ref(),
    )
    .await?;

    Ok(match decision {
        bakeoff::Decision::Adopt { model, rationale } => format!(
            "Default decidido: {model}.\n{rationale}\nPara adotar, mova o papel Default em \
             src-tauri/src/mia/provider/pins.rs para {model}.\nRelatório: {}",
            report_path.display()
        ),
        bakeoff::Decision::PendingBlindJudgment { .. } => {
            "O relatório segue pendente de julgamento.".to_string()
        }
        bakeoff::Decision::NoWinner { reason } => format!(
            "Sem default: {reason}\nRelatório: {}",
            report_path.display()
        ),
    })
}
