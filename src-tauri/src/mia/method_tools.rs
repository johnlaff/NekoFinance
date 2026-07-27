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

    /// O núcleo curado, que vira o prefixo estável do prompt.
    pub(crate) fn core(&self) -> PathBuf {
        self.root.join("core.md")
    }
}

/// Quanto um capítulo pode ocupar da janela da conversa. O capítulo é servido inteiro, e um
/// arquivo que cresce sem limite não vira resposta longa: vira rodada derrubada pelo provedor,
/// depois de já ter custado. Recusar cedo diz onde consertar.
const MAX_CHAPTER_TOKENS: usize = 6_000;

pub(crate) async fn method_guidance(
    pack: &MethodPack,
    args: &Args,
    today: NaiveDate,
) -> ToolResult {
    let topic = args.choice("topic", TOPICS)?.unwrap_or("metodo");
    let content = fs::read_to_string(pack.root.join("chapters").join(format!("{topic}.md")))
        .await
        .map_err(|_| method_not_installed())?;

    let tokens = super::prompt::estimate_tokens(&content);
    if tokens > MAX_CHAPTER_TOKENS {
        return Err(ToolError::new(
            ErrorCode::NotFound,
            format!(
                "O capítulo \"{topic}\" ocupa cerca de {tokens} tokens, acima do teto de {MAX_CHAPTER_TOKENS}."
            ),
            "Divida ou enxugue o capítulo no pack do método até ele caber no teto.",
        ));
    }

    privacy_scan(pack, &format!("o capítulo \"{topic}\""), &content).await?;

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

/// A varredura de privacidade do conteúdo curado, no instante de servir.
///
/// Vale para tudo o que sai do pack em direção ao provedor — capítulo servido por tópico e núcleo
/// montado no prefixo do prompt. `subject` nomeia o que está sendo varrido para que o erro diga
/// onde olhar ("o capítulo \"diario\"", "o prefixo do método").
///
/// Falha fechado: pack sem deny-list não serve conteúdo nenhum. Deny-list a mais é barata;
/// deny-list ausente cala a camada de método inteira, que é o lado certo de errar.
pub(crate) async fn privacy_scan(
    pack: &MethodPack,
    subject: &str,
    content: &str,
) -> Result<(), ToolError> {
    let unreadable = || {
        privacy_blocked(
            format!("Não foi possível validar a deny-list de {subject}."),
            "Instale o pack com pelo menos uma deny-list forbidden*.txt na raiz.",
        )
    };
    let mut entries = fs::read_dir(&pack.root).await.map_err(|_| unreadable())?;
    let mut deny_lists = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|_| unreadable())? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("forbidden") && name.ends_with(".txt") {
            deny_lists.push((name, entry.path()));
        }
    }

    if deny_lists.is_empty() {
        return Err(privacy_blocked(
            format!("Sem deny-list de privacidade, {subject} não pode ser servido."),
            "Instale o pack com pelo menos uma deny-list forbidden*.txt na raiz.",
        ));
    }

    deny_lists.sort_by(|left, right| left.0.cmp(&right.0));
    let mut patterns = Vec::with_capacity(deny_lists.len());
    for (name, path) in deny_lists {
        let list = fs::read_to_string(path).await.map_err(|_| {
            privacy_blocked(
                format!("A deny-list \"{name}\" não pôde ser lida para varrer {subject}."),
                "Repare a deny-list do pack.",
            )
        })?;
        let list = list.strip_prefix('\u{feff}').unwrap_or(&list).to_string();
        patterns.push((name, list));
    }

    // As listas são validadas ANTES de varrer o conteúdo: descobrir que a proteção não existe
    // depois de o conteúdo ter passado por ela seria descobrir tarde demais.
    let mut lists = Vec::with_capacity(patterns.len());
    for (name, list) in patterns {
        let entries = deny_list_entries(&name, &list)?;
        lists.push((name, entries));
    }

    if lists.iter().all(|(_, entries)| entries.is_empty()) {
        return Err(privacy_blocked(
            format!("Sem nenhum padrão na deny-list, {subject} não pode ser servido."),
            "Preencha a deny-list forbidden*.txt do pack com os termos que não podem sair desta máquina.",
        ));
    }

    let content_lowercase = content.to_lowercase();
    for (name, entries) in lists {
        for (number, pattern) in entries {
            if content_lowercase.contains(&pattern) {
                // O termo bloqueado nunca sai no erro: repeti-lo transformaria o próprio gate em
                // vazamento do conteúdo que ele precisa manter local.
                return Err(privacy_blocked(
                    format!("A deny-list \"{name}\" bloqueou {subject} na entrada #{number}."),
                    "Revise o conteúdo privado do pack.",
                ));
            }
        }
    }

    Ok(())
}

/// As entradas de uma deny-list, numeradas como o erro as reporta e já em caixa baixa para a
/// comparação. Linha vazia e comentário não são entradas; linha só com espaços é malformação, e
/// nunca uma entrada que se ignora em silêncio — um gate que diverge da curadoria serve o que ela
/// barraria.
fn deny_list_entries(name: &str, list: &str) -> Result<Vec<(usize, String)>, ToolError> {
    let mut entries = Vec::new();
    for line in list.lines() {
        if line.is_empty() {
            continue;
        }
        let pattern = line.trim();
        if pattern.starts_with('#') {
            continue;
        }
        let number = entries.len() + 1;
        if pattern.is_empty() {
            return Err(privacy_blocked(
                format!(
                    "A deny-list \"{name}\" está malformada na entrada #{number}: a linha só contém espaços."
                ),
                "Corrija a linha em branco da deny-list do pack.",
            ));
        }
        entries.push((number, pattern.to_lowercase()));
    }
    Ok(entries)
}

fn privacy_blocked(message: impl Into<String>, fix: impl Into<String>) -> ToolError {
    ToolError::new(ErrorCode::PrivacyBlocked, message, fix)
}
