//! O relatório datado de uma execução da bancada.
//!
//! O relatório é o artefato que se versiona: modelo, endpoint, operador, o veredito de cada
//! repetição e o custo total, num JSON que o bakeoff compara e uma pessoa lê. As respostas
//! entram inteiras — é delas que o julgamento cego da didática precisa, e são elas a evidência
//! quando um caso mecânico reprova.
//!
//! Antes de virar arquivo, o texto MONTADO passa pela mesma varredura de privacidade que o
//! conteúdo servido do pack, quando o pack está presente: uma resposta que ecoasse termo privado
//! não pode virar arquivo versionável. Sem pack não há deny-list — e também não houve núcleo do
//! método na rodada; o gate de privacidade do repositório segue valendo no commit.

use super::grade::Verdict;
use super::{BenchRun, CaseRun, RepetitionOutcome};
use crate::mia::method_tools::{self, MethodPack};
use crate::mia::run::AnswerProvenance;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Monta o relatório. Pura: o instante da execução entra por parâmetro, e a mesma execução
/// rende sempre o mesmo relatório.
pub(crate) fn render(run: &BenchRun, ran_at: &str) -> Value {
    let cases: Vec<Value> = run.cases.iter().map(case_json).collect();

    let all = || run.cases.iter().flat_map(|case| case.outcomes.iter());
    let verdicts = |wanted: fn(&Verdict) -> bool| -> usize {
        all().filter(|outcome| wanted(&outcome.verdict)).count()
    };

    json!({
        "ran_at": ran_at,
        "model": run.pin.model,
        "endpoint": run.pin.endpoint,
        "operator": run.pin.operator,
        "method_core": run.method_core,
        "max_spend_micro_usd": run.max_spend_micro_usd,
        "total_cost_micro_usd": run.total_cost_micro_usd,
        "spend_lock_hit": run.spend_lock_hit,
        "cost_gap": run.cost_gap,
        // A falha operacional que abortou a corrida sai no arquivo: ela explica os casos
        // abortados, e a evidência que só existe no processo morre com ele.
        "failure": run.failure,
        "totals": {
            "cases": run.cases.len(),
            "repetitions": all().count(),
            "passed": verdicts(|verdict| matches!(verdict, Verdict::Passed)),
            "failed": verdicts(|verdict| matches!(verdict, Verdict::Failed { .. })),
            "pending_judgment": verdicts(|verdict| matches!(verdict, Verdict::PendingJudgment)),
            "aborted_cases": run.cases.iter().filter(|case| case.aborted).count(),
        },
        "cases": cases,
    })
}

fn case_json(case_run: &CaseRun) -> Value {
    let repetitions: Vec<Value> = case_run.outcomes.iter().map(repetition_json).collect();
    json!({
        "id": case_run.case.id,
        "family": case_run.case.family.slug(),
        "fixture": case_run.case.fixture,
        "question": case_run.case.question,
        "aborted": case_run.aborted,
        "measured": case_run.measured(),
        "repetitions": repetitions,
    })
}

fn repetition_json(outcome: &RepetitionOutcome) -> Value {
    let (verdict, failures, echoed_forbidden) = match &outcome.verdict {
        Verdict::Passed => ("passed", vec![], false),
        Verdict::PendingJudgment => ("pending_judgment", vec![], false),
        Verdict::Failed {
            failures,
            echoed_forbidden,
        } => ("failed", failures.clone(), *echoed_forbidden),
    };
    json!({
        "verdict": verdict,
        "failures": failures,
        // Qual falha foi: a resposta ecoou o que o caso proíbe, ou reprovou por outro motivo? Só
        // a primeira elimina o candidato, e quem lê o relatório de volta precisa saber a
        // diferença sem reinterpretar o texto das falhas.
        "echoed_forbidden": echoed_forbidden,
        "stop": format!("{:?}", outcome.stop),
        "provenance": outcome.provenance.map(|provenance| match provenance {
            AnswerProvenance::Calculo => "calculo",
            AnswerProvenance::Metodo => "metodo",
        }),
        "tools_called": outcome.tools_called,
        "cost_micro_usd": outcome.cost_micro_usd,
        "cost_declared": outcome.cost_declared,
        // Sem este campo no arquivo, quem lê o relatório de volta não distingue a repetição que o
        // orçamento cortou da que o modelo errou — e recomputaria a decisão sobre outra história.
        "budget_truncated": outcome.budget_truncated,
        // O erro terminal, quando houve: código e mensagem NOSSOS — o texto cru do provedor é
        // dado não confiável e fica no rastro, nunca no arquivo. É o que explica um stop Failed
        // sem exigir arqueologia manual contra o provedor.
        "error": outcome.error.as_ref().map(|error| json!({
            "code": format!("{:?}", error.code),
            "message": error.message,
            "fix": error.fix,
        })),
        "turns": outcome.turns,
        "attempts": outcome.attempts,
        "answer": outcome.answer,
    })
}

