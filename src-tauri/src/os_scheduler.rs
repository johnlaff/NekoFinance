//! OS-level scheduled reminder (plan 039).
//!
//! The in-app loop (`reminder_task`) only fires while the desktop process is
//! running — if the app is closed at the configured time the nudge is lost.
//! This module registers a platform-native scheduled entry that launches the
//! app's own binary with `--remind` at the chosen time, so the reminder fires
//! even when the app is closed. The in-app loop stays as the fallback.
//!
//! Reuses the existing reminder settings (`daily_reminder_time` /
//! `daily_reminder_enabled`) — no new `app_setting` key. The entry is written
//! only when the user explicitly saves a time, and removed when they disable
//! the reminder.
//!
//! ## Platform status
//!
//! - **Windows (primary)**: Task Scheduler via `schtasks` — a user-level daily
//!   trigger whose action is `"<exe>" --remind`. Implemented.
//! - **macOS / Linux**: phased follow-up (see the `TODO plan-039-phase2:`
//!   markers below). `register`/`unregister` are no-ops that log what was
//!   skipped; the in-app loop already covers these platforms.
//!
//! The Windows command assembly is split into a pure `build_schtasks_args`
//! helper so the argument vector is unit-testable without spawning a process.

/// Stable name of the scheduled task / entry. Namespaced so it is easy to find
/// and remove, and unlikely to collide with anything else on the machine.
#[cfg(any(target_os = "windows", test))]
const TASK_NAME: &str = "NekoFinance\\DailyReminder";

/// Validates a wall-clock `"HH:MM"` (24h) string. Returns the trimmed value on
/// success, or `Err` with a human-readable reason. Mirrors the parsing rules of
/// `reminder_task::parse_hhmm` (hour 0–23, minute 0–59) so both paths agree.
#[cfg(any(target_os = "windows", test))]
fn validate_hhmm(time_hhmm: &str) -> Result<String, String> {
    let s = time_hhmm.trim();
    let mut parts = s.splitn(2, ':');
    let h: u32 = parts
        .next()
        .and_then(|p| p.trim().parse().ok())
        .ok_or_else(|| format!("horário inválido: {time_hhmm:?}"))?;
    let m: u32 = parts
        .next()
        .and_then(|p| p.trim().parse().ok())
        .ok_or_else(|| format!("horário inválido: {time_hhmm:?}"))?;
    if h > 23 || m > 59 {
        return Err(format!("horário fora do intervalo: {time_hhmm:?}"));
    }
    // Re-emit canonically (HH:MM) so the scheduler always gets a well-formed value.
    Ok(format!("{h:02}:{m:02}"))
}

/// Pure assembly of the `schtasks /Create` argument vector for the daily
/// reminder. Validates the time, points the action (`/TR`) at the given
/// executable path with the `--remind` flag, and uses `/F` to overwrite any
/// existing entry (idempotent — safe to call on every settings save) at a
/// user-level run scope (`/RL LIMITED`, no elevation).
///
/// Pure: no IO, no process spawn — unit-testable.
#[cfg(any(target_os = "windows", test))]
fn build_schtasks_args(time_hhmm: &str, exe: &str) -> Result<Vec<String>, String> {
    let time = validate_hhmm(time_hhmm)?;
    // The action quotes the exe path (it may contain spaces) and appends the flag.
    let action = format!("\"{exe}\" --remind");
    Ok(vec![
        "/Create".into(),
        "/F".into(),
        "/SC".into(),
        "DAILY".into(),
        "/TN".into(),
        TASK_NAME.into(),
        "/ST".into(),
        time,
        "/TR".into(),
        action,
        "/RL".into(),
        "LIMITED".into(),
    ])
}

/// Pure assembly of the `schtasks /Delete` argument vector. `/F` suppresses the
/// confirmation prompt so removal never blocks on stdin.
#[cfg(any(target_os = "windows", test))]
fn build_unregister_args() -> Vec<String> {
    vec![
        "/Delete".into(),
        "/F".into(),
        "/TN".into(),
        TASK_NAME.into(),
    ]
}

