# Plan 031: Surface write-back pending indicator + send-from-dashboard + conflict visibility

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat bf92101..HEAD -- src/screens/DashboardScreen.tsx src/features/sheets/WriteBackPreview.tsx src/features/reconcile/ConflictGate.tsx src/screens/TransactionsScreen.tsx src/lib/api.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: LOW
- **Category**: feature
- **Package**: A
- **Depends on**: none (plan 028 already enabled write-back; this plan only wires visibility)
- **Planned at**: commit `bf92101`, 2026-06-20

## Why this matters

After a local transaction is logged, the user has no signal that the app holds
changes not yet pushed to their spreadsheet. The write-back approval flow
(`WriteBackPreview`) is only reachable via Settings → Google Sheets → Mapping
step — a path a daily user never visits. `ConflictGate`, which blocks write-back
when conflicts are pending, is rendered only in `TransactionsScreen`, so a user
who stays on the dashboard never sees the block and cannot understand why the
send button is disabled. This plan adds a lightweight pending-write-back
indicator to the dashboard (count badge + entry point) that plugs directly into
the existing preview/approval flow from plan 028 — no new write mechanics.

## Current state

Verified against live code at commit `bf92101`. Re-open and re-confirm line
numbers before starting.

### Files and their roles

- `src/screens/DashboardScreen.tsx` — main dashboard; renders hero forecast +
  cards; **has no write-back status or conflict entry point** (lines 1–199).
- `src/features/sheets/WriteBackPreview.tsx` — full write-back UI (preview +
  approval + 2nd-confirm dialog); exported as `WriteBackPreview`; accepts
  `{spreadsheetId, sheetName, clientId}` (lines 387–674). Contains the complete
  diff/approval flow that this plan will reuse.
- `src/features/sheets/GoogleSheetsPanel.tsx` — the only caller of
  `WriteBackPreview`; renders it inside `MappingStep` (lines 838–842):
  ```tsx
  // GoogleSheetsPanel.tsx:838–842
  <WriteBackPreview
    spreadsheetId={selectedSpreadsheet}
    sheetName={selectedSheet}
    clientId={GOOGLE_CLIENT_ID}
  />
  ```
  `selectedSpreadsheet` and `selectedSheet` come from `useSheetImport` state;
  they are the user's currently mapped sheet tab.
- `src/features/reconcile/ConflictGate.tsx` — renders `null` when no conflicts;
  shows one `ApprovalDiffCard` per conflict when `getImportConflicts()` returns
  items; exported as `ConflictGate({ onResolved? })` (lines 36–161).
  **Currently rendered only in `TransactionsScreen.tsx:577`.**
- `src/screens/TransactionsScreen.tsx` — renders `<ConflictGate
onResolved={handleCreated} />` at line 577; no write-back indicator.
- `src/lib/api.ts` — relevant exports (lines 377–498):
  - `previewWriteBackStatus(spreadsheetId, sheetName, clientId)` →
    `Promise<WriteBackPreviewResult>` (line 413). Returns:
    ```ts
    // api.ts:399–404
    interface WriteBackPreviewResult {
      cells: CellWrite[];
      preview_revision: string;
      conflicts_pending: boolean;
      multi_card_warning: boolean;
    }
    ```
    `cells` filtered by `.changed` gives the pending count.
  - `getImportConflicts()` → `Promise<ImportConflict[]>` (line 488).
  - `writeBackEnabled()` → `Promise<boolean>` (line 407). Returns the master
    flag; when `false`, the pending indicator must still show (so the user knows
    there is something to send) but the send button stays disabled.
  - `GOOGLE_CLIENT_ID` — build-time constant; may be empty string in installs
    without OAuth configured (line 176 region; used throughout `GoogleSheetsPanel`).

### Repo conventions

- **React Compiler ON** — no manual `memo`, `useCallback`, `useMemo`; hoist
  static `CSSProperties` objects to module-level constants. See existing
  `const WARN_BANNER: CSSProperties = { ... }` in `WriteBackPreview.tsx:143–154`
  as the pattern.
- **`useCommand`** (`src/lib/useCommand.ts:49`) — SWR-lite for Tauri commands:
  `useCommand(stableKey, stableModuleLevelFetcher)`. For argumentful fetches,
  encode arguments into the key; the fetcher must be module-level (not inline
  arrow). Use this for polling `getImportConflicts` on mount.
- **Money is always positive-magnitude integer cents** — never render raw
  `amount`; use `formatBRL(cents)` from `src/lib/format.ts`.
