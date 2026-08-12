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

/// Anexa `suffix` ao PATH INTEIRO (não à extensão) — a convenção real dos sidecars WAL/SHM do
/// SQLite é `<caminho-completo-do-db>-wal` / `-shm`, nunca baseada em `file_stem`.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
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
pub(crate) fn swap_active_db_atomically(
    downloaded: &Path,
    active_db: &Path,
) -> Result<Option<PathBuf>, String> {
    let parent = active_db.parent().unwrap_or_else(|| Path::new("."));
    let stem = active_db
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("neko-finance");
    let safeguard = parent.join(format!("{stem}.pre-restore-{}.db", uuid::Uuid::new_v4()));

    let had_active = active_db.exists();
    if had_active {
        std::fs::copy(active_db, &safeguard)
            .map_err(|e| format!("salvaguardar banco ativo: {e}"))?;
        for suffix in ["-wal", "-shm"] {
            let src = sidecar(active_db, suffix);
            if src.exists() {
                let _ = std::fs::copy(&src, sidecar(&safeguard, suffix));
            }
        }
    }

    if let Err(e) = std::fs::rename(downloaded, active_db) {
        if had_active {
            let _ = std::fs::remove_file(&safeguard);
            for suffix in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(sidecar(&safeguard, suffix));
            }
        }
        return Err(format!("trocar pelo snapshot baixado: {e}"));
    }

    if had_active {
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(sidecar(active_db, suffix));
        }
    }

    Ok(had_active.then_some(safeguard))
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
}
