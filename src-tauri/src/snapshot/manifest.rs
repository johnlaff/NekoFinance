//! O manifest publicado ao lado do snapshot no `appDataFolder`: o contrato que o árbitro do lease
//! (`snapshot::lease`) e o transporte (`snapshot::transport`) compartilham. Nenhum teste inspeciona
//! o formato JSON além deste contrato.

use serde::{Deserialize, Serialize};

/// `sequence` decide posse (ver `lease::decide`); `app_version`/`schema_version` decidem
/// compatibilidade de restauração — um snapshot de schema mais novo que o app local recusa
/// restauração (ADR-0015). `created_at` é só informativo para a UI ("último check-in quando");
/// o árbitro nunca compara relógios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub device_id: String,
    pub sequence: i64,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
}
