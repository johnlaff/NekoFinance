//! A retomada de um bakeoff interrompido: o que já foi pago volta como evidência, não como gasto.
//!
//! A peneira custa dinheiro de verdade e mede a matriz inteira. Uma execução que cai depois dela —
//! na final, na rede, na máquina — deixa em disco um relatório com a peneira completa; refazê-la
//! seria pagar duas vezes pela mesma medição. A retomada lê esse relatório, RECOMPUTA as contas a
//! partir das repetições brutas e reaproveita o que resistiu à conferência.
//!
//! Ler é a operação perigosa aqui: este é um arquivo de fronteira, e é dele que sai quem disputa a
//! final. Por isso nada vem do bloco `score` do relatório — ele é resumo, e resumo é conveniência.
//! Cada corrida é reconferida repetição por repetição, com a mesma régua da corrida ao vivo: campo
//! ausente recusa, agregado declarado precisa fechar com a soma, e o catálogo medido precisa ser o
//! catálogo de hoje.
//!
//! A FINAL segue outra política que a peneira, e de propósito. A peneira é tudo ou nada: retomar
//! com uma peneira parcial compararia um prefixo da matriz, que é exatamente a aposta que o bakeoff
//! existe para não fazer — peneira incompleta recusa a retomada inteira. Na final, cada corrida é
//! de um finalista e responde só por ele: a que passa na conferência inteira é reaproveitada, e
//! qualquer dúvida — corrida truncada, repetição faltando, custo que não fecha, modelo que não
//! saiu da peneira — devolve aquele pin para a fila de correr de novo. O erro barato aqui é gastar
//! outra vez; o caro é decidir o default sobre uma corrida que ninguém conferiu.

use super::bakeoff::{Probe, Score, ceiling_slice, contenders, survivors};
use super::case::Case;
use crate::mia::provider::pins::{ModelPin, PinRole, pin};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Uma corrida herdada: o pin que a correu, as contas refeitas e o registro dela como o relatório
/// anterior o publicou. O registro viaja VERBATIM — reescrevê-lo com o instante de hoje diria que
/// aquelas rodadas correram agora.
#[derive(Debug)]
pub(crate) struct InheritedRun {
    pub pin: &'static ModelPin,
    pub score: Score,
    pub run: Value,
    /// A configuração da requisição não estava no arquivo e quem invocou respondeu por ela.
    pub identity_assumed: bool,
}

/// O que a retomada traz de um relatório anterior.
#[derive(Debug, Default)]
pub(crate) struct Resumed {
    /// De onde veio, para o relatório novo dizer o que herdou e de quem.
    pub source: PathBuf,
    pub probes: Vec<Probe>,
    pub estimate_micro_usd: i64,
    /// O dinheiro que a execução anterior já pagou. Não pesa na trava desta — ela protege gasto
    /// NOVO — e entra no custo total do relatório separado do que se gastou agora.
    pub spent_micro_usd: i64,
    pub phase_one: Vec<InheritedRun>,
    pub phase_two: Vec<InheritedRun>,
    /// Alguma corrida herdada teve a identidade do pin reconhecida por quem invocou, em vez de
    /// conferida contra o arquivo. Sai no relatório: a decisão não esconde em que ela se apoia.
    pub pin_identity_assumed: bool,
}

/// A sugestão que fecha toda recusa: retomar não é obrigatório, e correr do zero sempre resolve.
const FIX: &str = "Rode `mia-bench bakeoff` sem --resume para medir do zero.";

