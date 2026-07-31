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
use crate::mia::envelope::Clock;
use crate::mia::key_store::{self, ApiKey};
use crate::mia::method_tools::MethodPack;
use crate::mia::proposal_tools;
use crate::mia::provider::http::HttpAdapter;
use crate::mia::provider::pins::default_pin;
use crate::mia::run::{
    CancelToken, Round, RunErrorCode, RunEvent, RunLimits, Runner, StopReason, run_error,
};
use crate::mia::screen_events::{MiaScreenEvent, screen_event};
use crate::mia::store::{self, StoredMessage};
use crate::mia::{Context, prompt};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Eventos em trânsito entre o laço e o canal da tela. Folga para absorver rajada de um turno
/// sem prender a rodada esperando o webview desenhar.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// A recusa de quem ainda não ligou a conversa. Ela é literalmente verdadeira — sem chave no
/// cofre, nenhuma rodada existe — e traz o caminho de ligar, porque limitação sem saída devolve a
/// pessoa ao mesmo lugar.
const NOT_LINKED: &str = "A conversa ainda não está ligada nesta máquina.";
const NOT_LINKED_FIX: &str =
    "Abra Configurações › Conversa, registre o consentimento e guarde a chave do provedor.";

/// As rodadas em curso, pelo sinal que as interrompe.
///
/// O registro vive fora do laço porque quem cancela é outra chamada, vinda da mesma tela: sem ele,
/// o gesto de parar não teria como alcançar a rodada que já está falando com o provedor.
#[derive(Clone, Default)]
pub struct MiaRuns(Arc<Mutex<HashMap<String, CancelToken>>>);

impl MiaRuns {
    fn guard(&self) -> std::sync::MutexGuard<'_, HashMap<String, CancelToken>> {
        // Um envenenamento aqui viria de um panic com o mapa em mão; seguir com o conteúdo é
        // melhor que derrubar a conversa inteira por causa dele.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn register(&self, run_id: &str, cancel: CancelToken) {
        self.guard().insert(run_id.to_string(), cancel);
    }

    /// Cancela a rodada, se ela ainda estiver de pé. Rodada desconhecida é caso NORMAL: a que já
    /// terminou saiu do registro, e pedir para parar o que já parou não é erro.
    fn cancel(&self, run_id: &str) {
        if let Some(cancel) = self.guard().remove(run_id) {
            cancel.cancel();
        }
    }

    fn finish(&self, run_id: &str) {
        self.guard().remove(run_id);
    }
}

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

