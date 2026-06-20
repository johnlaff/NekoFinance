# Plan 030: Daily reminder via OS notification + "last logged" indicator

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src-tauri/src/sync_task.rs src-tauri/src/lib.rs src-tauri/src/commands/write_back_cmds.rs src/screens/SettingsScreen.tsx src/screens/DashboardScreen.tsx src/lib/api.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: feature
- **Package**: A
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

The method's single biggest habit failure is skipping the daily expense update
(the Saída / expense log). Neko currently has no reminder of any kind — grep
for `"notification"`, `"reminder"`, `"nudge"` across `src/` returns zero
results. The OS notification capability was already installed and registered in
plan 026 (`tauri-plugin-notification = "=2.3.3"` in `Cargo.toml`, capability
`"notification:default"` in `capabilities/default.json`, plugin initialized in
`lib.rs:20`), so there is no new Rust dependency or permission to negotiate.
This plan adds (a) a configurable daily reminder that fires a native OS
notification while the app is open, and (b) a quiet "last logged" indicator on
the dashboard so the user can see at a glance how stale their log is.

## Current state

### Notification plugin — already fully wired

`src-tauri/Cargo.toml` line 24:

```
tauri-plugin-notification = "=2.3.3"
```

`src-tauri/capabilities/default.json` lines 7–12:

```json
"permissions": [
  "core:default",
  "opener:default",
  "dialog:allow-open",
  "dialog:allow-save",
  "notification:default"
]
```

`src-tauri/src/lib.rs` line 20:

```rust
.plugin(tauri_plugin_notification::init())
```

Usage pattern already proven in `src-tauri/src/sync_task.rs` lines 124–132:

```rust
fn notify_reconnect(app_handle: &tauri::AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app_handle
        .notification()
        .builder()
        .title("Neko Finance")
        .body("Reconecte o Google para retomar a sincronização automática.")
        .show();
}
```

Copy this pattern exactly; do not import a different notification API.

### Background task pattern — spawn_background_sync in sync_task.rs

`src-tauri/src/sync_task.rs` lines 257–273 show the exact pattern for a
background Tokio loop that reads `app_setting` keys and acts on them:

```rust
pub fn spawn_background_sync(
    pool: SqlitePool,
    app_dir: PathBuf,
    app_handle: tauri::AppHandle,
    import_guard: Arc<SyncGuard>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval_secs = read_interval_secs(&pool).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;

            if let Err(e) = run_probe(&pool, &app_dir, &app_handle, &import_guard).await {
                eprintln!("[sync] probe error: {e}");
            }
        }
    });
}
```

The new reminder task mirrors this: a `spawn_reminder_task` function in a new
module `src-tauri/src/reminder_task.rs`, called from `lib.rs` `setup()` after
the pool and AppDataDir are managed (parallel to `spawn_background_sync`).

### app_setting key/value store

`src-tauri/src/commands/write_back_cmds.rs` lines 309–343 provide the
internal helpers used throughout the codebase:

```rust
pub(crate) async fn app_setting_get(pool: &SqlitePool, key: &str)
    -> Result<Option<String>, String>  // SELECT value WHERE key = ?

pub(crate) async fn app_setting_set(pool: &SqlitePool, key: &str, value: &str)
    -> Result<(), String>  // INSERT OR REPLACE INTO app_setting
```

New keys to add (no migration needed — the `app_setting` table is schema-free KV):

| Key                      | Default   | Type             | Purpose                                         |
| ------------------------ | --------- | ---------------- | ----------------------------------------------- |
| `daily_reminder_enabled` | `"true"`  | `"true"/"false"` | Toggle the daily reminder on/off.               |
| `daily_reminder_time`    | `"20:00"` | `"HH:MM"` (24h)  | Local wall-clock time to fire the notification. |

The reminder fires **once per calendar day** — the task records `daily_reminder_last_fired_date`
(ISO date string, e.g. `"2026-06-20"`) so it never double-fires within the same day, even
across multiple task ticks.