- **Functional-core / imperative-shell** — logic in hooks; JSX components are
  pure display.
- **Static styles hoisted** — any `CSSProperties` literal that does not vary
  per-render belongs at module scope (React Compiler relies on this). See
  `DashboardScreen.tsx` and `WriteBackPreview.tsx` for the pattern.
- **Test helper** — `src/test/commands.ts` exports `mockCommands` (routes
  `invoke` calls by name) and `mockInvoke`. Every test file must include
  `vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))` (hoisted
  per-file). See `ConflictGate.test.tsx` as the structural model.

### Key assumption

The user has already completed at least one import (so `getAppSetting(
"sheets_last_import")` is set) and the mapped `spreadsheetId`/`sheetName` are
available. The dashboard indicator **must degrade gracefully** when these are
absent (show nothing rather than an error).

## Commands you will need

| Purpose      | Command             | Expected on success       |
| ------------ | ------------------- | ------------------------- |
| Install      | `npm ci`            | exit 0                    |
| Typecheck    | `npm run typecheck` | exit 0, no errors         |
| Lint         | `npm run lint`      | exit 0                    |
| Unit tests   | `npm run test:run`  | all pass                  |
| Full gate    | `npm run check`     | exit 0                    |
| React Doctor | `npm run doctor`    | 0 issues                  |
| E2E smoke    | `npm run e2e`       | exit 0, screenshots clean |

## Suggested executor toolkit

Use `neko-finance-design` skill (`.agents/skills/neko-finance-design/SKILL.md`)
for token and component guidance when choosing colors and layout for the pending
badge. The indicator should use `--brass-400` / `--bg-subtle` (warm-amber tone
for "pending action") rather than `--danger-400` (reserved for errors/deficits).

## Scope

**In scope** (the only files you should create or modify):

- `src/hooks/useWriteBackPending.ts` (create) — shared hook that fetches the
  pending count + conflict count from the existing API functions.
- `src/screens/DashboardScreen.tsx` — add the indicator banner + wiring.
- `src/hooks/useWriteBackPending.test.ts` (create) — unit tests for the hook.

**Out of scope** (do NOT touch, even though they look related):

- `src/features/sheets/WriteBackPreview.tsx` — plan 028 owns the approval
  mechanics; do not modify the component itself.
- `src/features/sheets/GoogleSheetsPanel.tsx` — the Settings panel render of
  `WriteBackPreview` stays as-is.
- `src/features/reconcile/ConflictGate.tsx` — the component is reused as-is;
  it already handles its own data fetch and event subscription.
- `src/screens/TransactionsScreen.tsx` — `ConflictGate` stays there too; the
  dashboard adds a second entry point, it does not replace the one in
  Transactions.
- `src/lib/api.ts` — no new Tauri commands; the existing
  `previewWriteBackStatus` and `getImportConflicts` are sufficient.
- `src-tauri/` — no Rust changes.
- Any change to the write-back approval mechanics (human-approval gate, 2nd
  confirm dialog, `applyWriteBack` call). Those remain unchanged in
  `WriteBackPreview`.

## Git workflow

- Branch: `advisor/031-writeback-pending-indicator`
- Commit style (from `git log`): `feat: <verb phrase in PT-BR> — plano 031`
  e.g. `feat: indicador de write-back pendente no dashboard — plano 031`
- Commit after each step that leaves the codebase in a buildable state.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Create `src/hooks/useWriteBackPending.ts`

Create a new file at `src/hooks/useWriteBackPending.ts`. This hook:

1. Reads `getAppSetting("sheets_last_import")` on mount (same key as
   `GoogleSheetsPanel.tsx:64`: `const LAST_IMPORT_KEY = "sheets_last_import"`).
   Parse the JSON to extract `{ spreadsheetId, label }`. If absent or unparseable,
   return early with zeros (graceful degradation).
2. Reads `getAppSetting("sheets_client_id")` (same key as
   `GoogleSheetsPanel.tsx:68`: `const CLIENT_ID_KEY = "sheets_client_id"`) to
   get the stored OAuth client ID.
3. Reads `writeBackEnabled()` to know whether the send button should be enabled.
4. Calls `previewWriteBackStatus(spreadsheetId, sheetName, clientId)` to get
   the pending cell count. `sheetName` must come from the stored last-import
   (the `sheetName` field if you add it to persistence, see note below) OR from
   the current year string. **Preferred approach**: read `sheets_last_sheet` (a
   new app_setting key you persist alongside `sheets_last_import`; see Step 2).
   If the key is absent, fall back to the current year as a string
   (`String(new Date().getFullYear())`).
