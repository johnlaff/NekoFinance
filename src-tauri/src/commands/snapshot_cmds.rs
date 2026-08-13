use super::*;
use crate::snapshot::{
    checkout, conflict, lease, manifest::SnapshotManifest, restore, state,
    transport::DriveSnapshotClient,
};
use sha2::{Digest, Sha256};
use std::path::Path;

/// As duas frases da recusa do check-in começam pelo mesmo prefixo ("Check-in recusado: ") de
/// PROPÓSITO: o frontend reconhece a recusa por esse prefixo ESTRUTURAL
/// (`CHECKIN_REFUSED_PREFIX` em `src/screens/configView.ts`), nunca por regex sobre as palavras
/// da frase descritiva que segue — mudar a explicação depois do prefixo não quebra o
/// reconhecimento em produção. Mudar o PREFIXO em si é mudança de contrato: atualize os dois
/// lados juntos, no mesmo commit (o teste `checkin_refusal_messages_share_the_stable_contract_prefix`
/// trava essa invariante deste lado).
///
/// Veredito `Pull`: outro aparelho publicou depois do nosso último check-in. O check-out roda
/// sozinho na PRÓXIMA abertura do app (`snapshot::checkout`) — a copy pede esse gesto em vez de
/// prometer um botão de "baixar agora" que esta tela não tem.
pub const CHECKIN_REFUSED_PULL: &str = "Check-in recusado: outro aparelho publicou depois do seu último check-in — feche e abra \
     o app de novo para receber a versão dele antes de publicar.";

/// Veredito `Conflict`: os dois lados avançaram a partir da mesma base. Nunca dizer "baixe" —
/// aqui isso significaria descartar o trabalho local sem aviso.
pub const CHECKIN_REFUSED_CONFLICT: &str = "Check-in recusado: os dois lados mudaram desde o último ponto em comum entre os \
     aparelhos.";

/// Consentimento obsoleto (ADR-0015): `resolve_conflict_keep_local_core`/`resolve_conflict_use_remote_core`
/// rebuscam o manifest remoto antes de agir, mas a tela de conflito só sabe do manifest que
/// mostrou ao dono (`DriveConflictDetails.remote_manifest`, capturado no fetch anterior). Se o
/// remoto avançou DE NOVO entre a tela abrir e o clique, publicar/restaurar por cima do que o
/// dono nunca viu seria a mesma sobrescrita silenciosa que o lease impede no check-in normal —
/// mesmo prefixo de contrato ("Check-in recusado: "), a tela recarrega os detalhes em vez de
/// mostrar um erro parado (nunca oferece "tentar de novo" sobre um manifest que já mudou outra
/// vez).
pub const CHECKIN_REFUSED_STALE_CONFLICT: &str = "Check-in recusado: a disputa mudou de novo desde que você abriu esta tela — veja \
     os detalhes atualizados antes de escolher.";

/// Prefixo estável da recusa de restauração por schema mais novo — a versão numérica varia a
/// cada par de aparelhos, então só o PREFIXO entra no contrato (espelha `RESTORE_REFUSED_PREFIX`
/// em `src/features/snapshot-conflict/snapshotConflictView.ts`).
const RESTORE_REFUSED_PREFIX: &str = "Restauração recusada: ";

/// Sufixo compartilhado por TODO erro que `resolve_conflict_use_remote_core` devolve depois de
/// fechar o pool do banco ativo para trocar o arquivo (o "ponto de não-retorno" comentado lá
/// embaixo) — a partir daí não sobra pool para uma nova tentativa nesta sessão, então o frontend
/// reconhece este sufixo (`AFTER_POOL_CLOSED_SUFFIX`, `snapshotConflictView.ts`) e nunca reoferece
/// os botões de escolha, só a saída de reiniciar o app.
const AFTER_POOL_CLOSED_SUFFIX: &str = "; reinicie o app para continuar";

/// O que a UI de Conexão mostra sobre o último check-in E o último check-out — quando, e por/de
/// qual aparelho, em cada eixo (os dois avançam de forma independente: um check-out sem check-in
/// depois é normal, e vice-versa).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriveCheckinInfo {
    pub last_checkin_at: Option<String>,
    pub last_checkin_device_id: Option<String>,
    /// Quando este aparelho puxou por último o snapshot remoto (check-out ao abrir).
    pub last_checkout_at: Option<String>,
    /// `device_id` de quem publicou o snapshot que este aparelho baixou por último — de qual
    /// aparelho veio, nunca a identidade deste.
    pub last_checkout_device_id: Option<String>,
    /// Rótulo fechado do desfecho do ÚLTIMO check-out que mereceu aviso (ADR-0015):
    /// `"refused_newer_schema"` (o snapshot remoto tem schema mais nova — orientar a
    /// atualizar o app) ou `"error"` (rede/integridade — a leitura não aconteceu, tenta na
    /// próxima abertura). `None` quando o check-out mais recente não tem nada a avisar.
    pub last_checkout_outcome: Option<String>,
    /// Complemento do desfecho acima (versões de schema na recusa, mensagem de erro na falha).
    pub last_checkout_outcome_detail: Option<String>,
    /// Há mudanças locais que ainda não foram publicadas (ADR-0015) — calculado a
    /// cada tentativa de check-in (automática ou manual), honesto mesmo quando a tentativa mais
    /// recente falhou ou foi recusada.
    pub pending_local_changes: bool,
    /// Há uma disputa `Conflict` pendente de resolução — enquanto `true`, os gatilhos automáticos
    /// (foco, gesto material, fechar) não tentam nada; só a escolha do dono na tela de conflito
    /// limpa este estado.
    pub conflict_pending: bool,
    pub this_device_id: String,
}

/// Resultado do gesto de check-in. "Em dia" (nada mudou desde a última publicação) é SUCESSO —
/// o mesmo veredito do ADR-0015 —, mas a tela precisa distinguir dos dois para não anunciar uma
/// publicação que não aconteceu: `published` diz se este clique de fato subiu algo novo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriveCheckinResult {
    #[serde(flatten)]
    pub info: DriveCheckinInfo,
    pub published: bool,
}