/// Lê o relatório retomado contra o catálogo e a matriz de HOJE.
///
/// Puro: relatório e casos entram, evidência herdada sai. Nada aqui toca rede, chave ou disco.
pub(crate) fn parse(
    report: &Value,
    cases: &[Case],
    source: PathBuf,
    assume_pin_identity: bool,
) -> Result<Resumed, String> {
    let catalog: Vec<&str> = ids(report, "ids")?;
    let expected: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    if catalog != expected {
        return Err(format!(
            "O relatório retomado mediu outro catálogo: {} caso(s) contra os {} de hoje. Uma \
             peneira de outro catálogo não compara com a final deste. {FIX}",
            catalog.len(),
            expected.len()
        ));
    }
    let ceiling: Vec<&str> = ids(report, "ceiling_ids")?;
    let ceiling_expected: Vec<String> = ceiling_slice(cases)
        .iter()
        .map(|case| case.id.clone())
        .collect();
    if ceiling != ceiling_expected {
        return Err(format!(
            "O recorte da régua mudou: o relatório mediu {ceiling:?} e o catálogo de hoje deriva \
             {ceiling_expected:?}. {FIX}"
        ));
    }

    let sieve = phase(report, "phase_one")?;
    let matrix: Vec<String> = contenders()
        .iter()
        .map(|pin| pin.label.to_string())
        .collect();
    // Corrida sem o campo `candidate` entra pelo mesmo reconhecimento do resto da identidade:
    // com a flag, o nome do modelo responde por ela quando um único pin o corre — entre dois
    // esforços do mesmo modelo não há o que assumir, e o nome cru fica para a recusa nomear.
    let mut candidateless = false;
    let inherited_matrix: Vec<String> = sieve
        .iter()
        .map(|entry| {
            let run = &entry["run"];
            match run["candidate"].as_str() {
                Some(candidate) => candidate.to_string(),
                None => {
                    candidateless = true;
                    let model = run["model"].as_str().unwrap_or_default();
                    match crate::mia::provider::pins::by_model(model).as_slice() {
                        [only] if assume_pin_identity => only.label.to_string(),
                        _ => model.to_string(),
                    }
                }
            }
        })
        .collect();
    // A ordem também é conferida, não só o conjunto: a ordem de corrida é a da matriz, e um
    // relatório que a contradiz descreve outra matriz — ainda que os mesmos modelos apareçam.
    if inherited_matrix != matrix {
        let hint = if candidateless && assume_pin_identity {
            " Corrida sem o campo candidate só se assume para modelo que um único pin corre — \
             entre dois esforços do mesmo modelo não há o que assumir."
        } else if candidateless {
            " Corrida sem o campo candidate não prova qual candidato correu; \
             --assume-pin-identity responde por modelo que um único pin corre."
        } else {
            ""
        };
        return Err(format!(
            "A peneira do relatório correu {inherited_matrix:?} e a matriz de hoje corre \
             {matrix:?}, nesta ordem. Reaproveitar seria comparar candidatos que a matriz atual \
             não tem.{hint} {FIX}"
        ));
    }

    let mut phase_one = Vec::with_capacity(sieve.len());
    for entry in sieve {
        let run = parse_run(
            entry,
            cases,
            &catalog,
            &ceiling,
            super::bakeoff::PHASE_ONE_REPETITIONS,
            assume_pin_identity,
        )?;
        // Peneira parcial não se reaproveita: a final compararia um prefixo da matriz, e o
        // relatório leria como se todos tivessem concorrido.
        if !run.score.complete {
            return Err(format!(
                "A peneira do relatório não mediu {} por inteiro. {FIX}",
                run.pin.label
            ));
        }
        phase_one.push(run);
    }

    // Quem vai à final sai da peneira RECOMPUTADA, nunca da lista que o arquivo publicou: uma
    // corrida herdada de candidato eliminado mudaria o quórum da decisão — ou venceria sem ter
    // passado pela peneira que existe para eliminá-lo.
    let sieved: Vec<(&'static ModelPin, Score)> = phase_one
        .iter()
        .map(|run| (run.pin, run.score.clone()))
        .collect();
    let finalists = survivors(&sieved);

    let mut phase_two: Vec<InheritedRun> = Vec::new();
    for entry in phase(report, "phase_two")? {
        // A dúvida devolve o pin para a fila: uma corrida da final que não passa na conferência
        // inteira é refeita, e só ela.
        let Ok(run) = parse_run(
            entry,
            cases,
            &catalog,
            &ceiling,
            super::bakeoff::PHASE_TWO_REPETITIONS,
            assume_pin_identity,
        ) else {
            continue;
        };
        let repeated = phase_two
            .iter()
            .any(|other| other.pin.label == run.pin.label);
        if !run.score.complete
            || repeated
            || !finalists.iter().any(|pin| pin.label == run.pin.label)
        {
            continue;
        }
        phase_two.push(run);
    }

    let probes = probes(report)?;
    // Só as corridas ACEITAS contam: uma descartada não sustenta nada, nem o reconhecimento.
    let pin_identity_assumed = phase_one
        .iter()
        .chain(phase_two.iter())
        .any(|run| run.identity_assumed);
    Ok(Resumed {
        source,
        pin_identity_assumed,
        spent_micro_usd: spent(report, &probes)?,
        probes,
        estimate_micro_usd: positive(&report["probe"], "estimate_micro_usd")?,
        phase_one,
        phase_two,
    })
}

/// O dinheiro que as execuções anteriores já pagaram, conferido contra o que o próprio relatório
/// registra.
///
/// Numa retomada encadeada, o gasto de uma execução é só o dinheiro NOVO dela: somar apenas esse
/// campo perderia tudo o que a execução anterior herdou, e a terceira corrida publicaria um custo
/// total menor que o real. Por isso o herdado do arquivo entra na conta.
///
/// A conferência é por baixo: o declarado precisa cobrir a sonda e as corridas que o arquivo
/// registra. Declarar MAIS é possível e legítimo — uma corrida que caiu no meio pagou repetições
/// que não chegaram ao relatório —, mas declarar menos esconderia dinheiro gasto.
fn spent(report: &Value, probes: &[Probe]) -> Result<i64, String> {
    let declared = required_i64(report, "spent_micro_usd")?.saturating_add(
        match report.get("inherited_micro_usd") {
            None => 0,
            Some(_) => required_i64(report, "inherited_micro_usd")?,
        },
    );
    let mut registered: i64 = probes.iter().map(|probe| probe.cost_micro_usd).sum();
    for name in ["phase_one", "phase_two"] {
        for entry in phase(report, name)? {
            registered =
                registered.saturating_add(required_i64(&entry["run"], "total_cost_micro_usd")?);
        }
    }
    // O total publicado é DERIVADO dos dois componentes: quando o arquivo o traz e ele os
    // contradiz, não se sabe qual dos números é o gasto real — e é sobre esse número que a retomada
    // declara o custo da decisão.
    if report.get("total_cost_micro_usd").is_some() {
        let total = required_i64(report, "total_cost_micro_usd")?;
        if total != declared {
            return Err(format!(
                "O relatório publica total_cost_micro_usd {total} e os componentes somam \
                 {declared}. O arquivo não fecha consigo mesmo. {FIX}"
            ));
        }
    }
    if declared < registered {
        return Err(format!(
            "O relatório declara {declared} micro-USD gastos e as corridas com a sonda somam \
             {registered}: a conta não fecha, e retomar herdaria um custo menor que o pago. {FIX}"
        ));
    }
    Ok(declared)
}

/// Os identificadores de um recorte do catálogo declarado no relatório.
fn ids<'a>(report: &'a Value, field: &str) -> Result<Vec<&'a str>, String> {
    report["catalog"][field]
        .as_array()
        .ok_or_else(|| {
            format!("O relatório retomado não diz qual catálogo mediu ({field}). {FIX}")
        })?
        .iter()
        .map(|id| {
            id.as_str().ok_or_else(|| {
                "O catálogo do relatório tem um identificador não textual.".to_string()
            })
        })
        .collect()
}

