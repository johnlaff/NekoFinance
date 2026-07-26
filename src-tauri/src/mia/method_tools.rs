//! A camada de método ensina o percurso e nunca se apresenta como cálculo sobre os números da
//! pessoa.

use super::Args;
use super::envelope::{ErrorCode, Period, ToolError, ToolOutput, ToolResult};
use chrono::NaiveDate;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

pub(crate) const TOPICS: &[&str] = &[
    "metodo",
    "diario",
    "cartao",
    "economia",
    "reserva",
    "dividas",
    "financiamento",
    "patrimonio",
    "renda",
    "casal",
    "planejamento",
];

/// Onde o pack curado do método está montado nesta máquina. O conteúdo é privado por natureza
/// e nunca versionado: a fachada guarda só o caminho e lê no momento de servir.
pub(crate) struct MethodPack {
    root: PathBuf,
}

impl MethodPack {
    pub(crate) fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Em produção o pack é montado ao lado do banco, no diretório de dados do app.
    pub(crate) fn in_app_data(app_data_dir: &Path) -> Self {
        Self::at(app_data_dir.join("methodology-pack"))
    }
}

pub(crate) async fn method_guidance(
    pack: &MethodPack,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let topic = args.choice("topic", TOPICS)?.unwrap_or("metodo");
    let content = fs::read_to_string(pack.root.join("chapters").join(format!("{topic}.md")))
        .await
        .map_err(|_| method_not_installed())?;

    validate_privacy(pack, topic, &content).await?;

    let title = content
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or(topic)
        .to_string();

    Ok(ToolOutput {
        period: Period::day(today),
        data: json!({
            "topic": topic,
            "title": title,
            "provenance": "metodo",
            "content": content,
            "topics": TOPICS,
        }),
    })
}

fn method_not_installed() -> ToolError {
    ToolError::new(
        ErrorCode::NotFound,
        "O material do método não está instalado nesta máquina.",
        "Instale o pack do método; sem ele, a conversa responde sobre os números, não sobre o método.",
    )
}

async fn validate_privacy(pack: &MethodPack, topic: &str, chapter: &str) -> Result<(), ToolError> {
    let mut entries = fs::read_dir(&pack.root).await.map_err(|_| {
        privacy_blocked(
            format!("Não foi possível validar a deny-list do capítulo \"{topic}\"."),
            "Instale o pack com pelo menos uma deny-list forbidden*.txt na raiz antes de pedir orientação sobre o método.",
        )
    })?;
    let mut deny_lists = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|_| {
        privacy_blocked(
            format!("Não foi possível validar a deny-list do capítulo \"{topic}\"."),
            "Instale o pack com pelo menos uma deny-list forbidden*.txt na raiz antes de pedir orientação sobre o método.",
        )
    })? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("forbidden") && name.ends_with(".txt") {
            deny_lists.push((name, entry.path()));
        }
    }

    if deny_lists.is_empty() {
        return Err(privacy_blocked(
            format!("O capítulo \"{topic}\" não pode ser servido sem deny-list de privacidade."),
            "Instale o pack com pelo menos uma deny-list forbidden*.txt na raiz antes de pedir orientação sobre o método.",
        ));
    }

    deny_lists.sort_by(|left, right| left.0.cmp(&right.0));
    let mut patterns = Vec::with_capacity(deny_lists.len());
    for (name, path) in deny_lists {
        let content = fs::read_to_string(path).await.map_err(|_| {
            privacy_blocked(
                format!("A deny-list \"{name}\" do capítulo \"{topic}\" não pôde ser lida."),
                "Repare a deny-list do pack antes de pedir orientação sobre o método.",
            )
        })?;
        patterns.push((name, content));
    }

    let chapter_lowercase = chapter.to_lowercase();
    for (name, list) in patterns {
        let mut entry_number = 0;
        for line in list.lines() {
            if line.is_empty() {
                continue;
            }
            let pattern = line.trim();
            if pattern.starts_with('#') {
                continue;
            }
            entry_number += 1;
            // Um gate de privacidade que diverge da curadoria pode servir o que ela barraria;
            // por isso qualquer divergência vira recusa.
            if pattern.is_empty() {
                return Err(privacy_blocked(
                    format!(
                        "A deny-list \"{name}\" do capítulo \"{topic}\" está malformada na entrada #{entry_number}: a linha só contém espaços."
                    ),
                    "Corrija a linha em branco da deny-list antes de pedir orientação sobre o método.",
                ));
            }
            if chapter_lowercase.contains(&pattern.to_lowercase()) {
                // O termo bloqueado nunca sai no erro: repeti-lo transformaria o próprio gate em
                // vazamento do conteúdo que ele precisa manter local.
                return Err(privacy_blocked(
                    format!(
                        "O capítulo \"{topic}\" foi bloqueado pela deny-list \"{name}\" na entrada #{entry_number}."
                    ),
                    "Revise o conteúdo privado do pack antes de pedir orientação sobre o método.",
                ));
            }
        }
    }

    Ok(())
}

fn privacy_blocked(message: impl Into<String>, fix: impl Into<String>) -> ToolError {
    ToolError::new(ErrorCode::PrivacyBlocked, message, fix)
}
