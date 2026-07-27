//! O consentimento como garantia, não como tela.
//!
//! O registro vive no armazenamento de configurações do app, e não no do webview, porque quem
//! precisa lê-lo é o laço: sem ele a rodada é recusada no backend, independentemente do que a
//! interface mostre ou esconda.
//!
//! O registro carrega a IMPRESSÃO do texto aceito, e não um número de versão que alguém precisa
//! lembrar de subir. Um texto novo — outro processador, outro alcance de leitura — caduca o
//! consentimento sozinho, porque o que a pessoa aceitou foi aquele texto, e herdar o "sim" para um
//! texto que ela não leu seria consentimento presumido.
//!
//! Os processadores nomeados saem do pin em vigor: "a nuvem" sem nome não é informação.

use super::provider::pins::ModelPin;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

const CONSENT_KEY: &str = "mia_consent";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConsentState {
    Missing,
    Stale,
    Granted { granted_at: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsentRefusal {
    Missing,
    Stale,
    Unavailable,
}

#[derive(Deserialize, Serialize)]
struct ConsentRecord {
    /// A impressão do texto aceito. É ela, e não um número que alguém precisa lembrar de subir,
    /// que decide se o consentimento ainda vale: mudar uma frase, um processador ou o pin muda a
    /// impressão e caduca o registro sozinho.
    fingerprint: String,
    granted_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ConsentText {
    pub headline: String,
    pub processors: Vec<Processor>,
    pub paragraphs: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Processor {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ChecklistItem {
    pub title: String,
    pub detail: String,
}

/// A impressão do texto: o que a pessoa leu, reduzido a uma comparação barata.
pub(crate) fn fingerprint(text: &ConsentText) -> String {
    let serialized = serde_json::to_string(text).expect("o texto do consentimento é serializável");
    let digest = <Sha256 as Digest>::digest(serialized.as_bytes());
    // Metade do digest basta: o que se defende aqui é troca de texto por descuido, não colisão
    // procurada por alguém que já teria caminhos melhores.
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) async fn status(pool: &SqlitePool, pin: &ModelPin) -> Result<ConsentState, sqlx::Error> {
    let stored: Option<(String,)> = sqlx::query_as("SELECT value FROM app_setting WHERE key = ?1")
        .bind(CONSENT_KEY)
        .fetch_optional(pool)
        .await?;
    let Some((stored,)) = stored else {
        return Ok(ConsentState::Missing);
    };
    let record: ConsentRecord = serde_json::from_str(&stored)
        .map_err(|error| sqlx::Error::Protocol(format!("consentimento inválido: {error}")))?;
    chrono::DateTime::parse_from_rfc3339(&record.granted_at).map_err(|error| {
        sqlx::Error::Protocol(format!("data do consentimento inválida: {error}"))
    })?;

    if record.fingerprint == fingerprint(&consent_text(pin)) {
        Ok(ConsentState::Granted {
            granted_at: record.granted_at,
        })
    } else {
        Ok(ConsentState::Stale)
    }
}

pub(crate) async fn grant(
    pool: &SqlitePool,
    pin: &ModelPin,
    now_rfc3339: &str,
) -> Result<ConsentState, sqlx::Error> {
    let record = ConsentRecord {
        fingerprint: fingerprint(&consent_text(pin)),
        granted_at: now_rfc3339.to_string(),
    };
    let value = serde_json::to_string(&record).expect("o registro de consentimento é serializável");
    sqlx::query("INSERT OR REPLACE INTO app_setting (key, value) VALUES (?1, ?2)")
        .bind(CONSENT_KEY)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(ConsentState::Granted {
        granted_at: record.granted_at,
    })
}

pub(crate) async fn revoke(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_setting WHERE key = ?1")
        .bind(CONSENT_KEY)
        .execute(pool)
        .await?;
    Ok(())
}

/// Autoriza contra o pin que a rodada VAI usar: consentir para um processador não consente para
/// outro, e é o destino real dos dados que o "sim" precisa cobrir.
pub(crate) async fn authorize(pool: &SqlitePool, pin: &ModelPin) -> Result<(), ConsentRefusal> {
    match status(pool, pin).await {
        Ok(ConsentState::Granted { .. }) => Ok(()),
        Ok(ConsentState::Missing) => Err(ConsentRefusal::Missing),
        Ok(ConsentState::Stale) => Err(ConsentRefusal::Stale),
        Err(_) => Err(ConsentRefusal::Unavailable),
    }
}

pub(crate) fn consent_text(pin: &ModelPin) -> ConsentText {
    let operator = pin.operator;
    ConsentText {
        headline: "Autorizar a conversa aberta".to_string(),
        processors: vec![
            Processor {
                name: "OpenRouter".to_string(),
                role: "Roteia o pedido e aplica a retenção zero contratada.".to_string(),
            },
            Processor {
                name: operator.to_string(),
                role: "Executa o modelo no endpoint pinado.".to_string(),
            },
        ],
        paragraphs: vec![
            format!(
                "Para responder qualquer pergunta, a Mia envia a sua pergunta e os dados necessários para respondê-la a dois processadores: OpenRouter, que roteia o pedido, e {operator}, que executa o modelo."
            ),
            "A conversa lê tudo o que o app mostra: lançamentos completos, com descrições e notas, valores, contas, pessoas e tags. Não existe recorte — o que estiver no seu histórico pode ser enviado para responder a uma pergunta.".to_string(),
            "Os pedidos saem com retenção zero e coleta de dados negada, para o endpoint pinado, sem alternativa automática. O app fala só com openrouter.ai e nunca segue redirecionamento.".to_string(),
            "A sua chave fica no cofre do sistema — nunca em log, evento, banco ou tela.".to_string(),
            "Você pode revogar quando quiser. Revogar apaga o consentimento e a chave, e a conversa volta a responder só o que ela calcula aqui dentro.".to_string(),
        ],
        checklist: vec![
            ChecklistItem {
                title: "Desligue o treino com o que você envia".to_string(),
                detail: "Na sua conta do provedor, em Privacidade, recuse provedores que treinam com as suas entradas. A requisição do app não controla essa escolha.".to_string(),
            },
            ChecklistItem {
                title: "Desligue a publicação de prompts em endpoints gratuitos".to_string(),
                detail: "Ainda em Privacidade, recuse o desconto que publica os seus prompts. Essa escolha também vive só na sua conta.".to_string(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mia::provider::pins::{PINS, default_pin};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("o pool SQLite em memória deve abrir");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("as migrações devem preparar o pool de teste");
        pool
    }

    #[tokio::test]
    async fn consentimento_concedido_e_revogado_muda_o_estado_duravel() {
        let pool = pool().await;
        let granted_at = "2026-07-26T12:00:00Z";

        assert_eq!(
            status(&pool, default_pin())
                .await
                .expect("a leitura deve funcionar"),
            ConsentState::Missing
        );
        assert!(matches!(
            grant(&pool, default_pin(), granted_at).await.expect("a gravação deve funcionar"),
            ConsentState::Granted { granted_at: ref stored } if stored == granted_at
        ));
        revoke(&pool).await.expect("a revogação deve funcionar");
        assert_eq!(
            status(&pool, default_pin())
                .await
                .expect("a leitura deve funcionar"),
            ConsentState::Missing
        );
    }

    #[tokio::test]
    async fn consentimento_dado_a_outro_texto_recusa_a_autorizacao() {
        let pool = pool().await;
        sqlx::query("INSERT INTO app_setting (key, value) VALUES (?1, ?2)")
            .bind(CONSENT_KEY)
            .bind(r#"{"fingerprint":"0123456789abcdef0123456789abcdef","granted_at":"2026-07-26T12:00:00Z"}"#)
            .execute(&pool)
            .await
            .expect("o fixture deve gravar");

        assert_eq!(
            status(&pool, default_pin())
                .await
                .expect("a leitura deve funcionar"),
            ConsentState::Stale
        );
        assert_eq!(
            authorize(&pool, default_pin()).await,
            Err(ConsentRefusal::Stale)
        );
    }

    /// O que a pessoa aceitou foi um destino, não uma caixa: consentir com o pin em vigor não pode
    /// valer para um pin que nomeia outro operador.
    #[tokio::test]
    async fn trocar_o_operador_do_pin_caduca_o_consentimento() {
        let pool = pool().await;
        let outro = PINS
            .iter()
            .find(|pin| pin.operator != default_pin().operator)
            .expect("a matriz declara mais de um operador");

        grant(&pool, default_pin(), "2026-07-26T12:00:00Z")
            .await
            .expect("a gravação deve funcionar");

        assert_eq!(authorize(&pool, default_pin()).await, Ok(()));
        assert_eq!(authorize(&pool, outro).await, Err(ConsentRefusal::Stale));
    }

    #[test]
    fn a_impressao_muda_quando_o_texto_muda() {
        let text = consent_text(default_pin());
        let mut alterado = text.clone();
        alterado.paragraphs[0] = "Outra promessa.".to_string();

        assert_eq!(
            fingerprint(&text),
            fingerprint(&consent_text(default_pin()))
        );
        assert_ne!(fingerprint(&text), fingerprint(&alterado));
    }

    #[test]
    fn texto_nomeia_processadores_e_explica_o_alcance_da_conversa() {
        let text = consent_text(default_pin());

        assert_eq!(text.processors.len(), 2);
        assert_eq!(text.processors[0].name, "OpenRouter");
        assert_eq!(text.processors[1].name, default_pin().operator);
        assert!(text.paragraphs.iter().any(|paragraph| {
            paragraph.contains("lançamentos completos, com descrições e notas")
        }));
        assert_eq!(text.checklist.len(), 2);
    }
}
