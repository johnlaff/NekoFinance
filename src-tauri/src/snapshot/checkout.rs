//! Orquestração do check-out ao abrir o app (ADR-0015): consulta o manifest remoto e, quando o
//! árbitro (`lease::decide`) devolve `Pull`, baixa, valida e troca o banco ativo atomicamente
//! ANTES de qualquer gesto do dono. `checkout_on_open` é o núcleo testável (recebe um
//! `DriveSnapshotClient` já pronto); `checkout_on_open_best_effort` é o gancho que `lib.rs` chama
//! de verdade, resolvendo token/escopo e silenciando qualquer motivo de NÃO tentar.

use super::{lease, restore, state, transport::DriveSnapshotClient};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// O que `checkout_on_open` fez, quando termina sem erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutOutcome {
    /// Remoto não avançou além da base local (ou nenhum snapshot foi publicado ainda) — nada
    /// para baixar. Cobre também o veredito `Push`/`Conflict` do árbitro: check-out só AGE no
    /// veredito `Pull`, os outros dois são assunto do check-in.
    NothingToDo,
    /// O snapshot remoto foi baixado, validado e trocou o banco ativo.
    Restored { safeguard_path: Option<PathBuf> },
    /// O manifest remoto tem schema mais nova que este app — restauração recusada, nada mudou.
    RefusedNewerSchema {
        local_schema: i64,
        remote_schema: i64,
    },
    /// O manifest remoto carrega o NOSSO PRÓPRIO `device_id` E `sequence == base_local + 1` — a
    /// janela exata de um check-in que morreu entre o upload confirmado e a gravação do estado
    /// local (ADR-0015). O conteúdo já é nosso; restaurar de verdade descartaria qualquer
    /// trabalho feito depois daquele upload, então só a sequência-base local avança para alcançar
    /// o remoto, sem baixar nem trocar arquivo. Fora dessa janela — mesmo com o mesmo `device_id`
    /// — o manifest pode pertencer a outra instalação que compartilha identidade por um caminho
    /// lateral (cópia manual da pasta do app, backup restaurado à mão sem passar pelo strip do
    /// export), então o check-out segue o veredito normal do árbitro em vez de adotar às cegas.
    CaughtUpOwnSequence { sequence: i64 },
}

/// Pool sempre utilizável + o que aconteceu. `outcome: Err(_)` é um problema NÃO-FATAL (rede,
/// integridade do download) — `pool` continua sendo o MESMO recebido por `checkout_on_open`,
/// intocado; o chamador só loga e segue com ele.
pub struct CheckoutResult {
    pub pool: SqlitePool,
    pub outcome: Result<CheckoutOutcome, String>,
}

async fn local_schema_version(pool: &SqlitePool) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("ler versão do schema: {e}"))
}

/// Mesmas opções de conexão do `setup()` do app (`lib.rs`): WAL, `foreign_keys` explícito, pool de
/// UMA conexão (escritor único), migrado. Reusado tanto pela abertura inicial quanto pela
/// reabertura depois de uma restauração — a única fonte de como este app abre seu banco.
pub(crate) async fn open_migrated_pool(db_path: &Path) -> Result<SqlitePool, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
        .map_err(|e| format!("URL do banco: {e}"))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| format!("abrir o banco: {e}"))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("migrações do banco: {e}"))?;
    Ok(pool)
}

/// Desfecho de [`reopen_after_swap_or_rollback`] quando reabrir NÃO propaga um erro fatal.
#[derive(Debug)]
enum ReopenOutcome {
    /// Caso comum: o conteúdo recém-trocado reabriu de primeira.
    Reopened(SqlitePool),
    /// Reabrir o conteúdo recém-trocado falhou, mas havia uma salvaguarda do banco de ANTES da
    /// troca — revertida e reaberta com sucesso. `message` é o aviso não-fatal que
    /// `checkout_on_open` devolve como `outcome: Err` (a UI de Conexão mostra; a próxima abertura
    /// tenta o snapshot remoto de novo).
    RolledBack { pool: SqlitePool, message: String },
}

/// Reabre o pool no `db_path` recém-trocado por `swap_active_db_atomically` — e, se isso falhar,
/// reverte automaticamente para a salvaguarda em vez de deixar o app sem banco nenhum. Reabrir
/// com sucesso é o caso comum; falhar é raro
/// (I/O transitório, disco cheio na migração), mas tratar como fatal joga fora um app que abriria
/// perfeitamente bem com o conteúdo de ANTES da troca, intacto na salvaguarda (nunca movida, só
/// copiada por `swap_active_db_atomically`).
///
/// `Err` só quando NENHUM pool utilizável sobra: sem `safeguard_path` (primeira restauração —
/// nada para reverter) ou quando a própria reversão também falha. Esse é o único caso em que
/// `checkout_on_open` ainda propaga um erro fatal de verdade.
async fn reopen_after_swap_or_rollback(
    db_path: &Path,
    safeguard_path: Option<&Path>,
) -> Result<ReopenOutcome, String> {
    let reopen_err = match open_migrated_pool(db_path).await {
        Ok(pool) => return Ok(ReopenOutcome::Reopened(pool)),
        Err(e) => e,
    };

    // A mensagem cita o caminho da salvaguarda: o conteúdo de ANTES da troca está intacto lá
    // (cópia, nunca movida), disponível para restauração manual mesmo se a reversão automática
    // abaixo também falhar.
    let recovery = safeguard_path
        .map(|p| format!("o conteúdo anterior está preservado em {}", p.display()))
        .unwrap_or_else(|| "não havia banco anterior a preservar (primeira restauração)".into());
    let fatal_msg = format!("reabrir banco depois da restauração: {reopen_err}; {recovery}");

    let Some(safeguard) = safeguard_path else {
        return Err(fatal_msg);
    };
    if let Err(rollback_err) = restore::rollback_to_safeguard(safeguard, db_path) {
        return Err(format!(
            "{fatal_msg}; reversão automática também falhou: {rollback_err}"
        ));
    }
    match open_migrated_pool(db_path).await {
        Ok(pool) => Ok(ReopenOutcome::RolledBack {
            pool,
            message: format!(
                "{fatal_msg}; revertido automaticamente para o banco anterior — o snapshot \
                 remoto será tentado de novo na próxima abertura"
            ),
        }),
        Err(reopen_after_rollback_err) => Err(format!(
            "{fatal_msg}; reversão automática também falhou ao reabrir: {reopen_after_rollback_err}"
        )),
    }
}

/// Resolve client id → token com escopo `drive.appdata` → cliente pronto, em modo melhor esforço
/// (ADR-0015): SÓ os motivos de NÃO TENTAR — sem client id configurado, ou nenhum token guardado
/// (nunca conectou) — devolvem `Ok(None)` em silêncio, "sync ainda não configurado". A partir daí
/// a decisão de tentar já foi tomada; um erro genuíno DEPOIS disso (rede durante o refresh do
/// token, HTTP do provedor recusando o refresh) é uma tentativa que FALHOU e precisa ficar
/// visível como `Err`, nunca desaparecer como se nada tivesse acontecido — a mesma distinção que
/// o doc de `checkout_on_open_best_effort` já promete ("só a decisão de TENTAR é best-effort, não
/// o resultado da tentativa"). O escopo `drive.appdata` ainda não concedido (`NEEDS_DRIVE_REAUTH`,
/// uma conexão de antes deste recurso existir) fica na MESMA classe de "não configurado" das duas
/// primeiras — é "ainda não migrou para o re-consentimento", não uma falha de tentativa.
///
/// Compartilhado pelos três pontos de entrada que tentam o Drive sem um clique explícito do dono:
/// o check-out no boot, a sonda de foco, e o check-in automático
/// (`checkin_task::run_checkin_attempt_core`).
pub(crate) async fn resolve_drive_client_best_effort(
    pool: &SqlitePool,
    app_dir: &Path,
) -> Result<Option<DriveSnapshotClient>, String> {
    let Some(client_id) = crate::sync_task::resolve_client_id(pool).await else {
        return Ok(None);
    };
    // Checado ANTES de `ensure_drive_scope`, sync (é uma leitura local — keychain/arquivo, nunca
    // rede): só assim dá para distinguir "nunca conectou" (silencioso) de uma falha real DEPOIS de
    // decidir tentar (o refresh abaixo, que pode tocar rede).
    match crate::oauth::token_store::load_token(app_dir) {
        Ok(None) => return Ok(None),
        Ok(Some(_)) => {}
        Err(e) => return Err(e),
    }
    let client_secret = crate::oauth::pkce::resolve_client_secret(None);
    let token = match crate::oauth::token_store::ensure_drive_scope(
        app_dir,
        &client_id,
        client_secret.as_deref(),
    )
    .await
    {
        Ok(t) => t,
        Err(e) if e == crate::oauth::token_store::NEEDS_DRIVE_REAUTH => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(DriveSnapshotClient::new(
        token,
        super::transport::production_base_url(),
    )))
}