5. Calls `getImportConflicts()` to get the conflict count.
6. Returns `{ pendingCount, conflictCount, enabled, loading, error }`.

The hook must follow `useCommand` conventions: all fetches are in `useEffect`
with alive-guard; no inline arrow passed as a `useCommand` fetcher (use
module-level stubs or call the API functions directly in the effect since
arguments vary). Use `useState` + `useEffect` for simplicity, mirroring
`ConflictGate.tsx:42–50`.

Target shape:

```ts
export interface WriteBackPendingState {
  /** Cells that differ local→sheet (0 when unknown/no mapping). */
  pendingCount: number;
  /** Import conflicts blocking write-back (0 when none). */
  conflictCount: number;
  /** Master write-back flag; false = send button disabled. */
  enabled: boolean;
  loading: boolean;
  error: string | null;
}

export function useWriteBackPending(): WriteBackPendingState { ... }
```

Do NOT add `useCommand` for `previewWriteBackStatus` because it takes
arguments; call it directly in a `useEffect`. Do NOT use `useCallback` or
`useMemo` (React Compiler is on).

**Verify**: `npm run typecheck` → exit 0, no errors.

### Step 2: Persist `sheets_last_sheet` in `GoogleSheetsPanel.tsx`

The dashboard hook (Step 1) needs the last-imported sheet tab name
(`sheetName`) so it can call `previewWriteBackStatus(spreadsheetId, sheetName,
clientId)`. Currently `persistLastImport` in `GoogleSheetsPanel.tsx` (lines
308–324) persists `spreadsheetId` and `label` but not `sheetName`.

In `GoogleSheetsPanel.tsx`:

1. Add a constant at the top alongside the other keys (line 68 region):
   ```ts
   const LAST_SHEET_KEY = "sheets_last_sheet";
   ```
2. In `persistLastImport` (around line 308), after the existing
   `setAppSetting(LAST_IMPORT_KEY, ...)` call, add:
   ```ts
   if (state.selectedSheet) await setAppSetting(LAST_SHEET_KEY, state.selectedSheet);
   ```
3. In `useSheetImport`, in the mount `useEffect` that reads `BG_SYNC_KEY`
   (lines 478–511), add a third `getAppSetting` call to load `LAST_SHEET_KEY`
   and stash it in a new state field `lastSheet: string | null` (add to
   `SheetState` interface and `initialSheetState`). This is optional for the
   dashboard hook (which reads the key directly); the stash is only needed if
   the Settings panel itself needs it — skip if it adds too much complexity.

The **simplest correct approach**: have `useWriteBackPending` read
`getAppSetting("sheets_last_sheet")` directly (no new state needed in
`GoogleSheetsPanel`). Just add the `LAST_SHEET_KEY` constant and the persist
call; the hook reads the setting independently.

**Verify**: `npm run typecheck` → exit 0.

### Step 3: Build the `WriteBackBanner` component inline in `DashboardScreen.tsx`

Add a small local component (or render-function) at the top of
`DashboardScreen.tsx` that renders the pending indicator. Keep it in the same
file (it is dashboard-only; no need for a separate file).

**What it shows:**

- When `pendingCount > 0 && !conflictCount`: a brass-toned banner:
  `"N célula(s) local→planilha pendentes — enviar"` (clicking opens the write-back
  approval panel; see Step 4).
- When `conflictCount > 0`: an amber warning row:
  `"N conflito(s) de importação — resolver em Lançamentos"` (no action from
  dashboard; navigate to Transactions). This surfaces the block that prevents
  write-back without requiring the user to leave the dashboard to discover it.
- When both are non-zero: show both rows.
- When `pendingCount === 0 && conflictCount === 0`: render nothing (`null`).
- When `loading`: render nothing (avoid flash).

**Visual guidance (from the design system):**

- Use `--brass-400` / `--bg-subtle` for the pending row (warm, not alarming).
- Use `--warning-400` / `--bg-subtle` for the conflict row (matches
  `ConflictGate.tsx:105`).
- Use `--bw-hair solid var(--border)` for the border.
- Use `--radius-sm` for border-radius.
- Keep it compact: `padding: "6px 10px"`, `fontSize: "var(--fs-sm)"`.
- All `CSSProperties` objects that do not vary per-render must be hoisted to
  module-level constants (React Compiler / Doctor rule).

Example structure (exact styling is your call within the tokens above):

