# Plan 039: Daily ease — OS-scheduler reminder (fires when app closed) + 1-click Sincronizar fast path

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
>
> ```
> git diff --stat e62ecb6..HEAD -- \
>   src-tauri/src/reminder_task.rs \
>   src-tauri/src/lib.rs \
>   src/features/sheets/WriteBackPreview.tsx \
>   src/hooks/useWriteBackPending.ts \
>   src/screens/DashboardScreen.tsx
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: feature
- **Package**: D
- **Planned at**: commit `e62ecb6`, 2026-06-20

## Why this matters

The user's primary pain is daily-update consistency — recording everything
promptly so the method stays accurate. Two friction points actively work
against this:

1. The in-app reminder loop (`src-tauri/src/reminder_task.rs`) fires a native
   notification only while the desktop process is running. If the app is
   closed at the configured time (the common evening scenario), the nudge is
   silently lost — the feature that should encourage opening the app only
   works after it is already open.

2. Pushing a routine local change to the spreadsheet currently requires a
   5-step ceremony: click the banner → expand → "Gerar prévia do diff" →
   "Aprovar e enviar" → confirm dialog. When the change is a plain amount
   update (no formula-column involvement, no staleness race, no conflicts),
   that ceremony is disproportionate and discourages regular use.

Landing both improvements together tightens the "remember to log → open app
→ push change" loop that the method depends on.

## Current state

### Feature 1 — in-app-only reminder

`src-tauri/src/reminder_task.rs` — the existing Tokio background loop. Its
module docstring acknowledges the gap (lines 6–7):

```rust
// reminder_task.rs:6-7
//! Fires a native OS notification at the user-configured time when the app is
//! running. Desktop-only; no push when the app is closed (the loop lives inside
//! the desktop process — see Maintenance notes in the plan for the deferred
//! OS-scheduler approach).
```

The Tokio loop is spawned at startup (line 174 of `src-tauri/src/lib.rs`):

```rust
// lib.rs:171-174
// Daily reminder loop (plan 030): fires an OS notification at the user's
// configured time while the app is open. Clones the pool before `app.manage`
// moves it, same as the sync task above.
reminder_task::spawn_reminder_task(pool.clone(), app.handle().clone());
```

The two `app_setting` keys that the existing reminder reads (and that this
plan must reuse, never duplicate):

| Key                              | Default   | Purpose                                      |
| -------------------------------- | --------- | -------------------------------------------- |
| `daily_reminder_enabled`         | `"true"`  | Toggle; `"false"` disables all reminders.    |
| `daily_reminder_time`            | `"20:00"` | Local wall-clock time in `HH:MM` 24h format. |
| `daily_reminder_last_fired_date` | —         | ISO date; blocks same-day double-fire.       |

The existing `parse_hhmm` function (lines 38–46) parses the time string and
is testable in isolation — reuse it (or call it) in the new OS-scheduler
helper.

The notification plugin already initialised (line 21 of `lib.rs`):
`tauri_plugin_notification = "=2.3.3"`.

There is **no** existing OS-scheduler code anywhere in the Rust source.

### Feature 2 — multi-step write-back ceremony

The `WriteBackPreview` component (`src/features/sheets/WriteBackPreview.tsx`,
674 lines) implements the full multi-step flow:

- "Gerar prévia do diff" button (line 609–615)
- `GridDiffSection` with "Aprovar e enviar" button (line 314)
- `ConfirmDialog` (lines 182–229, native `<dialog>` with `showModal()`)
- Second `approve()` call to actually write (lines 488–511)

The `DashboardScreen` (`src/screens/DashboardScreen.tsx`) renders the
`WriteBackStatusBanner` and `WriteBackPreview` only when there is pending
count (lines 274–289):

```tsx
// DashboardScreen.tsx:274-289
{
  !writeBack.loading && writeBack.pendingCount > 0 && (
    <WriteBackStatusBanner
      pendingCount={writeBack.pendingCount}
      enabled={writeBack.enabled}
      expanded={showWriteBack}
      onToggle={() => setShowWriteBack((v) => !v)}
    />
  );
}