fn phase<'a>(report: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    report[name]
        .as_array()
        .ok_or_else(|| format!("O relatório retomado não traz {name}. {FIX}"))
}

/// A sonda de custo do relatório retomado: o custo por rodada de cada pin, aos preços da data em
/// que ela correu. Sondar de novo pagaria uma rodada por modelo para remedir o que o arquivo já
/// registra — e o que a retomada precisa dela é grosso, dizer se a final que falta cabe no teto, com
/// margem de um quarto por cima justamente porque a projeção não é exata.
///
/// É a ÚNICA projeção de custo que a retomada tem — é dela que sai a resposta a "a final que falta
/// cabe no teto?". Uma sonda que não cobriu a matriz, que veio pela metade, que não teve custo
/// declarado ou que mediu o mesmo pin duas vezes projetaria menos que o real, e o erro para baixo
/// abre a final sob um teto que ela não cabe. Sonda assim recusa a retomada inteira.
fn probes(report: &Value) -> Result<Vec<Probe>, String> {
    let rounds = report["probe"]["rounds"]
        .as_array()
        .ok_or_else(|| format!("O relatório retomado não traz a sonda de custo. {FIX}"))?;
    let matrix: Vec<&str> = contenders().iter().map(|pin| pin.label).collect();
    let measured: Vec<&str> = rounds
        .iter()
        .map(|round| round["candidate"].as_str().unwrap_or_default())
        .collect();
    // Cobertura, ordem e unicidade de uma vez: a sonda corre um pin por vez, na ordem da matriz, e
    // qualquer desvio disso descreve outra sonda. A comparação é pelo rótulo do candidato — o nome
    // do modelo não distingue dois esforços do mesmo modelo.
    if measured != matrix {
        return Err(format!(
            "A sonda do relatório mediu {measured:?} e a matriz de hoje corre {matrix:?}, nesta \
             ordem. Sem uma rodada por candidato não há projeção de custo para a final. {FIX}"
        ));
    }

    let mut probes = Vec::with_capacity(rounds.len());
    for round in rounds {
        let model = round["candidate"].as_str().unwrap_or_default();
        let pin = pin(model).ok_or_else(|| {
            format!("A sonda do relatório mediu {model}, que não está na matriz de pins. {FIX}")
        })?;
        let complete = required_bool(round, "complete", model)?;
        let cost_declared = required_bool(round, "cost_declared", model)?;
        // `failure` é nulo ou o texto da falha, e mais nada: aceitar um tipo estranho como "não
        // houve falha" deixaria uma sonda quebrada passar por sonda boa e autorizar gasto.
        let failure = match present(round, "failure")? {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            other => {
                return Err(format!(
                    "A sonda de {model} declara failure {other}, que não é nem nulo nem o texto de \
                     uma falha. {FIX}"
                ));
            }
        };
        let cost_micro_usd = positive(round, "cost_micro_usd")?;
        if !complete || !cost_declared || failure.is_some() {
            return Err(format!(
                "A sonda de {model} não mediu uma rodada inteira com custo declarado, então o que \
                 ela projeta fica abaixo do real. {FIX}"
            ));
        }
        probes.push(Probe {
            pin,
            cost_micro_usd,
            cost_declared,
            complete,
            failure,
        });
    }
    Ok(probes)
}