/// Exporta o banco ativo pelo mesmo caminho atômico do backup (`VACUUM INTO`), retira a
/// identidade/sequência local da cópia (`state::strip_from_export_copy` — nunca viaja no
/// snapshot compartilhado) e hasheia o resultado. Compartilhado pelo check-in normal
/// (`drive_checkin_core`) e pela resolução de conflito mantendo este aparelho
/// (`resolve_conflict_keep_local_core`) — os dois publicam um export candidato da MESMA forma, só
/// divergem em como decidem a sequência e se checam o árbitro antes de subir.
async fn export_candidate_snapshot(
    pool: &SqlitePool,
    app_dir: &Path,
) -> Result<(String, Vec<u8>), String> {
    let tmp_path = app_dir.join(format!("neko-checkin-{}.db", uuid::Uuid::new_v4()));
    db_export::vacuum_into_atomic(pool, &tmp_path)
        .await
        .map_err(|e| format!("exportar snapshot: {e}"))?;
    // A identidade/sequência DESTE aparelho nunca viaja no snapshot compartilhado — ver
    // `state::strip_from_export_copy`.
    if let Err(e) = state::strip_from_export_copy(&tmp_path).await {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("preparar snapshot para publicação: {e}"));
    }
    let db_bytes = std::fs::read(&tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    let db_bytes = db_bytes.map_err(|e| format!("ler snapshot exportado: {e}"))?;
    let export_hash = hex::encode(Sha256::digest(&db_bytes));
    Ok((export_hash, db_bytes))
}

/// Núcleo testável de `last_drive_checkin` — mesmo split de `drive_checkin`/`drive_checkin_core`:
/// o comando Tauri é um wrapper fino sobre `State<'_, SqlitePool>`, este recebe `&SqlitePool` puro
/// para os testes atravessarem a costura backend↔tela sem precisar de um runtime Tauri.
pub(crate) async fn last_drive_checkin_core(pool: &SqlitePool) -> Result<DriveCheckinInfo, String> {
    let st = state::load_or_init(pool).await?;
    Ok(DriveCheckinInfo {
        last_checkin_at: st.last_checkin_at,
        last_checkin_device_id: st.last_checkin_device_id,
        last_checkout_at: st.last_checkout_at,
        last_checkout_device_id: st.last_checkout_device_id,
        last_checkout_outcome: st.last_checkout_outcome,
        last_checkout_outcome_detail: st.last_checkout_outcome_detail,
        pending_local_changes: st.pending_local_changes,
        conflict_pending: st.conflict_pending_since.is_some(),
        this_device_id: st.device_id,
    })
}

#[tauri::command]
pub async fn last_drive_checkin(pool: State<'_, SqlitePool>) -> Result<DriveCheckinInfo, String> {
    last_drive_checkin_core(pool.inner()).await
}

/// O gesto de check-in: exporta um snapshot íntegro (`db_export::vacuum_into_atomic`, o mesmo
/// caminho do backup) e o publica no `appDataFolder`, com o manifest de sequência ao
/// lado. Recusa publicar quando o remoto avançou desde a última base local — force-with-lease,
/// nunca sobrescreve o outro aparelho em silêncio.
pub(crate) async fn drive_checkin_core(
    pool: &SqlitePool,
    app_dir: &std::path::Path,
    drive: &DriveSnapshotClient,
) -> Result<DriveCheckinResult, String> {
    let local_state = state::load_or_init(pool).await?;
    let remote = drive.fetch_manifest().await?;

    // Exporta e hasheia ANTES de decidir: sem hooks em todo gesto que muda o banco (fora do
    // escopo deste corte), comparar o hash do export atual contra o último publicado é o jeito
    // honesto de saber se há algo de fato novo — nunca assumir que houve mudança só porque o
    // dono clicou em "Fazer check-in" de novo.
    let (export_hash, db_bytes) = export_candidate_snapshot(pool, app_dir).await?;

    let content_changed = local_state.last_export_sha256.as_deref() != Some(export_hash.as_str());
    // Persistido a cada tentativa (ADR-0015), sucesso ou falha: a UI de Conexão
    // precisa mostrar "não publicado" mesmo quando a tentativa abaixo falhar/for recusada — só
    // uma publicação de fato (`state::record_checkin`, abaixo) limpa isto de novo.
    state::record_pending_local_changes(pool, content_changed).await?;
    // Cada publicação reivindica a PRÓXIMA sequência a partir da base local — só quando o
    // conteúdo de fato mudou; senão o candidato fica na própria base e o árbitro lê "em dia".
    let candidate_sequence = if content_changed {
        local_state.base_sequence + 1
    } else {
        local_state.base_sequence
    };

    // `decide` recusa subir quando o remoto já avançou além da base — a mesma semântica do
    // `git push --force-with-lease`.
    match lease::decide(
        candidate_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    ) {
        lease::LeaseVerdict::Push => {}
        lease::LeaseVerdict::UpToDate => {
            // Sucesso, não erro (ADR-0015): nada de novo para publicar. O estado local não
            // muda — devolve exatamente o que já estava registrado. `UpToDate` só é alcançável
            // com `content_changed = false` (ver a derivação de `candidate_sequence` acima), e
            // nunca é o veredito `Conflict` — qualquer disputa registrada antes já não se sustenta.
            state::record_conflict_pending(pool, None).await?;
            return Ok(DriveCheckinResult {
                info: DriveCheckinInfo {
                    last_checkin_at: local_state.last_checkin_at,
                    last_checkin_device_id: local_state.last_checkin_device_id,
                    last_checkout_at: local_state.last_checkout_at,
                    last_checkout_device_id: local_state.last_checkout_device_id,
                    last_checkout_outcome: local_state.last_checkout_outcome,
                    last_checkout_outcome_detail: local_state.last_checkout_outcome_detail,
                    pending_local_changes: false,
                    conflict_pending: false,
                    this_device_id: local_state.device_id,
                },
                published: false,
            });
        }
        // Pull e Conflict têm copy PRÓPRIA: Pull pede reabrir o app (o check-out roda sozinho na
        // próxima abertura, ver `snapshot::checkout`) e Conflict não sugere um gesto que
        // descartaria trabalho local sem aviso.
        lease::LeaseVerdict::Pull => return Err(CHECKIN_REFUSED_PULL.into()),
        lease::LeaseVerdict::Conflict => {
            // Gate dos gatilhos automáticos (ADR-0015): persistido para que
            // foco/gesto-material/fechar parem de tentar até o dono resolver na tela de conflito.
            let now = chrono::Utc::now().to_rfc3339();
            state::record_conflict_pending(pool, Some(&now)).await?;
            return Err(CHECKIN_REFUSED_CONFLICT.into());
        }
    }

    let schema_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("ler versão do schema: {e}"))?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let manifest = SnapshotManifest {
        device_id: local_state.device_id.clone(),
        sequence: candidate_sequence,
        created_at: created_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
    };

    drive.upload_snapshot(&db_bytes, &manifest).await?;

    // Só avança o estado local DEPOIS do upload confirmado — uma falha de rede no meio deixa a
    // base local intocada, então o próximo check-in tenta a MESMA sequência de novo.
    state::record_checkin(
        pool,
        candidate_sequence,
        &created_at,
        &local_state.device_id,
        &export_hash,
    )
    .await?;

    Ok(DriveCheckinResult {
        info: DriveCheckinInfo {
            last_checkin_at: Some(created_at),
            last_checkin_device_id: Some(local_state.device_id.clone()),
            last_checkout_at: local_state.last_checkout_at,
            last_checkout_device_id: local_state.last_checkout_device_id,
            last_checkout_outcome: local_state.last_checkout_outcome,
            last_checkout_outcome_detail: local_state.last_checkout_outcome_detail,
            // `state::record_checkin` (acima) já limpou os dois no banco — refletido aqui sem
            // reler, mesmo padrão dos outros campos desta struct.
            pending_local_changes: false,
            conflict_pending: false,
            this_device_id: local_state.device_id,
        },
        published: true,
    })
}