/// Registers (or updates) the OS-level scheduled notification for the given
/// `HH:MM` local time. Idempotent — safe to call on every settings save.
/// Returns `Ok(())` on success; `Err(String)` with a human-readable reason on
/// failure.
///
/// On unsupported platforms this is a logged no-op (`Ok`) — the in-app loop is
/// the fallback there.
#[cfg(target_os = "windows")]
pub fn register(time_hhmm: &str) -> Result<(), String> {
    use std::process::Command;
    let exe = std::env::current_exe()
        .map_err(|e| format!("não foi possível localizar o executável: {e}"))?;
    let exe = exe.to_string_lossy().to_string();
    let args = build_schtasks_args(time_hhmm, &exe)?;
    let output = Command::new("schtasks")
        .args(&args)
        .output()
        .map_err(|e| format!("falha ao executar o agendador do sistema: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("o agendador do sistema recusou a tarefa: {stderr}"));
    }
    Ok(())
}

/// macOS — phased follow-up.
// TODO plan-039-phase2: macOS launchd plist
// Write ~/Library/LaunchAgents/com.nekofinance.reminder.plist with a
// StartCalendarInterval at the chosen time and an action of `<exe> --remind`,
// then `launchctl load` it. Until then this is a no-op; the in-app loop covers
// macOS while the app is open.
#[cfg(target_os = "macos")]
pub fn register(time_hhmm: &str) -> Result<(), String> {
    eprintln!(
        "[os_scheduler] macOS: registro OS-level adiado (plan-039-phase2); \
         lembrete em-app permanece como fallback (horário {time_hhmm})"
    );
    Ok(())
}

/// Linux — phased follow-up.
// TODO plan-039-phase2: Linux systemd-timer / crontab
// Write a `systemd --user` service+timer (or edit the user crontab) whose
// action is `<exe> --remind` at the chosen time. Until then this is a no-op;
// the in-app loop covers Linux while the app is open.
#[cfg(target_os = "linux")]
pub fn register(time_hhmm: &str) -> Result<(), String> {
    eprintln!(
        "[os_scheduler] Linux: registro OS-level adiado (plan-039-phase2); \
         lembrete em-app permanece como fallback (horário {time_hhmm})"
    );
    Ok(())
}

/// Catch-all for any other target: logged no-op.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn register(time_hhmm: &str) -> Result<(), String> {
    eprintln!(
        "[os_scheduler] plataforma sem agendador OS-level; \
         lembrete em-app permanece como fallback (horário {time_hhmm})"
    );
    Ok(())
}

/// Removes the OS-level scheduled entry. No-op if the entry does not exist.
#[cfg(target_os = "windows")]
pub fn unregister() -> Result<(), String> {
    use std::process::Command;
    let args = build_unregister_args();
    let output = Command::new("schtasks")
        .args(&args)
        .output()
        .map_err(|e| format!("falha ao executar o agendador do sistema: {e}"))?;
    // A missing task is the success state we want (nothing to remove) — schtasks
    // returns a non-zero code with "cannot find" in that case, which we treat as OK.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let absent = stderr.contains("cannot find")
            || stderr.contains("does not exist")
            || stderr.contains("não foi possível encontrar");
        if !absent {
            return Err(format!(
                "o agendador do sistema recusou a remoção: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(())
}

/// Unsupported / phased platforms: logged no-op.
// TODO plan-039-phase2: macOS launchctl unload + plist removal; Linux timer/crontab removal.
#[cfg(not(target_os = "windows"))]
pub fn unregister() -> Result<(), String> {
    eprintln!("[os_scheduler] remoção OS-level adiada nesta plataforma (plan-039-phase2)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_hhmm_canonicalizes_valid_input() {
        assert_eq!(validate_hhmm("20:00").unwrap(), "20:00");
        assert_eq!(validate_hhmm("8:5").unwrap(), "08:05");
        assert_eq!(validate_hhmm(" 07:45 ").unwrap(), "07:45");
        assert_eq!(validate_hhmm("00:00").unwrap(), "00:00");
        assert_eq!(validate_hhmm("23:59").unwrap(), "23:59");
    }

    #[test]
    fn validate_hhmm_rejects_malformed_input() {
        assert!(validate_hhmm("not-a-time").is_err());
        assert!(validate_hhmm("").is_err());
        assert!(validate_hhmm("24:00").is_err()); // hour out of range
        assert!(validate_hhmm("20:60").is_err()); // minute out of range
        assert!(validate_hhmm("20").is_err()); // no minute component
    }

    #[test]
    fn build_schtasks_args_valid_time() {
        let exe = "C:\\Program Files\\Neko Finance\\neko-finance.exe";
        let args = build_schtasks_args("20:00", exe).unwrap();
        assert_eq!(
            args,
            vec![
                "/Create",
                "/F",
                "/SC",
                "DAILY",
                "/TN",
                TASK_NAME,
                "/ST",
                "20:00",
                "/TR",
                "\"C:\\Program Files\\Neko Finance\\neko-finance.exe\" --remind",
                "/RL",
                "LIMITED",
            ]
        );
    }

    #[test]
    fn build_schtasks_args_canonicalizes_short_time() {
        let args = build_schtasks_args("8:5", "neko.exe").unwrap();
        // The /ST value is normalized to HH:MM.
        let st_idx = args.iter().position(|a| a == "/ST").unwrap();
        assert_eq!(args[st_idx + 1], "08:05");
    }

    #[test]
    fn build_schtasks_args_malformed_time_returns_err() {
        assert!(build_schtasks_args("not-a-time", "neko.exe").is_err());
        assert!(build_schtasks_args("99:99", "neko.exe").is_err());
    }

    #[test]
    fn build_unregister_args_targets_the_named_task() {
        assert_eq!(
            build_unregister_args(),
            vec!["/Delete", "/F", "/TN", TASK_NAME]
        );
    }
}