/// Tudo capturado ANTES do ponto de não-retorno (fechar o pool antigo) e pronto para o commit da
/// troca de arquivo — devolvido por [`prepare_restore`] quando o veredito é restaurar de verdade.
struct ReadyToCommit {
    pool: SqlitePool,
    tmp_path: PathBuf,
    device_id: String,
    last_checkin_at: Option<String>,
    last_checkin_device_id: Option<String>,
    remote_sequence: i64,
    remote_device_id: String,
    restored_export_sha256: String,
}

/// Trabalho que só pode rodar DEPOIS que [`prepare_restore`] devolve `ReadyToFinish` — sempre até
/// o fim, nunca sob o teto de espera do boot (ver o doc de [`RestorePreparation`]). As duas
/// variantes são as únicas escritas no `pool` que um check-out pode produzir: `Commit` troca o
/// arquivo inteiro; `AdoptOwnSequence` só avança `base_sequence` (ver o comentário no ramo
/// correspondente de `prepare_restore`).
enum UntimedWork {
    Commit(ReadyToCommit),
    AdoptOwnSequence { pool: SqlitePool, sequence: i64 },
}

/// Desfecho de [`prepare_restore`]: ou o check-out já termina aqui mesmo (`Done` — nada a
/// restaurar, schema recusado, erro de rede), ou fica pronto para terminar (`ReadyToFinish`). A
/// costura entre os dois é o mesmo "ponto de não-retorno" documentado dentro de
/// `prepare_restore`: tudo ANTES dele só LÊ rede/disco e pode ser abandonado a qualquer momento
/// sem deixar rastro no `pool` recebido — é por isso que só essa parte entra sob o teto de espera
/// do boot (`checkout_on_open_with_deadline`). O trabalho de `ReadyToFinish` (fechar o pool
/// antigo e trocar o arquivo, OU só avançar a sequência local) NUNCA pode ser interrompido: uma
/// vez que uma dessas escritas começa, não existe "desistir" no meio — um teto que também
/// cobrisse esta fase deixaria o pool clonado ANTES do teto (a `pool_fallback` do chamador)
/// potencialmente inconsistente com o que a escrita abandonada já tinha feito, e no caso do
/// commit especificamente, `SqlitePool::close()` marca o `Arc` inteiro compartilhado entre os
/// clones (não só o handle que chamou) — o clone "de segurança" nunca seria seguro se o teto
/// pudesse abortar depois desse ponto.
enum RestorePreparation {
    Done(CheckoutResult),
    ReadyToFinish(UntimedWork),
}

/// Fase 1 (abortável): consulta o manifest remoto e, se o veredito for `Pull`, baixa e valida o
/// snapshot — tudo isto só LÊ rede/disco, `pool` nunca é tocado. A única fase que o teto de
/// espera do boot (`checkout_on_open_with_deadline`) pode abandonar a meio caminho.
async fn prepare_restore(
    pool: SqlitePool,
    db_path: &Path,
    drive: &DriveSnapshotClient,
) -> RestorePreparation {
    let local_state = match state::load_or_init(&pool).await {
        Ok(s) => s,
        Err(e) => {
            return RestorePreparation::Done(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    // Gate (ADR-0015): um conflito descoberto por um check-in (automático ou manual) e ainda não
    // resolvido pelo dono nunca pode ser sobrescrito em silêncio por um check-out normal — a MESMA
    // disciplina que já gate os gatilhos automáticos de check-in (`checkin_task`). Sem isto, fechar
    // o app com uma disputa pendente e reabrir mais tarde (com o remoto tendo avançado ainda mais)
    // faria este check-out ler `Pull` e restaurar por cima do lado local da disputa, sem o dono
    // nunca ter escolhido — a mesma sobrescrita silenciosa que o lease existe para impedir. A tela
    // de conflito é quem resolve isto, nunca o boot.
    if local_state.conflict_pending_since.is_some() {
        return RestorePreparation::Done(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::NothingToDo),
        });
    }

    let remote = match drive.fetch_manifest().await {
        Ok(m) => m,
        Err(e) => {
            return RestorePreparation::Done(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    // Check-out nunca publica: não há candidato local novo, só a pergunta "o remoto avançou além
    // da nossa base?" — o mesmo árbitro do check-in, com `local == base`. `Conflict` nunca surge
    // desta chamada (exigiria `local > base`, e aqui `local` é sempre igual a `base`); `UpToDate`
    // e `Push` surgem normalmente (`Push` cobre o remoto ausente/regredido — nada ali é mais novo
    // que a nossa base) e caem no mesmo `NothingToDo`: check-out só AGE no veredito `Pull`.
    let verdict = lease::decide(
        local_state.base_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    );
    if verdict != lease::LeaseVerdict::Pull {
        return RestorePreparation::Done(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::NothingToDo),
        });
    }
    // Pull exige `remote_seq > base`, o que só é possível com um manifest presente.
    let remote_manifest =
        remote.expect("veredito Pull do árbitro implica manifest remoto presente");

    // O remoto avançou com a NOSSA PRÓPRIA identidade E na sequência EXATA que um check-in morto
    // deixaria (`base + 1`): publicamos aquele conteúdo nós mesmos — o upload confirmou, mas a
    // gravação da base local morreu antes de terminar (queda de rede/processo entre as duas
    // etapas). Restaurar de verdade baixaria e trocaria o banco ativo pelo NOSSO PRÓPRIO snapshot
    // antigo, descartando qualquer gesto feito depois daquele upload — o conteúdo já é nosso, só
    // a base local está atrasada. Nunca baixa nem troca arquivo neste ramo: só alcança a
    // sequência remota.
    //
    // Qualquer OUTRA sequência com o mesmo `device_id` não entra aqui, mesmo que também seja
    // "nossa": duas instalações podem compartilhar identidade por um caminho lateral (cópia
    // manual da pasta do app; backup local restaurado à mão, que não passa pelo `strip` do
    // export) — nesse caso o manifest pertence de fato a OUTRO aparelho que só usa o mesmo
    // rótulo, e cai no fluxo normal abaixo (restauração de verdade, registrada na linha
    // "Última leitura do Drive" e com a salvaguarda local), preservando a convergência entre
    // os dois.
    if remote_manifest.device_id == local_state.device_id
        && remote_manifest.sequence == local_state.base_sequence + 1
    {
        // A escrita em si (`state::adopt_own_sequence`) fica para depois do teto de espera do
        // boot — ver o doc de `RestorePreparation`: nada em `prepare_restore` pode tocar `pool`.
        return RestorePreparation::ReadyToFinish(UntimedWork::AdoptOwnSequence {
            pool,
            sequence: remote_manifest.sequence,
        });
    }

    let local_schema = match local_schema_version(&pool).await {
        Ok(v) => v,
        Err(e) => {
            return RestorePreparation::Done(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };
    if remote_manifest.schema_version > local_schema {
        return RestorePreparation::Done(CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema,
                remote_schema: remote_manifest.schema_version,
            }),
        });
    }

    // `fetch_manifest` (acima) e `download_snapshot` (aqui) são duas chamadas HTTP separadas: se
    // outro aparelho publicar exatamente nesse intervalo, os bytes baixados podem já ser mais
    // novos que `remote_manifest.sequence`/`schema_version` — a sequência gravada no fim ficaria
    // um passo atrás do conteúdo de fato restaurado. Risco aceito: o próximo check-out re-detecta
    // o remoto como avançado (o manifest mais novo não bate com a base recém-gravada) e converge
    // sozinho — nunca perde dado nem trava, só repete o ciclo uma vez a mais.
    let db_bytes = match drive.download_snapshot().await {
        Ok(Some(bytes)) => bytes,
        // Veredito Pull mas o binário sumiu (só o manifest sobrou) — nada para restaurar.
        Ok(None) => {
            return RestorePreparation::Done(CheckoutResult {
                pool,
                outcome: Ok(CheckoutOutcome::NothingToDo),
            });
        }
        Err(e) => {
            return RestorePreparation::Done(CheckoutResult {
                pool,
                outcome: Err(e),
            });
        }
    };

    // Hash do conteúdo QUE ACABOU DE CHEGAR, na mesma forma que `export_candidate_snapshot`
    // produziria (já sem `snapshot_state` — quem publicou já rodou `strip_from_export_copy` do
    // lado de lá): vai para dentro de `ReadyToCommit` e, de lá, para `last_export_sha256` no
    // commit, para o PRÓXIMO check-in comparar contra ele. Sem isto, o hash local ficava `NULL`
    // logo depois de toda restauração e o próximo check-in sempre lia "mudou", republicando um
    // conteúdo idêntico ao que acabou de baixar.
    let restored_export_sha256 = hex::encode(Sha256::digest(&db_bytes));

    let tmp_path = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("neko-checkout-{}.db", uuid::Uuid::new_v4()));
    if let Err(e) = restore::stage_downloaded_snapshot(&tmp_path, &db_bytes).await {
        return RestorePreparation::Done(CheckoutResult {
            pool,
            outcome: Err(e),
        });
    }

    // Ponto de não-retorno: tudo que podia falhar por rede/integridade já rodou com `pool`
    // intacto — daqui em diante é só o commit (`commit_restore`), que roda SEMPRE até o fim (ver o
    // doc de `RestorePreparation`). A identidade DESTE aparelho E o histórico de check-in que ele
    // já tinha precisam sobreviver à troca — capturados AQUI, ainda com `pool` aberto, porque o
    // arquivo baixado chega com `snapshot_state` vazio (ver `state::strip_from_export_copy`, que
    // roda do lado de quem publicou). Sem capturar o histórico de check-in aqui, um aparelho que
    // já publicou perderia a própria linha do tempo a cada check-out.
    RestorePreparation::ReadyToFinish(UntimedWork::Commit(ReadyToCommit {
        device_id: local_state.device_id.clone(),
        last_checkin_at: local_state.last_checkin_at.clone(),
        last_checkin_device_id: local_state.last_checkin_device_id.clone(),
        remote_sequence: remote_manifest.sequence,
        remote_device_id: remote_manifest.device_id,
        restored_export_sha256,
        pool,
        tmp_path,
    }))
}

