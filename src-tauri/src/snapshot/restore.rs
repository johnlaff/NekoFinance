//! Restauração atômica do snapshot baixado do Drive — o espelho de `db_export::vacuum_into_atomic`
//! (ADR-0015): valida o arquivo baixado, salvaguarda o banco ativo, e troca por renomeio atômico.
//! Qualquer falha no meio deixa o banco ativo EXATAMENTE como estava antes — nunca parcial.

use std::path::{Path, PathBuf};

/// Valida que `path` é um arquivo SQLite íntegro ANTES de qualquer troca: o header mágico
/// (`SQLite format 3\0`) barra lixo evidente e barato de detectar; `PRAGMA integrity_check`
/// (aberto `read_only`, nunca escreve no arquivo baixado) pega truncamento/corrupção que só
/// aparece ao ler as páginas — um download interrompido pode ter o header certo e o resto vazio.
pub(crate) async fn validate_downloaded_db(path: &Path) -> Result<(), String> {
    let mut header = [0u8; 16];
    {
        let mut f =
            std::fs::File::open(path).map_err(|e| format!("abrir snapshot baixado: {e}"))?;
        std::io::Read::read_exact(&mut f, &mut header)
            .map_err(|_| "snapshot baixado não é um arquivo SQLite válido".to_string())?;
    }
    if &header != b"SQLite format 3\0" {
        return Err("snapshot baixado não é um arquivo SQLite válido".into());
    }

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
        .map_err(|e| format!("abrir snapshot baixado: {e}"))?
        .read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| format!("abrir snapshot baixado: {e}"))?;
    let check: Result<String, _> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await;
    pool.close().await;

    match check {
        Ok(ref ok) if ok == "ok" => Ok(()),
        Ok(problem) => Err(format!(
            "snapshot baixado falhou na verificação de integridade: {problem}"
        )),
        Err(e) => Err(format!("verificar integridade do snapshot baixado: {e}")),
    }
}

/// Grava `bytes` no `tmp_path` e valida a integridade antes de qualquer troca — o par de passos
/// que `checkout_on_open` roda ANTES do ponto de não-retorno (fechar o pool antigo). Limpa o
/// temporário em QUALQUER falha deste par, gravação ou validação: um download que falha no meio
/// (disco cheio, permissão) pode deixar bytes parciais no `tmp_path`, e sem a limpeza aqui esse
/// arquivo fica para sempre — a mesma classe de lixo que `swap_active_db_atomically` evita do
/// lado da salvaguarda.
pub(crate) async fn stage_downloaded_snapshot(tmp_path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Err(e) = tokio::fs::write(tmp_path, bytes).await {
        let _ = tokio::fs::remove_file(tmp_path).await;
        return Err(format!("gravar snapshot baixado: {e}"));
    }
    if let Err(e) = validate_downloaded_db(tmp_path).await {
        let _ = std::fs::remove_file(tmp_path);
        return Err(e);
    }
    Ok(())
}

/// Anexa `suffix` ao PATH INTEIRO (não à extensão) — a convenção real dos sidecars WAL/SHM do
/// SQLite é `<caminho-completo-do-db>-wal` / `-shm`, nunca baseada em `file_stem`.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

/// Cria a salvaguarda do banco ativo por CÓPIA (nunca renomeio) e devolve o caminho, ou `None`
/// quando não existe banco ativo para salvaguardar. Extraída de `swap_active_db_atomically` para
/// ser exercitável ISOLADAMENTE: um teste que chama só ESTE passo e verifica `active_db.exists()`
/// prova a garantia central do ADR-0015 (o banco ativo nunca fica ausente) de um jeito que nenhum
/// teste da função inteira, que só olha o desfecho FINAL do fluxo completo (sucesso ou erro),
/// consegue provar — um rollback bem escrito pode restaurar `active_db` no final mesmo que a
/// implementação deste passo o tenha feito desaparecer no meio do caminho.
fn create_safeguard_copy(active_db: &Path) -> Result<Option<PathBuf>, String> {
    if !active_db.exists() {
        return Ok(None);
    }
    let parent = active_db.parent().unwrap_or_else(|| Path::new("."));
    let stem = active_db
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("neko-finance");
    let safeguard = parent.join(format!("{stem}.pre-restore-{}.db", uuid::Uuid::new_v4()));

    std::fs::copy(active_db, &safeguard).map_err(|e| format!("salvaguardar banco ativo: {e}"))?;
    for suffix in ["-wal", "-shm"] {
        let src = sidecar(active_db, suffix);
        if src.exists() {
            let _ = std::fs::copy(&src, sidecar(&safeguard, suffix));
        }
    }
    Ok(Some(safeguard))
}

