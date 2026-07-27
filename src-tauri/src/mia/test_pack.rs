//! Pack temporário das suítes.
//!
//! Os limites da camada de método — deny-list ausente, termo bloqueado, capítulo que não existe —
//! precisam de um pack montado no teste, nunca do material privado da máquina: uma suíte que
//! dependesse dele passaria ou falharia conforme o que está instalado.

use std::path::{Path, PathBuf};

pub(crate) struct TempPack {
    root: PathBuf,
}

impl TempPack {
    pub(crate) fn new() -> Self {
        let root = Self::unique_root();
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    /// Um caminho que nunca chegou a existir, para o caso do pack não instalado.
    pub(crate) fn absent() -> Self {
        Self {
            root: Self::unique_root(),
        }
    }

    fn unique_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "neko-finance-mia-test-pack-{}",
            uuid::Uuid::new_v4()
        ))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.root
    }

    pub(crate) fn chapter(&self, topic: &str, content: &str) {
        let chapters = self.root.join("chapters");
        std::fs::create_dir_all(&chapters).unwrap();
        std::fs::write(chapters.join(format!("{topic}.md")), content).unwrap();
    }

    pub(crate) fn core(&self, content: &str) {
        std::fs::write(self.root.join("core.md"), content).unwrap();
    }

    pub(crate) fn root_file(&self, name: &str, content: &str) {
        std::fs::write(self.root.join(name), content).unwrap();
    }
}

impl Drop for TempPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