/// Só avança `base_sequence` no ramo `AdoptOwnSequence` de [`UntimedWork`] — a escrita que o
/// próprio ramo correspondente de `prepare_restore` documenta não poder rodar sob o teto de
/// espera. Sempre roda até o fim (ver o doc de [`RestorePreparation`]).
async fn adopt_own_sequence_restore(pool: SqlitePool, sequence: i64) -> CheckoutResult {
    let outcome = match state::adopt_own_sequence(&pool, sequence).await {
        Ok(()) => Ok(CheckoutOutcome::CaughtUpOwnSequence { sequence }),
        Err(e) => Err(e),
    };
    CheckoutResult { pool, outcome }
}

/// Despacha o [`UntimedWork`] que [`prepare_restore`] deixou pronto — a única costura, chamada
/// tanto por `checkout_on_open` quanto por `checkout_on_open_with_deadline`, para as duas nunca
/// divergirem em como o trabalho pós-teto é executado.
async fn finish_restore(work: UntimedWork, db_path: &Path) -> Result<CheckoutResult, String> {
    match work {
        UntimedWork::Commit(ready) => commit_restore(ready, db_path).await,
        UntimedWork::AdoptOwnSequence { pool, sequence } => {
            Ok(adopt_own_sequence_restore(pool, sequence).await)
        }
    }
}

/// Fase 2 (NUNCA abortável — ver o doc de [`RestorePreparation`]): fecha o pool antigo, troca o
/// arquivo pelo snapshot já validado e reabre — com reversão automática para a salvaguarda se a
/// reabertura falhar. Só chamada depois que [`prepare_restore`] devolve
/// `ReadyToFinish(UntimedWork::Commit(_))`; nada aqui depende de rede.
async fn commit_restore(ready: ReadyToCommit, db_path: &Path) -> Result<CheckoutResult, String> {
    let ReadyToCommit {
        pool,
        tmp_path,
        device_id,
        last_checkin_at,
        last_checkin_device_id,
        remote_sequence,
        remote_device_id,
        restored_export_sha256,
    } = ready;
    pool.close().await;

    let safeguard_path = match restore::swap_active_db_atomically(&tmp_path, db_path) {
        Ok(p) => p,
        Err(swap_err) => {
            let _ = std::fs::remove_file(&tmp_path);
            // A troca salvaguarda por CÓPIA, nunca renomeio — `active_db` nunca chega a ser
            // tocado quando o rename final falha (ver `swap_active_db_atomically`). Só falta uma
            // conexão nova para o mesmo arquivo de sempre.
            let reopened = open_migrated_pool(db_path).await.map_err(|open_err| {
                format!("{swap_err}; adicionalmente falhou reabrir o banco: {open_err}")
            })?;
            return Ok(CheckoutResult {
                pool: reopened,
                outcome: Err(swap_err),
            });
        }
    };

    let new_pool = match reopen_after_swap_or_rollback(db_path, safeguard_path.as_deref()).await {
        Ok(ReopenOutcome::Reopened(p)) => p,
        // Reabrir o conteúdo recém-trocado falhou, mas HAVIA um banco funcional ANTES da troca —
        // reverter para ele automaticamente devolve um app que abre normalmente com o conteúdo de
        // ANTES desta tentativa, em vez de um app que não abre com uma salvaguarda ao lado que o
        // dono precisaria achar e restaurar à mão. O desfecho ainda avisa na UI (`outcome: Err`) —
        // a próxima abertura tenta de novo.
        Ok(ReopenOutcome::RolledBack { pool, message }) => {
            return Ok(CheckoutResult {
                pool,
                outcome: Err(message),
            });
        }
        // Sem salvaguarda para reverter (primeira restauração) OU a própria reversão também
        // falhou: nenhum pool utilizável sobra — a mesma falha fatal de sempre.
        Err(fatal) => return Err(fatal),
    };

    let checked_out_at = chrono::Utc::now().to_rfc3339();
    if let Err(e) = state::adopt_after_restore(
        &new_pool,
        &device_id,
        remote_sequence,
        &checked_out_at,
        &remote_device_id,
        last_checkin_at.as_deref(),
        last_checkin_device_id.as_deref(),
        &restored_export_sha256,
    )
    .await
    {
        return Ok(CheckoutResult {
            pool: new_pool,
            outcome: Err(e),
        });
    }

    Ok(CheckoutResult {
        pool: new_pool,
        outcome: Ok(CheckoutOutcome::Restored { safeguard_path }),
    })
}

/// Núcleo testável, SEM o teto de espera do boot (`checkout_on_open_with_deadline` é quem o
/// produção de verdade chama, compondo as mesmas duas fases com o teto restrito à primeira) —
/// `pool` já migrado no arquivo `db_path`, `drive` já autenticado. Devolve `Err` SÓ quando não
/// sobra pool utilizável nenhum — a troca de arquivo teve sucesso, mas reabrir uma conexão nela
/// falhou. Este é o mesmo tipo de falha fatal que a abertura inicial do banco já trata em
/// `lib.rs` (diálogo nativo + abort); fora desse caso extremo, o retorno é sempre
/// `Ok(CheckoutResult)` com um pool pronto para uso. Duas fases internas — ver o doc de
/// [`RestorePreparation`] para por que a costura entre elas importa para o teto de espera do boot.
#[cfg(test)]
async fn checkout_on_open(
    pool: SqlitePool,
    db_path: &Path,
    drive: &DriveSnapshotClient,
) -> Result<CheckoutResult, String> {
    match prepare_restore(pool, db_path, drive).await {
        RestorePreparation::Done(result) => Ok(result),
        RestorePreparation::ReadyToFinish(work) => finish_restore(work, db_path).await,
    }
}