{
  showWriteBack && writeBack.spreadsheetId && writeBack.sheetName && (
    <WriteBackPreview
      spreadsheetId={writeBack.spreadsheetId}
      sheetName={writeBack.sheetName}
      clientId={writeBack.clientId}
    />
  );
}
```

The banner label (line 69): `"${pendingCount} célula(s) local → planilha pendente(s)"`.

The `useWriteBackPending` hook (`src/hooks/useWriteBackPending.ts`) already
computes `pendingCount`, `conflictCount`, `enabled`, `spreadsheetId`,
`sheetName`, and `clientId` (lines 21–38). The `previewWriteBackStatus`
result shape carries:

```ts
// api.ts (from src/lib/api.ts, around line 397-404)
// preview_revision: string   — Drive modifiedTime at preview time; write aborts if it advances
// conflicts_pending: boolean — frontend re-checks via getImportConflicts() in WriteBackPreview
// multi_card_warning: boolean
// cells: CellWrite[]
```

Safety invariants already in the backend (never bypass these):

- `guard_sheet_unchanged` (called by `apply_write_back`) aborts if
  `modifiedTime` advanced between preview and apply — staleness guard.
- `FORMULA_ONLY_FIELDS` (`write_back.rs` line 24): `["balance", "date"]` —
  the planner never emits `CellWrite` for formula columns, even if mappings
  include them.
- Conflict gate: `apply_write_back` rejects if unresolved conflicts remain
  (Rust-side; the frontend double-checks via `getImportConflicts`).
- `writeBackEnabled()` flag: the master switch.

A **safe** change is one where: `enabled === true`, `conflictCount === 0`,
`multi_card_warning === false`, `previewRevision` is fresh (i.e., the app
just computed it), and ALL `changed` cells have `kind` in
`["entrada", "saida", "diario"]` — i.e., amount-only non-formula cells.
The staleness guard still runs on the backend regardless; this only
collapses the UI steps for routine cases.

### Repo conventions

- Rust functional-core/imperative-shell: pure logic in free functions, IO
  in the outer async shell. See `reminder_task.rs:54-80` (`should_fire` is
  pure) and `write_back.rs:133-` (`plan_write_back` is pure).
- React Compiler ON: no manual `memo`; hoist static style objects as
  `const NAME: CSSProperties = { … }` outside the component. See
  `DashboardScreen.tsx:24-47` for the pattern.
- Money = positive-magnitude integer cents. UI formatting via `formatBRL`.
- Error handling in hooks: degrade gracefully to defaults, surface `error`
  string. See `useWriteBackPending.ts:113-125`.
- Tests: `vitest` + `@testing-library/react`. Model after
  `src/features/sheets/WriteBackPreview.test.tsx` (uses `mockCommands` /
  `mockInvoke`, `userEvent`).
- Method-neutral language: never name the external app/course/RE in code or
  comments. Use generic descriptions.
- New Tauri commands: add to `lib.rs`'s `tauri::generate_handler![]` array
  and expose via a `pub(crate) fn` in the appropriate `commands/` submodule.

## Commands you will need

| Purpose          | Command              | Expected on success         |
| ---------------- | -------------------- | --------------------------- |
| Typecheck        | `npm run typecheck`  | exit 0, no errors           |
| Lint             | `npm run lint`       | exit 0                      |
| Unit tests       | `npm run test:run`   | all pass                    |
| Rust checks      | `npm run rust:check` | exit 0 (check + clippy)     |
| React Doctor     | `npm run doctor`     | 0 findings (advisory only)  |
| Full gate        | `npm run check`      | exit 0                      |
| E2E visual smoke | `npm run e2e`        | all pass; inspect snapshots |

## Suggested executor toolkit

- Invoke `neko-finance-design` skill when adding new UI elements to ensure
  design-system token alignment.
- Read `src/design-system/components/Button.tsx` for the correct `variant`
  and `size` props before adding new buttons.
- Read `src-tauri/Cargo.toml` before adding any new Rust dependency.

## Scope

**In scope** (the only files you should modify):

**Feature 1 — OS-scheduler reminder:**

- `src-tauri/src/reminder_task.rs` — add `os_scheduler` submodule or helper
  functions that write/remove the platform-native scheduler entry; update
  module docstring; add a new `#[tauri::command]` `register_os_reminder`
  and `unregister_os_reminder`.
- `src-tauri/src/lib.rs` — register the new commands in `generate_handler!`.
- `src/lib/api.ts` — expose `registerOsReminder(time: string): Promise<void>`
  and `unregisterOsReminder(): Promise<void>`.