// Lista de parâmetros plana por design (cada um vem de state/request); `guard` (SyncGuard) é
// estado gerenciado — mesmo padrão de `import_sheet_data`.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn drive_checkin(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<DriveCheckinResult, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_drive_scope(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let drive = DriveSnapshotClient::new(token, crate::snapshot::transport::production_base_url());

    // Serializa contra import/sync de fundo no pool de 1 conexão — mesmo SyncGuard do import.
    let _lock = guard.inner().lock().await;
    drive_checkin_core(pool.inner(), &app_dir.0, &drive).await
}

/// Recusa compartilhada por `drive_conflict_details_core` e `resolve_conflict_use_remote_core`
/// quando não há manifest remoto nenhum para disputar — as duas chamadas só fazem sentido depois
/// de `drive_checkin` já ter visto um remoto avançado, então chegar aqui sem um é sempre a mesma
/// história (o remoto sumiu entre as duas chamadas), nunca duas frases por coincidência.
const NO_CONFLICT_NO_REMOTE_MANIFEST: &str =
    "Nenhum conflito pendente: nenhum snapshot foi publicado ainda.";

/// Detalhes do conflito para a tela de resolução (ADR-0015): o manifest remoto que disputa a
/// posse, e os gestos de CADA lado desde a última base em comum — a escolha do dono é simétrica
/// (manter este aparelho ou usar o outro), então o que se perderia em CADA direção precisa estar
/// visível antes de escolher ("nenhuma sobrescrita é silenciosa", ADR-0015). Só faz sentido chamar
/// depois de `drive_checkin` recusar com `CHECKIN_REFUSED_CONFLICT`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriveConflictDetails {
    pub remote_manifest: SnapshotManifest,
    pub local_gestures: Vec<conflict::ConflictGesture>,
    pub remote_gestures: Vec<conflict::ConflictGesture>,
}