```tsx
// Hoisted at module level:
const PENDING_BANNER: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 10px",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--bg-subtle)",
  fontSize: "var(--fs-sm)",
  cursor: "pointer",
  color: "var(--brass-400)",
};

const CONFLICT_BANNER: CSSProperties = {
  ...PENDING_BANNER,
  color: "var(--warning-400)",
  cursor: "default",
};
```

Do NOT add animation or motion to the pending count (money/finance numbers
must not animate per the design system principle "money is never animated").

**Verify**: `npm run typecheck` → exit 0.

### Step 4: Wire the pending banner to the write-back approval panel

When the user clicks the pending-cells banner, the existing `WriteBackPreview`
flow must open. The cleanest approach for this plan (avoiding a full modal
system) is a local disclosure: add a `showWriteBack: boolean` state to
`DashboardScreen` and conditionally render `WriteBackPreview` below the banner.

In `DashboardScreen.tsx`:

1. Add `const [showWriteBack, setShowWriteBack] = useState(false)` near the
   other `useState` at line 24.
2. Use `useWriteBackPending()` to get `{ pendingCount, conflictCount, enabled,
loading }`.
3. Read the stored mapping props for `WriteBackPreview`. The hook already reads
   `spreadsheetId` and `sheetName` internally; expose them from the hook return
   (add `spreadsheetId: string`, `sheetName: string`, `clientId: string` to
   `WriteBackPendingState`). When absent (no mapping yet), `spreadsheetId` and
   `sheetName` default to `""`.
4. In the return JSX, between the deficit banner (line 167–176) and the
   `DailyCheckinCard` (line 178), add:

   ```tsx
   {
     (pendingCount > 0 || conflictCount > 0) && (
       <WriteBackStatusBanner
         pendingCount={pendingCount}
         conflictCount={conflictCount}
         enabled={enabled}
         onOpenWriteBack={() => setShowWriteBack((v) => !v)}
       />
     );
   }

   {
     showWriteBack && spreadsheetId && sheetName && (
       <WriteBackPreview
         spreadsheetId={spreadsheetId}
         sheetName={sheetName}
         clientId={clientId}
       />
     );
   }
   ```

   `WriteBackStatusBanner` is the local component from Step 3. The
   `WriteBackPreview` component manages its own state; toggling `showWriteBack`
   mounts/unmounts it (the preview is regenerated on each open — this is
   intentional: stale previews should not linger).

5. Add the import at the top:
   ```ts
   import { WriteBackPreview } from "../features/sheets/WriteBackPreview";
   ```
   and update the `useWriteBackPending` import.

**Verify**: `npm run typecheck` → exit 0; `npm run lint` → exit 0.

### Step 5: Write unit tests for `useWriteBackPending`

Create `src/hooks/useWriteBackPending.test.ts`.

Model after `src/features/reconcile/ConflictGate.test.tsx` for the
`mockCommands`/`mockInvoke` pattern. Use `renderHook` from
`@testing-library/react`.

Cases to cover:

1. **No mapping set** (`get_app_setting` returns `null` for `sheets_last_import`):
   hook returns `{ pendingCount: 0, conflictCount: 0, enabled: false, loading:
false, error: null }`.
2. **Mapping present, 3 pending cells, 0 conflicts, flag enabled**: hook returns
   `pendingCount: 3, conflictCount: 0, enabled: true`.
3. **Mapping present, 0 pending cells, 2 conflicts, flag enabled**: hook returns
   `pendingCount: 0, conflictCount: 2, enabled: true`.
4. **Flag disabled** (`write_back_enabled` returns `false`): hook returns
   `enabled: false` even when `pendingCount > 0`.
5. **`previewWriteBackStatus` rejects**: hook sets `error` to a non-null string
   and returns `pendingCount: 0`.

For `vi.mock` — add at the top of the test file (hoisted):

```ts
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
```

Use `mockCommands` from `src/test/commands.ts`.

Mock return for `preview_write_back_status` with 3 changed cells:

```ts
const PREVIEW_3_PENDING = {
  cells: [
    {
      a1: "B5",
      row: 5,
      col: 2,
      date: "2026-06-01",
      kind: "saida",
      current: "R$ 100,00",
      proposed: "R$ 120,00",
      value_cents: 12000,
      changed: true,
    },
    {
      a1: "B6",
      row: 6,
      col: 2,
      date: "2026-06-02",
      kind: "saida",
      current: "R$ 50,00",
      proposed: "R$ 80,00",
      value_cents: 8000,
      changed: true,
    },
    {
      a1: "B7",
      row: 7,
      col: 2,
      date: "2026-06-03",
      kind: "saida",
      current: "R$ 30,00",
      proposed: "R$ 45,00",
      value_cents: 4500,
      changed: true,
    },
  ],
  preview_revision: "rev-abc",
  conflicts_pending: false,
  multi_card_warning: false,
};
```