- The existing reminder-settings UI (wherever `daily_reminder_time` /
  `daily_reminder_enabled` are set) — add a call to `registerOsReminder`
  when the user saves a time and `unregisterOsReminder` when disabled.
  Locate that UI by searching for `daily_reminder_time` in `src/`.
- Relevant test file(s) for the new Rust logic (pure functions only — the
  subprocess launcher is not unit-testable without mocking the OS, see
  Step 2).

**Feature 2 — 1-click Sincronizar fast path:**

- `src/features/sheets/WriteBackPreview.tsx` — add `isSafeForFastPath`
  helper; add fast-path branch to `WriteBackStatusBanner`'s rendered state.
- `src/screens/DashboardScreen.tsx` — wire the fast-path button/flow in the
  banner area.
- `src/hooks/useWriteBackPending.ts` — add `multiCardWarning: boolean` to
  `WriteBackPendingState` (sourced from `previewWriteBackStatus`); or pass it
  through the preview result when the fast path fetches it.
- `src/features/sheets/WriteBackPreview.test.tsx` — add fast-path cases.

**Out of scope** (do NOT touch):

- `src-tauri/src/google_sheets/write_back.rs` — backend planner; the
  fast-path is purely a UI shortcut over the SAME Rust commands.
- `src-tauri/src/commands/write_back_cmds.rs` — backend apply logic;
  safety guards stay unchanged.
- Any migration file — no schema changes are needed; the OS scheduler uses
  the existing `app_setting` KV and file-system entries, not a new table.
- `src/features/sheets/GoogleSheetsPanel.tsx` — the full multi-step flow
  in Settings remains unchanged; the fast path is dashboard-only.
- Plans other than 039 in `plans/`.

## Git workflow

- Branch: `advisor/039-daily-ease-reminder-quicksync`
- Commit per logical unit (Feature 1 / Feature 2) so each can be reviewed
  independently; do NOT squash to one commit.
- Message style from recent history: `feat: <short description> (#<n>)` or
  `fix: <short description>`. Example: `feat: OS-scheduler reminder + 1-click Sincronizar fast path`.
- Do NOT push or open a PR unless the operator explicitly instructs it.

## Steps

---

### Step 1: Add the OS-scheduler Rust helper (macOS primary, Windows + Linux phased)

Create a new internal module `os_scheduler` inside `reminder_task.rs` (or as
a new file `src-tauri/src/os_scheduler.rs` — choose whichever keeps the file
size reasonable). The module must export two pure-ish functions:

```rust
/// Registers (or updates) the OS-level scheduled notification for the given
/// `HH:MM` local time. Idempotent — safe to call on every settings save.
/// Returns `Ok(())` on success; `Err(String)` with a human-readable reason on failure.
pub fn register(time_hhmm: &str) -> Result<(), String> { … }

/// Removes the OS-level scheduled entry. No-op if the entry does not exist.
pub fn unregister() -> Result<(), String> { … }
```

**Phase — primary platform: Windows** (the user's confirmed desktop target
per `docs/version-matrix.md` / the Windows build pipeline). Use `schtasks`
(Task Scheduler CLI, present on all modern Windows versions) to create a
one-time daily trigger:

```
schtasks /Create /F /SC DAILY /TN "NekoFinance\DailyReminder" /ST <HH:MM>
  /TR "powershell -WindowStyle Hidden -Command
       [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime];
       ..."
```

Because spawning a Toast from `powershell` in a background task is verbose
and unreliable across editions, the simpler approach is to point `/TR` at the
app's own executable with a `--remind` CLI flag (see Step 2). The scheduler
entry calls the binary; the binary fires the notification and exits.

**macOS — phased follow-up** (flag explicitly at the bottom of this plan):
Write a `launchd` plist to `~/Library/LaunchAgents/com.nekofinance.reminder.plist`
and call `launchctl load`. Mark this as a TODO in the code:
`// TODO plan-039-phase2: macOS launchd plist`.

**Linux — phased follow-up**: Use `systemd --user` timer or `crontab` edit.
Mark: `// TODO plan-039-phase2: Linux systemd-timer / crontab`.

Use `#[cfg(target_os = "windows")]` / `#[cfg(target_os = "macos")]` /
`#[cfg(target_os = "linux")]` to gate the three branches. If the current
platform has no implementation, `register` should return `Ok(())` silently
(the in-app loop already covers it as fallback) and log a `eprintln!` so
CI on non-Windows knows what was skipped.

