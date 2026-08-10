//! Casca de IO da pré-checagem de espaço em disco antes de um update.
//!
//! Mede o disco de verdade (via `fs4`) e o footprint da instalação atual, e
//! delega o veredito à régua pura em `crate::update_space`. Qualquer
//! incerteza na medição degrada para o lado conservador: falha ao medir
//! espaço livre aborta com `Err` (o frontend cai de volta no fluxo sem
//! pré-checagem, nunca bloqueia por uma leitura incerta); falha ao
//! identificar o volume trata os dois caminhos como o mesmo volume; falha
//! total ao medir o footprint instalado cai no piso da régua (`0`).

use crate::update_space::{self, SpaceProbe, SpaceVerdict};
use std::path::{Component, Path, PathBuf};

/// Verifica se há espaço em disco suficiente para aplicar o update pendente.
/// Mede o volume da instalação (diretório do executável atual, que recebe o
/// binário novo na troca) e o volume do temporário do SO (onde o pacote
/// baixado é extraído), soma o footprint atual da instalação e devolve o
/// veredito da régua pura.
#[tauri::command]
pub async fn check_update_space() -> Result<SpaceVerdict, String> {
    // A medição anda o diretório da instalação inteiro (IO bloqueante) — roda fora do
    // runtime async para não prender um worker do Tauri enquanto o disco responde.
    tauri::async_runtime::spawn_blocking(measure_and_evaluate)
        .await
        .map_err(|e| format!("a medição de espaço não concluiu: {e}"))?
}

/// Corpo síncrono da medição — colhe o `SpaceProbe` real e aplica a régua pura.
fn measure_and_evaluate() -> Result<SpaceVerdict, String> {
    let temp_dir = std::env::temp_dir();
    let install_dir = install_dir()?;

    let install_free_bytes = fs4::available_space(&install_dir)
        .map_err(|e| format!("não foi possível medir o espaço livre da instalação: {e}"))?;
    let temp_free_bytes = fs4::available_space(&temp_dir)
        .map_err(|e| format!("não foi possível medir o espaço livre do temporário: {e}"))?;

    let probe = SpaceProbe {
        install_free_bytes,
        temp_free_bytes,
        same_volume: same_volume(&install_dir, &temp_dir),
        installed_footprint_bytes: dir_size(&install_dir),
    };
    Ok(update_space::evaluate(probe))
}

/// Diretório onde o executável atual vive — é ele que recebe o binário novo na troca do update.
fn install_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("não foi possível localizar o executável: {e}"))?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "executável sem diretório pai".to_string())
}

/// Soma recursiva do tamanho dos arquivos sob `dir` — aproxima o footprint da instalação atual
/// para a margem de segurança da régua. Entrada ilegível (permissão, link quebrado) é ignorada
/// em silêncio — best-effort, não é um relatório de auditoria; falha total de leitura do
/// diretório raiz devolve `0`, que a régua trata como "não conseguiu medir" e cai no piso.
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|entry| match entry.file_type() {
            Ok(ft) if ft.is_dir() => dir_size(&entry.path()),
            Ok(ft) if ft.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Compara os dois caminhos pelo componente de raiz/prefixo canonicalizado — no Windows, a
/// letra do drive (`C:`) ou o servidor UNC; no Unix, sempre a raiz `/`, então essa comparação só
/// discrimina volumes de fato no Windows (a plataforma primária do updater). Qualquer incerteza
/// — `canonicalize` falhou, prefixo ilegível — degrada para `true`: tratar como o mesmo volume
/// soma as duas exigências contra um único espaço livre, o cenário mais apertado possível.
fn same_volume(a: &Path, b: &Path) -> bool {
    let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) else {
        return true;
    };
    match (root_prefix(&a), root_prefix(&b)) {
        (Some(pa), Some(pb)) => pa == pb,
        _ => true,
    }
}

/// Primeiro componente do caminho (`Prefix`/`RootDir`) — a parte que identifica o volume.
fn root_prefix(path: &Path) -> Option<Component<'_>> {
    path.components().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_size_soma_arquivos_aninhados() {
        let root = std::env::temp_dir().join(format!("neko-dirsize-{}", uuid::Uuid::new_v4()));
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(root.join("a.bin"), vec![0u8; 100]).unwrap();
        std::fs::write(sub.join("b.bin"), vec![0u8; 250]).unwrap();

        assert_eq!(dir_size(&root), 350);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn dir_size_de_diretorio_inexistente_e_zero() {
        let ghost = std::env::temp_dir().join(format!("neko-ghost-{}", uuid::Uuid::new_v4()));
        assert_eq!(dir_size(&ghost), 0);
    }

    #[test]
    fn same_volume_de_dois_caminhos_sob_a_mesma_raiz_e_verdadeiro() {
        let root = std::env::temp_dir().join(format!("neko-vol-{}", uuid::Uuid::new_v4()));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        assert!(same_volume(&a, &b));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn same_volume_degrada_para_verdadeiro_quando_canonicalize_falha() {
        let ghost_a = std::env::temp_dir().join(format!("neko-ghost-a-{}", uuid::Uuid::new_v4()));
        let ghost_b = std::env::temp_dir().join(format!("neko-ghost-b-{}", uuid::Uuid::new_v4()));
        assert!(same_volume(&ghost_a, &ghost_b));
    }
}
