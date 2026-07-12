//! Tauri commands for the OS-level scheduled reminder.
//!
//! Thin shell over `crate::os_scheduler` — the UI calls these after saving the
//! reminder settings so the schedule lives at the OS level and fires even when
//! the app is closed. Best-effort: the in-app loop (`reminder_task`) is the
//! always-available fallback, so a registration failure surfaces as a UI hint
//! rather than blocking the settings save.

/// Registers (or updates) the OS-level scheduled reminder at `time_hhmm`
/// (`HH:MM`, 24h). Idempotent. Returns `Err` if the OS call fails (surfaced as a
/// non-blocking hint in the UI).
#[tauri::command]
pub async fn register_os_reminder(time_hhmm: String) -> Result<(), String> {
    crate::os_scheduler::register(&time_hhmm)
}

/// Removes the OS-level scheduled reminder. No-op if not registered.
#[tauri::command]
pub async fn unregister_os_reminder() -> Result<(), String> {
    crate::os_scheduler::unregister()
}