**Verify**: `npm run rust:check` → exit 0, no clippy warnings on the new
code. On a non-Windows CI runner, the Windows-specific code must compile
(gate it with `cfg` so it is excluded from non-Windows builds, not with
`if cfg!(...)` at runtime which still requires the symbols to link).

---

### Step 2: Add a `--remind` CLI entry point (the target of the scheduler)

When the app is started with `--remind` as a CLI argument, fire one
notification and exit immediately — never open a window.

In `src-tauri/src/main.rs` (or the binary entry), check `std::env::args()`
before `app::run()`:

```rust
fn main() {
    if std::env::args().any(|a| a == "--remind") {
        // Fire a native notification via a minimal notifier and exit.
        // On Windows: use the `winrt-notification` crate or `notify-rust`,
        // whichever is already in Cargo.toml (check first; add only if absent).
        // Body: "Hora de atualizar seu diário."  (same as reminder_task::tick)
        fire_standalone_notification();
        return;
    }
    neko_finance_lib::run();
}
```

Keep `fire_standalone_notification()` as a private function in `main.rs`
(not in `lib.rs` — it is process-level glue, not reusable domain logic).
Use `eprintln!` for any error in this path (no window available).

If adding a new crate dependency: check `src-tauri/Cargo.toml` first;
prefer reusing `tauri_plugin_notification`'s types if they are accessible
without a full Tauri `AppHandle`. If not, `notify-rust` (cross-platform,
MIT) is the conventional fallback for standalone use.

**Verify**: `npm run rust:check` → exit 0. Manually test (on the dev
machine) that `./target/debug/neko-finance --remind` shows a notification
and exits. Add a note in the PR description if the standalone path cannot be
tested in CI (it requires a desktop session).

---

### Step 3: Expose `register_os_reminder` / `unregister_os_reminder` as Tauri commands

In `src-tauri/src/commands/write_back_cmds.rs` (or a new
`src-tauri/src/commands/reminder_cmds.rs`), add:

```rust
/// Registers (or updates) the OS-level scheduled reminder at `time_hhmm` (HH:MM, 24h).
/// Idempotent. Returns Err if the OS call fails (surfaced as a toast in the UI).
#[tauri::command]
pub async fn register_os_reminder(time_hhmm: String) -> Result<(), String> {
    crate::os_scheduler::register(&time_hhmm)
}

/// Removes the OS-level scheduled reminder. No-op if not registered.
#[tauri::command]
pub async fn unregister_os_reminder() -> Result<(), String> {
    crate::os_scheduler::unregister()
}
```

Register both in `lib.rs`'s `tauri::generate_handler![]` (follow the pattern
of existing entries, e.g. `tags::create_tag_cmd`, `commands::backup_database`).

**Verify**: `npm run rust:check` → exit 0. The new command names must
appear in `generate_handler![]` — search `lib.rs` to confirm.

---

### Step 4: Expose the commands in `src/lib/api.ts`

Follow the existing pattern (e.g. `writeBackEnabled` at api.ts:409):

```ts
/** Registers (or updates) the OS-level scheduled reminder at the given HH:MM time. */
export function registerOsReminder(timeHhmm: string): Promise<void> {
  return invoke<void>("register_os_reminder", { timeHhmm });
}

/** Removes the OS-level scheduled reminder entry. */
export function unregisterOsReminder(): Promise<void> {
  return invoke<void>("unregister_os_reminder");
}
```

Find the reminder-settings UI that writes `daily_reminder_time` to locate
the call site:

```
grep -rn "daily_reminder_time\|daily_reminder_enabled" src/
```

In that settings component, call `registerOsReminder(time)` after a
successful `setAppSetting("daily_reminder_time", time)`, and
`unregisterOsReminder()` after `setAppSetting("daily_reminder_enabled", "false")`.
Handle errors by surfacing a user-visible string (same pattern as OAuth
error surfacing in `GoogleSheetsPanel.tsx`). Do NOT block the settings save
on the OS call — the in-app loop is the fallback; the OS-level registration
is best-effort.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

---

### Step 5: Unit-test the pure OS-scheduler logic

The subprocess invocation is not unit-testable, but the input validation and
command-string assembly are. Add tests in `reminder_task.rs` (or the new
`os_scheduler.rs`) for:

- `register` called with a valid `"HH:MM"` string produces the expected
  `schtasks` arguments (expose a pure `build_schtasks_args(time: &str, exe: &str) -> Vec<String>` helper).