/// Abre uma rodada da conversa e devolve o identificador dela.
///
/// O retorno é cedo de propósito: a tela precisa do identificador para poder cancelar antes de a
/// primeira resposta chegar. O que a rodada produz viaja pelo canal, um evento por vez.
///
/// O consentimento NÃO é conferido aqui: quem o exige é o laço, a cada tentativa, e duplicar a
/// régua criaria uma segunda cópia dela para divergir. O que este comando faz é transformar a
/// recusa do laço em evento honesto.
#[tauri::command]
pub async fn run_mia_round(
    question: String,
    on_event: tauri::ipc::Channel<MiaScreenEvent>,
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    runs: State<'_, MiaRuns>,
) -> Result<String, String> {
    if question.trim().is_empty() {
        return Err("Escreva a pergunta antes de enviar.".to_string());
    }

    // A chave é lida antes de qualquer coisa e nunca sai deste escopo: ela entra no adapter, vira
    // cabeçalho e não existe em mais lugar nenhum. Sem ela, a rodada é recusada sem tocar a rede.
    let Some(key) = key_store::load(&app_dir.0)? else {
        return Err(format!("{NOT_LINKED} {NOT_LINKED_FIX}"));
    };
    let adapter = HttpAdapter::new(key.expose().to_string())?;

    // A conversa é reidratada ANTES de a rodada abrir: o histórico é do app, e é ele que faz a
    // pergunta seguinte entender a anterior. A purga do rastro vencido pega carona no mesmo
    // caminho — quem conversa é quem produz rastro.
    let conversation = open_conversation(pool.inner()).await?;
    store::purge_stale_traces(pool.inner(), chrono::Utc::now())
        .await
        .map_err(|error| format!("limpar o rastro técnico vencido: {error}"))?;
    let history = store::load_history(pool.inner(), conversation)
        .await
        .map_err(|error| format!("ler a conversa guardada: {error}"))?;

    // Uma leitura só do relógio por rodada: o hoje do prefixo e o `as_of` dos envelopes saem da
    // mesma, senão o modelo receberia dois calendários e nenhum critério para escolher entre eles.
    let ctx = Context {
        clock: Clock::at(chrono::Local::now().fixed_offset()),
        pack: MethodPack::in_app_data(&app_dir.0),
        conversation_id: Some(conversation),
    };
    let system = prompt::system_prompt(&ctx.pack, ctx.clock.today())
        .await
        .map_err(|error| format!("{} {}", error.message, error.fix))?;

    let run_id = uuid::Uuid::new_v4().to_string();

    // O teto da janela é conferido antes de qualquer gasto: a rodada que não caberia seria recusada
    // pelo provedor depois de cobrada. Nada é resumido — a saída é apagar a conversa, e a recusa
    // diz isso em vez de deixar a pessoa reenviando a mesma pergunta.
    if store::window_exceeded(&history) {
        let refusal = run_error(RunErrorCode::ContextCap);
        let _ = on_event.send(screen_event(&run_id, RunEvent::Error(refusal)));
        let _ = on_event.send(screen_event(
            &run_id,
            RunEvent::RunFinished {
                stop: StopReason::Failed,
            },
        ));
        return Ok(run_id);
    }

    let cancel = CancelToken::new();
    runs.register(&run_id, cancel.clone());

    let (events, mut receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
    let channel_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = receiver.recv().await {
            // Canal fechado é a tela que foi embora: insistir publicaria numa janela que não
            // existe mais. A rodada segue e termina pelos próprios tetos.
            if on_event.send(screen_event(&channel_run_id, event)).is_err() {
                break;
            }
        }
    });

    let pool = pool.inner().clone();
    let runs = runs.inner().clone();
    let round_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = Runner {
            pool: &pool,
            ctx: &ctx,
            adapter: &adapter,
            pin: default_pin(),
            limits: RunLimits::default(),
            cancel,
            events,
        };
        let outcome = runner
            .run(Round {
                system: &system.text,
                history: &history,
                question: &question,
            })
            .await;
        runs.finish(&round_id);

        // A gravação é o último gesto da rodada, e a falha dela não tem tela: a resposta já foi
        // publicada pelo canal. Fica no console, para o próximo diagnóstico — engolir em silêncio
        // esconderia uma conversa que deixou de durar.
        if let Err(error) =
            store::save_round(&pool, conversation, &round_id, default_pin(), &outcome).await
        {
            eprintln!("[mia/store] gravar a rodada: {error}");
        }
    });

    Ok(run_id)
}

/// A conversa única, com a mensagem de erro que toda porta dela usa.
async fn open_conversation(pool: &SqlitePool) -> Result<i64, String> {
    store::active_conversation(pool)
        .await
        .map_err(|error| format!("abrir a conversa guardada: {error}"))
}

