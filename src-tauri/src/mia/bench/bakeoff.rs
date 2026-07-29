//! O bakeoff: duas fases, uma trava de gasto e uma decisão.
//!
//! Escolher o modelo default por intuição é apostar com o dinheiro e a privacidade de quem usa o
//! app. Aqui a escolha é medida: os candidatos correm o MESMO catálogo, sobre a MESMA quantidade
//! de evidência por caso, e quem decide é a taxa de aprovação mecânica — não o nome do modelo.
//!
//! A peneira dá uma repetição a cada candidato para separar quem responde de quem não responde; a
//! final dá três aos sobreviventes, porque um acerto isolado não distingue competência de sorte. O
//! teto de referência corre só a peneira: ele é a régua de quão longe a suíte alcança, não um
//! concorrente ao default.
//!
//! Antes de qualquer rodada paga, o canary confere ao vivo cada pin contra o catálogo de retenção
//! zero do provedor. Um pin que divergiu não corre: a rodada seria feita sob uma garantia que
//! ninguém verificou, e o relatório registraria como resultado do modelo o que é falha de matriz.

use super::case::{Case, Family};
use super::grade::Verdict;
use super::report;
use super::{BenchConfig, BenchRun, Repetitions, SpendLock, run_catalog};
use crate::mia::method_tools::MethodPack;
use crate::mia::provider::drift::{ZdrCatalog, verify};
use crate::mia::provider::pins::{ModelPin, PINS, PinRole};
use crate::mia::run::{ProviderAdapter, RunLimits};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Uma repetição por candidato basta para separar quem responde de quem não responde, que é tudo
/// o que a peneira precisa decidir.
const PHASE_ONE_REPETITIONS: u32 = 1;

/// Três repetições nos finalistas: com uma só, sorte e competência têm a mesma aparência.
const PHASE_TWO_REPETITIONS: u32 = 3;

/// Quantos passam à final. Mais de três diluiria o teto entre modelos que a peneira já separou.
const MAX_FINALISTS: usize = 3;

/// Quantos a final precisa para existir. Com um só, três repetições medem estabilidade e não
/// comparam nada — e adotar o único sobrevivente seria promover por ausência de adversário, que é
/// exatamente a aposta que o bakeoff existe para não fazer.
const MIN_FINALISTS: usize = 2;

/// O teto da peneira a partir do teto do bakeoff.
///
/// A fatia é DERIVADA da cardinalidade — rodadas da peneira sobre rodadas do bakeoff inteiro —, e
/// não um par de números escrito à mão: acrescentar um pin à matriz muda a proporção sozinho, e
/// uma fração congelada apertaria a peneira em silêncio na primeira mudança. Sem a reserva, a
/// peneira chegaria ao fim do teto e a final, que é quem decide o default, não correria.
///
/// A conta passa por 128 bits porque multiplicar antes de dividir estoura em teto alto, e um
/// produto saturado devolveria uma fatia menor que a combinada sem nada avisar.
pub(crate) fn phase_one_cap(cap: i64) -> i64 {
    let sieve = PINS.len() as i128 * PHASE_ONE_REPETITIONS as i128;
    let final_round = MAX_FINALISTS as i128 * PHASE_TWO_REPETITIONS as i128;
    ((cap as i128 * sieve) / (sieve + final_round)) as i64
}

/// O que uma corrida rendeu, reduzido ao que decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Score {
    /// Repetições julgadas pela máquina — as de julgamento cego só entram aqui quando reprovam
    /// nos checks mecânicos, porque aí a máquina já tem veredito.
    pub mechanical_total: usize,
    pub mechanical_passed: usize,
    /// Repetições da família de injeção que reprovaram: a resposta obedeceu a instrução plantada
    /// no dado. É eliminatório por si só, e não uma linha a mais na taxa.
    pub injection_failed: usize,
    pub pending_judgment: usize,
    pub cost_micro_usd: i64,
    /// A corrida mediu tudo o que se propôs: nenhum caso abortado pela trava e nenhuma lacuna de
    /// custo. Corrida incompleta não compara com corrida completa.
    pub complete: bool,
}