- `register` called with a malformed time string returns `Err(...)`.
- `unregister` produces the expected removal arguments.

Follow the pattern of the existing pure-function tests in
`reminder_task.rs:146-283`.

**Verify**: `npm run rust:check` → exit 0, no test failures.

---

### Step 6: Add `isSafeForFastPath` helper to `WriteBackPreview.tsx`

The fast path is safe when ALL of the following hold:

1. `enabled === true` (master write-back flag).
2. `conflictCount === 0` (no import conflicts blocking the write).
3. `multiCardWarning === false` (no ambiguous due-date scenario).
4. All `changed` cells have `kind` in `["entrada", "saida", "diario"]`
   (no formula-adjacent kind leaks through).
5. `previewRevision` is non-empty (a fresh preview was just computed).

Implement as a pure function at the top of `WriteBackPreview.tsx` (outside
any component, so it is tree-shakeable and testable):

```ts
const SAFE_KINDS = new Set(["entrada", "saida", "diario"]);

/** Returns true when the pending diff is safe to push via the 1-click fast path. */
export function isSafeForFastPath(
  enabled: boolean,
  conflictCount: number,
  multiCardWarning: boolean,
  changed: CellWrite[],
  previewRevision: string | null,
): boolean {
  return (
    enabled &&
    conflictCount === 0 &&
    !multiCardWarning &&
    changed.length > 0 &&
    changed.every((c) => SAFE_KINDS.has(c.kind)) &&
    !!previewRevision
  );
}
```

Note: `WriteBackPendingState` (in `useWriteBackPending.ts`) does not yet
expose `multiCardWarning`. The fast-path evaluation happens INSIDE
`WriteBackPreview` after `previewWriteBackStatus` resolves (where
`multi_card_warning` is already available), not from the hook — so no hook
change is required for the safety check itself.

**Verify**: `npm run typecheck` → exit 0. The function is exported so tests
can import it directly.

---

### Step 7: Wire the 1-click fast path in `DashboardScreen.tsx`

The goal: when `pendingCount > 0` and the diff is safe, the
`WriteBackStatusBanner` shows a single "Sincronizar" button instead of the
"Revisar e enviar" toggle. One click fires `previewWriteBackStatus` silently,
checks safety, shows an inline diff summary (cell count + kinds), then writes
on one confirm (the existing `ConfirmDialog` reused, not bypassed — the safety
check replaces the "generate preview" step, not the human confirmation).

**Implementation shape** (in `DashboardScreen.tsx`):

Add a new local state `fastPathResult` next to `showWriteBack`:

```tsx
const [fastPathResult, setFastPathResult] = useState<{
  changed: CellWrite[];
  previewRevision: string;
  confirm: boolean;
} | null>(null);
```

Add `handleSincronizar`:

```tsx
async function handleSincronizar() {
  // 1. Fetch preview silently (re-uses the same API call as WriteBackPreview).
  const result = await previewWriteBackStatus(
    writeBack.spreadsheetId,
    writeBack.sheetName,
    writeBack.clientId,
  );
  const changed = result.cells.filter((c) => c.changed);
  const safe = isSafeForFastPath(
    writeBack.enabled,
    writeBack.conflictCount,
    result.multi_card_warning,
    changed,
    result.preview_revision,
  );
  if (!safe) {
    // Fall back to the full flow (expand the WriteBackPreview panel).
    setShowWriteBack(true);
    return;
  }
  setFastPathResult({
    changed,
    previewRevision: result.preview_revision,
    confirm: true, // open the confirm dialog immediately
  });
}
```

When `fastPathResult?.confirm === true`, render the existing `ConfirmDialog`
(import it from `WriteBackPreview.tsx` — make it an exported component). On
confirm, call `applyWriteBack(...)` with the `previewRevision` from
`fastPathResult`, then call `writeBack.refresh()` and clear `fastPathResult`.

Inline diff summary: between the banner and the confirm dialog, show a
compact `<output>` listing the changed cells (e.g.
`"3 célula(s): Diário 01/06, Saída 15/06, Entrada 20/06"`). Use the existing
`formatBRL` and `ApprovalDiffCard` component if space allows, or a single
line summary if the cell count is ≤ 5; use `ApprovalDiffCard` for > 5 cells.

**The full multi-step `WriteBackPreview` panel (the existing toggle flow)
is preserved for all non-safe cases and remains accessible via the "Revisar
e enviar" fallback path.**