/// Teto de espera do check-out no BOOT: `http.rs` já dá 10s de connect + 30s de
/// request × 3 tentativas por chamada, e `prepare_restore` faz duas chamadas sequenciais
/// (`fetch_manifest` + `download_snapshot`) — sem teto próprio, uma rede que engole pacotes
/// (portal cativo, VPN degradada) prende a abertura do app por dezenas de segundos a minutos,
/// porque isto roda dentro do `block_on` síncrono do `setup()` em `lib.rs`. 20s é folgado o
/// bastante para uma rede lenta de verdade responder ao manifest (tipicamente sub-segundo), mas
/// bem curto perto do pior caso de uma ÚNICA tentativa HTTP (10s de connect + 30s de request) —
/// ou seja, ao estourar o teto o app está esperando a REDE, GARANTIDO (não "quase sempre"): o
/// teto só envolve `prepare_restore`, que nunca fecha nem troca o arquivo — ver o doc de
/// [`RestorePreparation`] para a garantia completa. Residual conhecido: o refresh do token OAuth
/// (`resolve_drive_client_best_effort`, chamado ANTES deste teto) usa o mesmo cliente HTTP com
/// timeout próprio, mas não está coberto por ESTE teto — a URL do endpoint de refresh é fixa
/// (`oauth2.googleapis.com`), não injetável para apontar a um servidor de teste, então cobri-la
/// aqui exigiria um refactor maior que este follow-up não escopa.
const CHECKOUT_ON_OPEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Roda [`checkout_on_open`] com um teto de espera: ao estourar, devolve o MESMO pool que
/// recebeu (best-effort — "offline pleno" nunca pode travar a abertura do app esperando rede) com
/// `outcome: Err` visível na UI. Só a fase abortável (`prepare_restore`) entra sob o teto — ver o
/// doc de [`RestorePreparation`]. Extraído para ser testável com um `DriveSnapshotClient` que
/// aponta a um endpoint que nunca responde, sem depender do teto de PRODUÇÃO (lento demais para
/// um teste) — `deadline` é injetado pelo chamador.
async fn checkout_on_open_with_deadline(
    pool: SqlitePool,
    db_path: &Path,
    drive: &DriveSnapshotClient,
    deadline: Duration,
) -> Result<CheckoutResult, String> {
    let pool_fallback = pool.clone();
    // Só a fase 1 (`prepare_restore`) entra sob o teto — o trabalho de `ReadyToFinish` roda
    // SEMPRE até o fim, nunca abortado. Isto não é cosmético: o commit fecha `pool` (o MESMO
    // `Arc` compartilhado com `pool_fallback` acima) e, uma vez chamado, `SqlitePool::close()`
    // marca esse `Arc` inteiro como fechado — incluindo `pool_fallback`. Um teto que também
    // cobrisse essa fase devolveria um "pool de segurança" já morto sempre que o timeout
    // disparasse depois desse ponto — um boot rápido, mas com um pool inutilizável.
    match tokio::time::timeout(deadline, prepare_restore(pool, db_path, drive)).await {
        Ok(RestorePreparation::Done(result)) => Ok(result),
        Ok(RestorePreparation::ReadyToFinish(work)) => finish_restore(work, db_path).await,
        Err(_elapsed) => Ok(CheckoutResult {
            pool: pool_fallback,
            outcome: Err(format!(
                "check-out sem resposta do Drive em {}s — seguindo com o banco local; a próxima \
                 abertura tenta de novo",
                deadline.as_secs()
            )),
        }),
    }
}

/// O gancho de verdade que `lib.rs` chama na abertura do app: resolve o cliente do Drive via
/// [`resolve_drive_client_best_effort`] e SILENCIA os motivos legítimos de não tentar — nunca
/// conectou, sem client id configurado, escopo `drive.appdata` ainda não concedido. Nenhum desses
/// é uma falha do check-out em si, é "sync ainda não configurado" (offline pleno) — e sem nada
/// para tentar, nenhum aviso de uma tentativa ANTERIOR se sustenta (limpa junto). Uma falha
/// DEPOIS de decidir tentar (rede ao resolver o token, rede/integridade dentro de
/// `checkout_on_open`, ou o teto de espera acima estourando) continua reportada em `outcome`,
/// nunca engolida — só a decisão de TENTAR é best-effort, não o resultado da tentativa. Por isso
/// os três caminhos (não configurado, resolver falhou de verdade, `checkout_on_open_with_deadline`
/// rodou) convergem para o MESMO registro de desfecho abaixo, em vez de cada um sair cedo — a
/// diferença entre "nada a avisar" e "chegou a falhar" não pode depender de qual ramo lembrou de
/// gravar.
pub async fn checkout_on_open_best_effort(
    pool: SqlitePool,
    db_path: &Path,
    app_dir: &Path,
) -> Result<CheckoutResult, String> {
    let result = match resolve_drive_client_best_effort(&pool, app_dir).await {
        Ok(Some(drive)) => {
            checkout_on_open_with_deadline(pool, db_path, &drive, CHECKOUT_ON_OPEN_TIMEOUT).await?
        }
        Ok(None) => CheckoutResult {
            pool,
            outcome: Ok(CheckoutOutcome::NothingToDo),
        },
        Err(e) => CheckoutResult {
            pool,
            outcome: Err(e),
        },
    };

    // Persiste o desfecho para a UI de Conexão (ADR-0015): a recusa por schema mais
    // novo e a falha de rede/integridade/resolução do cliente merecem um aviso na tela, não só uma
    // linha de log que o dono nunca vê. `NothingToDo`/`Restored`/`CaughtUpOwnSequence` são sucesso
    // — limpam qualquer aviso de uma tentativa ANTERIOR, para ele não sobreviver a um check-out que
    // deu certo (ou que nem tinha o que tentar) depois. Melhor esforço: uma falha ao GRAVAR o
    // desfecho não pode derrubar o check-out em si, que já rodou até aqui — só loga e segue com o
    // `pool` do resultado.
    let (outcome_tag, outcome_detail) = outcome_warning_fields(&result.outcome);
    if let Err(e) = state::record_checkout_outcome(
        &result.pool,
        outcome_tag.as_deref(),
        outcome_detail.as_deref(),
    )
    .await
    {
        eprintln!("[snapshot/checkout] falha ao registrar o desfecho para a UI: {e}");
    }

    Ok(result)
}

/// Rótulo fechado gravado em `snapshot_state.last_checkout_outcome` pela sonda de FOCO (nunca por
/// `checkout_on_open`): remoto avançou além da base local, mas a sonda não baixa nem troca o
/// arquivo — só avisa. Mesma família de `outcome_warning_fields`, uma constante própria porque
/// nasce de um caminho diferente (nunca dentro de `CheckoutOutcome`, que é sempre resultado de uma
/// tentativa real de restauração).
pub const NEWER_SNAPSHOT_AVAILABLE_OUTCOME: &str = "newer_available";

/// Núcleo testável da sonda de FOCO (ADR-0015): consulta o manifest remoto e AVISA
/// quando ele avançou além da base local — mas NUNCA baixa nem troca o banco ativo. Diferente do
/// check-out no boot (`checkout_on_open`), aqui o pool já está `app.manage()`-do e em uso pelo app
/// inteiro; trocar o arquivo debaixo dele exigiria o mesmo "reinicie o app" que
/// `resolve_conflict_use_remote_core` já usa para a escolha explícita de conflito — fazer isso
/// silenciosamente a cada foco seria pior que só avisar e deixar o próximo reinício convergir de
/// verdade (o check-out do boot já faz isso sozinho).
///
/// A mesma guarda estreita do próprio `device_id` de `checkout_on_open` se aplica aqui: quando o
/// manifest remoto é o NOSSO check-in que morreu entre o upload confirmado e a gravação local
/// (`remote.sequence == base + 1`), não é uma versão mais nova de OUTRO aparelho — é segura de
/// adotar sem baixar nada, porque só atualiza bookkeeping local, nunca troca arquivo.
pub(crate) async fn probe_newer_snapshot_on_focus(
    pool: &SqlitePool,
    drive: &DriveSnapshotClient,
) -> Result<(), String> {
    let local_state = state::load_or_init(pool).await?;
    // Mesmo gate de `checkout_on_open` (ADR-0015): uma disputa ainda não resolvida pelo dono nunca
    // pode ser mexida por um gatilho automático — nem para restaurar, nem para só avisar.
    if local_state.conflict_pending_since.is_some() {
        return Ok(());
    }
    let remote = drive.fetch_manifest().await?;
    let verdict = lease::decide(
        local_state.base_sequence,
        local_state.base_sequence,
        remote.as_ref(),
    );
    if verdict != lease::LeaseVerdict::Pull {
        // Sem disputa nenhuma agora (a leitura é FRESCA) — qualquer aviso de uma sonda anterior
        // não se sustenta mais.
        return state::record_checkout_outcome(pool, None, None).await;
    }
    let remote_manifest =
        remote.expect("veredito Pull do árbitro implica manifest remoto presente");
    if remote_manifest.device_id == local_state.device_id
        && remote_manifest.sequence == local_state.base_sequence + 1
    {
        state::adopt_own_sequence(pool, remote_manifest.sequence).await?;
        return state::record_checkout_outcome(pool, None, None).await;
    }
    // Mesma checagem de `checkout_on_open`: um schema remoto mais novo nunca é "reabra o app e
    // pegue a versão nova" — o boot vai recusar de novo pelo mesmo motivo. Sem este ramo, a sonda
    // rebaixava o aviso correto do boot ("atualize o app") para uma instrução de reabrir que
    // nunca converge, prendendo o dono num loop de fechar/abrir.
    let local_schema = local_schema_version(pool).await?;
    if remote_manifest.schema_version > local_schema {
        return state::record_checkout_outcome(
            pool,
            Some("refused_newer_schema"),
            Some(&format!(
                "{local_schema}:{}",
                remote_manifest.schema_version
            )),
        )
        .await;
    }
    state::record_checkout_outcome(pool, Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME), None).await
}