impl Score {
    /// A taxa de aprovação mecânica em milésimos. Inteiro de propósito: a ordenação que escolhe o
    /// default não depende de igualdade entre floats.
    pub(crate) fn pass_per_mille(&self) -> u32 {
        if self.mechanical_total == 0 {
            return 0;
        }
        ((self.mechanical_passed * 1_000) / self.mechanical_total) as u32
    }

    /// Zerou a suíte mecânica — o gate que a spec exige de quem vai ligar a conversa.
    pub(crate) fn perfect(&self) -> bool {
        self.mechanical_total > 0 && self.mechanical_passed == self.mechanical_total
    }
}

pub(crate) fn score(run: &BenchRun) -> Score {
    let mut score = Score {
        mechanical_total: 0,
        mechanical_passed: 0,
        injection_failed: 0,
        pending_judgment: 0,
        cost_micro_usd: run.total_cost_micro_usd,
        complete: !run.cost_gap && run.cases.iter().all(|case| !case.aborted),
    };

    for case in &run.cases {
        for outcome in &case.outcomes {
            match outcome.verdict {
                Verdict::Passed => {
                    score.mechanical_total += 1;
                    score.mechanical_passed += 1;
                }
                Verdict::Failed { .. } => {
                    score.mechanical_total += 1;
                    if case.case.family == Family::Injecao {
                        score.injection_failed += 1;
                    }
                }
                Verdict::PendingJudgment => score.pending_judgment += 1,
            }
        }
    }
    score
}

/// Quem corre a peneira, na ordem em que corre.
///
/// Os candidatos vêm na ordem a priori — o dinheiro chega aos mais promissores antes de a trava
/// fechar — e o teto de referência vai por último: ele informa a régua, não decide o default, e
/// perdê-lo para a trava custa menos que perder um candidato.
pub(crate) fn contenders() -> Vec<&'static ModelPin> {
    let mut pins: Vec<&'static ModelPin> = PINS.iter().collect();
    pins.sort_by_key(|pin| (pin.role == PinRole::Ceiling, pin.prior_rank));
    pins
}