**Important**: `ConfirmDialog` must be exported from `WriteBackPreview.tsx`
(`export function ConfirmDialog`) so `DashboardScreen.tsx` can import it
without duplicating the component.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

---

### Step 8: Add tests for the fast-path

In `src/features/sheets/WriteBackPreview.test.tsx`, add a describe block
`"isSafeForFastPath"`:

```ts
import { isSafeForFastPath } from "./WriteBackPreview";

describe("isSafeForFastPath", () => {
  const SAFE_CHANGED: CellWrite[] = [
    {
      a1: "E3",
      row: 2,
      col: 4,
      date: "2026-06-01",
      kind: "diario",
      current: "50,00",
      proposed: "75,00",
      value_cents: 7500,
      changed: true,
    },
  ];

  it("returns true for a clean amount-only diff", () => {
    expect(isSafeForFastPath(true, 0, false, SAFE_CHANGED, "rev-1")).toBe(true);
  });

  it("returns false when disabled", () => {
    expect(isSafeForFastPath(false, 0, false, SAFE_CHANGED, "rev-1")).toBe(false);
  });

  it("returns false when conflict count > 0", () => {
    expect(isSafeForFastPath(true, 1, false, SAFE_CHANGED, "rev-1")).toBe(false);
  });

  it("returns false when multiCardWarning is true", () => {
    expect(isSafeForFastPath(true, 0, true, SAFE_CHANGED, "rev-1")).toBe(false);
  });

  it("returns false when changed list is empty", () => {
    expect(isSafeForFastPath(true, 0, false, [], "rev-1")).toBe(false);
  });

  it("returns false when previewRevision is null or empty", () => {
    expect(isSafeForFastPath(true, 0, false, SAFE_CHANGED, null)).toBe(false);
    expect(isSafeForFastPath(true, 0, false, SAFE_CHANGED, "")).toBe(false);
  });

  it("returns false when any changed cell has a non-safe kind", () => {
    const withBalance: CellWrite[] = [
      ...SAFE_CHANGED,
      { ...SAFE_CHANGED[0], kind: "balance", a1: "F3", col: 5 },
    ];
    expect(isSafeForFastPath(true, 0, false, withBalance, "rev-1")).toBe(false);
  });
});
```

Also add an integration test in the same file: render a minimal
`WriteBackStatusBanner`-equivalent (or the `DashboardScreen` with mocked
commands) and assert that when `preview_write_back_status` returns a safe
diff, clicking "Sincronizar" opens the confirm dialog without showing the
full `WriteBackPreview` panel, and that after confirm, `apply_write_back` is
called exactly once with the correct `preview_revision`.

**Verify**: `npm run test:run` → all pass, including the new `isSafeForFastPath`
tests (at minimum 7 new test cases).

---

### Step 9: Full gate

Run the complete check suite:

```
npm run check
```

Expected: exit 0. All lint, typecheck, tests, rust:check, doctor, privacy:scan
must pass.

Run E2E visual smoke:

```
npm run e2e
```

Inspect the screenshots or traces for any visual regression in the dashboard
(the banner area should still look correct with no pending write-back, with
a safe diff, and with an unsafe diff).

**Verify**: `npm run check` → exit 0. `npm run e2e` → all pass, no new
visual regressions in the dashboard screenshots.

---

## Test plan

### Rust (pure-function unit tests)

New tests in `src-tauri/src/reminder_task.rs` (or `os_scheduler.rs`):

- `build_schtasks_args_valid_time` — given `"20:00"` and a known exe path, asserts the
  expected argument vector (task name, trigger, `/TR` value).
- `build_schtasks_args_malformed_time_returns_err` — `"not-a-time"` returns `Err`.
- `build_unregister_args` — produces the expected `/Delete` argument vector.

Pattern: follow `reminder_task.rs:146-283` (pure `#[test]` blocks, no async,
no filesystem).

### TypeScript (unit tests, `vitest`)

New test block `"isSafeForFastPath"` in `WriteBackPreview.test.tsx` — 7
cases listed in Step 8.

Integration test: fast-path confirm → `apply_write_back` called exactly once
(use `mockCommands` / `mockInvoke` pattern already in that file).

Regression: full multi-step flow still works when `isSafeForFastPath` returns
`false` (fall-back expansion of `WriteBackPreview` panel).