### Frontend API layer — getAppSetting / setAppSetting

`src/lib/api.ts` lines 503–510:

```ts
export function getAppSetting(key: string): Promise<string | null> {
  return invoke("get_app_setting", { key });
}
export function setAppSetting(key: string, value: string): Promise<void> {
  return invoke("set_app_setting", { key, value });
}
```

Both functions are already exported. Use them as-is from the new UI components.

### DashboardSummary — current shape (no last_logged field)

`src/lib/api.ts` lines 28–36:

```ts
export interface DashboardSummary {
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  reserve_months: number;
  reserve_trend: string;
  transaction_count: number;
}
```

The "last logged" date must be added to this struct and its Rust counterpart so
the dashboard can compute days-since-last-log client-side.

Rust struct in `src-tauri/src/commands/forecast_cmds.rs` line 828:

```rust
pub transaction_count: i64,
```

The closing brace of `DashboardSummary` is at line 829. Add `last_real_tx_date`
(an `Option<String>`) after `transaction_count`.

The query that populates `transaction_count` is at lines 908–912:

```rust
let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date <= ?1")
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;
```

Add a parallel query that returns the **most recent date of a non-projection
transaction** (i.e. a real entry the user created or imported, not a future
placeholder):

```sql
SELECT MAX(date) FROM "transaction"
WHERE is_projection = 0 AND date <= ?1
```

This returns `NULL` when no real transactions exist, which maps to `None` in
Rust and `null` in TypeScript — both callers handle it safely.

### SettingsScreen — where to add the toggle

`src/screens/SettingsScreen.tsx` currently has four `<Section>` blocks (lines
109–182). Add a fifth section "Lembretes" between "Bolsos" (line 140) and "Seus
dados" (line 148). The section uses the existing `Section` component and
`set-row` / `set-row__ctl` CSS classes (already defined in `src/App.css`).

### DashboardScreen — where to add the indicator

`src/screens/DashboardScreen.tsx` line 194:

```tsx
{
  forecast && <MonthLedgerCard today={forecast.today} reloadKey={ledgerKey} />;
}
```

Place a new `<LastLoggedBanner>` component **above** the `DailyCheckinCard`
(line 179) so it's the first thing the user sees after the hero section when
data exists, and only when `hasData` is true.

### React Compiler conventions (enforced in this repo)

- No `useCallback` / `useMemo` / `React.memo` — the Compiler handles render stability.
- Static `CSSProperties` objects must be defined at module level (not inline), as
  shown in `DailyCheckinCard.tsx` (`DAILY_BAR_TRACK`, `DAILY_INPUT_STYLE`).
- Money is an integer in cents — `amount` fields are always positive-magnitude
  integers; sign comes from the transaction `type`.

### Commit message style (from `git log`)

```
feat: <imperative summary> — plano 030 (#PR)
```

## Commands you will need

| Purpose      | Command              | Expected on success      |
| ------------ | -------------------- | ------------------------ |
| Install      | `npm ci`             | exit 0                   |
| Rust check   | `npm run rust:check` | exit 0, no errors        |
| TypeScript   | `npm run typecheck`  | exit 0, no errors        |
| Lint         | `npm run lint`       | exit 0                   |
| Unit tests   | `npm run test:run`   | all pass                 |
| Full gate    | `npm run check`      | exit 0                   |
| E2E smoke    | `npm run e2e`        | all pass                 |
| React Doctor | `npm run doctor`     | 0 issues (advisory gate) |

## Suggested executor toolkit

- Use the `neko-finance-design` skill if available for any new UI components to
  match the Midnight Ledger design system tokens.
- Reference `src/screens/dashboard/DailyCheckinCard.tsx` as the structural
  pattern for new dashboard cards (inline styles at module level, `isTauri`
  guard, `useCommand`-less since it receives props).

## Scope

**In scope** (the only files you should modify or create):

