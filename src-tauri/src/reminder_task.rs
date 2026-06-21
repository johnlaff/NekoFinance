//! Daily reminder notification (plan 030).
//!
//! Fires a native OS notification at the user-configured time when the app is
//! running. Desktop-only; this loop lives inside the desktop process, so on its
//! own it cannot push when the app is closed. Plan 039 adds an OS-level
//! scheduled entry (`os_scheduler`) that launches the binary with `--remind` at
//! the chosen time even when the app is closed; this in-app loop is kept as the
//! always-available fallback (and the only path on platforms whose OS-scheduler
//! is still phased — see `os_scheduler`).
//!
//! Reuses the plan-026 notification plugin (`tauri_plugin_notification`) and the
//! background-task shape of `sync_task::spawn_background_sync` (spawn a Tokio
//! loop, sleep a tick, act on `app_setting` keys). No second scheduler.
//!
//! ## `app_setting` keys this module reads
//!
//! | Key                              | Default   | Purpose                                  |
//! | -------------------------------- | --------- | ---------------------------------------- |
//! | `daily_reminder_enabled`         | `"true"`  | Toggle on/off.                           |
//! | `daily_reminder_time`            | `"20:00"` | Local wall-clock time (`HH:MM`, 24h).    |
//! | `daily_reminder_last_fired_date` | —         | ISO date of the last fired notification; |
//! |                                  |           | prevents double-firing in the same day.  |

use sqlx::SqlitePool;

/// How often the task wakes to check whether the reminder time has passed.
const TICK_SECS: u64 = 60;
/// Default reminder time when the key is absent or unparseable.
const DEFAULT_TIME: &str = "20:00";

/// Returns the current local date as `YYYY-MM-DD` and the current `HH:MM`.
fn local_now() -> (String, String) {
    let now = chrono::Local::now();
    (
        now.format("%Y-%m-%d").to_string(),
        now.format("%H:%M").to_string(),
    )
}

/// Parses `"HH:MM"` into `(hour, minute)`. Returns `None` on malformed input.
fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.splitn(2, ':');
    let h: u32 = parts.next()?.trim().parse().ok()?;
    let m: u32 = parts.next()?.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

/// Decides whether a reminder is due, given the persisted settings and the
/// current local date/time. Pure (no IO, no clock, no notification) so the
/// once-per-day / skip-if-fired / toggle / time logic is unit-testable.
///
/// Returns `true` only when: the reminder is enabled, the target time has
/// already passed today, and it has not already fired today.
fn should_fire(
    enabled_setting: Option<&str>,
    time_setting: Option<&str>,
    last_fired_setting: Option<&str>,
    today: &str,
    now_hm: &str,
) -> bool {
    // Absent toggle key = default ON.
    let enabled = enabled_setting.map(|v| v != "false").unwrap_or(true);
    if !enabled {
        return false;
    }

    // Configured time, falling back to the default; malformed input → stay quiet.
    let time_str = time_setting.unwrap_or(DEFAULT_TIME);
    let Some(target) = parse_hhmm(time_str) else {
        return false;
    };

    // Current wall-clock; malformed (should never happen) → stay quiet.
    let Some(now) = parse_hhmm(now_hm) else {
        return false;
    };

    // Has the target time passed for today, and not yet fired today?
    now >= target && last_fired_setting != Some(today)
}

/// One reminder tick. Returns `Ok(())` on every "nothing to do" path; `Err` is
/// logged by the loop but does not stop it.
async fn tick(pool: &SqlitePool, app_handle: &tauri::AppHandle) -> Result<(), String> {
    let enabled = crate::commands::app_setting_get(pool, "daily_reminder_enabled").await?;
    let time = crate::commands::app_setting_get(pool, "daily_reminder_time").await?;
    let last_fired =
        crate::commands::app_setting_get(pool, "daily_reminder_last_fired_date").await?;
    let (today, now_hm) = local_now();

    if !should_fire(
        enabled.as_deref(),
        time.as_deref(),
        last_fired.as_deref(),
        &today,
        &now_hm,
    ) {
        return Ok(());
    }

    // Fire the notification (best-effort; failure must not crash the loop). Uses the
    // same plugin API as `sync_task::notify_reconnect` (plan 026).
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = app_handle
            .notification()
            .builder()
            .title("Neko Finance")
            .body("Hora de atualizar seu diário.")
            .show();
    }

    // Record the date so we don't fire again today, even across ticks.
    crate::commands::app_setting_set(pool, "daily_reminder_last_fired_date", &today).await?;

    Ok(())
}

