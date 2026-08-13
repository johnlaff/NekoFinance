//! Convergência entre aparelhos por snapshot íntegro no Drive, com lease (ADR-0015).
//!
//! `lease` é o árbitro puro (core, sem tipos do Tauri — ADR-0014): decide quem pode publicar.
//! `manifest` é o contrato serializado ao lado do snapshot. `transport` fala com o
//! `appDataFolder` do Drive atrás da borda HTTP existente (`crate::http`).

pub mod checkout;
pub(crate) mod conflict;
pub mod lease;
pub mod manifest;
pub mod restore;
pub mod state;
pub mod transport;