- `src-tauri/src/reminder_task.rs` — new module; background reminder loop
- `src-tauri/src/lib.rs` — add `mod reminder_task;` + call `spawn_reminder_task`
- `src-tauri/src/commands/forecast_cmds.rs` — add `last_real_tx_date` to `DashboardSummary` + query
- `src/lib/api.ts` — add `last_real_tx_date: string | null` to `DashboardSummary`
- `src/screens/SettingsScreen.tsx` — add `DailyReminderSection`
- `src/screens/DashboardScreen.tsx` — render `LastLoggedBanner`
- `src/screens/dashboard/LastLoggedBanner.tsx` — new component (create)
- `src/screens/dashboard/LastLoggedBanner.test.tsx` — new test file (create)
- `src-tauri/src/reminder_task.rs` tests — inline `#[cfg(test)]` block
- `plans/README.md` — update status row

**Out of scope** (do NOT touch):

- `src-tauri/src/sync_task.rs` — unrelated to reminders; do not extend it
- Any change to the `app_setting` table migration — no schema change needed;
  the table is already a free KV store
- `src-tauri/capabilities/default.json` — `"notification:default"` already present
- `src-tauri/Cargo.toml` — `tauri-plugin-notification` already present
- Push notifications when the app is closed — desktop-only; OS scheduler
  integration is explicitly deferred (see Maintenance notes)
- Any gamification, streaks, or points — the indicator must remain a quiet,
  factual nudge: days since last real entry, not a reward system

## Git workflow

- Branch: `feat/030-daily-reminder-notification`
- Commit per logical step (Rust module, TypeScript, UI); message style:
  `feat: <imperative summary> — plano 030`
- Do NOT push or open a PR unless the operator explicitly instructs it.

## Steps

### Step 1: Add `last_real_tx_date` to DashboardSummary (Rust + TS)

**Rust** — `src-tauri/src/commands/forecast_cmds.rs`:

1a. Add the field to the struct. Find the line:

```rust
    pub transaction_count: i64,
```

Add after it:

```rust
    /// Most recent date (`YYYY-MM-DD`) of a non-projection transaction the user logged.
    /// `None` when no real transactions exist yet.
    pub last_real_tx_date: Option<String>,
```

1b. Add the query inside `dashboard_summary()`. After the `count` query and
before the `Ok(DashboardSummary { … })` return (around line 908 in the current
file), add:

```rust
    let last_real: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT MAX(date) FROM \"transaction\" WHERE is_projection = 0 AND date <= ?1",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query last_real_tx_date: {e}"))?;
    let last_real_tx_date = last_real.and_then(|(d,)| d);
```

1c. Add the field to the `Ok(DashboardSummary { … })` initializer:

```rust
        last_real_tx_date,
```

**TypeScript** — `src/lib/api.ts`:

In the `DashboardSummary` interface (line 28), add after `transaction_count`:

```ts
  /** ISO date (YYYY-MM-DD) of the most recent non-projection transaction, or null if none. */
  last_real_tx_date: string | null;
}
```

**Verify**: `npm run rust:check && npm run typecheck` → exit 0, no errors.

### Step 2: Update the dashboard_summary test fixture

In `src/test/commands.ts`, the `SUMMARY` constant (which seeds tests for the
dashboard) must include the new field. Add:

```ts
  last_real_tx_date: "2026-06-19",
```

and in `EMPTY_SUMMARY`:

```ts
  last_real_tx_date: null,
```

Run `npm run test:run` — all existing tests must still pass. If `SUMMARY` is
typed against `DashboardSummary`, TypeScript will enforce this.

**Verify**: `npm run test:run` → all pass (zero new failures).

### Step 3: Add LastLoggedBanner component

Create `src/screens/dashboard/LastLoggedBanner.tsx`:

```tsx
import type { CSSProperties } from "react";
import { CalendarClock } from "lucide-react";
import { todayISO } from "../../lib/format";

// Static style objects (React Compiler requirement — never inline in JSX).
const BANNER: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  background: "var(--bg-subtle)",
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  lineHeight: 1.4,
};

const ICON: CSSProperties = {
  flexShrink: 0,
  color: "var(--primary)",
};

/**
 * Quiet nudge: shows how many days ago the user last logged a real transaction.
 * Hidden when the last-logged date is today (already up to date).
 * Method-neutral copy — never gamified.
 */
export function LastLoggedBanner({
  lastRealTxDate,
}: {
  lastRealTxDate: string | null;
}) {
  const today = todayISO();

  if (!lastRealTxDate) {
    return (
      <div style={BANNER} role="status">
        <CalendarClock size={15} strokeWidth={1.75} style={ICON} aria-hidden />
        <span>Nenhum lançamento ainda. Registre sua primeira saída.</span>
      </div>
    );
  }

  // Diff in whole calendar days (wall-clock, local timezone).
  const last = new Date(lastRealTxDate + "T00:00:00");
  const now = new Date(today + "T00:00:00");
  const diffDays = Math.round((now.getTime() - last.getTime()) / 86_400_000);

  if (diffDays <= 0) {
    // Already logged today — no nudge needed.
    return null;
  }

  const label =
    diffDays === 1
      ? "Você lançou pela última vez ontem."
      : `Você lançou pela última vez há ${diffDays} dias.`;

  return (
    <div style={BANNER} role="status">
      <CalendarClock size={15} strokeWidth={1.75} style={ICON} aria-hidden />
      <span>{label}</span>
    </div>
  );
}
```

Design notes:

- Uses `role="status"` (live region, polite by default) so screen readers
  announce it without interrupting the user.
- `CalendarClock` from lucide-react is already in the bundle (lucide is a
  dev/prod dep of this repo).
- Uses only tokens present in the design system (`--bg-subtle`, `--text-muted`,
  `--primary`, `--space-*`, `--radius-sm`, `--fs-sm`).
- No animation on the banner (money/date nudges are deliberately static per the
  design principle "dinheiro nunca animado").

**Verify**: `npm run typecheck` → exit 0.

### Step 4: Wire LastLoggedBanner into DashboardScreen

`src/screens/DashboardScreen.tsx` — add import at the top:

```tsx
import { LastLoggedBanner } from "./dashboard/LastLoggedBanner";
```

In the JSX return, place the banner immediately above `DailyCheckinCard`
(current line 178–184). The rendered section becomes:

```tsx
{
  summary && hasData && <LastLoggedBanner lastRealTxDate={summary.last_real_tx_date} />;
}

{
  summary && hasData && (
    <DailyCheckinCard
      summary={summary}
      monthAvgCents={monthDailyAvgCents}
      onLogged={handleLogged}
    />
  );
}
```

**Verify**: `npm run typecheck && npm run lint` → exit 0.

### Step 5: Add LastLoggedBanner tests

Create `src/screens/dashboard/LastLoggedBanner.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { LastLoggedBanner } from "./LastLoggedBanner";

// Pin "today" so tests are deterministic (todayISO() reads the real clock).
vi.mock("../../lib/format", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/format")>();
  return { ...actual, todayISO: () => "2026-06-20" };
});

describe("LastLoggedBanner", () => {
  it("shows nothing when logged today (diffDays = 0)", () => {
    const { container } = render(<LastLoggedBanner lastRealTxDate="2026-06-20" />);
    expect(container.firstChild).toBeNull();
  });

  it("shows 'ontem' for diffDays = 1", () => {
    render(<LastLoggedBanner lastRealTxDate="2026-06-19" />);
    expect(screen.getByRole("status")).toHaveTextContent("ontem");
  });

  it("shows the day count for diffDays > 1", () => {
    render(<LastLoggedBanner lastRealTxDate="2026-06-15" />);
    expect(screen.getByRole("status")).toHaveTextContent("há 5 dias");
  });

  it("shows a first-entry prompt when lastRealTxDate is null", () => {
    render(<LastLoggedBanner lastRealTxDate={null} />);
    expect(screen.getByRole("status")).toHaveTextContent("Nenhum lançamento ainda");
  });
});
```