/// Spawns the background reminder loop. Wakes every `TICK_SECS` seconds (60 s).
/// Errors are logged; the loop never panics. Mirrors `sync_task::spawn_background_sync`.
pub fn spawn_reminder_task(pool: SqlitePool, app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(TICK_SECS)).await;
            if let Err(e) = tick(&pool, &app_handle).await {
                eprintln!("[reminder] tick error: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        p
    }

    #[test]
    fn parse_hhmm_valid() {
        assert_eq!(parse_hhmm("20:00"), Some((20, 0)));
        assert_eq!(parse_hhmm("08:30"), Some((8, 30)));
        assert_eq!(parse_hhmm("00:00"), Some((0, 0)));
        assert_eq!(parse_hhmm("23:59"), Some((23, 59)));
    }

    #[test]
    fn parse_hhmm_invalid() {
        assert_eq!(parse_hhmm("24:00"), None); // hour out of range
        assert_eq!(parse_hhmm("abc"), None);
        assert_eq!(parse_hhmm(""), None);
        assert_eq!(parse_hhmm("20:60"), None); // minute out of range
    }

    #[test]
    fn should_fire_when_time_passed_and_not_yet_fired() {
        // Enabled (absent = default ON), target 20:00 already passed, never fired today.
        assert!(should_fire(
            None,
            Some("20:00"),
            None,
            "2026-06-20",
            "20:00"
        ));
        assert!(should_fire(
            None,
            Some("20:00"),
            None,
            "2026-06-20",
            "21:30"
        ));
    }

    #[test]
    fn should_not_fire_before_target_time() {
        assert!(!should_fire(
            Some("true"),
            Some("20:00"),
            None,
            "2026-06-20",
            "19:59"
        ));
    }

    #[test]
    fn should_not_fire_when_disabled() {
        // Disabled overrides everything, even if the time has passed.
        assert!(!should_fire(
            Some("false"),
            Some("20:00"),
            None,
            "2026-06-20",
            "23:00"
        ));
    }

    #[test]
    fn should_not_fire_twice_in_the_same_day() {
        // Already fired today → no double-fire, even though the time has passed.
        assert!(!should_fire(
            None,
            Some("20:00"),
            Some("2026-06-20"),
            "2026-06-20",
            "22:00"
        ));
        // A stale last-fired date (yesterday) must NOT block today's reminder.
        assert!(should_fire(
            None,
            Some("20:00"),
            Some("2026-06-19"),
            "2026-06-20",
            "22:00"
        ));
    }

    #[test]
    fn should_use_default_time_when_setting_absent_or_malformed() {
        // Absent time → default 20:00; 20:30 is past it.
        assert!(should_fire(None, None, None, "2026-06-20", "20:30"));
        // Malformed time → stay quiet rather than guess.
        assert!(!should_fire(
            None,
            Some("not-a-time"),
            None,
            "2026-06-20",
            "23:00"
        ));
    }

    #[tokio::test]
    async fn persisting_settings_round_trips_through_app_setting() {
        // Confirms the reminder keys persist via the shared KV helpers (no migration).
        let p = pool().await;
        crate::commands::app_setting_set(&p, "daily_reminder_enabled", "false")
            .await
            .unwrap();
        crate::commands::app_setting_set(&p, "daily_reminder_time", "07:45")
            .await
            .unwrap();
        assert_eq!(
            crate::commands::app_setting_get(&p, "daily_reminder_enabled")
                .await
                .unwrap()
                .as_deref(),
            Some("false")
        );
        assert_eq!(
            crate::commands::app_setting_get(&p, "daily_reminder_time")
                .await
                .unwrap()
                .as_deref(),
            Some("07:45")
        );
    }

    #[tokio::test]
    async fn last_fired_date_is_recorded_then_blocks_same_day() {
        // After recording today's date, should_fire must short-circuit for the rest of the day.
        let p = pool().await;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        crate::commands::app_setting_set(&p, "daily_reminder_last_fired_date", &today)
            .await
            .unwrap();
        let last = crate::commands::app_setting_get(&p, "daily_reminder_last_fired_date")
            .await
            .unwrap();
        assert_eq!(last.as_deref(), Some(today.as_str()));
        assert!(!should_fire(
            None,
            Some("00:00"),
            last.as_deref(),
            &today,
            "23:59"
        ));
    }
}
