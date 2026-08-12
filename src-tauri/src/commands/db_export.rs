//! Exportação atômica do banco via `VACUUM INTO`, compartilhada pelo backup manual
//! (`write_back_cmds::backup_db`) e pelo check-in do snapshot no Drive (`snapshot_cmds`) — os
//! dois precisam do MESMO cuidado: nunca deixar um `.db` parcial no destino final.

use sqlx::SqlitePool;
use std::path::Path;

/// Exporta o banco para `dest` via `VACUUM INTO`, com escrita-em-TEMPORÁRIO no mesmo diretório do
/// destino seguida de `rename` atômico: o destino anterior (se houver) só é substituído quando o
/// novo arquivo está completo. Se o `VACUUM` falhar, o destino antigo permanece intacto.
///
/// `VACUUM INTO` roda como SQL BRUTO (`raw_sql`) — um prepared statement o silenciaria — e recusa
/// sobrescrever um arquivo já existente, daí o nome de temporário único por chamada.
pub(crate) async fn vacuum_into_atomic(pool: &SqlitePool, dest: &Path) -> Result<(), String> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".neko-vacuum-{}.tmp", uuid::Uuid::new_v4()));
    let tmp_sql = tmp.to_string_lossy().replace('\'', "''");
    let stmt = format!("VACUUM INTO '{tmp_sql}'");
    if let Err(e) = sqlx::raw_sql(sqlx::AssertSqlSafe(stmt)).execute(pool).await {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("vacuum into: {e}"));
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("finalizar exportação: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    async fn single_connection_pool() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("neko-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("neko-src.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{}", src.display()))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        (pool, dir)
    }

    #[tokio::test]
    async fn vacuum_into_atomic_writes_a_valid_sqlite_file() {
        let (pool, dir) = single_connection_pool().await;
        let dest = dir.join("export.db");

        vacuum_into_atomic(&pool, &dest).await.expect("exportar");

        let bytes = std::fs::read(&dest).unwrap();
        assert!(bytes.starts_with(b"SQLite format 3\0"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn vacuum_into_atomic_leaves_no_temp_file_behind_on_success() {
        let (pool, dir) = single_connection_pool().await;
        let dest = dir.join("export.db");

        vacuum_into_atomic(&pool, &dest).await.expect("exportar");

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".neko-vacuum-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporário deve sumir após o rename atômico"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn vacuum_into_atomic_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        // Mesma classe de regressão do estado do snapshot: com pool de 1 conexão, exportar
        // enquanto uma transação de escrita está aberta deve ENFILEIRAR e completar assim que a
        // tx solta a conexão — nunca travar para sempre. Um pool default (múltiplas conexões)
        // não pegaria esta regressão.
        let (pool, dir) = single_connection_pool().await;
        let dest = dir.join("export.db");

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("INSERT INTO person (id, name) VALUES ('pe-lock', 'Segurando a conexão')")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let pool_for_export = pool.clone();
        let dest_for_export = dest.clone();
        let export =
            tokio::spawn(
                async move { vacuum_into_atomic(&pool_for_export, &dest_for_export).await },
            );

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(Duration::from_secs(5), export)
            .await
            .expect("a exportação NÃO pode travar para sempre esperando a única conexão")
            .expect("a task de exportação não deve entrar em panic");
        assert!(
            result.is_ok(),
            "exportação deve ter sucesso assim que a conexão é liberada: {result:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