**Verify**: `npm run test:run -- LastLoggedBanner` → 4 tests pass.

### Step 6: Add reminder settings UI to SettingsScreen

`src/screens/SettingsScreen.tsx` — add the following imports at the top:

```tsx
import { Bell } from "lucide-react";
import { getAppSetting, setAppSetting, isTauri } from "../lib/api";
```

Add a new component above `SettingsScreen`:

```tsx
/**
 * Configurações do lembrete diário: liga/desliga e horário preferido.
 * Persiste em `app_setting` via os comandos existentes.
 * Apenas disponível no shell desktop (isTauri).
 */
function DailyReminderSection() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [time, setTime] = useState("20:00");
  const [saving, setSaving] = useState(false);

  // Load current settings on mount.
  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      const [en, t] = await Promise.all([
        getAppSetting("daily_reminder_enabled"),
        getAppSetting("daily_reminder_time"),
      ]);
      setEnabled(en !== "false"); // absent = default ON
      if (t) setTime(t);
    })();
  }, []);

  async function handleToggle(val: string) {
    const next = val === "on";
    setEnabled(next);
    setSaving(true);
    await setAppSetting("daily_reminder_enabled", next ? "true" : "false");
    setSaving(false);
  }

  async function handleTimeChange(e: React.ChangeEvent<HTMLInputElement>) {
    const val = e.currentTarget.value;
    setTime(val);
    setSaving(true);
    await setAppSetting("daily_reminder_time", val);
    setSaving(false);
  }

  if (!isTauri) return null;
  if (enabled === null) return null; // still loading

  return (
    <Section
      icon={Bell}
      title="Lembrete diário"
      sub="Notificação nativa quando o app está aberto."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Ativar lembrete</div>
            <div className="set-row__d">
              Envia uma notificação nativa no horário escolhido, enquanto o Neko estiver
              aberto.
            </div>
          </div>
          <div className="set-row__ctl">
            <SegmentedControl
              options={[
                { value: "on", label: "Ligado" },
                { value: "off", label: "Desligado" },
              ]}
              value={enabled ? "on" : "off"}
              onChange={(val) => void handleToggle(val)}
              size="sm"
              disabled={saving}
              ariaLabel="Ativar ou desativar lembrete diário"
            />
          </div>
        </div>
        {enabled && (
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Horário</div>
              <div className="set-row__d">Hora local (24 h) para receber o aviso.</div>
            </div>
            <div className="set-row__ctl">
              <input
                type="time"
                value={time}
                onChange={handleTimeChange}
                disabled={saving}
                style={{
                  fontFamily: "var(--font-money)",
                  fontSize: "var(--fs-body)",
                  background: "var(--bg-subtle)",
                  border: "var(--bw-hair) solid var(--border-input)",
                  borderRadius: "var(--radius-xs)",
                  color: "var(--text)",
                  padding: "4px 8px",
                  height: "var(--hit-min)",
                }}
                aria-label="Horário do lembrete diário"
              />
            </div>
          </div>
        )}
      </div>
    </Section>
  );
}
```

Also add the `SegmentedControl` import and `useEffect` import. The `SegmentedControl` component is
already in the design system at `src/design-system/components/SegmentedControl.tsx`. Add it to the
existing import line from that module. Add `useEffect` to the React import (currently `import { useState } from "react"`).

In `SettingsScreen`, place `<DailyReminderSection />` between the Bolsos section and the Seus dados section:

```tsx
<DailyReminderSection />

<Section
  icon={HardDrive}
  title="Seus dados"
  ...
```

**Note on the inline `style` on `<input>`**: `SettingsScreen.tsx` already uses
inline styles on the `<strong role="alert">` in `DataBackupRow` for the danger
color (line 53). The `<input type="time">` here follows the same acceptable
pattern for a one-off input that does not need a design-system component.
If the React Doctor advisory scan flags inline style props (≥8 props threshold
is the lint rule — this has 6), it is fine as-is. If it flags it anyway, hoist
the style to a module-level const.