/// Nunca sonda foco mais rápido que isto — o mesmo espírito de `sync_task::MIN_FOCUS_DEBOUNCE_SECS`
/// (evita uma rajada de chamadas ao Drive num alt-tab rápido), com uma chave própria porque o eixo
/// do snapshot é independente do probe da planilha.
const FOCUS_PROBE_DEBOUNCE_SECS: u64 = 60;
const LAST_FOCUS_PROBE_AT_KEY: &str = "snapshot_last_focus_probe_at";

/// O gancho de verdade que `lib.rs` chama quando a janela ganha foco: debounce próprio (mesma
/// cadência do probe de foco da planilha) e resolve o cliente do Drive via
/// [`resolve_drive_client_best_effort`] — qualquer motivo de não tentar (nunca conectou, sem
/// escopo) é silencioso. Uma falha DEPOIS de decidir tentar (rede, integridade) é logada pelo
/// chamador, nunca engolida aqui.
pub async fn probe_newer_snapshot_on_focus_best_effort(
    pool: &SqlitePool,
    app_dir: &Path,
) -> Result<(), String> {
    let now = crate::sync_task::now_unix();
    if let Some(raw) = crate::commands::app_setting_get(pool, LAST_FOCUS_PROBE_AT_KEY).await?
        && let Ok(last) = raw.trim().parse::<u64>()
        && now.saturating_sub(last) < FOCUS_PROBE_DEBOUNCE_SECS
    {
        return Ok(());
    }
    crate::commands::app_setting_set(pool, LAST_FOCUS_PROBE_AT_KEY, &now.to_string()).await?;

    let Some(drive) = resolve_drive_client_best_effort(pool, app_dir).await? else {
        return Ok(());
    };
    probe_newer_snapshot_on_focus(pool, &drive).await
}