/// Remove salvaguardas `.pre-restore-*` mais antigas que a que acabou de ser criada (`keep`), ao
/// lado de `active_db` — cada check-out bem-sucedido copiava o banco inteiro para uma nova
/// salvaguarda e nunca removia as anteriores, o que crescia sem limite (uma cópia integral do
/// banco por check-out, para sempre). Só a mais recente vale a pena reter: é a única com chance
/// real de servir como recuperação manual do estado imediatamente anterior. Melhor esforço:
/// entradas ilegíveis (permissão, corrida com outro processo) são ignoradas, nunca propagadas —
/// retenção de lixo não pode derrubar uma restauração que já teve sucesso.
fn prune_older_safeguards(active_db: &Path, keep: Option<&Path>) {
    let parent = active_db.parent().unwrap_or_else(|| Path::new("."));
    let stem = active_db
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("neko-finance");
    let prefix = format!("{stem}.pre-restore-");
    // O nome do arquivo `.db` da salvaguarda RECÉM-CRIADA é o prefixo dos SEUS PRÓPRIOS sidecars
    // (`<mesmo-nome>.db-wal`/`-shm`) — comparar por PREFIXO, não só por igualdade exata, para não
    // apagar os sidecars da salvaguarda que esta troca acabou de criar junto com as antigas.
    let keep_name = keep
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());

    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(keep_name) = &keep_name
            && name_str.starts_with(keep_name.as_str())
        {
            continue;
        }
        // Casa o prefixo tanto do `.db` principal quanto dos sidecars `-wal`/`-shm` de
        // salvaguardas antigas (ambos começam pelo mesmo `{stem}.pre-restore-`).
        if name_str.starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Troca `active_db` pelo arquivo já baixado e validado em `downloaded`. Quando `active_db` já
/// existe, ele é salvaguardado primeiro — COPIADO (nunca renomeado) para um nome
/// `.pre-restore-<uuid>.db` único ao lado, o retorno é esse caminho, ou `None` na primeira
/// restauração (nenhum banco ativo para salvaguardar). SÓ ENTÃO um único `rename(downloaded,
/// active_db)` troca o arquivo ativo — a MESMA garantia que `db_export::vacuum_into_atomic` usa
/// para o backup (um rename substitui atomicamente um destino já existente num único syscall).
///
/// A ordem importa: copiar em vez de renomear a salvaguarda significa que `active_db` NUNCA fica
/// ausente entre os dois passos — uma queda de energia/kill do processo entre a cópia e o rename
/// final encontra `active_db` intacto (o rename nem começou) ou já trocado (o rename terminou),
/// nunca um diretório sem banco algum. A versão anterior desta função renomeava `active_db` para
/// a salvaguarda ANTES do rename final: um crash exatamente nesse intervalo deixava `active_db`
/// inexistente, e `open_migrated_pool` (com `create_if_missing`) criaria um banco novo e VAZIO no
/// próximo boot — perda silenciosa dos dados, que ainda estariam intactos na salvaguarda.
///
/// Os sidecars `-wal`/`-shm` do banco ativo ANTIGO são copiados para a salvaguarda pela mesma
/// razão, e só REMOVIDOS do caminho ativo depois que a troca principal já confirmou — deixá-los
/// órfãos ao lado do arquivo novo faria o SQLite lê-los como se fossem WAL do banco RECÉM-TROCADO.
///
/// Falha no rename final (`downloaded` sumiu, por exemplo) nunca chega a tocar `active_db` — a
/// cópia órfã da salvaguarda é apagada e o erro devolvido, com `active_db` exatamente como estava.
///
/// Pré-condição do chamador: nenhum pool/conexão está com `active_db` aberto neste momento (o
/// `checkout_on_open` fecha o pool de leitura ANTES de chamar isto) — um arquivo em uso não pode
/// ser substituído de forma atômica no Windows, e no Unix desvincularia o handle aberto.
///
/// Retenção: só a salvaguarda que ESTA troca acabou de criar sobrevive — qualquer
/// `.pre-restore-*` mais antiga ao lado de `active_db` (de check-outs anteriores) é removida
/// depois que o rename final confirma, nunca antes (uma salvaguarda velha só pode sumir depois
/// que a nova já está no lugar). Sem isso, cada check-out bem-sucedido deixava mais uma cópia
/// integral do banco no disco, para sempre.
pub(crate) fn swap_active_db_atomically(
    downloaded: &Path,
    active_db: &Path,
) -> Result<Option<PathBuf>, String> {
    let safeguard = create_safeguard_copy(active_db)?;
    let had_active = safeguard.is_some();

    if let Err(e) = std::fs::rename(downloaded, active_db) {
        if let Some(safeguard) = &safeguard {
            let _ = std::fs::remove_file(safeguard);
            for suffix in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(sidecar(safeguard, suffix));
            }
        }
        return Err(format!("trocar pelo snapshot baixado: {e}"));
    }

    if had_active {
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(sidecar(active_db, suffix));
        }
    }

    prune_older_safeguards(active_db, safeguard.as_deref());

    Ok(safeguard)
}