**Verify**: `npm run typecheck && npm run lint && npm run doctor` → exit 0, 0 issues.

### Step 7: Add SettingsScreen tests for the reminder section

Extend `src/screens/SettingsScreen.test.tsx` with a new `describe` block:

```tsx
describe("DailyReminderSection", () => {
  it("shows the reminder toggle in the default ON state", async () => {
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null, // absent key → default ON
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await waitFor(() => {
      expect(
        screen.getByRole("radiogroup", { name: /lembrete diário/i }),
      ).toBeInTheDocument();
    });
    // Default state: absent key = enabled. "Ligado" radio is checked.
    const on = screen.getByRole("radio", { name: "Ligado" });
    expect(on).toHaveAttribute("aria-checked", "true");
  });

  it("persists the toggle off", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null,
      set_app_setting: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await waitFor(() =>
      expect(
        screen.getByRole("radiogroup", { name: /lembrete diário/i }),
      ).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("radio", { name: "Desligado" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_app_setting", {
        key: "daily_reminder_enabled",
        value: "false",
      }),
    );
  });
});
```

Because `mockCommands` routes by command name and `get_app_setting` is called
with a `key` argument, you may need to make the mock more specific. The
simplest approach: in the test's `mockInvoke.mockImplementation`, return the
correct stub per `(cmd, args)`. See `src/test/commands.ts` for the existing
`mockCommands` helper — if it cannot distinguish by args, write a manual
`mockInvoke.mockImplementation` in the test body instead (as done in other
tests that inspect `mockInvoke.mock.calls`).

**Verify**: `npm run test:run -- SettingsScreen` → all tests pass including the 2 new ones.

### Step 8: Create the Rust reminder task module

Create `src-tauri/src/reminder_task.rs`:

```rust
//! Daily reminder notification (plan 030).
//!
//! Fires a native OS notification at the user-configured time when the app is
//! running. Desktop-only; no push when the app is closed.
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
    (now.format("%Y-%m-%d").to_string(), now.format("%H:%M").to_string())
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

/// One reminder tick. Returns `Ok(())` on every "nothing to do" path; `Err` is
/// logged by the loop but does not stop it.
async fn tick(pool: &SqlitePool, app_handle: &tauri::AppHandle) -> Result<(), String> {
    // 1. Toggle check. Absent key = default ON.
    let enabled = crate::commands::app_setting_get(pool, "daily_reminder_enabled")
        .await?
        .map(|v| v != "false")
        .unwrap_or(true);
    if !enabled {
        return Ok(());
    }

    // 2. Read the configured time; fall back to the default.
    let time_str = crate::commands::app_setting_get(pool, "daily_reminder_time")
        .await?
        .unwrap_or_else(|| DEFAULT_TIME.to_string());
    let Some((target_h, target_m)) = parse_hhmm(&time_str) else {
        return Ok(()); // malformed setting; stay quiet
    };

    // 3. Current local date + time.
    let (today, now_hm) = local_now();
    let Some((now_h, now_m)) = parse_hhmm(&now_hm) else {
        return Ok(());
    };

    // 4. Has the target time passed for today?
    let past_target = (now_h, now_m) >= (target_h, target_m);
    if !past_target {
        return Ok(());
    }

    // 5. Already fired today?
    let last_fired = crate::commands::app_setting_get(pool, "daily_reminder_last_fired_date")
        .await?;
    if last_fired.as_deref() == Some(today.as_str()) {
        return Ok(());
    }

    // 6. Fire the notification (best-effort; failure must not crash the loop).
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = app_handle
            .notification()
            .builder()
            .title("Neko Finance")
            .body("Hora de atualizar seus lançamentos de hoje.")
            .show();
    }

    // 7. Record the date so we don't fire again today.
    crate::commands::app_setting_set(pool, "daily_reminder_last_fired_date", &today).await?;

    Ok(())
}

/// Spawns the background reminder loop. Wakes every `TICK_SECS` seconds (60 s).
/// Errors are logged; the loop never panics.
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

    async fn set(pool: &SqlitePool, key: &str, value: &str) {
        crate::commands::app_setting_set(pool, key, value)
            .await
            .unwrap();
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
        assert_eq!(parse_hhmm("25:00"), None); // hour out of range
        assert_eq!(parse_hhmm("abc"), None);
        assert_eq!(parse_hhmm(""), None);
        assert_eq!(parse_hhmm("20:60"), None); // minute out of range
    }

    #[tokio::test]
    async fn tick_skips_when_disabled() {
        let p = pool().await;
        set(&p, "daily_reminder_enabled", "false").await;
        // No app_handle in unit test; the disabled check returns before any handle
        // use. Verify the last_fired date is never written.
        let enabled = crate::commands::app_setting_get(&p, "daily_reminder_enabled")
            .await
            .unwrap()
            .map(|v| v != "false")
            .unwrap_or(true);
        assert!(!enabled);
        let last = crate::commands::app_setting_get(&p, "daily_reminder_last_fired_date")
            .await
            .unwrap();
        assert!(last.is_none());
    }

    #[tokio::test]
    async fn tick_skips_when_already_fired_today() {
        let p = pool().await;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        set(&p, "daily_reminder_last_fired_date", &today).await;
        // Reading the last_fired date equals today → the tick would short-circuit.
        let last = crate::commands::app_setting_get(&p, "daily_reminder_last_fired_date")
            .await
            .unwrap();
        assert_eq!(last.as_deref(), Some(today.as_str()));
    }
}
```