/// O nome do arquivo: instante da execução (até o segundo, com `:` trocado por `-` para valer
/// em qualquer sistema de arquivos) mais o modelo. Dois relatórios nunca disputam o mesmo nome,
/// e o diretório lista a história em ordem.
pub(crate) fn file_name(ran_at: &str, model: &str) -> String {
    let stamp: String = ran_at
        .chars()
        .take("2026-07-29T14:33:05".len())
        .map(|character| if character == ':' { '-' } else { character })
        .collect();
    let slug: String = model
        .chars()
        .map(|character| match character {
            '/' | '.' => '-',
            other => other,
        })
        .collect();
    format!("{stamp}-{slug}.json")
}

/// Escreve o relatório datado de uma corrida. Falha fechado: se a varredura de privacidade
/// bloquear, nenhum arquivo nasce.
pub(crate) async fn write(
    dir: &Path,
    run: &BenchRun,
    ran_at: &str,
    pack: Option<&MethodPack>,
) -> Result<PathBuf, String> {
    let report = render(run, ran_at);
    write_json(dir, &file_name(ran_at, run.pin.model), None, &report, pack).await
}

/// Escreve um relatório em JSON, com a varredura de privacidade antes de qualquer byte tocar o
/// disco.
///
/// `existing` é o arquivo que uma escrita anterior já abriu: sem ele, o nome nasce exclusivo —
/// duas execuções no mesmo segundo ganham sufixos, nunca o direito de apagar a evidência uma da
/// outra. Com ele, o mesmo arquivo é reescrito, que é como o bakeoff publica o andamento sem
/// deixar uma corrida paga fora do disco.
pub(crate) async fn write_json(
    dir: &Path,
    base: &str,
    existing: Option<&Path>,
    report: &Value,
    pack: Option<&MethodPack>,
) -> Result<PathBuf, String> {
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(report).expect("o relatório da bancada é serializável")
    );

    if let Some(pack) = pack {
        method_tools::privacy_scan(pack, "o relatório da bancada", &text)
            .await
            .map_err(|error| format!("{} {}", error.message, error.fix))?;
    }

    if let Some(path) = existing {
        return swap_into(path, &text).map(|()| path.to_path_buf());
    }

    let stem = base.strip_suffix(".json").unwrap_or(base);
    for attempt in 0..10 {
        let candidate = if attempt == 0 {
            dir.join(base)
        } else {
            dir.join(format!("{stem}-{}.json", attempt + 1))
        };
        // A primeira escrita vai direta no arquivo que ela mesma cria, e não pela troca das
        // reescritas: reservar o nome com um arquivo vazio para preencher depois abriria uma
        // janela em que uma queda deixaria em disco um JSON vazio — pior que o arquivo truncado
        // que a escrita direta pode deixar, porque aqui ainda não há checkpoint a proteger.
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(text.as_bytes())
                    .and_then(|()| file.sync_all())
                    // O nome também precisa chegar ao disco: sem isso, uma queda logo depois
                    // deixaria o diretório sem a entrada de um arquivo que já foi pago.
                    .and_then(|()| sync_dir(&candidate))
                    .map_err(|error| {
                        format!(
                            "O relatório não pôde ser escrito em {}: {error}.",
                            candidate.display()
                        )
                    })?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "O relatório não pôde ser escrito em {}: {error}.",
                    candidate.display()
                ));
            }
        }
    }
    Err(format!(
        "Dez relatórios com o mesmo instante já existem em {} — algo está reexecutando a \
         bancada em laço.",
        dir.display()
    ))
}

/// Grava e leva ao disco antes de devolver: sem o `sync`, a troca de nome poderia publicar um
/// arquivo cujo conteúdo ainda mora em cache.
fn write_synced(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// Escreve ao lado e troca o nome. Gravar por cima abriria uma janela em que uma queda no meio da
/// escrita levaria junto o checkpoint anterior — e o que se perde ali é a evidência de rodadas que
/// já foram pagas.
fn swap_into(path: &Path, text: &str) -> Result<(), String> {
    let staging = path.with_extension("json.parcial");
    write_synced(&staging, text)
        .and_then(|()| std::fs::rename(&staging, path))
        // Sincronizar o arquivo não publica o NOME: a entrada nova do diretório também precisa
        // chegar ao disco, ou uma queda logo depois da troca deixaria o diretório apontando para
        // o arquivo antigo.
        .and_then(|()| sync_dir(path))
        .map_err(|error| {
            let _ = std::fs::remove_file(&staging);
            format!(
                "O relatório não pôde ser escrito em {}: {error}.",
                path.display()
            )
        })
}

/// Leva a entrada de diretório ao disco. Sem efeito onde o sistema não permite abrir diretório
/// como arquivo — o que não é regressão: é o mesmo que se tinha antes.
fn sync_dir(path: &Path) -> std::io::Result<()> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    match std::fs::File::open(dir) {
        Ok(handle) => handle.sync_all(),
        Err(_) => Ok(()),
    }
}