/// Reverte `active_db` para o conteúdo da salvaguarda depois que REABRIR o banco recém-trocado
/// falhou: copia a salvaguarda para um TEMPORÁRIO ao lado e só então troca por `rename` —
/// nunca apaga a salvaguarda em si (se a reabertura falhar de novo, o caminho dela ainda serve
/// para recuperação manual) — e limpa sidecars `-wal`/`-shm` que a restauração abandonada possa
/// ter deixado ao lado do arquivo novo, para a reabertura seguinte não ler um WAL órfão de um
/// conteúdo já descartado.
///
/// Copiar DIRETO para `active_db` (a versão anterior desta função) sobrescreveria o arquivo ativo
/// bytes-a-bytes; uma cópia que falha no MEIO (disco cheio, processo morto) deixaria `active_db`
/// truncado — o próprio conteúdo que a reversão existe para preservar, corrompido pela reversão
/// em si. O par copiar-para-tmp-e-renomear é a MESMA garantia que `swap_active_db_atomically` já
/// usa para a troca de verdade: só o `rename` final (um único syscall) troca o que `active_db`
/// aponta, então uma falha na cópia nunca toca o arquivo ativo, e uma falha no rename nunca deixa
/// nada pela metade.
pub(crate) fn rollback_to_safeguard(safeguard: &Path, active_db: &Path) -> Result<(), String> {
    let parent = active_db.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = parent.join(format!("neko-rollback-{}.db", uuid::Uuid::new_v4()));

    if let Err(e) = std::fs::copy(safeguard, &tmp_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("reverter para a salvaguarda: {e}"));
    }
    if let Err(e) = std::fs::rename(&tmp_path, active_db) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("reverter para a salvaguarda: {e}"));
    }
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(sidecar(active_db, suffix));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neko-restore-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn write_real_sqlite_db(path: &Path) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn validate_downloaded_db_rejects_bytes_without_the_sqlite_magic_header() {
        let dir = test_dir();
        let path = dir.join("garbage.db");
        std::fs::write(&path, b"nao sou um sqlite").unwrap();

        let err = validate_downloaded_db(&path).await.unwrap_err();
        assert!(
            err.contains("não é um arquivo SQLite válido"),
            "erro: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn validate_downloaded_db_accepts_a_real_sqlite_file() {
        let dir = test_dir();
        let path = dir.join("real.db");
        write_real_sqlite_db(&path).await;

        validate_downloaded_db(&path)
            .await
            .expect("um banco real e migrado deve validar");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn validate_downloaded_db_rejects_a_truncated_file_that_fails_integrity_check() {
        let dir = test_dir();
        let real_path = dir.join("real.db");
        write_real_sqlite_db(&real_path).await;
        let bytes = std::fs::read(&real_path).unwrap();
        assert!(
            bytes.len() > 4096,
            "banco migrado precisa passar de uma página para o truncamento fazer sentido"
        );

        let truncated_path = dir.join("truncated.db");
        std::fs::write(&truncated_path, &bytes[..bytes.len() / 2]).unwrap();

        let err = validate_downloaded_db(&truncated_path).await.unwrap_err();
        assert!(
            err.contains("integridade") || err.contains("SQLite"),
            "erro: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn swap_active_db_atomically_replaces_the_active_file_and_returns_the_safeguard_path() {
        let dir = test_dir();
        let active = dir.join("active.db");
        let downloaded = dir.join("downloaded.db");
        std::fs::write(&active, b"conteudo antigo").unwrap();
        std::fs::write(&downloaded, b"conteudo novo").unwrap();

        let safeguard = swap_active_db_atomically(&downloaded, &active)
            .expect("troca deve suceder")
            .expect("banco ativo pré-existente gera salvaguarda");

        assert_eq!(std::fs::read(&active).unwrap(), b"conteudo novo");
        assert_eq!(std::fs::read(&safeguard).unwrap(), b"conteudo antigo");
        assert!(
            !downloaded.exists(),
            "o temporário foi consumido pela troca"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn swap_active_db_atomically_moves_wal_and_shm_sidecars_of_the_old_db_into_the_safeguard()
    {
        let dir = test_dir();
        let active = dir.join("active.db");
        let downloaded = dir.join("downloaded.db");
        std::fs::write(&active, b"conteudo antigo").unwrap();
        std::fs::write(dir.join("active.db-wal"), b"wal antigo").unwrap();
        std::fs::write(dir.join("active.db-shm"), b"shm antigo").unwrap();
        std::fs::write(&downloaded, b"conteudo novo").unwrap();

        let safeguard = swap_active_db_atomically(&downloaded, &active)
            .expect("troca deve suceder")
            .expect("salvaguarda existe");

        assert!(
            !dir.join("active.db-wal").exists(),
            "wal antigo não pode sobrar ao lado do banco novo"
        );
        assert!(!dir.join("active.db-shm").exists());
        assert_eq!(
            std::fs::read(sidecar(&safeguard, "-wal")).unwrap(),
            b"wal antigo"
        );
        assert_eq!(
            std::fs::read(sidecar(&safeguard, "-shm")).unwrap(),
            b"shm antigo"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn swap_active_db_atomically_first_run_with_no_existing_active_db_returns_no_safeguard() {
        let dir = test_dir();
        let active = dir.join("active.db");
        let downloaded = dir.join("downloaded.db");
        std::fs::write(&downloaded, b"conteudo novo").unwrap();

        let safeguard =
            swap_active_db_atomically(&downloaded, &active).expect("troca deve suceder");

        assert!(
            safeguard.is_none(),
            "nada para salvaguardar na primeira vez"
        );
        assert_eq!(std::fs::read(&active).unwrap(), b"conteudo novo");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn swap_active_db_atomically_rolls_back_the_safeguard_when_the_final_rename_fails() {
        let dir = test_dir();
        let active = dir.join("active.db");
        // `downloaded` nunca é criado — simula uma falha entre a validação e a troca (ex.: outro
        // processo removeu o temporário). A troca deve devolver erro E deixar `active` intocado.
        let downloaded = dir.join("nao-existe.db");
        std::fs::write(&active, b"conteudo original").unwrap();

        let err = swap_active_db_atomically(&downloaded, &active).unwrap_err();
        assert!(err.contains("trocar pelo snapshot baixado"), "erro: {err}");
        assert_eq!(
            std::fs::read(&active).unwrap(),
            b"conteudo original",
            "falha no meio nunca pode deixar o banco ativo parcial ou ausente"
        );
        // Nenhuma salvaguarda órfã sobra no diretório depois do rollback.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-restore"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "salvaguarda deve voltar ao lugar após o rollback"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_safeguard_copy_never_removes_the_active_db() {
        // Regressão do defeito descrito no ADR-0015: uma implementação que RENOMEIA `active_db`
        // para criar a salvaguarda (em vez de copiar) faz `active_db` desaparecer NESTE passo — um
        // crash exatamente entre este passo e o rename final encontraria `active_db` ausente. Um
        // teste da função `swap_active_db_atomically` INTEIRA, que só observa o desfecho FINAL
        // (sucesso ou erro do fluxo completo), não pega isso: dependendo de como o caminho de erro
        // trata a salvaguarda depois, `active_db` pode acabar presente de novo no final mesmo
        // tendo ficado ausente NO MEIO. Testar a etapa da salvaguarda EM ISOLADO — sem chegar perto
        // do rename final — é o que distingue as duas implementações de verdade: com cópia,
        // `active_db` nunca deixa de existir neste passo; com renomeio, deixaria.
        let dir = test_dir();
        let active = dir.join("active.db");
        std::fs::write(&active, b"conteudo original").unwrap();

        let safeguard = create_safeguard_copy(&active).expect("salvaguardar deve suceder");

        assert!(
            active.exists(),
            "o banco ativo não pode sumir ao criar a salvaguarda"
        );
        assert_eq!(std::fs::read(&active).unwrap(), b"conteudo original");
        let safeguard = safeguard.expect("banco ativo pré-existente gera salvaguarda");
        assert_eq!(std::fs::read(&safeguard).unwrap(), b"conteudo original");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_safeguard_copy_returns_none_when_there_is_no_active_db_to_protect() {
        let dir = test_dir();
        let active = dir.join("active.db");

        let safeguard = create_safeguard_copy(&active).expect("não há erro sem banco ativo");

        assert!(safeguard.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn swap_active_db_atomically_retains_only_the_most_recent_safeguard() {
        // Cada check-out bem-sucedido antes desta correção deixava mais uma cópia integral do
        // banco no disco, sem limite — simula duas salvaguardas "antigas" já no diretório (de
        // check-outs anteriores) e confirma que só a NOVA sobrevive depois da troca.
        let dir = test_dir();
        let active = dir.join("active.db");
        let downloaded = dir.join("downloaded.db");
        std::fs::write(&active, b"conteudo antigo").unwrap();
        std::fs::write(&downloaded, b"conteudo novo").unwrap();

        let old_safeguard_1 =
            dir.join("active.pre-restore-11111111-1111-1111-1111-111111111111.db");
        let old_safeguard_2 =
            dir.join("active.pre-restore-22222222-2222-2222-2222-222222222222.db");
        std::fs::write(&old_safeguard_1, b"salvaguarda antiga 1").unwrap();
        std::fs::write(&old_safeguard_2, b"salvaguarda antiga 2").unwrap();
        // Sidecar de uma salvaguarda antiga (o restore atual guarda WAL/SHM junto da salvaguarda):
        // precisa ser removido junto do `.db`, nunca sobrar órfão.
        std::fs::write(sidecar(&old_safeguard_1, "-wal"), b"wal antigo").unwrap();

        let new_safeguard = swap_active_db_atomically(&downloaded, &active)
            .expect("troca deve suceder")
            .expect("banco ativo pré-existente gera salvaguarda");

        assert!(
            !old_safeguard_1.exists(),
            "salvaguarda antiga deve ser removida após a troca bem-sucedida"
        );
        assert!(!old_safeguard_2.exists());
        assert!(
            !sidecar(&old_safeguard_1, "-wal").exists(),
            "sidecar de salvaguarda antiga não pode sobrar órfão"
        );
        assert!(
            new_safeguard.exists(),
            "a salvaguarda que ESTA troca acabou de criar deve permanecer"
        );
        assert_eq!(std::fs::read(&new_safeguard).unwrap(), b"conteudo antigo");

        // Só a salvaguarda mais recente sobra no diretório.
        let remaining: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-restore"))
            .collect();
        assert_eq!(
            remaining.len(),
            1,
            "só a salvaguarda mais recente deve sobrar: {remaining:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rollback_to_safeguard_restores_the_previous_content_and_clears_stray_sidecars() {
        let dir = test_dir();
        let active = dir.join("active.db");
        let safeguard = dir.join("active.pre-restore-test.db");
        std::fs::write(&active, b"conteudo recem-trocado que nao abre").unwrap();
        std::fs::write(&safeguard, b"conteudo de antes da troca").unwrap();
        // Sidecars órfãos que a tentativa abandonada de restauração deixou ao lado do arquivo
        // novo — precisam sumir, senão a reabertura seguinte os lê como WAL do conteúdo revertido.
        std::fs::write(dir.join("active.db-wal"), b"wal da tentativa abandonada").unwrap();
        std::fs::write(dir.join("active.db-shm"), b"shm da tentativa abandonada").unwrap();

        rollback_to_safeguard(&safeguard, &active).expect("reversão deve suceder");

        assert_eq!(
            std::fs::read(&active).unwrap(),
            b"conteudo de antes da troca"
        );
        assert!(
            safeguard.exists(),
            "a salvaguarda em si nunca é apagada pela reversão"
        );
        assert!(!dir.join("active.db-wal").exists());
        assert!(!dir.join("active.db-shm").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rollback_to_safeguard_fails_when_the_safeguard_itself_is_unreadable() {
        let dir = test_dir();
        let active = dir.join("active.db");
        let safeguard = dir.join("nao-existe.db");
        std::fs::write(&active, b"conteudo recem-trocado").unwrap();

        let err = rollback_to_safeguard(&safeguard, &active).unwrap_err();
        assert!(err.contains("reverter para a salvaguarda"), "erro: {err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rollback_to_safeguard_leaves_active_db_untouched_when_the_final_swap_fails() {
        // Regressão (issue #446, D3 do PR #452): a reversão copiava DIRETO para `active_db`, o
        // que deixaria o arquivo ativo truncado se a cópia falhasse no meio (disco cheio,
        // processo morto). O conserto copia para um TEMPORÁRIO e só troca por `rename` — aqui
        // forçamos a falha exatamente no passo final (`active_db` é um DIRETÓRIO, então o
        // `rename` do arquivo temporário sobre ele falha) para provar que: (a) `active_db`
        // continua exatamente como estava (a cópia nunca o tocou, só o temporário) e (b) nenhum
        // `neko-rollback-*.db` órfão sobra no diretório.
        let dir = test_dir();
        let active = dir.join("active.db");
        let safeguard = dir.join("active.pre-restore-test.db");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::write(&safeguard, b"conteudo de antes da troca").unwrap();

        let err = rollback_to_safeguard(&safeguard, &active).unwrap_err();
        assert!(err.contains("reverter para a salvaguarda"), "erro: {err}");

        assert!(
            active.is_dir(),
            "active_db continua exatamente como estava — a cópia foi para um temporário, nunca \
             direto nele"
        );
        let leftover: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("neko-rollback-")
            })
            .collect();
        assert!(
            leftover.is_empty(),
            "o temporário órfão da tentativa falha precisa ser limpo, não sobrar para sempre"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn stage_downloaded_snapshot_removes_the_leftover_file_when_the_write_fails() {
        // Simula uma falha de gravação real (permissão negada) que ainda assim deixa um arquivo
        // no caminho: pré-cria `tmp_path` como somente-leitura, então `tokio::fs::write` (que
        // trunca antes de escrever) falha por permissão SEM remover o arquivo pré-existente. O
        // caminho de validação já limpava o temporário na própria falha; este é o caminho de
        // ESCRITA, que antes desta correção deixava o arquivo órfão para sempre.
        let dir = test_dir();
        let tmp_path = dir.join("staged.db");
        std::fs::write(&tmp_path, b"lixo residual de uma tentativa anterior").unwrap();
        let mut perms = std::fs::metadata(&tmp_path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&tmp_path, perms).unwrap();

        let err = stage_downloaded_snapshot(&tmp_path, b"novos bytes baixados")
            .await
            .unwrap_err();
        assert!(err.contains("gravar snapshot baixado"), "erro: {err}");

        // Precisa poder apagar mesmo com o arquivo somente-leitura (permissão de escrita mora no
        // DIRETÓRIO, não no arquivo, no Unix) — se a limpeza não rodasse, o arquivo continuaria
        // com o conteúdo antigo em vez de sumir.
        assert!(
            !tmp_path.exists(),
            "o temporário não pode sobrar quando a gravação falha"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn stage_downloaded_snapshot_accepts_a_valid_download_and_leaves_it_in_place() {
        let dir = test_dir();
        let real_source = dir.join("real-source.db");
        write_real_sqlite_db(&real_source).await;
        let bytes = std::fs::read(&real_source).unwrap();
        let tmp_path = dir.join("staged.db");

        stage_downloaded_snapshot(&tmp_path, &bytes)
            .await
            .expect("um download íntegro deve ficar pronto para a troca");

        assert!(tmp_path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