/// A conversa guardada, como a tela a desenha ao abrir.
#[tauri::command]
pub async fn load_mia_conversation(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<StoredMessage>, String> {
    let conversation = open_conversation(pool.inner()).await?;
    store::visible_messages(pool.inner(), conversation)
        .await
        .map_err(|error| format!("ler a conversa guardada: {error}"))
}

/// Registra o par pergunta/resposta que a tela acabou de desenhar.
///
/// O backend guarda o JSON como ele vem: quem reduz os eventos da rodada à resposta visível é a
/// interface, e conhecer esse formato aqui seria uma segunda definição dele para divergir.
#[tauri::command]
pub async fn append_mia_exchange(
    question: String,
    answer_json: String,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    let conversation = open_conversation(pool.inner()).await?;
    store::append_exchange(pool.inner(), conversation, &question, &answer_json)
        .await
        .map_err(|error| format!("guardar a mensagem da conversa: {error}"))
}

/// Apaga a conversa de verdade: o que a pessoa leu e o rastro técnico das rodadas somem juntos. A
/// proveniência de um lançamento aprovado permanece — ela é histórico financeiro, não da conversa.
#[tauri::command]
pub async fn delete_mia_conversation(pool: State<'_, SqlitePool>) -> Result<(), String> {
    let conversation = open_conversation(pool.inner()).await?;
    store::delete_conversation(pool.inner(), conversation)
        .await
        .map_err(|error| format!("apagar a conversa: {error}"))
}

/// Cria o lançamento de uma proposta. O gesto vive AQUI, fora do laço: nenhum caminho da conversa
/// alcança este comando, e é isso que garante que texto no chat não aprove nada.
///
/// `payload_json` é o lançamento como o cartão o mostra e `hash` é a assinatura que a proposta
/// trouxe. Os dois viajam juntos porque a conferência é entre eles: um campo alterado no caminho
/// deixa de corresponder à assinatura e a aprovação é recusada.
#[tauri::command]
pub async fn approve_mia_proposal(
    proposal_id: i64,
    payload_json: String,
    hash: String,
    pool: State<'_, SqlitePool>,
) -> Result<String, String> {
    proposal_tools::approve(
        pool.inner(),
        proposal_id,
        &payload_json,
        &hash,
        chrono::Local::now().fixed_offset(),
    )
    .await
}

/// Registra que a proposta foi recusada. Nada é criado, e a linha permanece no ledger.
#[tauri::command]
pub async fn reject_mia_proposal(
    proposal_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    proposal_tools::reject(pool.inner(), proposal_id).await
}

/// Interrompe uma rodada em curso. Cancelar o que já terminou é gesto sem efeito, nunca erro: a
/// tela pede a parada sem ter como saber se a última resposta chegou primeiro.
#[tauri::command]
pub async fn cancel_mia_round(run_id: String, runs: State<'_, MiaRuns>) -> Result<(), String> {
    runs.cancel(&run_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O sinal chega à rodada registrada, e o registro sai do caminho quando ela termina.
    #[test]
    fn o_cancelamento_alcanca_a_rodada_registrada() {
        let runs = MiaRuns::default();
        let cancel = CancelToken::new();
        runs.register("run-1", cancel.clone());

        runs.cancel("run-1");

        assert!(cancel.is_cancelled());
    }

    /// Rodada desconhecida — já terminada, ou nunca aberta — é caso normal.
    #[test]
    fn cancelar_rodada_inexistente_nao_faz_nada() {
        let runs = MiaRuns::default();

        runs.cancel("run-fantasma");

        assert!(runs.guard().is_empty());
    }

    /// Terminada a rodada, o sinal dela não fica de pé para cancelar a próxima que reusar o nome.
    #[test]
    fn a_rodada_terminada_sai_do_registro() {
        let runs = MiaRuns::default();
        let cancel = CancelToken::new();
        runs.register("run-1", cancel.clone());

        runs.finish("run-1");
        runs.cancel("run-1");

        assert!(!cancel.is_cancelled());
        assert!(runs.guard().is_empty());
    }

    /// A recusa de quem não ligou a conversa diz o que fazer — e nunca cita chave nem cofre.
    #[test]
    fn a_recusa_de_conversa_desligada_traz_o_caminho_de_ligar() {
        let recusa = format!("{NOT_LINKED} {NOT_LINKED_FIX}");

        assert!(recusa.contains("não está ligada"));
        assert!(recusa.contains("Configurações › Conversa"));
    }
}