Run: `npm run test:run` — all pass, including ≥ 10 new test cases total.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0; new Rust tests pass.
- [ ] `npm run typecheck` exits 0.
- [ ] `npm run test:run` exits 0; at least 7 new `isSafeForFastPath` tests exist
      and pass; the `build_schtasks_args_valid_time` test exists and passes.
- [ ] `npm run lint` exits 0.
- [ ] `npm run doctor` exits 0 (no new findings).
- [ ] `npm run e2e` passes; dashboard screenshots show no regression.
- [ ] `npm run check` exits 0 (full gate).
- [ ] `isSafeForFastPath` is exported from `WriteBackPreview.tsx`.
- [ ] `ConfirmDialog` is exported from `WriteBackPreview.tsx`.
- [ ] `register_os_reminder` and `unregister_os_reminder` appear in
      `lib.rs`'s `tauri::generate_handler![]`.
- [ ] The in-app Tokio loop (`reminder_task::spawn_reminder_task`) remains
      unchanged and is still called at startup.
- [ ] No new `app_setting` keys introduced other than the existing three
      (`daily_reminder_enabled`, `daily_reminder_time`,
      `daily_reminder_last_fired_date`).
- [ ] macOS and Linux OS-scheduler implementations are marked as TODO with the
      `// TODO plan-039-phase2:` tag so they are findable.
- [ ] `plans/README.md` status row for plan 039 updated to DONE (or
      IN PROGRESS if partially landed).

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the "Current state" locations does not match the excerpts —
  `reminder_task.rs:6-7`, `lib.rs:171-174`, `DashboardScreen.tsx:274-289`
  differ from the plan (the codebase drifted).
- The `apply_write_back` or `preview_write_back_status` Tauri command
  signatures changed (the fast path calls them directly; a signature change
  requires re-evaluation of the fast-path flow).
- `isSafeForFastPath` logic requires inspecting formula-field logic on the
  Rust side at runtime (it must NOT cross the Tauri boundary — if the backend
  needs to be queried for safety, stop and re-evaluate).
- A step's `npm run rust:check` or `npm run typecheck` fails twice after a
  reasonable fix attempt.
- Adding the `--remind` CLI flag requires modifying `src-tauri/src/lib.rs`'s
  `run()` in a way that conflicts with any in-progress plan (check git status).
- The Windows Task Scheduler entry requires elevated privileges to register
  for the current user — escalation is not acceptable; in that case use a
  user-level `schtasks` call with `/RL LIMITED` or fall back to a startup
  Registry entry instead, and note the change in the PR.
- Any safety guard in `write_back_cmds.rs` (staleness check, conflict gate,
  formula-field blocklist) would need to be weakened to implement the fast path
  — stop immediately; the fast path must never weaken backend safety.

## Maintenance notes

- **The in-app Tokio loop is kept as the fallback** for all platforms and for
  users who never change the reminder settings (the OS-scheduler entry is only
  written when the user explicitly saves a time). Do not remove it.
- **macOS + Linux OS-scheduler are deliberately phased out of this plan.**
  Search `// TODO plan-039-phase2:` to find the stubs. A follow-up plan
  (`040-reminder-macos-linux.md`) should implement the `launchd` plist write
  and the `systemd --user` timer.
- **The `ConfirmDialog` export** is the only behavioral change to
  `WriteBackPreview.tsx` visible to `GoogleSheetsPanel.tsx` consumers. It is
  backward-compatible (existing callers are unaffected). If `WriteBackPreview`
  is later refactored, ensure `ConfirmDialog` stays independently importable.
- **Reviewer checklist for the PR**:
  - Verify that `isSafeForFastPath` is called with the `previewRevision` from
    the _just-fetched_ preview, not a stale one from `useWriteBackPending`
    (the hook does not cache `previewRevision`).
  - Verify that `apply_write_back` is called with `previewRevision` forwarded
    to the backend (so the staleness guard has a token to compare against).
  - Verify the fallback branch (`!safe → setShowWriteBack(true)`) is tested.
  - On Windows: verify the Task Scheduler entry is created with a user-level
    (non-elevated) scope and uses the correct path to the installed binary.
- **If the user later changes the reminder time**, the UI must call
  `registerOsReminder(newTime)` again (idempotent — it overwrites the entry).
  If the user disables reminders, call `unregisterOsReminder()`. If the user
  uninstalls the app, the Task Scheduler entry will remain as an orphan —
  document this in the app's uninstall notes or add an uninstall hook
  (`tauri_build` allows registering an uninstall command).