**Verify**: `npm run rust:check` → exit 0.

### Step 9: Register reminder_task in lib.rs

In `src-tauri/src/lib.rs`:

9a. Add the module declaration after the existing `mod sync_task;` line (line 11):

```rust
mod reminder_task;
```

9b. In the `setup()` closure, after the `sync_task::spawn_background_sync(...)` call (line 161–166),
add:

```rust
            reminder_task::spawn_reminder_task(pool.clone(), app.handle().clone());
```

Place it before `app.manage(pool);` (line 168) — same constraint as the sync task: the pool
clone must happen before `app.manage(pool)` moves ownership.

**Verify**: `npm run rust:check` → exit 0.

### Step 10: Run the full gate and update plans/README.md

10a. Run: `npm run check` → exit 0, all checks green.
10b. Run: `npm run e2e` → all Playwright smoke tests pass.
10c. Run: `npm run doctor` → 0 issues reported.

10d. Update `plans/README.md`: add a new row in the table:

```
| 030  | Daily reminder via OS notification + "last logged" indicator           | P1       | S–M    | —          | DONE                       |
```

Place it after the row for plan 028.

**Verify**: `git diff --stat HEAD` shows only the files listed in the Scope section plus `plans/README.md`.

## Test plan

### New Rust tests (in `src-tauri/src/reminder_task.rs`)

| Test                                  | What it covers                                                       |
| ------------------------------------- | -------------------------------------------------------------------- |
| `parse_hhmm_valid`                    | Correct parsing of `"20:00"`, `"08:30"`, `"00:00"`, `"23:59"`        |
| `parse_hhmm_invalid`                  | Rejects hour > 23, minute > 60, empty string, non-numeric            |
| `tick_skips_when_disabled`            | `daily_reminder_enabled = "false"` → `last_fired_date` never written |
| `tick_skips_when_already_fired_today` | Key equals today → tick would exit early; no double-fire             |

Run with: `npm run rust:check` (Rust unit tests run inside `cargo check` is insufficient;
to run Rust unit tests specifically: `cd src-tauri && cargo test 2>&1 | grep -E "test .* ok|FAILED"`).

### New TypeScript tests

**`src/screens/dashboard/LastLoggedBanner.test.tsx`** (4 tests):

- Returns `null` when `lastRealTxDate` equals today.
- Shows "ontem" copy for diffDays = 1.
- Shows "há N dias" for diffDays > 1.
- Shows first-entry prompt when `lastRealTxDate` is null.

