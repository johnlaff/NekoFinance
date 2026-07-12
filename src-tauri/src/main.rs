// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Fires a single native notification and returns. Used by the `--remind` CLI
/// path: the OS scheduler launches this binary with `--remind` at the
/// user's reminder time, even when the app is closed. There is no Tauri
/// `AppHandle` in this path, so we use a standalone cross-platform notifier.
/// Errors are printed (no window is available) and never block — a missed
/// notification must not crash the launcher.
fn fire_standalone_notification() {
    // Same copy as `reminder_task::tick` so both reminder paths look identical.
    let result = notify_rust::Notification::new()
        .summary("Neko Finance")
        .body("Hora de atualizar seu diário.")
        .show();
    if let Err(e) = result {
        eprintln!("[remind] não foi possível exibir a notificação: {e}");
    }
}

fn main() {
    // Reminder fast path: fire one notification and exit without opening a window.
    if std::env::args().any(|a| a == "--remind") {
        fire_standalone_notification();
        return;
    }
    neko_finance_lib::run()
}