/// Núcleo testável de `drive_conflict_details`. Não repete o export/hash caro do check-in — só
/// confirma que o remoto de fato avançou além da nossa base (o mesmo sinal que levou `drive_checkin`
/// a recusar) antes de listar os gestos; um remoto que não avançou não tem conflito nenhum para
/// explicar (defensivo: a tela só chama isto depois de ver a recusa, mas o remoto pode ter mudado
/// de novo entre as duas chamadas). Os gestos do OUTRO aparelho exigem baixar o snapshot dele para
/// um arquivo temporário e ler o `sync_log` de lá em modo só-leitura — nunca migrado, nunca trocado
/// pelo banco ativo (essa troca só acontece se o dono escolher `use_remote`, em
/// `resolve_conflict_use_remote_core`); o temporário é removido antes de devolver, sucesso ou erro.
pub(crate) async fn drive_conflict_details_core(
    pool: &SqlitePool,
    app_dir: &Path,
    drive: &DriveSnapshotClient,
) -> Result<DriveConflictDetails, String> {
    let local_state = state::load_or_init(pool).await?;
    let remote_manifest = drive
        .fetch_manifest()
        .await?
        .ok_or_else(|| NO_CONFLICT_NO_REMOTE_MANIFEST.to_string())?;
    if remote_manifest.sequence <= local_state.base_sequence {
        return Err(
            "Nenhum conflito pendente: o remoto não avançou além da última base local.".into(),
        );
    }

    let since = conflict::base_anchor(
        local_state.last_checkin_at.as_deref(),
        local_state.last_checkout_at.as_deref(),
    );
    let local_gestures = conflict::gestures_since(pool, since.as_deref()).await?;

    let remote_bytes = drive.download_snapshot().await?.ok_or_else(|| {
        "Nenhum conflito pendente: o snapshot do outro aparelho sumiu do Drive.".to_string()
    })?;
    let tmp_path = app_dir.join(format!("neko-conflict-peek-{}.db", uuid::Uuid::new_v4()));
    let remote_gestures = async {
        restore::stage_downloaded_snapshot(&tmp_path, &remote_bytes).await?;
        conflict::gestures_since_in_file(&tmp_path, since.as_deref()).await
    }
    .await;
    let _ = std::fs::remove_file(&tmp_path);
    let remote_gestures = remote_gestures?;

    Ok(DriveConflictDetails {
        remote_manifest,
        local_gestures,
        remote_gestures,
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn drive_conflict_details(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<DriveConflictDetails, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_drive_scope(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let drive = DriveSnapshotClient::new(token, crate::snapshot::transport::production_base_url());

    let _lock = guard.inner().lock().await;
    drive_conflict_details_core(pool.inner(), &app_dir.0, &drive).await
}

/// Desfecho de resolver um conflito: a sequência que passou a ser a base, e se este aparelho
/// precisa reiniciar para o resultado valer — só `use_remote` troca o arquivo ativo debaixo do
/// pool já em uso (ver a nota em `resolve_conflict_use_remote_core`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConflictResolutionOutcome {
    pub choice: String,
    pub requires_restart: bool,
    pub sequence: i64,
}

/// Mantém ESTE aparelho: publica o conteúdo local por cima do remoto com uma sequência que supera
/// os dois lados — nunca só `base + 1`, que o árbitro recusaria de novo com `Conflict`, o mesmo
/// veredito que trouxe o dono a esta tela. Rebusca o manifest para descobrir a sequência ATUAL,
/// mas só publica se ela bater com `seen_remote_sequence` (o manifest que a TELA mostrou ao
/// dono) — um avanço novo desde então é consentimento obsoleto (ADR-0015): publicar por cima do
/// que o dono nunca viu seria a mesma sobrescrita silenciosa que o lease impede no check-in
/// normal, só que um clique tarde demais. O pool continua o MESMO depois em qualquer caso — nada
/// no arquivo ativo muda, só o que fica publicado no Drive.
pub(crate) async fn resolve_conflict_keep_local_core(
    pool: &SqlitePool,
    app_dir: &Path,
    drive: &DriveSnapshotClient,
    seen_remote_sequence: i64,
) -> Result<ConflictResolutionOutcome, String> {
    let local_state = state::load_or_init(pool).await?;
    let remote = drive.fetch_manifest().await?;
    let remote_sequence = remote.as_ref().map(|m| m.sequence).unwrap_or(0);
    if remote_sequence != seen_remote_sequence {
        return Err(CHECKIN_REFUSED_STALE_CONFLICT.into());
    }

    let (export_hash, db_bytes) = export_candidate_snapshot(pool, app_dir).await?;
    let resolved_sequence = (local_state.base_sequence + 1).max(remote_sequence + 1);

    let schema_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("ler versão do schema: {e}"))?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let manifest = SnapshotManifest {
        device_id: local_state.device_id.clone(),
        sequence: resolved_sequence,
        created_at: created_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
    };

    drive.upload_snapshot(&db_bytes, &manifest).await?;
    // Só avança o estado local DEPOIS do upload confirmado — mesmo cuidado do check-in normal.
    state::record_checkin(
        pool,
        resolved_sequence,
        &created_at,
        &local_state.device_id,
        &export_hash,
    )
    .await?;

    Ok(ConflictResolutionOutcome {
        choice: "keep_local".to_string(),
        requires_restart: false,
        sequence: resolved_sequence,
    })
}

/// Usa o OUTRO aparelho: baixa, valida e troca o banco ativo pelo remoto — o mesmo caminho do
/// check-out (`checkout::checkout_on_open`), mas entrado explicitamente pela escolha do dono em
/// vez do veredito `Pull` do árbitro (aqui o veredito que trouxe o dono a esta tela era
/// `Conflict`). Consome `pool` por VALOR porque precisa FECHÁ-LO antes da troca de arquivo — a
/// mesma pré-condição de `restore::swap_active_db_atomically` — e não há como devolver um pool
/// novo ao estado gerenciado do Tauri no meio de uma sessão (só `app.manage()` dentro do
/// `setup()` faz isso). Por isso o resultado sempre pede reinício em vez de devolver um pool
/// utilizável, no mesmo espírito de `CHECKIN_REFUSED_PULL` ("feche e abra o app de novo") — a UI
/// trava a tela até o dono reiniciar, nunca finge que o app continua operável com o pool fechado.
///
/// `seen_remote_sequence` é o mesmo consentimento obsoleto de `resolve_conflict_keep_local_core`,
/// só que na outra direção: baixar e restaurar um remoto que avançou de novo depois da tela abrir
/// aplicaria conteúdo que o dono nunca viu, sem ele nunca ter escolhido especificamente AQUELE
/// estado. A checagem roda ANTES de fechar o pool (ponto de não-retorno mais abaixo), então a
/// recusa aqui ainda permite uma nova tentativa na mesma sessão.
pub(crate) async fn resolve_conflict_use_remote_core(
    pool: SqlitePool,
    db_path: &Path,
    drive: &DriveSnapshotClient,
    seen_remote_sequence: i64,
) -> Result<ConflictResolutionOutcome, String> {
    let local_state = state::load_or_init(&pool).await?;
    let local_schema: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("ler versão do schema: {e}"))?;

    let remote_manifest = drive
        .fetch_manifest()
        .await?
        .ok_or_else(|| NO_CONFLICT_NO_REMOTE_MANIFEST.to_string())?;
    if remote_manifest.sequence != seen_remote_sequence {
        return Err(CHECKIN_REFUSED_STALE_CONFLICT.into());
    }
    // Mesma recusa do check-out (ADR-0015): um aparelho desatualizado nunca rebaixa dados
    // migrados, mesmo quando o dono escolheu explicitamente "usar o outro aparelho".
    if remote_manifest.schema_version > local_schema {
        return Err(format!(
            "{RESTORE_REFUSED_PREFIX}o snapshot do outro aparelho foi publicado por uma versão \
             mais nova do Neko Finance (schema {} > {}) — atualize o app antes de continuar.",
            remote_manifest.schema_version, local_schema
        ));
    }

    let db_bytes = drive.download_snapshot().await?.ok_or_else(|| {
        "Nenhum conflito pendente: o snapshot do outro aparelho sumiu do Drive.".to_string()
    })?;
    // Mesmo raciocínio de `checkout::checkout_on_open`: o hash do conteúdo QUE ACABOU DE CHEGAR
    // vira `last_export_sha256`, para o próximo check-in não ler "mudou" e republicar à toa um
    // conteúdo idêntico ao que este aparelho acabou de adotar.
    let restored_export_sha256 = hex::encode(Sha256::digest(&db_bytes));

    let tmp_path = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("neko-conflict-{}.db", uuid::Uuid::new_v4()));
    restore::stage_downloaded_snapshot(&tmp_path, &db_bytes).await?;

    // Ponto de não-retorno — captura ANTES de fechar o pool: o arquivo baixado chega com
    // `snapshot_state` vazio (`state::strip_from_export_copy`, do lado de quem publicou).
    let device_id = local_state.device_id.clone();
    let last_checkin_at = local_state.last_checkin_at.clone();
    let last_checkin_device_id = local_state.last_checkin_device_id.clone();
    pool.close().await;

    if let Err(e) = restore::swap_active_db_atomically(&tmp_path, db_path) {
        let _ = std::fs::remove_file(&tmp_path);
        // O pool que este comando recebeu já está fechado — não há como devolver um substituto
        // utilizável ao estado gerenciado do Tauri a partir daqui, então mesmo uma falha na troca
        // (que deixa `db_path` intocado) exige reiniciar o app para voltar a operar. Sufixo
        // COMPARTILHADO com as duas falhas abaixo — o frontend reconhece por ele e nunca reoferece
        // um botão de repetir sem pool para operar (`isAfterPoolClosedError`, TypeScript).
        return Err(format!("{e}{AFTER_POOL_CLOSED_SUFFIX}"));
    }

    let new_pool = match checkout::open_migrated_pool(db_path).await {
        Ok(p) => p,
        // Mesmo raciocínio: o pool ORIGINAL já foi fechado antes desta linha — abrir o pool NOVO
        // sobre o arquivo já trocado falhando também não tem retentativa possível nesta sessão.
        Err(e) => return Err(format!("{e}{AFTER_POOL_CLOSED_SUFFIX}")),
    };
    let checked_out_at = chrono::Utc::now().to_rfc3339();
    let adopt_result = state::adopt_after_restore(
        &new_pool,
        &device_id,
        remote_manifest.sequence,
        &checked_out_at,
        &remote_manifest.device_id,
        last_checkin_at.as_deref(),
        last_checkin_device_id.as_deref(),
        &restored_export_sha256,
    )
    .await;
    new_pool.close().await;
    if let Err(e) = adopt_result {
        // O ARQUIVO ativo já é o remoto neste ponto (a troca em si teve sucesso) — só a gravação
        // do bookkeeping local falhou. Ainda assim não há pool para tentar de novo: reiniciar é a
        // única saída, igual às duas falhas acima.
        return Err(format!("{e}{AFTER_POOL_CLOSED_SUFFIX}"));
    }

    Ok(ConflictResolutionOutcome {
        choice: "use_remote".to_string(),
        requires_restart: true,
        sequence: remote_manifest.sequence,
    })
}

