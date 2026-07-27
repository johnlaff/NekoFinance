//! A porta do consentimento da conversa.
//!
//! O que a interface pode fazer aqui é ler o texto, registrar, guardar a chave e revogar — e é só.
//! Quem decide se uma rodada acontece é o laço, que lê o mesmo registro durável: uma tela que
//! escondesse o gesto não mudaria nada, e uma tela adulterada tampouco liberaria a rodada.
//!
//! A chave só trafega de ida. Nenhum retorno daqui a carrega, porque um valor que chega à
//! interface passa a existir em memória de webview, em log de erro e em qualquer captura de tela.

use super::*;
use crate::mia::consent::{self, ConsentState, ConsentText};
use crate::mia::key_store::{self, ApiKey};
use crate::mia::provider::pins::default_pin;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct MiaConsentView {
    granted: bool,
    needs_renewal: bool,
    granted_at: Option<String>,
    has_key: bool,
    linked: bool,
    text: ConsentText,
}

async fn consent_view(
    pool: &SqlitePool,
    app_dir: &std::path::Path,
) -> Result<MiaConsentView, String> {
    // O pin em vigor é a fonte do texto E do veredito: a tela mostra o consentimento do mesmo
    // destino que a rodada usaria, nunca o de outro.
    let pin = default_pin();
    let state = consent::status(pool, pin)
        .await
        .map_err(|error| format!("ler consentimento da conversa: {error}"))?;
    let (granted, needs_renewal, granted_at) = match state {
        ConsentState::Granted { granted_at } => (true, false, Some(granted_at)),
        ConsentState::Stale => (false, true, None),
        ConsentState::Missing => (false, false, None),
    };
    let has_key = key_store::has_key(app_dir);

    Ok(MiaConsentView {
        granted,
        needs_renewal,
        granted_at,
        has_key,
        linked: granted && has_key,
        text: consent::consent_text(pin),
    })
}

#[tauri::command]
pub async fn get_mia_consent(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
) -> Result<MiaConsentView, String> {
    consent_view(pool.inner(), &app_dir.0).await
}

#[tauri::command]
pub async fn grant_mia_consent(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
) -> Result<MiaConsentView, String> {
    consent::grant(
        pool.inner(),
        default_pin(),
        &chrono::Utc::now().to_rfc3339(),
    )
    .await
    .map_err(|error| format!("gravar consentimento da conversa: {error}"))?;
    consent_view(pool.inner(), &app_dir.0).await
}

/// Revogar apaga o registro ANTES da chave: é o registro que abre a porta, e fechá-la primeiro
/// garante que uma falha ao apagar a credencial não deixe rodadas autorizadas. A falha ao apagar a
/// chave é reportada — nunca engolida —, para que "revoguei" não signifique uma chave que ficou.
#[tauri::command]
pub async fn revoke_mia_consent(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
) -> Result<MiaConsentView, String> {
    consent::revoke(pool.inner())
        .await
        .map_err(|error| format!("revogar consentimento da conversa: {error}"))?;
    key_store::delete(&app_dir.0).map_err(|error| {
        format!("O consentimento foi retirado e a conversa já está recusada, mas a chave pode ter permanecido no cofre do sistema: {error}")
    })?;
    consent_view(pool.inner(), &app_dir.0).await
}

#[tauri::command]
pub async fn set_mia_api_key(
    key: String,
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
) -> Result<MiaConsentView, String> {
    if key.trim().is_empty() {
        return Err("Informe a chave do provedor para ligar a conversa.".to_string());
    }
    key_store::store(&app_dir.0, &ApiKey::new(key))?;
    consent_view(pool.inner(), &app_dir.0).await
}