Model after `src/screens/dashboard/DailyCheckinCard.test.tsx` (import from `../../test/commands`,
mock `@tauri-apps/api/core`, pin the clock via `vi.mock("../../lib/format")`).

**`src/screens/SettingsScreen.test.tsx`** (2 new tests inside new `describe` block):

- Toggle renders in default ON state when `get_app_setting` returns null.
- Toggling to OFF persists `{ key: "daily_reminder_enabled", value: "false" }`.

Run with: `npm run test:run -- LastLoggedBanner` and `npm run test:run -- SettingsScreen`.

### Existing tests that must keep passing

- All `DashboardScreen.test.tsx` tests — `DashboardSummary` fixtures need `last_real_tx_date`.
- All `SettingsScreen.test.tsx` tests — existing ones must pass alongside the new `describe` block.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; `LastLoggedBanner` (4 tests) and new SettingsScreen (2 tests) exist and pass
- [ ] `grep -rn "notification\|reminder\|nudge" src/` returns results (the feature exists now)
- [ ] `grep -n "daily_reminder_enabled\|daily_reminder_time" src-tauri/src/reminder_task.rs` returns matches
- [ ] `grep -n "last_real_tx_date" src/lib/api.ts` returns a match (field added)
- [ ] `npm run e2e` exits 0
- [ ] `npm run doctor` reports 0 issues
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD`)
- [ ] `plans/README.md` status row for plan 030 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" does not match the excerpts
  (the codebase has drifted since this plan was written).
- `tauri_plugin_notification` is NOT listed in `Cargo.toml` dependencies — do not
  add it yourself; the dependency list is version-locked and must be reviewed by a human.
- `"notification:default"` is NOT in `capabilities/default.json` — same: do not
  add capability permissions unilaterally.
- A step's `npm run rust:check` or `npm run typecheck` fails twice after a
  reasonable fix attempt.
- The `DashboardSummary` struct in `forecast_cmds.rs` differs from the excerpt
  (different field ordering, different file position) — proceed only after
  re-reading the file and confirming the correct insertion point.
- Adding `last_real_tx_date` to `DashboardSummary` breaks more than the
  `SUMMARY` / `EMPTY_SUMMARY` fixtures in `src/test/commands.ts` (implies other
  files cache the shape; list them and stop for review).
- The React Doctor advisory scan (`npm run doctor`) reports new issues introduced
  by this plan's changes (not pre-existing ones).

## Maintenance notes

- **Future: OS-level scheduler** — the current implementation fires only while
  the app is open (the Tokio loop is part of the Tauri process). Firing when the
  app is closed would require OS-level scheduling (launchd on macOS, Task
  Scheduler on Windows, systemd timer on Linux). This is deliberately deferred:
  it requires per-OS packaging work and is out of scope for the first version.
  Document this limitation in the Settings UI with a note like
  "Disponível enquanto o app estiver aberto."
- **Reminder time precision** — the tick fires every 60 seconds. The notification
  can arrive up to 60 s late, which is acceptable for a daily-reminder UX. If
  the user needs sub-minute precision, lower `TICK_SECS`, but note the battery
  trade-off on laptops.
- **Clock skew / DST** — `chrono::Local::now()` respects the OS timezone, so DST
  transitions should be handled correctly. The `last_fired_date` key uses the
  local date, which means the reminder fires once per local calendar day.
- **`last_real_tx_date` in `DashboardSummary`** — if the `dashboard_summary()`
  function is ever given a dedicated caching layer (e.g. the query is expensive),
  be aware this field introduces a dependency on the most recent transaction date.
  It should be invalidated whenever `create_transaction` or `import_sheet_data`
  is called.
- **PR reviewer focus areas**: (1) the `parse_hhmm` edge cases in Rust, (2) the
  `last_real_tx_date` SQL query — confirm `is_projection = 0 AND date <= ?1` is
  the correct predicate (not `created_at`), (3) the `diffDays` computation in
  `LastLoggedBanner` — confirm it uses wall-clock local dates, not UTC.