/// Refaz as contas de uma corrida a partir das repetições brutas, com a régua da fase.
///
/// Estrita de propósito, na mesma linha da leitura que fecha o julgamento cego: o bloco `score` do
/// arquivo não é lido, campo ausente recusa em vez de virar zero cômodo, o total declarado precisa
/// fechar com a soma do cobrado e a corrida precisa cobrir o catálogo que ela diz ter medido —
/// apagar uma repetição reprovada faria o resto parecer uma suíte perfeita.
fn parse_run(
    entry: &Value,
    catalog_cases: &[Case],
    catalog: &[&str],
    ceiling: &[&str],
    repetitions: u32,
    assume_pin_identity: bool,
) -> Result<InheritedRun, String> {
    let run = &entry["run"];
    let model = run["model"]
        .as_str()
        .ok_or_else(|| format!("Uma corrida do relatório não nomeia o modelo. {FIX}"))?;
    // O rótulo do candidato identifica QUEM correu; os campos de configuração abaixo provam COMO.
    // Sem o rótulo, o nome do modelo só basta quando um único pin o corre — entre dois esforços do
    // mesmo modelo não há o que assumir, porque a ambiguidade é entre candidatos de HOJE, não
    // entre o arquivo e a matriz.
    let mut identity_assumed = false;
    let pin = match run["candidate"].as_str() {
        Some(candidate) => pin(candidate).ok_or_else(|| {
            format!("O relatório aponta {candidate}, que não está na matriz. {FIX}")
        })?,
        None => match crate::mia::provider::pins::by_model(model).as_slice() {
            [] => {
                return Err(format!(
                    "O relatório aponta {model}, que não está na matriz. {FIX}"
                ));
            }
            [only] if assume_pin_identity => {
                identity_assumed = true;
                only
            }
            [_] => {
                return Err(format!(
                    "A corrida de {model} não registra o candidato, então o arquivo não prova \
                     ter nascido do pin que a matriz declara hoje. Rode com --assume-pin-identity \
                     para responder por essa identidade, ou {FIX}"
                ));
            }
            _ => {
                return Err(format!(
                    "A corrida de {model} não registra o candidato, e a matriz de hoje corre \
                     {model} sob mais de um esforço — não há como saber qual deles correu, e \
                     identidade ambígua não se assume. {FIX}"
                ));
            }
        },
    };
    // O pin é por ENDPOINT e pela configuração da requisição, não por nome de modelo: o mesmo
    // modelo servido de outro lugar — ou pedido com outro cabeçalho beta, outro esforço de raciocínio
    // ou outro nome de teto de saída — responde outra coisa, e é outro candidato. O canary prova que
    // o pin de HOJE está limpo; é esta conferência que prova que a evidência herdada nasceu dele.
    let refuse = |field: &str, recorded: &Value, current: &Value| {
        format!(
            "A corrida de {model} registra {field} {recorded}, e a matriz de hoje pina {current}. \
             Outra configuração de requisição é outro candidato. {FIX}"
        )
    };
    for (field, current) in [
        ("endpoint", json!(pin.endpoint)),
        ("operator", json!(pin.operator)),
    ] {
        let recorded = present(run, field)?;
        if *recorded != current {
            return Err(refuse(field, recorded, &current));
        }
    }
    // A configuração da requisição não existe nos relatórios de formato anterior: a prova de
    // identidade fica FORA do arquivo, e reconhecê-la transfere a responsabilidade para quem
    // invoca. O reconhecimento supre a ausência e nada mais — campo registrado que diverge continua
    // recusando, porque aí a divergência está provada e nenhuma afirmação de fora a desfaz.
    // Relatório que registra o esforço sob o modelo de PISO nasceu de outra configuração de
    // requisição — a divergência está provada no próprio arquivo, então o reconhecimento de
    // identidade não a cobre: ele supre ausência, nunca desfaz prova.
    if run.get("reasoning_floor").is_some() {
        return Err(refuse(
            "reasoning_floor",
            &run["reasoning_floor"],
            &json!(pin.reasoning_effort.wire()),
        ));
    }
    for (field, current) in [
        ("beta_headers", json!(pin.beta_headers)),
        ("reasoning_effort", json!(pin.reasoning_effort.wire())),
        ("token_cap", json!(pin.token_cap.field())),
        ("turn_max_tokens", json!(pin.turn_max_tokens)),
    ] {
        match run.get(field) {
            Some(recorded) if *recorded != current => {
                return Err(refuse(field, recorded, &current));
            }
            Some(_) => {}
            None if assume_pin_identity => identity_assumed = true,
            None => {
                return Err(format!(
                    "A corrida de {model} não registra {field}, então o arquivo não prova ter \
                     nascido da configuração que a matriz declara hoje. Rode com \
                     --assume-pin-identity para responder por essa identidade — a garantia passa a \
                     ser de quem invoca, e o relatório sai dizendo isso —, ou {FIX}"
                ));
            }
        }
    }

    let declared_cost = required_i64(run, "total_cost_micro_usd")?;
    let cases = run["cases"]
        .as_array()
        .ok_or_else(|| format!("A corrida de {model} não traz os casos."))?;
    // Cada pin responde pela cobertura DELE: quem disputa corre o catálogo inteiro, o teto de
    // referência corre o recorte da régua. Cobrar do teto o catálogo todo leria desenho como
    // truncamento; cobrar do candidato o recorte aceitaria uma peneira pela metade.
    let coverage: &[&str] = if pin.role == PinRole::Ceiling && repetitions == 1 {
        ceiling
    } else {
        catalog
    };
    let measured_ids: Vec<&str> = cases
        .iter()
        .map(|case| {
            case["id"]
                .as_str()
                .ok_or_else(|| format!("Um caso da corrida de {model} não tem identificador."))
        })
        .collect::<Result<_, _>>()?;
    if measured_ids != coverage {
        return Err(format!(
            "A corrida de {model} mediu {measured_ids:?}, e a cobertura dela é {coverage:?}. {FIX}"
        ));
    }

    // A data da corrida é parte do custo dela: é a tarifa daquele dia que a cobrou, e é isso que
    // impede a decisão de comparar herdado com novo como se fossem o mesmo dinheiro.
    let priced_at = present(run, "ran_at")?
        .as_str()
        .ok_or_else(|| format!("A corrida de {model} não diz quando correu. {FIX}"))?
        .to_string();
    // O motivo da parada é lido, não deduzido: é ele que separa a corrida que o candidato não
    // mediu da corrida que o medidor do provedor impediu de medir.
    let halted_by = match present(run, "halted_by")? {
        Value::Null => None,
        Value::String(slug) => Some(super::Halt::from_slug(slug).ok_or_else(|| {
            format!("A corrida de {model} parou por {slug}, que o relatório não usa. {FIX}")
        })?),
        other => {
            return Err(format!(
                "A corrida de {model} declara halted_by {other}, que não é nem nulo nem um motivo \
                 de parada. {FIX}"
            ));
        }
    };
    let mut score = Score {
        priced_at,
        halted_by,
        mechanical_total: 0,
        mechanical_passed: 0,
        injection_failed: 0,
        pending_judgment: 0,
        cost_micro_usd: declared_cost,
        complete: true,
    };
    // Halt e falha operacional não são detalhe de leitura: uma corrida que parou por teto, medidor
    // quebrado ou pool caído mediu menos do que se propôs, ainda que os casos que sobraram no
    // arquivo pareçam inteiros. Os dois campos são EXIGIDOS: um relatório sem eles não é o de uma
    // corrida que terminou bem, é um arquivo que não diz como terminou.
    if score.halted_by.is_some() || !present(run, "failure")?.is_null() {
        score.complete = false;
    }
    let mut summed = 0_i64;
    for case in cases {
        let case_id = case["id"].as_str().unwrap_or_default();
        // A família decide o que ELIMINA — só a isca obedecida num caso de injeção tira o candidato
        // da disputa. Aceitá-la do arquivo deixaria uma troca de string apagar a eliminação, então
        // ela vem do catálogo de hoje e o arquivo só precisa concordar.
        let authored = catalog_cases
            .iter()
            .find(|authored| authored.id == case_id)
            .ok_or_else(|| format!("O caso {case_id} de {model} não está no catálogo. {FIX}"))?
            .family;
        if case["family"].as_str() != Some(authored.slug()) {
            return Err(format!(
                "O caso {case_id} de {model} diz ser da família {:?}, e o catálogo o declara como \
                 {}. {FIX}",
                case["family"].as_str().unwrap_or_default(),
                authored.slug()
            ));
        }
        let family = authored;
        let measured = required_bool(case, "measured", model)?;
        let aborted = required_bool(case, "aborted", model)?;
        score.complete = score.complete && measured && !aborted;

        let reps = case["repetitions"]
            .as_array()
            .ok_or_else(|| format!("O caso {case_id} de {model} não traz as repetições."))?;
        // Uma repetição apagada faria o resto parecer perfeito: caso medido traz exatamente as
        // repetições que a fase determina.
        if measured && reps.len() != repetitions as usize {
            return Err(format!(
                "O caso {case_id} de {model} diz ter sido medido e traz {} repetição(ões); a fase \
                 mede {repetitions}. {FIX}",
                reps.len()
            ));
        }
        for repetition in reps {
            let declared = required_i64(repetition, "cost_micro_usd")?;
            let charged = required_i64(repetition, "charged_micro_usd")?;
            if charged < declared {
                return Err(format!(
                    "Uma repetição de {model} cobra {charged} micro-USD abaixo do parcial \
                     declarado de {declared}. {FIX}"
                ));
            }
            summed = summed.saturating_add(charged);
            let _ = required_bool(repetition, "cost_declared", model)?;
            if required_bool(repetition, "budget_truncated", model)? {
                score.complete = false;
                continue;
            }
            match repetition["verdict"].as_str() {
                Some("passed") => {
                    score.mechanical_total += 1;
                    score.mechanical_passed += 1;
                }
                Some("failed") => {
                    score.mechanical_total += 1;
                    if family == super::case::Family::Injecao
                        && required_bool(repetition, "echoed_forbidden", model)?
                    {
                        score.injection_failed += 1;
                    }
                }
                Some("pending_judgment") => {
                    // Pendente sem resposta é bilhete que ninguém pode julgar: reaproveitar a
                    // corrida faria nascer um relatório que o `julgar` recusa — depois de a final
                    // já ter sido paga.
                    if repetition["answer"].as_str().is_none_or(str::is_empty) {
                        return Err(format!(
                            "Uma repetição de {model} no caso {case_id} espera julgamento cego e \
                             não traz a resposta que alguém precisa ler. {FIX}"
                        ));
                    }
                    score.pending_judgment += 1;
                }
                other => {
                    return Err(format!(
                        "Uma repetição de {model} traz o veredito {other:?}, que o relatório não \
                         usa. {FIX}"
                    ));
                }
            }
        }
    }
    // O custo agregado decide empate de qualidade: baixá-lo à mão escolheria o vencedor sem tocar
    // em nenhuma repetição.
    if summed != declared_cost {
        return Err(format!(
            "A corrida de {model} declara {declared_cost} micro-USD e as repetições somam {summed}. \
             {FIX}"
        ));
    }

    Ok(InheritedRun {
        pin,
        score,
        run: run.clone(),
        identity_assumed,
    })
}

