# Plan 042: Write-back/sync correctness: Economia audit id, fast-path invalidate, preview TOCTOU

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
> git diff --stat d3922d2..HEAD -- \
>   src-tauri/src/commands/write_back_cmds.rs \
>   src/screens/dashboard/WriteBackPending.tsx
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d3922d2`, 2026-06-20

## Why this matters

Three write-back correctness bugs cause spurious import conflicts and stale
finance numbers after a 1-click sync. The Economia audit id bug (Bug 1) is
described in the finding brief as `economia:YYYY-MM` vs `economia:YYYY-MM-DD`,
but **re-verification against the live code shows both sides already produce
`economia:YYYY-MM`** (see "Drift note" in Current state). The plan therefore
focuses the id-mismatch fix as a hardening step (add a regression test and a
code-level assert) rather than a code change. The two confirmed bugs are: the
fast-path write (`confirmFastWrite`) calls `writeBack.refresh()` but omits
`invalidateCommands()`, leaving Dashboard/MonthLedger/Totais showing stale
metrics until navigation; and `preview_write_back_status` fetches sheet values
before fetching `modifiedTime`, so the preview_revision token can correspond to
a state AFTER the diff was computed (TOCTOU), allowing a stale-approved diff to
pass the staleness gate.

## Current state

### Files and their roles

- `src-tauri/src/commands/write_back_cmds.rs` — Tauri commands for write-back:
  `preview_write_back_status`, `apply_write_back`, `apply_economia_write_back`,
  `record_write_back_audit`, `store_economia_entries`.
- `src-tauri/src/google_sheets/write_back.rs` — Pure core: `CellWrite`,
  `plan_write_back`, `plan_economia_write_back`.
- `src/screens/dashboard/WriteBackPending.tsx` — Dashboard banner with fast-path
  "Sincronizar" button (`confirmFastWrite`).

### Bug 1 (Economia audit id) — DRIFT NOTE

The advisor brief described a mismatch: `record_write_back_audit` using
`format!("economia:{}", c.date)` where `c.date` is `"YYYY-MM"`, but
`store_economia_entries` creating ids as `"economia:YYYY-MM-DD"`.

**Re-verification against live code shows no mismatch:**

`write_back_cmds.rs:973` (inside `store_economia_entries`):

```rust
// write_back_cmds.rs:970-973
for (year, month, cents) in entries {
    let last = forecast::last_day_of_month(*year, *month);
    let date = last.format("%Y-%m-%d").to_string();  // transaction date field (YYYY-MM-DD)
    let id = format!("economia:{year:04}-{month:02}");  // id = "economia:YYYY-MM"
```

`write_back_cmds.rs:627-635` (inside `record_write_back_audit`, "economia" arm):

```rust
// write_back_cmds.rs:622-635
"economia" => {
    // Economia é mensal: a célula carrega `date = "YYYY-MM"` ...
    sqlx::query(
        "UPDATE \"transaction\" SET source_amount = ?1, updated_at = ?2 \
         WHERE id = ?3 AND type = 'transfer'",
    )
    .bind(c.value_cents)
    .bind(&now)
    .bind(format!("economia:{}", c.date))  // c.date = "YYYY-MM" → "economia:YYYY-MM" ✓
    .execute(&mut *tx)
    .await
}
```

`google_sheets/write_back.rs:389` (inside `plan_economia_write_back`):

```rust
// write_back.rs:385-398
out.push(CellWrite {
    a1: format!("{}{}", col_to_a1(econ_col), r + 1),
    row: r,
    col: econ_col,
    date: format!("{year}-{month:02}"),  // "YYYY-MM", not "YYYY-MM-DD"
    kind: "economia".to_string(),
    ...
});
```

Both sides produce `"economia:YYYY-MM"` — they match. The advisor brief confused
the transaction `date` field (`YYYY-MM-DD`, last day of month, used for ledger
display) with the transaction `id` (`YYYY-MM`, used as the audit target). The
code is already correct; the regression test below ensures it stays correct.

### Bug 2 (fast-path doesn't invalidate finance caches) — CONFIRMED

`src/screens/dashboard/WriteBackPending.tsx:187-211` (`confirmFastWrite`):

```tsx
// WriteBackPending.tsx:187-211
async function confirmFastWrite() {
  if (!fastPath || applyingFastRef.current) return;
  applyingFastRef.current = true;
  try {
    await applyWriteBack(
      writeBack.spreadsheetId,
      writeBack.sheetName,
      writeBack.clientId,
      fastPath.previewRevision,
    );
    applyingFastRef.current = false;
    setFastPath(null);
    writeBack.refresh();           // ← only refreshes pending count
    // ← MISSING: invalidateCommands()
  } catch (e) {
    ...
  }
}
```

Pattern from `src/screens/TransactionsScreen.tsx:638-641` (`handleSaved`):

```tsx
// TransactionsScreen.tsx:638-641
function handleSaved() {
  invalidateCommands(); // finance numbers changed — drop every cached screen
  dispatchUi({ type: "editDone" });
}
```

`invalidateCommands` is imported at `TransactionsScreen.tsx:30` from
`"../lib/useCommand"`. The same import pattern is already used in
`GoogleSheetsPanel.tsx:39,355,413,448`. The fast-path write changes the same
finance data as any import, so it requires the same cache invalidation.

### Bug 3 (preview modifiedTime TOCTOU) — CONFIRMED

`src-tauri/src/commands/write_back_cmds.rs:302-329` (`preview_write_back_status`):

```rust
// write_back_cmds.rs:311-329
#[tauri::command]
pub async fn preview_write_back_status(
    ...
) -> Result<WriteBackPreviewResult, String> {
    let (client, cells) = build_write_back_plan(   // ← fetches VALUES at T1 (line 246 inside)
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;
    let preview_revision = client.get_file_modified_time(&spreadsheet_id).await?; // ← modifiedTime at T2 > T1
    ...
}
```

`build_write_back_plan` fetches values inside at `write_back_cmds.rs:246`:

```rust
// write_back_cmds.rs:243-246
let client = SheetsClient::new(token);
let range = quote_sheet(sheet_name);
let values = client.get_sheet_values(spreadsheet_id, &range).await?;  // T1
```

A concurrent sheet edit between T1 and T2 means `preview_revision` (T2) is newer
than the state the diff was computed from (T1). On apply, `guard_sheet_unchanged`
compares apply-time modifiedTime against `preview_revision` (T2). If no further
edit occurs after T2, the check passes even though the approved diff is stale.

The same TOCTOU exists in `preview_economia_write_back_status` at lines 849-865:

```rust
// write_back_cmds.rs:849-865
pub async fn preview_economia_write_back_status(...) {
    let (client, cells) = build_economia_plan(...).await?;   // fetches values inside (T1)
    let preview_revision = client.get_file_modified_time(&spreadsheet_id).await?; // T2 > T1
    ...
}
```

Fix: call `get_file_modified_time` BEFORE the function that fetches values, so
`preview_revision` is the modifiedTime corresponding to (or earlier than) the
diffed state. A modifiedTime fetched before values is conservative: if values
have not changed between the modifiedTime fetch and the values fetch, the token
is correct; if they have, the apply-time modifiedTime will be newer and the gate
will trigger re-preview. This avoids approving a stale diff.

`build_write_back_plan` returns a `SheetsClient`, so the caller can reuse it for
`get_file_modified_time`. But to fetch modifiedTime BEFORE calling
`build_write_back_plan`, we need a client BEFORE the plan is built. Extract a
helper `make_sheets_client` that creates and returns an authenticated client, or
restructure the two commands to fetch modifiedTime as the first RPC after
authentication.

Alternatively, the simplest approach: add a `get_file_modified_time` call at the
TOP of `build_write_back_plan` (before `get_sheet_values`) and return the
timestamp alongside the client and plan. This keeps the change contained. See
Step 3.

### Conventions

- All `unsafe` / `async` Tauri command signatures follow the pattern in
  `write_back_cmds.rs`.
- Frontend imports follow: `import { invalidateCommands } from "../../lib/useCommand"`.
- React Compiler is ON: no manual memo, no inline-object literals in JSX — hoist
  static styles as `const` outside the component. `confirmFastWrite` already
  follows this rule.
- Money = positive-magnitude integer cents. No floats in business logic.
- Do NOT bypass, weaken, or remove any safety gate from plan 028 (flag,
  conflict gate, scope check, staleness check, blocklist). All gates must remain
  intact and run BEFORE any write.

## Commands you will need

| Purpose             | Command                                     | Expected on success              |
| ------------------- | ------------------------------------------- | -------------------------------- |
| Rust check          | `npm run rust:check`                        | exit 0, no errors                |
| Typecheck           | `npm run typecheck`                         | exit 0, no errors                |
| Unit tests          | `npm run test:run`                          | all pass                         |
| Filtered Rust tests | `cargo test -p neko-finance-lib write_back` | all pass (run from `src-tauri/`) |
| Full gate           | `npm run check`                             | exit 0                           |

## Suggested executor toolkit

- No special skills required. All changes are in files you can read directly.
- Read `src-tauri/src/google_sheets/write_back.rs` before touching the
  `CellWrite`/`plan_economia_write_back` shape; the plan does not require changes
  to that file but the executor should understand the data flow.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/write_back_cmds.rs` — Bug 1 regression test (inside
  `#[cfg(test)]`), Bug 3 TOCTOU fix.
- `src/screens/dashboard/WriteBackPending.tsx` — Bug 2 `invalidateCommands()` call.
- `src/screens/dashboard/WriteBackPending.test.tsx` (create) — Bug 2 regression
  test.

**Out of scope** (do NOT touch, even if it looks related):

- `src-tauri/src/google_sheets/write_back.rs` — pure core; no changes needed.
- `src/features/sheets/WriteBackPreview.tsx` / `GoogleSheetsPanel.tsx` — full
  review/apply flow; already correct.
- Plan 028 safety gates (`guard_no_pending_conflicts`, `guard_sheet_unchanged`,
  `ensure_write_back_enabled`, `ensure_write_scope`) — do NOT weaken, bypass, or
  remove any of them.
- Any Tauri command other than `preview_write_back_status` and
  `preview_economia_write_back_status`.

## Git workflow

- Branch: `advisor/042-writeback-sync-correctness`
- Commit style: `fix:` prefix (matches recent history, e.g. `fix: revisão completa
da app`). One commit per logical unit (Rust fix, frontend fix, tests).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Drift check

Run the drift check from the header:

```bash
git diff --stat d3922d2..HEAD -- \
  src-tauri/src/commands/write_back_cmds.rs \
  src/screens/dashboard/WriteBackPending.tsx
```

Then open `src-tauri/src/commands/write_back_cmds.rs` and confirm:

- Line 633: `format!("economia:{}", c.date)` (economia arm of `record_write_back_audit`)
- Line 973: `let id = format!("economia:{year:04}-{month:02}");` (inside `store_economia_entries`)
- Line 320: `let preview_revision = client.get_file_modified_time(...)` (inside `preview_write_back_status`, AFTER `build_write_back_plan`)
- Line 858: same pattern in `preview_economia_write_back_status`

Open `src/screens/dashboard/WriteBackPending.tsx` and confirm:

- Line 199: `writeBack.refresh()` with no `invalidateCommands()` call in `confirmFastWrite`

If any excerpt doesn't match, STOP and report.

**Verify**: `git diff --stat d3922d2..HEAD -- src-tauri/src/commands/write_back_cmds.rs src/screens/dashboard/WriteBackPending.tsx` → any output is acceptable (shows drift to report), but code at the above lines must match for the plan to proceed.

### Step 2: Bug 1 regression test (Rust) — Economia audit id round-trip

Add a `#[tokio::test]` inside the `#[cfg(test)]` block at the bottom of
`src-tauri/src/commands/write_back_cmds.rs` (after the existing
`credit_lump_writeback_realigns_source_amount` test, around line 1163).

The test:

1. Inserts a profile, a reserve account, and a transfer transaction with
   `id = "economia:2026-06"` (as `store_economia_entries` would create it),
   with a non-zero `source_amount`.
2. Builds a `CellWrite` with `kind = "economia"` and `date = "2026-06"`.
3. Calls `record_write_back_audit(&p, "Economia", &[&cell]).await`.
4. Asserts that exactly 1 row was realigned.
5. Queries `source_amount` for `id = "economia:2026-06"` and asserts it equals
   the written `value_cents`.

Name the test: `economia_write_back_audit_realigns_source_amount`.

The test proves that `format!("economia:{}", c.date)` with `c.date = "2026-06"`
matches the stored id `"economia:2026-06"`, so no rows are missed. If a future
refactor of either `store_economia_entries` or `plan_economia_write_back`
changes the id/date format asymmetrically, this test will catch it.

Model the test setup after the existing
`credit_lump_writeback_realigns_source_amount` test: use the same `pool()` async
helper, the same inline `sqlx::query` inserts, and the same assertion pattern.

The `CellWrite` shape (from `google_sheets/write_back.rs:57-77`):

```rust
CellWrite {
    a1: "C3".into(),         // arbitrary; audit doesn't use a1
    row: 2, col: 2,
    date: "2026-06".into(),  // YYYY-MM — what plan_economia_write_back sets
    kind: "economia".into(),
    current: "0,00".into(),
    proposed: "150,00".into(),
    value_cents: 15000,
    changed: true,
    formula: None,
    note_text: None,
}
```

The INSERT for the economia transaction must include all NOT NULL columns; model
after the existing test's INSERT pattern. Use `source_amount = 99999` (a sentinel
not equal to `value_cents = 15000`) so the assert on the updated value is
unambiguous.

Also insert a `sync_log` dependency: `record_write_back_log` inside the audit
needs a `profile_id` FK. The pool() helper runs migrations which create the
`sync_log` table. Insert a profile row so the log INSERT succeeds (same pattern
as existing test: `INSERT INTO person (id, name) VALUES ('pe-test', 'Tester')`
and a profile row if the migration requires one — check the migration DDL).

**Verify**: `cargo test -p neko-finance-lib economia_write_back_audit_realigns_source_amount` (run from `src-tauri/`) → 1 test passed.

### Step 3: Bug 3 fix — preview TOCTOU (Rust)

In `src-tauri/src/commands/write_back_cmds.rs`, restructure BOTH
`preview_write_back_status` (around line 303) and
`preview_economia_write_back_status` (around line 841) to fetch the
`modifiedTime` BEFORE the function that fetches sheet values.

**For `preview_write_back_status`:**

The current order (lines 311-320):

```rust
let (client, cells) = build_write_back_plan(...).await?;  // values fetched inside
let preview_revision = client.get_file_modified_time(&spreadsheet_id).await?;
```

The fix requires an authenticated `SheetsClient` before `build_write_back_plan`.
`build_write_back_plan` internally creates the client; to get it earlier, add a
small private helper function `make_sheets_client` that authenticates and returns
a `SheetsClient`, OR — simpler — move `get_file_modified_time` to be the first
RPC after authentication by restructuring `build_write_back_plan` to accept an
optional pre-fetched `preview_revision` out-param, OR — **simplest** — inline
the authentication in `preview_write_back_status` before calling
`build_write_back_plan`, fetch the timestamp, then call `build_write_back_plan`.

The simplest approach that avoids changing the `build_write_back_plan` signature:
extract a `make_authenticated_client` helper (private `async fn`) that takes
`app_dir`, `client_id`, `client_secret` and returns `Result<SheetsClient, String>`.
Then in `preview_write_back_status`:

```rust
// 1. Authenticate and get a client just for the modifiedTime fetch.
let early_client = make_authenticated_client(&app_dir.0, &client_id, client_secret.clone()).await?;
let preview_revision = early_client.get_file_modified_time(&spreadsheet_id).await?;
// 2. Build the plan (authenticates again internally — two token reads, both from cache).
let (_client, cells) = build_write_back_plan(
    &app_dir.0, pool.inner(), &spreadsheet_id, &sheet_name, &client_id, client_secret,
).await?;
```

The double authentication is acceptable: `ensure_valid_token` reads from the
local token store (fast path, no network round-trip when the token is fresh). The
important property is that `preview_revision` corresponds to a state AT OR BEFORE
the values were fetched.

Apply the same restructuring to `preview_economia_write_back_status`.

The `make_authenticated_client` helper:

```rust
async fn make_authenticated_client(
    app_dir: &std::path::Path,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<SheetsClient, String> {
    let secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, secret.as_deref()).await?;
    Ok(SheetsClient::new(token))
}
```

Model after the identical pattern inside `build_write_back_plan` (lines 240-244):

```rust
// write_back_cmds.rs:240-244
let client_secret = oauth::pkce::resolve_client_secret(client_secret);
let token =
    oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
        .await?;
let client = SheetsClient::new(token);
```

After the fix, the call sequence is:

1. Authenticate → `SheetsClient`
2. `get_file_modified_time` → `preview_revision` (T1)
3. `build_write_back_plan` (fetches values at T2 ≥ T1)
4. Return `{ cells, preview_revision, ... }`

If a concurrent edit occurs between T1 and T2, `preview_revision` (T1) is OLDER
than the actual state the diff was computed from (T2). On apply, if no further
edit occurs, apply-time modifiedTime = T2 > T1 → `guard_sheet_unchanged` fires
→ re-preview required. This is the safe side: conservative, never approves a
stale diff. No safety gate is weakened.

**Verify**: `npm run rust:check` → exit 0.

### Step 4: Bug 2 fix — fast-path cache invalidation (TypeScript)

In `src/screens/dashboard/WriteBackPending.tsx`, add `invalidateCommands()` to
`confirmFastWrite` immediately after the `applyWriteBack` call succeeds, before
`writeBack.refresh()`.

Current `confirmFastWrite` success path (lines 196-199):

```tsx
applyingFastRef.current = false;
setFastPath(null);
writeBack.refresh();
```

After the fix:

```tsx
applyingFastRef.current = false;
setFastPath(null);
invalidateCommands(); // finance numbers changed — drop every cached screen
writeBack.refresh();
```

Add the import at the top of the file (after the existing imports):

```tsx
import { invalidateCommands } from "../../lib/useCommand";
```

Check whether `invalidateCommands` is already imported in this file before
adding — if it is, skip the import line.

No other changes to `WriteBackPending.tsx`. Do not alter any JSX, styles, or
other handlers.

**Verify**: `npm run typecheck` → exit 0.

### Step 5: Bug 2 regression test (TypeScript)

Create `src/screens/dashboard/WriteBackPending.test.tsx`.

Model after `src/screens/dashboard/DailyCheckinCard.test.tsx` for the test
scaffolding (vi.mock for Tauri, mockCommands/mockInvoke helpers, RTL render).

Model after `src/features/sheets/WriteBackPreview.test.tsx` for the
`WriteBackPreviewResult` shape and `mockCommands` handler keys.

The test file needs these mocks at the top:

```tsx
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../lib/useCommand", async (importOriginal) => {
  const mod = await importOriginal<typeof import("../../lib/useCommand")>();
  return { ...mod, invalidateCommands: vi.fn() };
});
```

Import `invalidateCommands` from `"../../lib/useCommand"` and cast it as
`vi.Mock` to assert call count.

The `WriteBackPending` component expects a `writeBack: WriteBackPendingState`
prop. Look at `src/hooks/useWriteBackPending.ts` to get the shape of
`WriteBackPendingState`. Construct a minimal `WriteBackPendingState` stub:

```ts
const wb: WriteBackPendingState = {
  loading: false,
  pendingCount: 1,
  enabled: true,
  spreadsheetId: "ss-1",
  sheetName: "2026",
  clientId: "cid-1",
  conflictCount: 0,
  refresh: vi.fn(),
};
```

Write the following tests:

**Test 1** (`"confirmFastWrite calls invalidateCommands after a successful fast-path apply"`):

1. `mockCommands` with `preview_write_back_status` returning a result with one
   changed cell (safe for fast path: `preview_revision` non-empty,
   `conflicts_pending: false`, `multi_card_warning: false`), and
   `apply_write_back` returning `{ written: 1, note_warning: null }`.
2. Render `<WriteBackPending writeBack={wb} />`.
3. Click the "Sincronizar" button.
4. Wait for a "Confirmar envio" button (or whatever the confirm dialog renders) to
   appear, then click it.
5. Wait for `invalidateCommands` to have been called at least once.
6. Assert `wb.refresh` was also called.

Check the actual button labels by reading `ConfirmDialog` in
`src/features/sheets/WriteBackPreview.tsx` or `WriteBackPreview.test.tsx` for the
confirm/cancel button text before writing the test.

**Test 2** (`"confirmFastWrite does NOT call invalidateCommands on error"`):

1. `mockCommands` with `preview_write_back_status` returning a safe fast-path
   result and `apply_write_back` throwing (use `mockInvoke.mockRejectedValueOnce`
   for the apply call, or set `apply_write_back` to throw in `mockCommands`).
2. Render and trigger the fast path through the confirm dialog.
3. Wait for the error message to appear.
4. Assert `invalidateCommands` was NOT called.
5. Assert `wb.refresh` was NOT called.

**Verify**: `npm run test:run -- WriteBackPending` → all tests pass, including the
2 new tests.

### Step 6: Full gate

```bash
npm run check
```

Expected: exit 0. All sub-checks must pass (typecheck, lint, test:run,
rust:check, doctor).

## Test plan

| Test                                                                           | File                                | What it proves                                                                                                                                               |
| ------------------------------------------------------------------------------ | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `economia_write_back_audit_realigns_source_amount`                             | `write_back_cmds.rs` `#[cfg(test)]` | `record_write_back_audit` with `kind="economia"` and `date="2026-06"` matches id `"economia:2026-06"` — regression guard against future id/date format drift |
| `confirmFastWrite calls invalidateCommands after a successful fast-path apply` | `WriteBackPending.test.tsx` (new)   | finance caches are invalidated after a 1-click sync                                                                                                          |
| `confirmFastWrite does NOT call invalidateCommands on error`                   | `WriteBackPending.test.tsx` (new)   | no spurious invalidation on failed apply                                                                                                                     |

Existing tests that cover related paths and must continue to pass:

- `src/features/sheets/WriteBackPreview.test.tsx` — full review/apply flow
- `src/hooks/useWriteBackPending.test.ts` — pending count / refresh behaviour

## Done criteria

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run test:run` exits 0; new tests `economia_write_back_audit_realigns_source_amount`, and both `WriteBackPending` tests exist and pass
- [ ] `npm run check` exits 0 (full gate)
- [ ] `src/screens/dashboard/WriteBackPending.tsx` imports and calls `invalidateCommands()` in `confirmFastWrite` on the success path
- [ ] `preview_write_back_status` and `preview_economia_write_back_status` fetch `get_file_modified_time` BEFORE `build_write_back_plan` / `build_economia_plan` fetches values
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to reflect this plan

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the "Current state" excerpts doesn't match (line numbers or content
  differ materially — the codebase has drifted since this plan was written).
- `guard_no_pending_conflicts`, `guard_sheet_unchanged`, `ensure_write_back_enabled`,
  or `ensure_write_scope` are missing or have changed signature — do not proceed
  without human review of the safety model.
- The `CellWrite` struct or `record_write_back_audit` signature has changed in a
  way that makes the Rust test shape above invalid.
- `WriteBackPendingState` shape has changed such that the stub cannot be
  constructed without additional mock setup.
- A step's verification fails twice after a reasonable fix attempt.
- The fix for Step 3 (TOCTOU) appears to require changing
  `build_write_back_plan`'s signature (adding an out-param or return tuple) — this
  is a larger refactor; report first.
- Any fix appears to require touching an out-of-scope file.

## Maintenance notes

- **Bug 1 (id format)**: the regression test added here is the guard. If
  `store_economia_entries` or `plan_economia_write_back` ever changes the
  `YYYY-MM` convention (e.g. to add day precision), the audit's `format!` at
  line 633 must change in lockstep. The test will fail fast.
- **Bug 2 (invalidation)**: any future fast-path write (e.g. Economia fast-path)
  must also call `invalidateCommands()` after success. Establish the pattern:
  write → invalidateCommands → refresh.
- **Bug 3 (TOCTOU)**: the fix is conservative (pre-fetching modifiedTime means an
  innocuous concurrent edit forces a re-preview). If the window between
  modifiedTime fetch and values fetch becomes a concern in practice, the correct
  solution is a Drive Files.get with `fields=modifiedTime,values` in a single
  RPC — out of scope here, tracked as a future improvement.
- **PR reviewer**: check that the two `get_file_modified_time` calls
  (one new, one existing inside `build_write_back_plan` used by `apply_write_back`)
  are not confused. The preview commands now call it once early; the apply command
  calls it inside `guard_sheet_unchanged` (which reads it from the Drive API
  again on apply). This double-fetch-on-apply is correct and intentional.