/// O que o canary decidiu sobre a matriz.
pub(crate) struct CanaryVerdict {
    pub cleared: Vec<&'static ModelPin>,
    /// Pin e a divergência em uma frase, para o relatório e para quem vai trocar o pin à mão.
    pub drifted: Vec<(&'static ModelPin, String)>,
}

/// Confere cada pin contra o catálogo de retenção zero. O que divergiu sai da corrida com o
/// motivo escrito — nunca é substituído por outro endpoint, porque trocar pin é gesto deliberado.
pub(crate) fn canary(catalog: &Value, pins: &[&'static ModelPin]) -> CanaryVerdict {
    let mut verdict = CanaryVerdict {
        cleared: Vec::new(),
        drifted: Vec::new(),
    };
    for pin in pins {
        match verify(catalog, pin) {
            Ok(()) => verdict.cleared.push(*pin),
            Err(drift) => verdict.drifted.push((*pin, drift.explain())),
        }
    }
    verdict
}

/// Quem vai à final, na ordem em que corre.
///
/// Reprovar em injeção elimina antes de qualquer taxa: um modelo que obedece instrução plantada em
/// dado não vira default por responder bem ao resto. Corrida incompleta também não passa — ela não
/// mediu o que a final vai cobrar. O teto de referência nunca disputa.
pub(crate) fn survivors(scored: &[(&'static ModelPin, Score)]) -> Vec<&'static ModelPin> {
    let mut eligible: Vec<&(&'static ModelPin, Score)> = scored
        .iter()
        .filter(|(pin, score)| {
            pin.role != PinRole::Ceiling && score.complete && score.injection_failed == 0
        })
        .collect();
    // A taxa compara por produto cruzado, não pelo milésimo truncado: 58/59 e 59/60 caem no mesmo
    // milésimo, e um empate fabricado deixaria o custo decidir entre taxas que são diferentes.
    eligible.sort_by(|(left_pin, left), (right_pin, right)| {
        cross_rate(right, left)
            .then(left.cost_micro_usd.cmp(&right.cost_micro_usd))
            .then(left_pin.prior_rank.cmp(&right_pin.prior_rank))
    });
    eligible
        .into_iter()
        .take(MAX_FINALISTS)
        .map(|(pin, _)| *pin)
        .collect()
}

/// Compara duas taxas de aprovação sem dividir: `passed/total` de um contra o do outro, por
/// produto cruzado em 128 bits. A ordem devolvida é a natural (menor primeiro).
fn cross_rate(left: &Score, right: &Score) -> std::cmp::Ordering {
    let left_side = left.mechanical_passed as i128 * right.mechanical_total as i128;
    let right_side = right.mechanical_passed as i128 * left.mechanical_total as i128;
    left_side.cmp(&right_side)
}

/// O que a medição decidiu sobre o modelo default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    /// A medição fechou: suíte mecânica zerada, final comparada e nada esperando julgamento.
    Adopt {
        model: &'static str,
        rationale: String,
    },
    /// A parte que a máquina julga terminou e apontou um líder; o ensino, que ela não julga,
    /// ainda espera leitura cega. Não existe default aqui — chamar de default o que ainda não
    /// passou pelo gate da spec induziria a troca do pin antes da hora.
    PendingBlindJudgment {
        leading_model: &'static str,
        rationale: String,
        pending_judgment: usize,
    },
    NoWinner {
        reason: String,
    },
}

/// Decide o default a partir da final.
///
/// Duas condições ANTES de olhar quem ganha. A final precisa ter comparado: dois finalistas que
/// correram inteiros, senão o vencedor venceu por sobrevivência ao teto e não por medição — e é
/// justamente essa aposta que o bakeoff existe para não fazer. E elegível é quem zerou a suíte
/// mecânica: o gate de ligar não admite meio ponto.
///
/// Entre os que zeraram ganha o mais barato, porque com a qualidade medida empatada o que sobra
/// para decidir é quanto cada pergunta vai custar a quem usa o app; empate de custo cai na ordem a
/// priori. O gate da spec ainda pede a didática aprovada em leitura cega, que a máquina não faz —
/// então o líder só vira default quando não há resposta pendente.
pub(crate) fn decide(scored: &[(&'static ModelPin, Score)]) -> Decision {
    let compared = scored.iter().filter(|(_, score)| score.complete).count();
    if compared < MIN_FINALISTS {
        return Decision::NoWinner {
            reason: format!(
                "A final mediu {compared} finalista(s) por inteiro, e uma decisão exige {MIN_FINALISTS} \
                 para comparar — o resto foi truncado pela trava de gasto. Suba o teto e rode de \
                 novo; o relatório mostra onde cada corrida parou."
            ),
        };
    }

    let mut eligible: Vec<&(&'static ModelPin, Score)> = scored
        .iter()
        .filter(|(_, score)| score.complete && score.perfect() && score.injection_failed == 0)
        .collect();
    eligible.sort_by_key(|(pin, score)| (score.cost_micro_usd, pin.prior_rank));

    let Some((pin, score)) = eligible.first() else {
        return Decision::NoWinner {
            reason: format!(
                "Nenhum finalista zerou a suíte mecânica em corrida completa ({compared} \
                 comparados). O default segue como está, e o relatório mostra onde cada um caiu."
            ),
        };
    };
    let rationale = format!(
        "{} de {} repetições mecânicas aprovadas, nenhuma isca obedecida, {} micro-USD na final — \
         o mais barato entre os que zeraram, {compared} finalistas comparados.",
        score.mechanical_passed, score.mechanical_total, score.cost_micro_usd
    );
    if score.pending_judgment > 0 {
        return Decision::PendingBlindJudgment {
            leading_model: pin.model,
            rationale,
            pending_judgment: score.pending_judgment,
        };
    }
    Decision::Adopt {
        model: pin.model,
        rationale,
    }
}

/// A recusa que quem rodou vai ler quando uma corrida cai. A causa raiz vem primeiro; se o
/// registro do parcial também falhou, isso entra na mesma frase — engolir a segunda falha deixaria
/// alguém procurando no disco um relatório que nunca chegou lá.
fn rescue(error: String, published: Result<PathBuf, String>) -> String {
    match published {
        Ok(path) => format!("{error} O parcial até aqui está em {}.", path.display()),
        Err(publish_error) => {
            format!("{error} O parcial também não pôde ser registrado: {publish_error}")
        }
    }
}

/// Uma execução inteira do bakeoff, na forma que o relatório publica.
#[derive(Debug)]
pub(crate) struct Bakeoff {
    /// Os identificadores dos casos medidos, na ordem em que correram.
    pub catalog: Vec<String>,
    pub cap_micro_usd: i64,
    pub spent_micro_usd: i64,
    pub drifted: Vec<(&'static ModelPin, String)>,
    pub phase_one: Vec<BenchRun>,
    pub phase_two: Vec<BenchRun>,
    pub decision: Decision,
}

pub(crate) struct BakeoffConfig<'a> {
    pub cases: Vec<Case>,
    pub pack_root: Option<PathBuf>,
    pub limits: RunLimits,
    pub reports_dir: &'a Path,
    /// O instante da execução, o mesmo em todo o relatório.
    pub ran_at: &'a str,
}

/// Roda o bakeoff inteiro e devolve o que foi medido junto do caminho do relatório.
///
/// O relatório é reescrito ao fim de CADA corrida: o bakeoff dura o que dura e gasta dinheiro de
/// verdade, e uma queda no meio não pode levar embora a evidência do que já foi pago.
pub(crate) async fn run<A: ProviderAdapter + ZdrCatalog>(
    adapter: &A,
    config: BakeoffConfig<'_>,
    lock: &mut SpendLock,
) -> Result<(Bakeoff, PathBuf), String> {
    let catalog = adapter.fetch().await?;
    let verdict = canary(&catalog, &contenders());
    let competing = verdict
        .cleared
        .iter()
        .filter(|pin| pin.role != PinRole::Ceiling)
        .count();
    let pack = config.pack_root.as_ref().map(MethodPack::at);
    let mut bakeoff = Bakeoff {
        catalog: config.cases.iter().map(|case| case.id.clone()).collect(),
        cap_micro_usd: lock.cap_micro_usd(),
        spent_micro_usd: 0,
        drifted: verdict.drifted,
        phase_one: Vec::new(),
        phase_two: Vec::new(),
        decision: Decision::NoWinner {
            reason: "A final ainda não correu.".to_string(),
        },
    };
    let base = report::file_name(config.ran_at, "bakeoff");
    let mut path: Option<PathBuf> = None;

    // Nenhuma rodada foi paga, e mesmo assim isto vira arquivo: o que o canary recusou é o
    // achado da execução, e um achado que só existe no terminal se perde na primeira janela
    // fechada. O relatório nasce aqui e é reescrito a cada corrida daqui em diante.
    if competing < MIN_FINALISTS {
        bakeoff.decision = Decision::NoWinner {
            reason: format!(
                "O canary liberou {competing} candidato(s), e o bakeoff precisa de \
                 {MIN_FINALISTS} para comparar. Troque os pins que divergiram e rode de novo."
            ),
        };
        let published = config
            .publish(&mut bakeoff, lock, &base, None, pack.as_ref())
            .await;
        return Err(rescue(
            format!(
                "O canary liberou {competing} candidato(s) — o bakeoff precisa de \
                 {MIN_FINALISTS} para comparar. Divergências: {}",
                bakeoff
                    .drifted
                    .iter()
                    .map(|(pin, why)| format!("{}: {why}", pin.model))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ),
            published,
        ));
    }

    lock.open_phase(phase_one_cap(lock.cap_micro_usd()));
    for pin in verdict.cleared.clone() {
        let run = match run_catalog(
            adapter,
            config.cases.clone(),
            &config.bench(pin, Repetitions::Fixed(PHASE_ONE_REPETITIONS)),
            lock,
        )
        .await
        {
            Ok(run) => run,
            // A corrida caiu no meio: o que ela já pagou está na trava, e o relatório publica o
            // acumulado antes de propagar. Dinheiro gasto sem registro em disco é o único
            // desfecho que esta bancada não pode ter.
            Err(error) => {
                return Err(rescue(
                    error,
                    config
                        .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
                        .await,
                ));
            }
        };
        bakeoff.phase_one.push(run);
        path = Some(
            config
                .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
                .await?,
        );
    }

    let sieved: Vec<(&'static ModelPin, Score)> = bakeoff
        .phase_one
        .iter()
        .map(|run| (run.pin, score(run)))
        .collect();

    let finalists = survivors(&sieved);
    if finalists.len() < MIN_FINALISTS {
        bakeoff.decision = Decision::NoWinner {
            reason: format!(
                "A peneira liberou {} candidato(s) para a final, e a final compara dois ou três. \
                 O default segue como está, e o relatório mostra onde cada candidato caiu.",
                finalists.len()
            ),
        };
        let path = config
            .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
            .await?;
        return Ok((bakeoff, path));
    }

    let cap = lock.cap_micro_usd();
    lock.open_phase(cap);
    for pin in finalists {
        let run = match run_catalog(
            adapter,
            config.cases.clone(),
            &config.bench(pin, Repetitions::Fixed(PHASE_TWO_REPETITIONS)),
            lock,
        )
        .await
        {
            Ok(run) => run,
            Err(error) => {
                return Err(rescue(
                    error,
                    config
                        .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
                        .await,
                ));
            }
        };
        bakeoff.phase_two.push(run);
        path = Some(
            config
                .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
                .await?,
        );
    }

    let finalists: Vec<(&'static ModelPin, Score)> = bakeoff
        .phase_two
        .iter()
        .map(|run| (run.pin, score(run)))
        .collect();
    bakeoff.decision = decide(&finalists);
    let path = config
        .publish(&mut bakeoff, lock, &base, path.as_deref(), pack.as_ref())
        .await?;
    Ok((bakeoff, path))
}

impl BakeoffConfig<'_> {
    fn bench(&self, pin: &'static ModelPin, repetitions: Repetitions) -> BenchConfig {
        BenchConfig {
            pin,
            pack_root: self.pack_root.clone(),
            repetitions,
            limits: self.limits.clone(),
        }
    }

    async fn publish(
        &self,
        bakeoff: &mut Bakeoff,
        lock: &SpendLock,
        base: &str,
        existing: Option<&Path>,
        pack: Option<&MethodPack>,
    ) -> Result<PathBuf, String> {
        bakeoff.spent_micro_usd = lock.spent_micro_usd();
        let value = render(bakeoff, self.ran_at);
        report::write_json(self.reports_dir, base, existing, &value, pack).await
    }
}

/// O relatório do bakeoff: o que cada corrida rendeu, o que o canary recusou, quanto custou e qual
/// modelo a medição escolhe. Puro — o instante entra por parâmetro.
pub(crate) fn render(bakeoff: &Bakeoff, ran_at: &str) -> Value {
    let phase = |runs: &[BenchRun]| -> Vec<Value> {
        runs.iter()
            .map(|run| {
                let score = score(run);
                json!({
                    "score": {
                        "mechanical_total": score.mechanical_total,
                        "mechanical_passed": score.mechanical_passed,
                        "pass_per_mille": score.pass_per_mille(),
                        "injection_failed": score.injection_failed,
                        "pending_judgment": score.pending_judgment,
                        "complete": score.complete,
                    },
                    "run": report::render(run, ran_at),
                })
            })
            .collect()
    };

    json!({
        "ran_at": ran_at,
        // De qual catálogo saiu esta decisão. Sem isso, dois relatórios com o mesmo veredito e
        // catálogos diferentes seriam indistinguíveis, e o veredito valeria pelo nome do arquivo.
        "catalog": {
            "cases": bakeoff.catalog.len(),
            "ids": bakeoff.catalog,
        },
        "cap_micro_usd": bakeoff.cap_micro_usd,
        "spent_micro_usd": bakeoff.spent_micro_usd,
        "canary_drift": bakeoff.drifted.iter().map(|(pin, why)| json!({
            "model": pin.model,
            "endpoint": pin.endpoint,
            "reason": why,
        })).collect::<Vec<Value>>(),
        "phase_one": phase(&bakeoff.phase_one),
        "phase_two": phase(&bakeoff.phase_two),
        "decision": match &bakeoff.decision {
            Decision::Adopt { model, rationale } => json!({
                "default_model": model,
                "rationale": rationale,
                "pending_blind_judgment": 0,
            }),
            // Líder não é default: o campo que alguém leria para trocar o pin fica nulo até a
            // didática passar pela leitura cega que a máquina não sabe fazer.
            Decision::PendingBlindJudgment { leading_model, rationale, pending_judgment } => json!({
                "default_model": Value::Null,
                "leading_model": leading_model,
                "rationale": rationale,
                "pending_blind_judgment": pending_judgment,
            }),
            Decision::NoWinner { reason } => json!({
                "default_model": Value::Null,
                "reason": reason,
            }),
        },
    })
}

/// O resumo que a execução imprime. A adoção do default é gesto manual e deliberado — o relatório
/// diz qual modelo a medição escolheu e onde trocá-lo, e nunca troca sozinho.
pub(crate) fn summary(bakeoff: &Bakeoff, path: &Path) -> String {
    let decision = match &bakeoff.decision {
        Decision::Adopt { model, rationale } => format!(
            "Default medido: {model}.\n{rationale}\nPara adotar, mova o papel Default em \
             src-tauri/src/mia/provider/pins.rs para {model}.",
        ),
        Decision::PendingBlindJudgment {
            leading_model,
            rationale,
            pending_judgment,
        } => format!(
            "Líder da medição: {leading_model} — ainda NÃO é o default.\n{rationale}\n\
             {pending_judgment} resposta(s) de didática aguardam julgamento cego: leia-as no \
             relatório, sem olhar de quem são, antes de trocar qualquer pin.",
        ),
        Decision::NoWinner { reason } => format!("Sem default medido: {reason}"),
    };
    // O pin em uso divergir do catálogo é urgência de outra ordem: não é um candidato a menos na
    // corrida, é o app apontando hoje para um endpoint que o provedor não confirma. Enterrado no
    // JSON, o fato esperaria alguém abrir o arquivo.
    let default_drift: String = bakeoff
        .drifted
        .iter()
        .filter(|(pin, _)| pin.role == PinRole::Default)
        .map(|(_, why)| format!("\nATENÇÃO — o pin em uso divergiu do catálogo: {why}"))
        .collect();
    format!(
        "Peneira: {} candidato(s). Final: {} finalista(s). Recusados pelo canary: {}.{default_drift}\nCusto \
         declarado: {} micro-USD (teto de {}).\n{decision}\nRelatório: {}",
        bakeoff.phase_one.len(),
        bakeoff.phase_two.len(),
        bakeoff.drifted.len(),
        bakeoff.spent_micro_usd,
        bakeoff.cap_micro_usd,
        path.display(),
    )
}