// `seen_remote_sequence` vem da tela: a sequência do manifest que `drive_conflict_details`
// mostrou ao dono, nunca rebuscada aqui — é o que sustenta a checagem de consentimento obsoleto
// dentro de cada `resolve_conflict_*_core` (ADR-0015).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn resolve_drive_conflict(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    client_id: String,
    client_secret: Option<String>,
    choice: String,
    seen_remote_sequence: i64,
) -> Result<ConflictResolutionOutcome, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_drive_scope(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let drive = DriveSnapshotClient::new(token, crate::snapshot::transport::production_base_url());

    let _lock = guard.inner().lock().await;
    match choice.as_str() {
        "keep_local" => {
            resolve_conflict_keep_local_core(pool.inner(), &app_dir.0, &drive, seen_remote_sequence)
                .await
        }
        "use_remote" => {
            let db_path = app_dir.0.join("neko-finance.db");
            resolve_conflict_use_remote_core(
                pool.inner().clone(),
                &db_path,
                &drive,
                seen_remote_sequence,
            )
            .await
        }
        other => Err(format!("Escolha de conflito desconhecida: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::token_store::StoredToken;

    #[test]
    fn checkin_refusal_messages_share_the_stable_contract_prefix() {
        // Espelha `CHECKIN_REFUSED_PREFIX` de `src/screens/configView.ts`, onde o frontend
        // reconhece a recusa do lease por este prefixo ESTRUTURAL — nunca por regex sobre as
        // palavras da frase descritiva. Se um dos dois textos deixar de começar por ele, o
        // reconhecimento quebra em produção mesmo com a suíte inteira verde.
        const CHECKIN_REFUSED_PREFIX: &str = "Check-in recusado: ";
        assert!(CHECKIN_REFUSED_PULL.starts_with(CHECKIN_REFUSED_PREFIX));
        assert!(CHECKIN_REFUSED_CONFLICT.starts_with(CHECKIN_REFUSED_PREFIX));
        assert!(CHECKIN_REFUSED_STALE_CONFLICT.starts_with(CHECKIN_REFUSED_PREFIX));
    }

    #[test]
    fn restore_refusal_shares_the_stable_contract_prefix_with_the_frontend() {
        // Espelha `RESTORE_REFUSED_PREFIX` de
        // `src/features/snapshot-conflict/snapshotConflictView.ts` — mesma disciplina do teste
        // acima, para o outro prefixo de contrato desta tela.
        assert_eq!(RESTORE_REFUSED_PREFIX, "Restauração recusada: ");
    }

    // `VACUUM INTO` exige um banco de ORIGEM em arquivo — a partir de `:memory:` ele não
    // materializa o destino (mesma observação já documentada no teste do backup, `commands::mod`).
    async fn test_pool(app_dir: &std::path::Path) -> SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        let src = app_dir.join("neko-src.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{}", src.display()))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .expect("pool SQLite em arquivo");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações");
        pool
    }

    fn test_app_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-checkin-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn token() -> StoredToken {
        StoredToken {
            access_token: "ya29.test".into(),
            refresh_token: "1//test".into(),
            expires_at: 9_999_999_999,
            scope: "".into(),
        }
    }

    #[tokio::test]
    async fn first_checkin_ever_publishes_and_records_sequence_one() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let mut server = mockito::Server::new_async().await;
        // Nenhum manifest/snapshot publicado ainda: toda busca por nome devolve lista vazia.
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect("primeiro check-in deve publicar (primeira subida)");
        assert!(result.published);
        assert!(result.info.last_checkin_at.is_some());
        assert_eq!(
            result.info.last_checkin_device_id,
            Some(result.info.this_device_id.clone())
        );

        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_right_after_a_restore_is_up_to_date_instead_of_republishing_identical_content()
    {
        // Sem `last_export_sha256` refletir o conteúdo recém-restaurado, ele ficaria `NULL` logo
        // depois de toda restauração — o check-in SEGUINTE sempre leria "mudou" e republicaria um
        // conteúdo IDÊNTICO ao que acabou de baixar, anulando o "em dia" logo após toda
        // restauração. Simula o pós-restauração: `adopt_after_restore` com o hash do
        // conteúdo ATUAL do banco (a mesma forma que `checkout::checkout_on_open` calcula dos
        // bytes baixados), sem NENHUMA mudança de domínio depois disso.
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        let (restored_export_sha256, _bytes) = export_candidate_snapshot(&pool, &app_dir)
            .await
            .expect("hash do conteúdo atual, a mesma forma que um export produziria");

        state::adopt_after_restore(
            &pool,
            "device-local",
            3,
            "2026-08-13T09:00:00Z",
            "outro-aparelho",
            None,
            None,
            &restored_export_sha256,
        )
        .await
        .expect("adopt_after_restore");

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 3,
            created_at: "2026-08-13T09:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        // Nenhum mock de upload registrado: se o check-in tentasse republicar mesmo com o
        // conteúdo inalterado, a chamada bateria numa rota não-mockada e o teste acusaria a
        // diferença.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect("conteúdo em dia não é uma recusa, é sucesso sem publicação");
        assert!(
            !result.published,
            "nada mudou desde a restauração — não deve haver publicação nova"
        );
        assert!(!result.info.pending_local_changes);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn second_checkin_pushes_again_when_remote_unchanged_since_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11 10:00:00",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Remoto na MESMA base (sequência 1, publicada por este mesmo aparelho) — subir de novo é
        // seguro (avanço unilateral local).
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local.device_id.clone(),
            sequence: 1,
            created_at: "2026-08-11T10:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            // O snapshot ainda não existe (mock acima devolve lista vazia) → cria via POST.
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "snap-created"}"#)
            .create_async()
            .await;
        server
            // O manifest JÁ existe (id `man-1`, achado acima) → atualiza pelo MESMO id via PATCH,
            // nunca cria um segundo arquivo.
            .mock("PATCH", "/upload/drive/v3/files/man-1")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "man-1"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect("subir de novo com o remoto na mesma base deve ser seguro");
        assert!(result.published);
        assert!(result.info.last_checkin_at.is_some());

        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 2);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_refuses_with_conflict_message_when_both_sides_advanced_from_same_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11 10:00:00",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Remoto avançou para 5 (outro aparelho publicou) enquanto nossa base ainda é 1, E o
        // conteúdo local mudou desde a base (o hash semeado acima não bate com o export real) —
        // os dois lados avançaram a partir da mesma base: Conflict, não Pull.
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 5,
            created_at: "2026-08-11T11:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let err = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect_err("deve recusar publicar por cima do avanço do outro aparelho");
        // Conflito nunca instrui "baixe" — aqui significaria descartar trabalho local sem aviso.
        assert_eq!(err, CHECKIN_REFUSED_CONFLICT);

        // Estado local intocado: a base continua 1, nenhuma sequência foi reivindicada em vão.
        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        // ADR-0015/#427: a UI de Conexão e os gatilhos automáticos precisam saber, sem rede, que
        // há mudança local não publicada E que uma disputa está pendente.
        let info = last_drive_checkin_core(&pool).await.unwrap();
        assert!(
            info.pending_local_changes,
            "o conteúdo local mudou desde a base — a recusa não apaga esse fato"
        );
        assert!(
            info.conflict_pending,
            "o veredito Conflict precisa gatear os gatilhos automáticos até resolução"
        );

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn resolve_conflict_keep_local_clears_pending_local_changes_and_conflict_pending() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11T10:00:00Z",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 5,
            created_at: "2026-08-11T11:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        // Descobre o conflito (mesma configuração do teste acima) — deixa `conflict_pending`
        // gravado, o estado que a resolução abaixo precisa limpar.
        let err = drive_checkin_core(&pool, &app_dir, &drive)
            .await
            .expect_err("conflito esperado");
        assert_eq!(err, CHECKIN_REFUSED_CONFLICT);
        assert!(
            last_drive_checkin_core(&pool)
                .await
                .unwrap()
                .conflict_pending
        );

        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "snap-created"}"#)
            .create_async()
            .await;
        server
            .mock("PATCH", "/upload/drive/v3/files/man-1")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "man-1"}"#)
            .create_async()
            .await;

        resolve_conflict_keep_local_core(&pool, &app_dir, &drive, 5)
            .await
            .expect("manter local publica por cima da disputa");

        let info = last_drive_checkin_core(&pool).await.unwrap();
        assert!(
            !info.pending_local_changes,
            "acabou de publicar — nada mais pendente"
        );
        assert!(
            !info.conflict_pending,
            "a resolução precisa liberar os gatilhos automáticos de novo"
        );

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_refuses_with_pull_message_when_remote_advanced_and_local_unchanged() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        // Primeiro check-in: publica a sequência 1 (primeira subida).
        let mut server1 = mockito::Server::new_async().await;
        server1
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server1
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive1 = DriveSnapshotClient::new(token(), server1.url());
        let first = drive_checkin_core(&pool, &app_dir, &drive1)
            .await
            .expect("primeiro check-in deve publicar");
        assert!(first.published);

        // Nenhuma escrita no banco depois disso: o próximo export teria o MESMO conteúdo. Mas
        // outro aparelho publicou por cima (sequência 2) — o remoto avançou sem que este
        // aparelho tivesse mudança própria para reivindicar: Pull, nunca Conflict.
        let mut server2 = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 2,
            created_at: "2026-08-11T12:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server2
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server2
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive2 = DriveSnapshotClient::new(token(), server2.url());

        let err = drive_checkin_core(&pool, &app_dir, &drive2)
            .await
            .expect_err("deve recusar com o veredito Pull, sem instruir um botão que não existe");
        // O check-out roda sozinho na próxima abertura do app — a copy pede esse gesto.
        assert_eq!(err, CHECKIN_REFUSED_PULL);

        // Estado local intocado: a base continua 1.
        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn checkin_twice_with_unchanged_content_is_up_to_date_and_never_republishes() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        // Primeiro check-in: nenhum snapshot publicado ainda (primeira subida).
        let mut server1 = mockito::Server::new_async().await;
        server1
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server1
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive1 = DriveSnapshotClient::new(token(), server1.url());
        let first = drive_checkin_core(&pool, &app_dir, &drive1)
            .await
            .expect("primeiro check-in deve publicar");
        assert!(first.published);
        let state_after_first = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after_first.base_sequence, 1);

        // Segundo check-in, banco INALTERADO desde o primeiro (nenhuma escrita entre as duas
        // chamadas): o remoto reflete exatamente o que o primeiro check-in publicou. Nenhum mock
        // de upload aqui — se o código tentar subir de novo, a chamada não-mockada devolve 501 e
        // o teste acusa a diferença.
        let mut server2 = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: state_after_first.device_id.clone(),
            sequence: 1,
            created_at: first.info.last_checkin_at.clone().unwrap(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: 1,
        })
        .unwrap();
        server2
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server2
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive2 = DriveSnapshotClient::new(token(), server2.url());

        // "Em dia" é SUCESSO (ADR-0015), não erro: o segundo clique não publica de novo, mas
        // também não deve virar mensagem de falha para o dono.
        let second = drive_checkin_core(&pool, &app_dir, &drive2)
            .await
            .expect("nada mudou desde o último check-in — 'em dia' é sucesso, não erro");
        assert!(
            !second.published,
            "clique redundante não deve reivindicar ter publicado algo novo"
        );
        assert_eq!(second.info.last_checkin_at, first.info.last_checkin_at);

        // Sequência intocada: um clique redundante nunca avança a base sem mudança real.
        let state_after_second = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after_second.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    // --- drive_conflict_details_core --------------------------------------------------------

    async fn seed_gesture(pool: &SqlitePool, timestamp: &str, event_type: &str) {
        let person_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Dono')")
            .bind(&person_id)
            .execute(pool)
            .await
            .unwrap();
        let profile_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
            .bind(&profile_id)
            .bind(&person_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp) \
             VALUES (?1, ?2, 'transaction', 'e1', ?3, ?4)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(event_type)
        .bind(&profile_id)
        .bind(timestamp)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Um segundo banco migrado com um gesto próprio no `sync_log` e `snapshot_state` vazio (o
    /// mesmo perfil de `checkout.rs::build_remote_db_bytes`) — o "outro aparelho" que
    /// `drive_conflict_details_core` baixa para ler os gestos REMOTOS.
    async fn build_remote_db_bytes_with_gesture(
        dir: &std::path::Path,
        timestamp: &str,
        event_type: &str,
    ) -> Vec<u8> {
        let remote_path = dir.join(format!("remote-conflict-{}.db", uuid::Uuid::new_v4()));
        let remote_pool = checkout::open_migrated_pool(&remote_path).await.unwrap();
        seed_gesture(&remote_pool, timestamp, event_type).await;
        sqlx::query("DELETE FROM snapshot_state")
            .execute(&remote_pool)
            .await
            .unwrap();
        remote_pool.close().await;
        std::fs::read(&remote_path).unwrap()
    }

    #[tokio::test]
    async fn conflict_details_returns_remote_manifest_and_only_gestures_after_the_common_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(&pool, 1, "2026-08-11T10:00:00Z", &local.device_id, "hash-1")
            .await
            .unwrap();
        // Antes da base em comum: não pode aparecer na lista.
        seed_gesture(&pool, "2026-08-10 09:00:00", "import").await;
        // Depois da base em comum: é exatamente o que a tela de conflito precisa mostrar.
        seed_gesture(&pool, "2026-08-12 09:00:00", "write_back").await;
        // O outro aparelho tem o SEU PRÓPRIO gesto — precisa aparecer em `remote_gestures`, nunca
        // misturado com `local_gestures`.
        let remote_bytes =
            build_remote_db_bytes_with_gesture(&app_dir, "2026-08-12 07:00:00", "import").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 5,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "snap-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/snap-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(remote_bytes)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let details = drive_conflict_details_core(&pool, &app_dir, &drive)
            .await
            .expect("conflito de fato pendente: remoto avançou além da base");
        assert_eq!(details.remote_manifest.sequence, 5);
        assert_eq!(details.remote_manifest.device_id, "outro-aparelho");
        assert_eq!(
            details.local_gestures.len(),
            1,
            "só o gesto POSTERIOR à base"
        );
        assert_eq!(details.local_gestures[0].event_type, "write_back");
        assert_eq!(
            details.remote_gestures.len(),
            1,
            "o gesto do outro aparelho vem numa lista PRÓPRIA"
        );
        assert_eq!(details.remote_gestures[0].event_type, "import");

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn conflict_details_errs_when_the_remote_never_advanced_past_the_base() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(&pool, 3, "2026-08-11T10:00:00Z", &local.device_id, "hash-1")
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local.device_id.clone(),
            sequence: 3,
            created_at: "2026-08-11T10:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let err = drive_conflict_details_core(&pool, &app_dir, &drive)
            .await
            .expect_err("remoto na própria base não é conflito nenhum");
        assert!(err.contains("Nenhum conflito pendente"), "erro: {err}");

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn conflict_details_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        let remote_bytes =
            build_remote_db_bytes_with_gesture(&app_dir, "2026-08-12 07:00:00", "import").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "snap-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/snap-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(remote_bytes)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let pool_for_read = pool.clone();
        let app_dir_for_read = app_dir.clone();
        let read = tokio::spawn(async move {
            drive_conflict_details_core(&pool_for_read, &app_dir_for_read, &drive).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), read)
            .await
            .expect("a leitura NÃO pode travar para sempre esperando a única conexão")
            .expect("a task de leitura não deve entrar em panic");
        assert!(result.is_ok());
        let _ = local;
        std::fs::remove_dir_all(&app_dir).ok();
    }

    // --- resolve_conflict_keep_local_core ---------------------------------------------------

    #[tokio::test]
    async fn resolve_keep_local_publishes_past_both_sides_and_records_the_resolved_sequence() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        // Base 1, mas o hash semeado NUNCA bate com o export real — conteúdo local mudou desde
        // então, exatamente como no conflito que levaria `drive_checkin` a recusar.
        state::record_checkin(
            &pool,
            1,
            "2026-08-11T10:00:00Z",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 5,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            // Snapshot ainda não existe (mock acima devolve lista vazia) → cria via POST.
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "snap-created"}"#)
            .create_async()
            .await;
        server
            // O manifest JÁ existe (id `man-1`) → atualiza pelo MESMO id via PATCH.
            .mock("PATCH", "/upload/drive/v3/files/man-1")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "man-1"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        // A tela mostrou o remoto na sequência 5 (o mesmo manifest mockado acima) — nada avançou
        // de novo entre o fetch e o clique, então o consentimento continua válido.
        let outcome = resolve_conflict_keep_local_core(&pool, &app_dir, &drive, 5)
            .await
            .expect("manter este aparelho deve publicar por cima do remoto");
        assert_eq!(outcome.choice, "keep_local");
        assert!(
            !outcome.requires_restart,
            "keep_local nunca troca o arquivo ativo"
        );
        // max(base+1=2, remote_seq+1=6) = 6 — nunca só base+1, que o árbitro recusaria de novo.
        assert_eq!(outcome.sequence, 6);

        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 6);
        assert_eq!(
            state_after.last_checkin_device_id.as_deref(),
            Some(local.device_id.as_str())
        );

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn resolve_keep_local_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"id": "created"}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let pool_for_resolve = pool.clone();
        let app_dir_for_resolve = app_dir.clone();
        // Nenhum manifest publicado (mock devolve lista vazia) — remoto ausente conta como
        // sequência 0, o que a tela também teria visto no fetch anterior.
        let resolve = tokio::spawn(async move {
            resolve_conflict_keep_local_core(&pool_for_resolve, &app_dir_for_resolve, &drive, 0)
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), resolve)
            .await
            .expect("NÃO pode travar para sempre esperando a única conexão (VACUUM INTO incluso)")
            .expect("a task não deve entrar em panic");
        assert!(result.is_ok());

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[tokio::test]
    async fn resolve_keep_local_refuses_with_stale_conflict_when_the_remote_advanced_again_since_the_screen_fetched_it()
     {
        let app_dir = test_app_dir();
        let pool = test_pool(&app_dir).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            1,
            "2026-08-11T10:00:00Z",
            &local.device_id,
            "seed-hash-nao-bate-com-export-real",
        )
        .await
        .unwrap();

        // A tela buscou os detalhes do conflito e mostrou o remoto na sequência 5. Antes do dono
        // clicar "Manter este aparelho", um TERCEIRO check-in publicou de novo (sequência 7) — o
        // clique não pode publicar por cima de um estado que o dono nunca viu.
        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 7,
            created_at: "2026-08-12T09:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        // Nenhum mock de upload: se o código tentasse publicar mesmo com a disputa velha, a
        // chamada não-mockada devolveria 501 e o teste acusaria a diferença.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let err = resolve_conflict_keep_local_core(&pool, &app_dir, &drive, 5)
            .await
            .expect_err("consentimento obsoleto: o remoto avançou de novo desde o fetch da tela");
        assert_eq!(err, CHECKIN_REFUSED_STALE_CONFLICT);

        // Estado local intocado: nenhuma sequência foi reivindicada em vão.
        let state_after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 1);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    // --- resolve_conflict_use_remote_core ---------------------------------------------------

    fn conflict_test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-conflict-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Espelha `checkout.rs::build_remote_db_bytes`: um segundo banco migrado, com um marcador
    /// exclusivo dele e `snapshot_state` vazio (o snapshot publicado nunca carrega a identidade
    /// de quem publicou — `state::strip_from_export_copy`).
    async fn build_remote_db_bytes(dir: &std::path::Path, marker: &str) -> Vec<u8> {
        let remote_path = dir.join(format!("remote-source-{}.db", uuid::Uuid::new_v4()));
        let remote_pool = checkout::open_migrated_pool(&remote_path).await.unwrap();
        crate::commands::app_setting_set(&remote_pool, "restore_marker", marker)
            .await
            .unwrap();
        sqlx::query("DELETE FROM snapshot_state")
            .execute(&remote_pool)
            .await
            .unwrap();
        remote_pool.close().await;
        std::fs::read(&remote_path).unwrap()
    }

    #[tokio::test]
    async fn resolve_use_remote_restores_the_remote_snapshot_and_requires_restart() {
        let dir = conflict_test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = checkout::open_migrated_pool(&db_path).await.unwrap();
        let local_before = state::load_or_init(&pool).await.unwrap();
        let local_schema: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_one(&pool)
                .await
                .unwrap();
        crate::commands::app_setting_set(&pool, "local_only_marker", "perdido-ao-usar-o-remoto")
            .await
            .unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-outro-aparelho").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 9,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: local_schema,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-snapshot.db' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "snap-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/snap-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(remote_bytes)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        // A tela mostrou o remoto na sequência 9 (o mesmo manifest mockado acima).
        let outcome = resolve_conflict_use_remote_core(pool, &db_path, &drive, 9)
            .await
            .expect("usar o outro aparelho deve restaurar com sucesso");
        assert_eq!(outcome.choice, "use_remote");
        assert!(
            outcome.requires_restart,
            "trocar o arquivo ativo debaixo do pool em uso exige reiniciar o app"
        );
        assert_eq!(outcome.sequence, 9);

        // Reabre um pool novo no MESMO caminho — o jeito honesto de provar que o conteúdo ativo
        // é mesmo o do remoto, não uma cópia do que já estava aqui.
        let reopened = checkout::open_migrated_pool(&db_path).await.unwrap();
        let marker = crate::commands::app_setting_get(&reopened, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-outro-aparelho"));
        let local_only = crate::commands::app_setting_get(&reopened, "local_only_marker")
            .await
            .unwrap();
        assert!(
            local_only.is_none(),
            "o remoto substitui o conteúdo local de verdade"
        );

        let state_after = state::load_or_init(&reopened).await.unwrap();
        assert_eq!(state_after.device_id, local_before.device_id);
        assert_eq!(state_after.base_sequence, 9);
        assert_eq!(
            state_after.last_checkout_device_id.as_deref(),
            Some("outro-aparelho")
        );
        reopened.close().await;

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_use_remote_refuses_when_the_remote_schema_is_newer_and_changes_nothing() {
        let dir = conflict_test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = checkout::open_migrated_pool(&db_path).await.unwrap();
        let local_schema: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_one(&pool)
                .await
                .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 9,
            created_at: "2026-08-12T08:00:00Z".into(),
            app_version: "9.9.9".into(),
            schema_version: local_schema + 1000,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        // Nenhum mock para o download do snapshot: se o código tentasse baixar mesmo com o
        // schema recusado, a chamada não-mockada devolveria 501 e o teste acusaria a diferença.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let err = resolve_conflict_use_remote_core(pool, &db_path, &drive, 9)
            .await
            .expect_err("schema remoto mais novo nunca pode restaurar, mesmo escolhido");
        assert!(err.contains("mais nova"), "erro: {err}");
        // Ainda ANTES do ponto de não-retorno (o pool não foi fechado) — a tela pode oferecer
        // tentar de novo depois de atualizar o app, então o erro nunca carrega o sufixo
        // compartilhado das falhas pós-fechamento do pool.
        assert!(!err.ends_with(AFTER_POOL_CLOSED_SUFFIX), "erro: {err}");

        // Nada mudou: o arquivo ativo continua o mesmo (reabrir prova que ele ainda existe e
        // migra normalmente, sem qualquer sinal do marcador do remoto).
        let reopened = checkout::open_migrated_pool(&db_path).await.unwrap();
        let state_after = state::load_or_init(&reopened).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        reopened.close().await;

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_use_remote_refuses_with_stale_conflict_when_the_remote_advanced_again_since_the_screen_fetched_it()
     {
        let dir = conflict_test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = checkout::open_migrated_pool(&db_path).await.unwrap();

        // A tela buscou os detalhes do conflito e mostrou o remoto na sequência 9. Antes do dono
        // clicar "Usar o outro aparelho", um TERCEIRO check-in publicou de novo (sequência 12) —
        // baixar e restaurar por cima aplicaria um conteúdo que o dono nunca viu nem escolheu.
        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 12,
            created_at: "2026-08-12T09:00:00Z".into(),
            app_version: "0.2.1".into(),
            schema_version: 1,
        })
        .unwrap();
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "q".into(),
                "name = 'neko-manifest.json' and trashed = false".into(),
            ))
            .with_status(200)
            .with_body(r#"{"files": [{"id": "man-1"}]}"#)
            .create_async()
            .await;
        server
            .mock("GET", "/drive/v3/files/man-1")
            .match_query(mockito::Matcher::UrlEncoded("alt".into(), "media".into()))
            .with_status(200)
            .with_body(manifest_json)
            .create_async()
            .await;
        // Nenhum mock para o download do snapshot: se o código tentasse baixar mesmo com a
        // disputa velha, a chamada não-mockada devolveria 501 e o teste acusaria a diferença.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let err = resolve_conflict_use_remote_core(pool, &db_path, &drive, 9)
            .await
            .expect_err("consentimento obsoleto: o remoto avançou de novo desde o fetch da tela");
        assert_eq!(err, CHECKIN_REFUSED_STALE_CONFLICT);
        // Ainda ANTES do ponto de não-retorno — o pool segue utilizável, a tela pode tentar de
        // novo depois de recarregar os detalhes.
        assert!(!err.ends_with(AFTER_POOL_CLOSED_SUFFIX), "erro: {err}");

        // Nada mudou: o arquivo ativo continua o local original (reabrir prova que ele ainda
        // existe e migra normalmente, sem qualquer sinal de conteúdo remoto).
        let reopened = checkout::open_migrated_pool(&db_path).await.unwrap();
        let state_after = state::load_or_init(&reopened).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        reopened.close().await;

        std::fs::remove_dir_all(&dir).ok();
    }
}