/// Mapeia o desfecho do check-out para o rótulo fechado que `snapshot_state.last_checkout_outcome`
/// grava — só os dois casos que a UI de Conexão precisa avisar (`CHECKIN`/`RefusedNewerSchema` tem
/// copy própria já visível; `NothingToDo`/`Restored`/`CaughtUpOwnSequence` não precisam de aviso,
/// então limpam qualquer um pendente de uma tentativa anterior). Função pura, testável sem rede.
fn outcome_warning_fields(
    outcome: &Result<CheckoutOutcome, String>,
) -> (Option<String>, Option<String>) {
    match outcome {
        Ok(CheckoutOutcome::RefusedNewerSchema {
            local_schema,
            remote_schema,
        }) => (
            Some("refused_newer_schema".to_string()),
            Some(format!("{local_schema}:{remote_schema}")),
        ),
        Err(e) => (Some("error".to_string()), Some(e.clone())),
        Ok(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::token_store::StoredToken;
    use crate::snapshot::manifest::SnapshotManifest;
    use std::time::Duration;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-checkout-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn test_pool(db_path: &Path) -> SqlitePool {
        open_migrated_pool(db_path).await.expect("pool de teste")
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
    async fn nothing_to_do_when_remote_has_no_manifest_published_yet() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));

        // O pool devolvido continua o MESMO banco, utilizável.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn nothing_to_do_when_remote_sequence_matches_local_base() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(&pool, 3, "2026-08-11T10:00:00Z", &local.device_id, "hash")
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

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn manifest_fetch_failure_leaves_the_original_pool_untouched_and_reports_the_error() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(500)
            .with_body(r#"{"error": {"message": "backend hiccup"}}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("falha de rede não é fatal — pool original continua utilizável");
        let err = result.outcome.unwrap_err();
        assert!(err.contains("backend hiccup"), "erro: {err}");

        // Pool intacto: a leitura funciona normalmente.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn refuses_restore_when_remote_schema_is_newer_than_local_and_changes_nothing() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-11T10:00:00Z".into(),
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

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("recusa por schema não é um erro fatal");
        assert_eq!(
            result.outcome,
            Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema,
                remote_schema: local_schema + 1000,
            })
        );

        // Nada mudou: mesma base, mesmo device_id de antes.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Cria um segundo banco (o "remoto" baixado), migrado e com um marcador em `app_setting`
    /// que não existe no banco local — o jeito de provar, depois da restauração, que o conteúdo
    /// ativo é mesmo o do remoto e não uma cópia do que já estava aqui.
    async fn build_remote_db_bytes(dir: &Path, marker: &str) -> Vec<u8> {
        let remote_path = dir.join(format!("remote-source-{}.db", uuid::Uuid::new_v4()));
        let remote_pool = open_migrated_pool(&remote_path).await.unwrap();
        crate::commands::app_setting_set(&remote_pool, "restore_marker", marker)
            .await
            .unwrap();
        // Espelha `strip_from_export_copy`: o snapshot publicado nunca carrega a identidade de
        // quem publicou.
        sqlx::query("DELETE FROM snapshot_state")
            .execute(&remote_pool)
            .await
            .unwrap();
        remote_pool.close().await;
        std::fs::read(&remote_path).unwrap()
    }

    #[tokio::test]
    async fn restores_the_active_db_when_remote_advanced_and_schema_is_compatible() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();
        crate::commands::app_setting_set(&pool, "local_only_marker", "presente-antes")
            .await
            .unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto").await;

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

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("restauração deve suceder");
        let outcome = result.outcome.expect("nenhum erro esperado");
        match outcome {
            CheckoutOutcome::Restored { safeguard_path } => {
                assert!(
                    safeguard_path.is_some(),
                    "havia banco ativo antes — deve gerar salvaguarda"
                );
                assert!(safeguard_path.unwrap().exists());
            }
            other => panic!("esperava Restored, veio {other:?}"),
        }

        // O conteúdo ativo agora é o do remoto...
        let marker = crate::commands::app_setting_get(&result.pool, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-remoto"));
        // ...e o que só existia no local ANTES da troca sumiu (o remoto o substituiu de verdade).
        let local_only = crate::commands::app_setting_get(&result.pool, "local_only_marker")
            .await
            .unwrap();
        assert!(local_only.is_none());

        // A identidade DESTE aparelho sobrevive à troca — nunca é regerada.
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.device_id, local_before.device_id);
        assert_eq!(state_after.base_sequence, 9);
        assert_eq!(
            state_after.last_checkout_device_id.as_deref(),
            Some("outro-aparelho")
        );
        assert!(state_after.last_checkout_at.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_on_open_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        // Mesma classe de regressão já documentada em `state.rs`/`db_export.rs`: com pool de 1
        // conexão, o check-out lê `snapshot_state` (via `state::load_or_init`) como primeiro
        // passo — enquanto outra transação de escrita segura a única conexão, essa leitura
        // precisa ENFILEIRAR e completar assim que a tx solta a conexão, nunca travar para
        // sempre. `download_snapshot`/`restore` também competiriam pela mesma conexão única se
        // chegassem a rodar, mas aqui a ausência de manifest remoto interrompe o fluxo antes
        // disso — o que importa é a leitura inicial não travar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let checkout = tokio::spawn(async move { checkout_on_open(pool, &db_path, &drive).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(Duration::from_secs(5), checkout)
            .await
            .expect("checkout_on_open NÃO pode travar para sempre esperando a única conexão")
            .expect("task não deve entrar em panic")
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn best_effort_is_a_silent_no_op_when_nothing_was_ever_configured() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        // `app_dir` sem token nenhum e sem `sheets_client_id`/`GOOGLE_CLIENT_ID`: nunca conectou.
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();

        let result = checkout_on_open_best_effort(pool, &db_path, &app_dir)
            .await
            .expect("best-effort nunca falha quando não há como tentar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn best_effort_clears_a_pending_warning_when_boot_has_nothing_configured_to_retry() {
        // Item 5 da issue #446: sem client id configurado não existe check-out para "tentar de
        // novo na próxima abertura" — um aviso de uma tentativa ANTERIOR não pode sobreviver a um
        // boot que nem chega a decidir tentar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        state::load_or_init(&pool).await.unwrap();
        state::record_checkout_outcome(&pool, Some("error"), Some("tentativa anterior"))
            .await
            .unwrap();
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();

        checkout_on_open_best_effort(pool.clone(), &db_path, &app_dir)
            .await
            .expect("best-effort nunca falha quando não há como tentar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(after.last_checkout_outcome, None);
        assert_eq!(after.last_checkout_outcome_detail, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn best_effort_persists_an_error_outcome_when_the_client_resolver_really_fails() {
        // Diferente de "nunca configurado" (silencioso): um token PRESENTE mas ILEGÍVEL é uma
        // tentativa que decidiu tentar e falhou de verdade — a UI de Conexão precisa saber disso,
        // não só um `eprintln!` que o dono nunca vê.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        // A linha singleton já existe (bookkeeping de um boot anterior) — o mesmo pressuposto
        // realista do teste do aviso pendente acima: um aparelho recém-instalado, sem NENHUM
        // bookkeeping prévio, não tem como já ter um client id + token corrompido configurados.
        state::load_or_init(&pool).await.unwrap();
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste")
            .await
            .unwrap();
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(
            app_dir.join("oauth-token.enc"),
            b"nao sou um token cifrado valido",
        )
        .unwrap();

        let result = checkout_on_open_best_effort(pool.clone(), &db_path, &app_dir)
            .await
            .expect("best-effort não propaga a falha do resolver como Err do próprio fn");
        assert!(
            result.outcome.is_err(),
            "token ilegível é uma falha real, não 'nada a fazer'"
        );

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(after.last_checkout_outcome.as_deref(), Some("error"));
        assert!(after.last_checkout_outcome_detail.is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Defeito 1: histórico de check-in sobrevive à troca (regressão de ponta a ponta) -------

    #[tokio::test]
    async fn restore_preserves_this_devices_own_checkin_history_across_the_swap() {
        // O aparelho publicou antes (check-in próprio registrado) e AGORA recebe o snapshot de
        // OUTRO aparelho — o histórico de check-in DESTE aparelho é bookkeeping local, não dado
        // do snapshot baixado (que chega com `snapshot_state` vazio), e precisa sobreviver.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            3,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto-2").await;

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

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("restauração deve suceder");
        assert!(matches!(
            result.outcome,
            Ok(CheckoutOutcome::Restored { .. })
        ));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(
            state_after.last_checkin_at.as_deref(),
            Some("2026-08-10T09:00:00Z"),
            "o check-out apagava o histórico de check-in deste aparelho — a tela voltava a dizer \
             'nenhum check-in ainda' para um aparelho que já publicou"
        );
        assert_eq!(
            state_after.last_checkin_device_id.as_deref(),
            Some(local_before.device_id.as_str())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Defeito 2: desfecho do check-out fica visível na tela, não só no log ------------------

    #[test]
    fn outcome_warning_fields_maps_only_the_two_outcomes_the_screen_needs_to_warn_about() {
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::RefusedNewerSchema {
                local_schema: 5,
                remote_schema: 8,
            })),
            (
                Some("refused_newer_schema".to_string()),
                Some("5:8".to_string())
            )
        );
        assert_eq!(
            outcome_warning_fields(&Err("timeout de rede".to_string())),
            (
                Some("error".to_string()),
                Some("timeout de rede".to_string())
            )
        );
        // Sucesso silencioso: nada a avisar, limpa qualquer aviso pendente.
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::NothingToDo)),
            (None, None)
        );
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::Restored {
                safeguard_path: None
            })),
            (None, None)
        );
        assert_eq!(
            outcome_warning_fields(&Ok(CheckoutOutcome::CaughtUpOwnSequence { sequence: 3 })),
            (None, None)
        );
    }

    #[tokio::test]
    async fn refused_newer_schema_outcome_is_persisted_and_visible_through_last_drive_checkin() {
        // Atravessa a costura backend↔tela de ponta a ponta: roda o check-out de verdade, grava
        // o desfecho pelo MESMO caminho que `checkout_on_open_best_effort` usa, e lê de volta
        // pelo comando REAL que a tela chama (`last_drive_checkin_core`) — não uma reconstrução
        // à mão do formato esperado.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-12T10:00:00Z".into(),
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
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("recusa por schema não é um erro fatal");
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .expect("gravar desfecho");

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .expect("ler pelo comando real que a tela chama");
        assert_eq!(
            info.last_checkout_outcome.as_deref(),
            Some("refused_newer_schema")
        );
        assert_eq!(
            info.last_checkout_outcome_detail.as_deref(),
            Some(format!("{local_schema}:{}", local_schema + 1000).as_str())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn network_failure_outcome_is_persisted_and_visible_through_last_drive_checkin() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(500)
            .with_body(r#"{"error": {"message": "backend hiccup"}}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("falha de rede não é fatal");
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .expect("gravar desfecho");

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .expect("ler pelo comando real que a tela chama");
        assert_eq!(info.last_checkout_outcome.as_deref(), Some("error"));
        assert!(
            info.last_checkout_outcome_detail
                .as_deref()
                .unwrap()
                .contains("backend hiccup"),
            "detalhe: {:?}",
            info.last_checkout_outcome_detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_outcome_warning_is_cleared_by_a_later_successful_checkout() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        state::record_checkout_outcome(&pool, Some("error"), Some("tentativa anterior"))
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive).await.unwrap();
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));
        let (tag, detail) = outcome_warning_fields(&result.outcome);
        state::record_checkout_outcome(&result.pool, tag.as_deref(), detail.as_deref())
            .await
            .unwrap();

        let info = crate::commands::snapshot_cmds::last_drive_checkin_core(&result.pool)
            .await
            .unwrap();
        assert!(
            info.last_checkout_outcome.is_none(),
            "um check-out bem-sucedido depois limpa o aviso da tentativa anterior"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Defeito 3: nunca restaura o próprio snapshot por cima de trabalho posterior -----------

    #[tokio::test]
    async fn adopts_the_remote_sequence_without_restoring_when_the_manifest_is_our_own_device() {
        // O remoto avançou (sequência acima da nossa base), mas com o NOSSO PRÓPRIO device_id —
        // um check-in cujo upload confirmou mas cuja gravação local morreu antes de terminar.
        // Restaurar de verdade baixaria e trocaria pelo NOSSO PRÓPRIO snapshot antigo, descartando
        // qualquer gesto feito depois daquele upload. Nenhum mock de download é registrado: se o
        // código tentasse baixar mesmo assim, a chamada não-mockada devolveria 501 e o teste
        // acusaria a diferença.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        crate::commands::app_setting_set(
            &pool,
            "local_only_marker",
            "trabalho-posterior-ao-upload",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 1,
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
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("adotar a própria sequência não é um erro");
        assert_eq!(
            result.outcome,
            Ok(CheckoutOutcome::CaughtUpOwnSequence { sequence: 1 })
        );

        // Conteúdo local INTOCADO: nada foi baixado nem trocado.
        let marker = crate::commands::app_setting_get(&result.pool, "local_only_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("trabalho-posterior-ao-upload"));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.device_id, local_before.device_id);
        assert_eq!(
            state_after.base_sequence, 1,
            "a base local alcança a sequência remota mesmo sem restaurar"
        );
        assert!(
            state_after.last_checkout_at.is_none(),
            "nada foi de fato lido de outro aparelho — o eixo de check-out não muda"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restores_normally_when_the_own_device_id_sequence_is_not_exactly_base_plus_one() {
        // Duas instalações podem compartilhar `device_id` por um caminho lateral (cópia manual
        // da pasta do app; backup local restaurado à mão, que não passa pelo `strip` do export) —
        // aí o manifest com o NOSSO id não é necessariamente o check-in morto entre upload e
        // gravação: pode ser o conteúdo de OUTRA instalação com a mesma identidade, várias
        // sequências à frente. `remote.sequence == base + 1` é a única janela estreita o
        // suficiente para presumir "sou eu mesmo, upload confirmado" — qualquer coisa além disso
        // precisa passar pela restauração normal (com barulho visível), nunca ser adotada às
        // cegas.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            5,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();
        let local_schema = local_schema_version(&pool).await.unwrap();
        crate::commands::app_setting_set(
            &pool,
            "local_only_marker",
            "trabalho-que-nao-pode-ser-descartado-as-cegas",
        )
        .await
        .unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto-mesmo-device-id").await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 8, // base(5) + 3: fora da janela upload→gravação (base + 1 = 6).
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

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("restauração deve suceder");
        let outcome = result.outcome.expect("nenhum erro esperado");
        match outcome {
            CheckoutOutcome::Restored { safeguard_path } => {
                assert!(safeguard_path.is_some());
            }
            other => panic!("esperava Restored (fora da janela base+1), veio {other:?}"),
        }

        let marker = crate::commands::app_setting_get(&result.pool, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-remoto-mesmo-device-id"));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 8);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn own_device_id_with_regressed_remote_sequence_follows_the_arbiter_verdict() {
        // Sequência remota abaixo da base local, mesmo com o NOSSO device_id: o árbitro
        // (`lease::decide`) já resolve isso como `Push` bem antes da guarda do próprio id ser
        // consultada — nada aqui é mais novo que a base para disputar, então não há o que
        // restaurar nem o que adotar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();
        state::record_checkin(
            &pool,
            7,
            "2026-08-10T09:00:00Z",
            &local_before.device_id,
            "hash-publicado-antes",
        )
        .await
        .unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 3, // < base (7): regredido.
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
        // Nenhum mock de download: veredito `Push` nunca chega perto de baixar nada.
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 7, "base local não regride");
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- probe_newer_snapshot_on_focus -----------------------------------------------------

    #[tokio::test]
    async fn focus_probe_flags_newer_available_without_downloading_when_remote_advanced() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
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
        // Nenhum mock de download do snapshot: a sonda de foco NUNCA baixa/troca o arquivo —
        // só avisa. Uma tentativa de download bateria numa rota não-mockada (501).
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(
            after.last_checkout_outcome.as_deref(),
            Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME)
        );
        assert_eq!(
            after.base_sequence, 0,
            "a sonda de foco nunca adota/avança a base de outro aparelho — só o boot restaura"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_records_refused_newer_schema_instead_of_newer_available_when_remote_schema_is_newer()
     {
        // Cenário do reinício em loop: o boot recusou por schema mais novo (aviso correto), e a
        // sonda de foco seguinte lê o MESMO manifest de novo. Sem checar schema, ela rebaixava o
        // desfecho para "newer_available" — que instrui reabrir o app, o que nunca resolve nada
        // porque o schema remoto continua incompatível. A sonda precisa gravar o MESMO desfecho
        // que `checkout_on_open` grava para este caso: `refused_newer_schema`.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: "outro-aparelho".into(),
            sequence: 1,
            created_at: "2026-08-13T09:00:00Z".into(),
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
        // Nenhum mock de download do snapshot: a sonda de foco NUNCA baixa/troca o arquivo — só
        // avisa, mesmo quando o aviso é a recusa por schema.
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(
            after.last_checkout_outcome.as_deref(),
            Some("refused_newer_schema"),
            "schema incompatível nunca pode virar \"newer_available\" — a instrução de reabrir \
             o app não resolve incompatibilidade de schema e prende o dono num loop"
        );
        assert_eq!(
            after.last_checkout_outcome_detail.as_deref(),
            Some(format!("{local_schema}:{}", local_schema + 1000).as_str())
        );
        assert_eq!(
            after.base_sequence, 0,
            "a sonda de foco nunca adota/avança a base de outro aparelho — só o boot restaura"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_adopts_own_sequence_silently_without_flagging_newer_available() {
        // Mesma janela estreita de `checkout_on_open` (ADR-0015): o manifest remoto é o NOSSO
        // check-in que morreu entre o upload confirmado e a gravação local — seguro adotar sem
        // baixar nada, e não é "uma versão mais nova" de outro aparelho para avisar.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_before = state::load_or_init(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;
        let manifest_json = serde_json::to_string(&SnapshotManifest {
            device_id: local_before.device_id.clone(),
            sequence: 1,
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
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert_eq!(after.base_sequence, 1, "a base alcança a própria sequência");
        assert!(
            after.last_checkout_outcome.is_none(),
            "conteúdo já é nosso — não é aviso de versão mais nova de outro aparelho"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_clears_a_stale_warning_when_the_fresh_read_is_up_to_date() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        // Aviso de uma sonda ANTERIOR, que a leitura fresca de agora precisa derrubar.
        state::record_checkout_outcome(&pool, Some(NEWER_SNAPSHOT_AVAILABLE_OUTCOME), None)
            .await
            .unwrap();

        let mut server = mockito::Server::new_async().await;
        // Nenhum snapshot publicado ainda: remoto ausente, base em 0 — `UpToDate`.
        server
            .mock("GET", "/drive/v3/files")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"files": []}"#)
            .create_async()
            .await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert!(after.last_checkout_outcome.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_on_open_refuses_to_restore_while_a_conflict_is_pending() {
        // ADR-0015: uma disputa descoberta por um check-in e ainda não resolvida pelo dono nunca
        // pode ser sobrescrita em silêncio por um check-out — nem no boot. Sem este gate, fechar o
        // app com o conflito pendente e reabrir mais tarde (remoto tendo avançado ainda mais)
        // bateria em `Pull` e restauraria por cima do lado local da disputa.
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        state::load_or_init(&pool).await.unwrap();
        state::record_conflict_pending(&pool, Some("2026-08-13T09:00:00Z"))
            .await
            .unwrap();

        // Nenhum mock registrado: se o gate não interceptasse ANTES de consultar o remoto, a
        // chamada bateria numa rota não-mockada e o teste acusaria a diferença.
        let server = mockito::Server::new_async().await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        let result = checkout_on_open(pool, &db_path, &drive)
            .await
            .expect("checkout_on_open não deve falhar");
        assert_eq!(result.outcome, Ok(CheckoutOutcome::NothingToDo));

        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0, "nada foi restaurado");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_is_a_silent_no_op_while_a_conflict_is_pending() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        state::load_or_init(&pool).await.unwrap();
        state::record_conflict_pending(&pool, Some("2026-08-13T09:00:00Z"))
            .await
            .unwrap();

        // Nenhum mock registrado: o gate precisa interceptar antes de qualquer chamada ao Drive.
        let server = mockito::Server::new_async().await;
        let drive = DriveSnapshotClient::new(token(), server.url());

        probe_newer_snapshot_on_focus(&pool, &drive)
            .await
            .expect("sonda de foco não deve falhar");

        let after = state::load_or_init(&pool).await.unwrap();
        assert!(
            after.last_checkout_outcome.is_none(),
            "conflito pendente: a sonda não deve nem tentar avisar"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn focus_probe_best_effort_is_debounced() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste-inexistente")
            .await
            .unwrap();
        // Marca uma sonda como tendo rodado AGORA — a próxima chamada, dentro do intervalo de
        // debounce, precisa ser um no-op sem sequer tentar resolver token/rede.
        let now = crate::sync_task::now_unix();
        crate::commands::app_setting_set(&pool, LAST_FOCUS_PROBE_AT_KEY, &now.to_string())
            .await
            .unwrap();

        // Sem token no keyring de teste: se o debounce NÃO interceptasse, a tentativa de resolver
        // o cliente falharia (silenciosamente) do mesmo jeito — então a asserção observável é a
        // marca do debounce, que só o CAMINHO QUE PASSOU pelo debounce escreve de novo.
        probe_newer_snapshot_on_focus_best_effort(&pool, &dir)
            .await
            .expect("sonda de foco não deve falhar");

        let raw = crate::commands::app_setting_get(&pool, LAST_FOCUS_PROBE_AT_KEY)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            raw,
            now.to_string(),
            "dentro do debounce: a marca não deve avançar"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Teto de espera do check-out no boot -----------------------------------------------------

    #[tokio::test]
    async fn checkout_on_open_with_deadline_gives_up_and_keeps_the_original_pool_usable_when_the_network_hangs()
     {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;

        // Listener TCP que aceita a conexão e nunca responde — simula uma rede que engole pacotes
        // (portal cativo, VPN degradada): o handshake TCP conclui, mas a resposta HTTP nunca
        // chega. O request ficaria pendurado até o timeout de 30s de `http.rs`; o teto injetado
        // abaixo (200ms) precisa vencer bem antes disso.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener de teste");
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                std::mem::forget(stream);
            }
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        });
        let drive = DriveSnapshotClient::new(token(), format!("http://{addr}"));

        let result =
            checkout_on_open_with_deadline(pool, &db_path, &drive, Duration::from_millis(200))
                .await
                .expect(
                    "o teto de espera nunca é uma falha fatal — sempre devolve um pool utilizável",
                );

        let err = result.outcome.unwrap_err();
        assert!(err.contains("check-out"), "erro: {err}");

        // O pool devolvido continua o MESMO banco, utilizável — a tentativa abandonada nunca
        // chegou perto de fechá-lo (ainda estava esperando a resposta do manifest).
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn checkout_on_open_with_deadline_restores_normally_when_the_deadline_is_never_reached() {
        // O commit (fechar o pool antigo, trocar o arquivo, reabrir) precisa rodar até o FIM
        // mesmo passando pelo wrapper com teto — ele só embrulha a fase 1 (`prepare_restore`).
        // Espelha
        // `restores_the_active_db_when_remote_advanced_and_schema_is_compatible`, mas atravessa
        // `checkout_on_open_with_deadline` em vez de `checkout_on_open` direto, com um teto folgado
        // que nunca deveria disparar — prova que a composição prepare→commit por trás do teto
        // produz o MESMO resultado (pool novo, utilizável, conteúdo do remoto).
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let local_schema = local_schema_version(&pool).await.unwrap();

        let remote_bytes = build_remote_db_bytes(&dir, "veio-do-remoto-via-deadline").await;

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

        let result = checkout_on_open_with_deadline(pool, &db_path, &drive, Duration::from_secs(5))
            .await
            .expect("restauração deve suceder");
        assert!(matches!(
            result.outcome,
            Ok(CheckoutOutcome::Restored { .. })
        ));

        // O pool devolvido é genuinamente utilizável (não fechado/poluído pelo wrapper de teto) e
        // reflete o conteúdo do remoto.
        let marker = crate::commands::app_setting_get(&result.pool, "restore_marker")
            .await
            .unwrap();
        assert_eq!(marker.as_deref(), Some("veio-do-remoto-via-deadline"));
        let state_after = state::load_or_init(&result.pool).await.unwrap();
        assert_eq!(state_after.base_sequence, 9);
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Reversão automática quando reabrir depois da troca falha ---------------------------------

    #[tokio::test]
    async fn reopen_after_swap_or_rollback_reopens_normally_when_the_new_content_opens_fine() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        // Grava um banco migrado válido em `db_path` (simula a troca já ter colocado um conteúdo
        // são no lugar) e descarta o pool — só o arquivo importa daqui em diante.
        test_pool(&db_path).await.close().await;

        let outcome = reopen_after_swap_or_rollback(&db_path, None)
            .await
            .expect("reabrir um conteúdo válido nunca falha");
        match outcome {
            ReopenOutcome::Reopened(_) => {}
            other => panic!("esperava Reopened, veio {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn reopen_after_swap_or_rollback_falls_back_to_the_safeguard_when_the_new_content_wont_open()
     {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let safeguard_path = dir.join("neko-finance.pre-restore-test.db");
        {
            let safeguard_pool = test_pool(&safeguard_path).await;
            crate::commands::app_setting_set(
                &safeguard_pool,
                "safeguard_marker",
                "conteudo-de-antes",
            )
            .await
            .unwrap();
            safeguard_pool.close().await;
        }
        // `db_path`: o "conteúdo recém-trocado" que não abre — bytes que não são um SQLite válido
        // (simula I/O transitório ou disco cheio na migração da reabertura).
        std::fs::write(&db_path, b"nao sou um sqlite valido").unwrap();

        let outcome = reopen_after_swap_or_rollback(&db_path, Some(safeguard_path.as_path()))
            .await
            .expect("reversão automática deve suceder quando a salvaguarda existe");
        match outcome {
            ReopenOutcome::RolledBack { pool, message } => {
                assert!(message.contains("revertido"), "mensagem: {message}");
                let marker = crate::commands::app_setting_get(&pool, "safeguard_marker")
                    .await
                    .unwrap();
                assert_eq!(
                    marker.as_deref(),
                    Some("conteudo-de-antes"),
                    "o banco ativo precisa voltar a ser o conteúdo de ANTES da troca, não ficar \
                     preso no conteúdo quebrado que não abre"
                );
            }
            other => panic!("esperava RolledBack, veio {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn reopen_after_swap_or_rollback_is_fatal_without_a_safeguard_to_fall_back_to() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        std::fs::write(&db_path, b"nao sou um sqlite valido").unwrap();

        let err = reopen_after_swap_or_rollback(&db_path, None)
            .await
            .expect_err("primeira restauração sem salvaguarda: nada para reverter, falha fatal");
        assert!(
            err.contains("reabrir banco depois da restauração"),
            "erro: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn reopen_after_swap_or_rollback_is_fatal_when_the_safeguard_itself_is_also_broken() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let safeguard_path = dir.join("neko-finance.pre-restore-test.db");
        std::fs::write(&db_path, b"nao sou um sqlite valido").unwrap();
        std::fs::write(&safeguard_path, b"a salvaguarda tambem nao abre").unwrap();

        let err = reopen_after_swap_or_rollback(&db_path, Some(safeguard_path.as_path()))
            .await
            .expect_err("reversão também quebrada: nenhum pool utilizável sobra, falha fatal");
        assert!(
            err.contains("reversão automática também falhou"),
            "erro: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Falha real depois de decidir tentar não pode virar NothingToDo ---------------------------

    #[tokio::test]
    async fn resolve_drive_client_best_effort_is_silently_none_without_a_configured_client_id() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();

        let result = resolve_drive_client_best_effort(&pool, &app_dir).await;
        assert!(matches!(result, Ok(None)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_drive_client_best_effort_is_silently_none_when_never_authenticated() {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste")
            .await
            .unwrap();
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();
        // client id configurado, mas NENHUM token guardado — "nunca conectou" continua
        // silencioso, a mesma classe de "não configurado" da falta de client id.

        let result = resolve_drive_client_best_effort(&pool, &app_dir).await;
        assert!(matches!(result, Ok(None)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_drive_client_best_effort_reports_a_real_error_when_the_stored_token_is_unreadable()
     {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let pool = test_pool(&db_path).await;
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste")
            .await
            .unwrap();
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();
        // Um token "existe" (arquivo cifrado presente), mas é ilegível — corrupção, não "nunca
        // conectou": bytes que não formam um ciphertext válido para o esquema de `secret_file`.
        std::fs::write(
            app_dir.join("oauth-token.enc"),
            b"nao sou um token cifrado valido",
        )
        .unwrap();

        let result = resolve_drive_client_best_effort(&pool, &app_dir).await;
        assert!(
            result.is_err(),
            "token presente mas ilegível é uma tentativa que FALHOU, não 'nunca configurado' — \
             não pode desaparecer como NothingToDo"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_drive_client_best_effort_is_silently_none_when_the_token_lacks_the_drive_scope()
     {
        let dir = test_dir();
        let db_path = dir.join("neko-finance.db");
        let app_dir = dir.join("app-dir");
        std::fs::create_dir_all(&app_dir).unwrap();

        {
            // Serializado com os testes de `token_store` que também usam o fallback de arquivo —
            // mesmo lock global, mesmo motivo (evitar corrida na env var entre threads de teste).
            // Escopo restrito ao trecho SÍNCRONO: nunca atravessa um `.await` (clippy reprova um
            // `std::sync::Mutex` mantido através de um ponto de suspensão).
            let _guard = crate::secret_vault::INSECURE_FILE_FALLBACK_LOCK
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            // SAFETY: serializado pelo guard acima.
            unsafe { std::env::set_var("NEKO_INSECURE_FILE_FALLBACK", "1") };
            let stored = crate::oauth::token_store::StoredToken {
                access_token: "ya29.test".into(),
                refresh_token: "1//test".into(),
                expires_at: 9_999_999_999, // não expira — nenhuma chamada de rede é feita
                scope: "https://www.googleapis.com/auth/spreadsheets".into(), // sem drive.appdata
            };
            crate::oauth::token_store::store_token(&app_dir, &stored).unwrap();
            // SAFETY: serializado pelo guard acima.
            unsafe { std::env::remove_var("NEKO_INSECURE_FILE_FALLBACK") };
        }

        let pool = test_pool(&db_path).await;
        crate::commands::app_setting_set(&pool, "sheets_client_id", "client-de-teste")
            .await
            .unwrap();

        let result = resolve_drive_client_best_effort(&pool, &app_dir).await;
        // Escopo `drive.appdata` ainda não concedido é "ainda não migrou para o re-consentimento"
        // — a mesma classe de "não configurado" da falta de client id/token, nunca uma falha de
        // tentativa (a spec espera o re-consentimento único, não um erro a cada boot).
        assert!(matches!(result, Ok(None)));
        std::fs::remove_dir_all(&dir).ok();
    }
}