**Verify**: `npm run test:run` → all pass, including the 5 new hook tests.

### Step 6: Run the full gate and E2E smoke

Run the full quality gate:

```
npm run check
```

Then run the E2E visual smoke:

```
npm run e2e
```

Open the Playwright report (`npm run e2e:report`) and inspect the dashboard
screenshot:

- When `preview_write_back_status` is mocked to return pending cells, the
  banner must be visible.
- Clicking the banner must show `WriteBackPreview` below it.
- When no pending cells and no conflicts, the banner must not be present.
- The existing dashboard layout (hero section, cards) must be visually unchanged
  when the banner is absent.

**Verify**: `npm run check` → exit 0; `npm run e2e` → exit 0; React Doctor
`npm run doctor` → 0 issues.

## Test plan

**New tests** (file: `src/hooks/useWriteBackPending.test.ts`):

| #   | Scenario                         | Key assertion                                   |
| --- | -------------------------------- | ----------------------------------------------- |
| 1   | No mapping in app_setting        | `pendingCount === 0`, no error                  |
| 2   | 3 changed cells, flag on         | `pendingCount === 3`, `enabled === true`        |
| 3   | 0 changed cells, 2 conflicts     | `conflictCount === 2`                           |
| 4   | Flag off                         | `enabled === false` regardless of pending count |
| 5   | `previewWriteBackStatus` rejects | `error !== null`, `pendingCount === 0`          |

**Existing tests** — all must continue to pass; no changes to
`ConflictGate.test.tsx` or any other test file are expected (the component is
reused as-is).

Verification:

```
npm run test:run
```

→ all pass; 5 new tests in `useWriteBackPending.test.ts` appear in output.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; output includes 5 new tests in
      `useWriteBackPending.test.ts`
- [ ] `npm run doctor` reports 0 issues
- [ ] `npm run check` exits 0
- [ ] `npm run e2e` exits 0; dashboard screenshot shows no regression when
      pending banner is absent
- [ ] `git diff --name-only` shows only files in the in-scope list
- [ ] `grep -rn "WriteBackPreview\|ConflictGate" src/screens/DashboardScreen.tsx`
      returns matches (both are wired)
- [ ] `grep -rn "useWriteBackPending" src/screens/DashboardScreen.tsx` returns a
      match
- [ ] No inline `CSSProperties` object literals inside JSX (all hoisted to
      module-level constants)
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" does not match the excerpts
  (the codebase has drifted since this plan was written — treat it as a STOP,
  compare carefully before proceeding).
- `previewWriteBackStatus` is not exported from `src/lib/api.ts` or its
  signature differs from `(spreadsheetId: string, sheetName: string, clientId:
string) => Promise<WriteBackPreviewResult>`.
- `WriteBackPreview` props differ from `{ spreadsheetId: string, sheetName:
string, clientId: string }` (plan 028 changed them).
- A step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching an out-of-scope file (e.g. you discover
  `WriteBackPreview` needs a new prop to work from the dashboard).
- React Doctor reports new issues introduced by this plan after Step 6.
- `npm run e2e` produces a screenshot showing the pending banner appears when
  there are no pending cells (false-positive indicator).
- `getAppSetting` or `setAppSetting` are not available in `src/lib/api.ts` (the
  settings persistence layer changed).

## Maintenance notes

- **The hook polls on mount only** — it does not subscribe to `neko://sync-done`.
  A future improvement would listen to the event (like `ConflictGate` does in
  `ConflictGate.tsx:56–68`) so the badge auto-updates when background sync
  finishes. Deferred because this plan's scope is minimal.
- **`showWriteBack` is local state** — if the user imports a new sheet tab while
  the dashboard panel is open, the panel will show the old tab's diff until
  remounted. This is acceptable for now; a future plan could invalidate the panel
  on `invalidateCommands()`.
- **No E2E mock for Tauri commands** — the E2E tests run against the real
  desktop build where no import has occurred, so the banner will not appear.
  If E2E tests gain Tauri command mocking in the future, add a case for the
  pending banner.
- **Review focus**: check that `showWriteBack && spreadsheetId && sheetName`
  guards adequately prevent rendering `WriteBackPreview` with empty strings
  (which would call `previewWriteBackStatus("", "", "")` and likely error).