/// Um campo que precisa ESTAR no arquivo. A indexação de JSON devolve nulo tanto para o campo
/// ausente quanto para o campo nulo, e as duas coisas não são a mesma: a conferência estrita só vale
/// se o arquivo tiver dito o que diz.
fn present<'a>(value: &'a Value, field: &str) -> Result<&'a Value, String> {
    value
        .get(field)
        .ok_or_else(|| format!("O relatório retomado não traz o campo {field}. {FIX}"))
}

/// Um inteiro estritamente positivo: zero aqui é projeção que não projeta nada.
fn positive(value: &Value, field: &str) -> Result<i64, String> {
    value[field]
        .as_i64()
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("O relatório retomado não declara {field} positivo. {FIX}"))
}

/// Um inteiro não negativo que precisa existir: ausente é recusa, nunca zero.
fn required_i64(value: &Value, field: &str) -> Result<i64, String> {
    value[field]
        .as_i64()
        .filter(|number| *number >= 0)
        .ok_or_else(|| format!("O relatório retomado não declara {field} válido. {FIX}"))
}

/// Um booleano que precisa existir: ausente é recusa, nunca falso.
fn required_bool(value: &Value, field: &str, model: &str) -> Result<bool, String> {
    value[field]
        .as_bool()
        .ok_or_else(|| format!("A corrida de {model} não declara {field}. {FIX}"))
}
