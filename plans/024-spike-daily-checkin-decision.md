# Plan 024: SPIKE: decide the fate of the vestigial daily_checkin table

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **This is a DESIGN/SPIKE plan.** The deliverable is a written investigation
> record with a concrete recommendation (Option A, B, or C defined below).
> No production schema changes, no new commands, no UI. The executor's job
> is to read, map, reason, and produce a decision document. Do NOT implement
> anything; that is a follow-up implementation plan that will cite this spike.
>
> **Drift check (run first)**:
> `git diff --stat 51afe33..HEAD -- src-tauri/src/commands/forecast_cmds.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/migrations/20240608000010_daily_checkin.sql docs/adr/0001-dual-tracking-daily-credit.md CONTEXT.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: — (spike, no priority ranking)
- **Effort**: spike
- **Risk**: MED
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `51afe33`, 2026-06-20

## Why this matters

The `daily_checkin` table has no production writer: every real daily check-in
writes a normal Diário `transaction`, yet the table persists in the schema,
two production read-paths fall back to it, and its `credit_spend` column is
the structural ghost of a dual-accumulator approach that the project decided
to drop (credit is a lump on the due date — not a per-day accumulator).
The mismatch between "no writer" and "has readers" is a correctness hazard:
if any future path writes a check-in row it will silently double-count the
same money, because the reads treat check-in data as a fallback when no
transaction exists for that day/month. Resolving this removes a maintenance
trap, realigns the code with the confirmed method model (credit = lump on
due date), and enables the `daily_checkin` column in CONTEXT.md to be
corrected or removed. This spike maps the blast radius and produces a
recommendation before any schema change is made.

## Current state

### Schema (verified: `src-tauri/migrations/20240608000010_daily_checkin.sql`)

```sql
CREATE TABLE IF NOT EXISTS daily_checkin (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    daily_spend INTEGER NOT NULL DEFAULT 0,
    credit_spend INTEGER NOT NULL DEFAULT 0,
    daily_budget_id TEXT REFERENCES daily_budget(id),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_daily_checkin_person_date ON daily_checkin(person_id, date);
```

`daily_spend` is the method's per-day debit/cash spend metric ("Régua 1" in
Neko's internal naming). `credit_spend` is the per-day credit accumulator
("Régua 2" — the fork the project decided to retire). Both are `INTEGER NOT NULL DEFAULT 0`.

### Writers (all the code that INSERTs into daily_checkin)

**1. Demo seed — `src-tauri/src/lib.rs` lines 297–303** (fixture only):

```rust
let checkin_id = uuid::Uuid::new_v4().to_string();
sqlx::query(
    "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,?5)"
)
.bind(&checkin_id).bind(&person_id).bind("2025-03-15")
.bind(4300).bind(0)
.execute(&pool).await.unwrap();
```

This runs only in the `#[cfg(test)] async fn setup_demo_db()` fixture block
(`lib.rs` line 200 lists `daily_checkin` in the schema-verification table
list). It is not a production path.

**2. Test — `src-tauri/src/commands/mod.rs` lines 246–255** (regression test
for double-count prevention):

```rust
sqlx::query(
    "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,0)",
)
.bind(uuid::Uuid::new_v4().to_string())
.bind(&pid)
.bind("2026-06-13")
.bind(9_999i64)
.execute(&pool)
.await
.unwrap();
```

Test name: `dashboard_daily_spend_no_double_count_checkin_and_txn` (line 235).
Purpose: asserts that if a check-in row and a transaction exist for the same
day, the transaction wins. This test would need updating or removing if
the table is retired.

**3. Test — `src-tauri/src/commands/mod.rs` lines 1481–1491** (credit-lump
regression test, `dashboard_credit_lump_at_due_day`, line 1431):

```rust
sqlx::query(
    "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,?5)",
)
.bind(&checkin_id)
.bind(&pid)
.bind("2026-03-15")
.bind(0i64)
.bind(50_000i64)
.execute(&pool)
.await
.unwrap();
```

This test validates that `credit_spend` from `daily_checkin` aggregates as a
lump at the card due date. If the table is retired, this test exercises a
dead code path and must be replaced or removed.

There is no INSERT in `src-tauri/src/commands/sheets_import.rs`,
`src-tauri/src/commands/transactions.rs`, or any other production module.
Every real daily check-in instead inserts a `transaction` row of
`type='expense'`, `is_fixed=0`, `payment_method` in (`debit`,`pix`,`cash`).

### Readers (production code that SELECTs from daily_checkin)

**Read-path 1: `load_cashflow_events` in `src-tauri/src/commands/forecast_cmds.rs` lines 345–384**

Function signature (line 302):

```rust
pub(crate) async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String>
```

The relevant block (lines 345–384):

```rust
let checkins: Vec<(String, i64, i64)> = sqlx::query_as(
    "SELECT date, daily_spend, credit_spend FROM daily_checkin WHERE date > ?1 AND date <= ?2",
)
.bind(&today)
.bind(&horizon)
.fetch_all(pool)
.await
.map_err(|e| format!("query checkins: {e}"))?;

let mut credit_by_due: std::collections::HashMap<NaiveDate, i64> =
    std::collections::HashMap::new();

for (date_str, daily_spend, credit_spend) in checkins {
    if let Ok(checkin_date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
        // Daily spend (Régua 1) → Daily event on its day
        if daily_spend > 0 {
            all_events.push(CashflowEvent {
                date: checkin_date,
                kind: forecast::EventKind::Daily,
                amount_cents: daily_spend,
                realized: true,
            });
        }

        // Credit spend (Régua 2) → aggregate by due_date
        if credit_spend > 0 {
            let due_date = forecast::cycle_due_date(checkin_date, closing_day, due_day);
            *credit_by_due.entry(due_date).or_insert(0) += credit_spend;
        }
    }
}

for (due_date, total_credit) in credit_by_due {
    all_events.push(CashflowEvent {
        date: due_date,
        kind: forecast::EventKind::FixedOut,
        amount_cents: total_credit,
        realized: false,
    });
}
```

This block is guarded by `if !credit_cards.is_empty()` (line 339). It runs
only when at least one credit card account exists. Since no production writer
populates `daily_checkin`, the `SELECT` returns zero rows in all real
deployments. The `credit_spend` branch (lines 369–373) specifically implements
the Régua-2 accumulator pattern the project decided to drop.

**Read-path 2: `dashboard_summary` in `src-tauri/src/commands/forecast_cmds.rs` lines 938–950**

`daily_spend` fallback (lines 938–951):

```rust
let daily_spend: (i64,) = sqlx::query_as(
    "SELECT CASE WHEN EXISTS(SELECT 1 FROM \"transaction\" \
                             WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                               AND (payment_method IS NULL OR payment_method <> 'credit')) \
                 THEN ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                                    WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                                      AND (payment_method IS NULL OR payment_method <> 'credit')), 0)) \
                 ELSE COALESCE((SELECT SUM(daily_spend) FROM daily_checkin WHERE date = ?1), 0) \
            END",
)
```

`credit_spend` fallback (lines 963–978):

```rust
let credit_spend: (i64,) = sqlx::query_as(
    "SELECT CASE WHEN EXISTS(SELECT 1 FROM \"transaction\" \
                             WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                               AND date >= ?1 AND date <= ?2) \
                 THEN ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                                    WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                                      AND date >= ?1 AND date <= ?2), 0)) \
                 ELSE COALESCE((SELECT SUM(credit_spend) FROM daily_checkin \
                                WHERE date >= ?1 AND date <= ?2), 0) \
            END",
)
```

`has_credit` flag (lines 982–990):

```rust
let has_credit: (i64,) = sqlx::query_as(
    "SELECT CASE WHEN EXISTS(SELECT 1 FROM account WHERE type='credit_card') \
              OR EXISTS(SELECT 1 FROM \"transaction\" WHERE payment_method='credit') \
              OR COALESCE((SELECT SUM(credit_spend) FROM daily_checkin), 0) > 0 \
            THEN 1 ELSE 0 END",
)
```

The inline comment at line 934–935 already documents this:

```
// o check-in (`daily_checkin`, sem writer em produção hoje) só preenche dias SEM transação.
```

All three queries implement a transaction-wins / checkin-fallback pattern.
With no production writer, the fallback branch never fires in real use.
The `has_credit` query's third OR clause — `COALESCE((SELECT SUM(credit_spend) FROM daily_checkin), 0) > 0` — is the only production check that could change the observable UI flag based on check-in data alone.

### ADR-0001 (verified: `docs/adr/0001-dual-tracking-daily-credit.md`)

```markdown
## Decision

Track two parallel metrics per daily check-in:

> "Régua 1/2" are Neko's internal names for the two tracks; they are not the
> method's terminology.

1. **Daily/debit track** ("Régua 1", daily_spend): sum of debit/PIX/cash
   expenses. Compared against daily_budget.
2. **Credit/invoice track** ("Régua 2", credit_spend): sum of credit card
   expenses. Accumulates into the invoice that lands on the due date, so a
   "green" daily track does not hide a growing bill. The engine tracks the two
   independently; it does not compare credit against income.

Both are stored in `daily_checkin`. The Mia copilot reports both metrics
independently, preventing self-deception when the user is 100% credit.
```

ADR-0001's "Régua 2" half is the structural source of `credit_spend`. As
established this session (ground truth from the method and the spreadsheet),
credit is a lump on the due date — not a per-day accumulator. The
"Régua 2 / credit_spend accumulator" is a Neko fork that exists in neither
source. This spike is the gate to revising ADR-0001 accordingly.

### CONTEXT.md vocabulary (lines 50–61, relevant excerpt)

```markdown
**Daily Check-in** (daily_checkin):
A daily record of actual spending vs budget. Contains two independent metrics:

- **daily_spend**: sum of debit/PIX/cash expenses for the day (Régua 1 —
  methodology pure)
- **credit_spend**: sum of credit card expenses for the day (Régua 2 — reality
  check)
  _Avoid_: Daily log, spending log

**Débito/Diário track** (internal name "Régua 1" — Neko's term, not the
method's):
The method's core metric: daily_spend compared against daily_budget. Green/
amber/red based on budget compliance. Goes silent (always green) when the user
pays exclusively with credit.

**Crédito/Fatura track** (internal name "Régua 2"):
Credit bill tracking: SUM(credit_spend for the month) accumulates into the
invoice that lands on the due date. Prevents self-deception when the daily
track is green but the credit bill is accumulating silently. The engine tracks
the two independently; it does not compare credit against income.
```

CONTEXT.md still describes `daily_checkin` and "Régua 2" as active design.
This spike's decision record will determine how CONTEXT.md needs to change.

### The three options to evaluate

- **Option A**: Retire `daily_checkin` entirely. Drop the table (migration) and
  remove all three production read-paths. `daily_spend` already comes from
  transactions; `credit_spend` is always zero (no writer); `has_credit` already
  has two cleaner conditions. ADR-0001's credit-accumulator half is marked
  superseded. Simplest, most faithful to the confirmed method model.

- **Option B**: Retire only `credit_spend` (the Régua-2 half). Keep
  `daily_checkin` if `daily_spend` is judged useful as a future explicit
  check-in surface (e.g., a daily ritual UI where the person manually enters
  debit spend). Drop only the `credit_spend` column and all its read-paths.
  Partial.

- **Option C**: Introduce a dedicated daily check-in entity distinct from the
  movement ledger (a separate table for an explicit daily ritual confirmation,
  distinct from a `transaction` row). Larger scope; only justified if there is a
  concrete feature reason that transactions cannot fulfill (e.g., the method
  requires a ritual confirmation gesture separate from the ledger entry).

## Commands you will need

| Purpose                       | Command                                                    | Expected on success                         |
| ----------------------------- | ---------------------------------------------------------- | ------------------------------------------- |
| Rust typecheck + clippy + fmt | `npm run rust:check`                                       | exit 0, no warnings                         |
| Rust unit tests only          | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass                                    |
| Full gate                     | `npm run check`                                            | exit 0                                      |
| Drift check                   | `git diff --stat 51afe33..HEAD -- <paths>`                 | clean or known diffs                        |
| Count daily_checkin writers   | `grep -rn "INSERT INTO daily_checkin" src-tauri/`          | 3 matches (seed+2 tests)                    |
| Count daily_checkin readers   | `grep -rn "FROM daily_checkin" src-tauri/src/`             | 4 matches (3 in forecast_cmds, 1 docstring) |

## Suggested executor toolkit

- Read `docs/adr/0001-dual-tracking-daily-credit.md` in full before writing the
  decision record — the spike revises its "Régua 2" half.
- Read `docs/adr/0003-sqlite-system-of-record-collapsed-writeback.md` — confirms
  write-back constraint is unrelated to this table.
- Read `CONTEXT.md` lines 50–105 — the spike outcome will determine which
  vocabulary entries need updating.
- Read `plans/019-spike-invoice-entity.md` — plan 019 deferred
  `daily_checkin.credit_spend` deprecation to "FT-6, after invoice entity
  lands." This spike is the gate to deciding whether FT-6 means Option A or B.

## Scope

**In scope** (the only files the executor reads and the one new file it creates):

- `src-tauri/src/commands/forecast_cmds.rs` — read-path mapping (read-only).
- `src-tauri/src/commands/mod.rs` — test writer mapping (read-only).
- `src-tauri/src/lib.rs` — demo seed mapping (read-only).
- `src-tauri/migrations/20240608000010_daily_checkin.sql` — schema (read-only).
- `docs/adr/0001-dual-tracking-daily-credit.md` — read-only reference; the
  spike record will state what revision it requires, but does NOT edit the ADR.
- `CONTEXT.md` — read-only reference; same rule.
- `specs/024-daily-checkin-fate/spike.md` — the single deliverable (create).

**Out of scope** (do NOT touch):

- Any migration SQL — no schema changes in this spike.
- `src-tauri/src/commands/forecast_cmds.rs` — read only; no code edits.
- `src-tauri/src/forecast/mod.rs` — no engine changes.
- Any frontend file — no UI in this spike.
- `docs/adr/0001-dual-tracking-daily-credit.md` — do not edit the ADR; the
  decision record states what revision is needed, and the implementation plan
  will make it.
- `CONTEXT.md` — do not edit; same rule.

## Git workflow

- Branch: `advisor/024-spike-daily-checkin`
- One commit: the spike design record only.
- Commit message style: `docs: spike — daily_checkin fate decision`
  (conventional commits, lower-case).
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Map the complete blast radius

**What to do**: Before writing the decision record, grep the repository for every
reference to `daily_checkin`, `daily_spend`, and `credit_spend` in Rust source
files, migration files, and any TypeScript/frontend files. Record the exact file,
line number, and what the reference does (write / read / schema / comment / test).

Run:

```
grep -rn "daily_checkin" src-tauri/ --include="*.rs"
grep -rn "daily_checkin" src-tauri/ --include="*.sql"
grep -rn "daily_checkin\|daily_spend\|credit_spend" src/ --include="*.ts" --include="*.tsx"
grep -rn "daily_checkin\|credit_spend" docs/
```

Expected summary (as of commit 51afe33):

| Location                                                | Type    | What it does                                                                  |
| ------------------------------------------------------- | ------- | ----------------------------------------------------------------------------- |
| `src-tauri/migrations/20240608000010_daily_checkin.sql` | schema  | creates the table                                                             |
| `src-tauri/src/lib.rs:208`                              | read    | schema verification table list (test fixture)                                 |
| `src-tauri/src/lib.rs:299`                              | write   | demo seed INSERT (test only)                                                  |
| `src-tauri/src/commands/mod.rs:247`                     | write   | regression test INSERT (`daily_spend` only, `credit_spend=0`)                 |
| `src-tauri/src/commands/mod.rs:1482`                    | write   | regression test INSERT (`credit_spend=50_000`)                                |
| `src-tauri/src/commands/forecast_cmds.rs:300`           | comment | doc-comment for `load_cashflow_events`                                        |
| `src-tauri/src/commands/forecast_cmds.rs:346`           | read    | `load_cashflow_events`: SELECT daily_spend + credit_spend for forecast window |
| `src-tauri/src/commands/forecast_cmds.rs:945`           | read    | `dashboard_summary`: daily_spend ELSE fallback                                |
| `src-tauri/src/commands/forecast_cmds.rs:970`           | read    | `dashboard_summary`: credit_spend ELSE fallback                               |
| `src-tauri/src/commands/forecast_cmds.rs:985`           | read    | `dashboard_summary`: has_credit third OR clause                               |
| `docs/adr/0001-dual-tracking-daily-credit.md`           | doc     | defines both columns as active design                                         |
| `CONTEXT.md:50–61`                                      | doc     | vocabulary entries for daily_checkin, Régua 1, Régua 2                        |

If the grep produces results not in this table (additional writers, frontend
references, or missing rows), record them explicitly in the spike document —
this is a STOP-adjacent finding to report, not to silently ignore.

**Verify**: `grep -rn "INSERT INTO daily_checkin" src-tauri/` → exactly 3 matches.
`grep -c "FROM daily_checkin" src-tauri/src/commands/forecast_cmds.rs` → 3.

---

### Step 2: Evaluate each production read-path for removability

**What to do**: For each of the three production read-paths in
`src-tauri/src/commands/forecast_cmds.rs`, answer:

**Read-path 1: `load_cashflow_events` — `daily_spend` branch (line 361–367)**

If this branch is removed, what breaks? The answer: nothing in production (no
rows exist). In tests, `dashboard_credit_lump_at_due_day` (`mod.rs:1431`)
tests the `credit_spend` aggregation via this path. If the table is retired,
that test must be replaced with a test that uses `payment_method='credit'`
transactions instead (which already feeds `EventKind::FixedOut` via
`forecast::classify()`).

Is `daily_spend` data from check-ins conceptually useful for the forecast if a
writer existed? Evaluate: a check-in `daily_spend` row for a future date would
inject a `Daily` event into the forecast. But the method does not call for
check-in rows to project future daily spend — `load_forecast_events` (line 393)
already injects a `daily_ceiling` projection for future days. So this branch
would double-count future daily spend if a check-in writer existed.

**Read-path 2: `dashboard_summary` — `daily_spend` ELSE fallback (lines 944–946)**

If removed (replace with `0`), what breaks? The `daily_spend_today` tile would
show 0 on days with no transaction. Since in production there is never a
check-in row, today's behavior is already: transaction wins → check-in fallback
returns 0 (no rows) → tile shows 0. Simplifying to a plain transaction query
(no CASE/ELSE) has no observable production effect.

**Read-path 3: `dashboard_summary` — `credit_spend` ELSE fallback (lines 969–972) and `has_credit` OR clause (line 985)**

If removed: `credit_spend_month` tile already reads from `transaction` (the
`THEN` branch fires whenever any credit transaction exists). `has_credit` third
OR clause (`daily_checkin.credit_spend > 0`) — with no production writer, this
never returns true. Removing it simplifies the query without observable effect.

Document these conclusions in the spike record. The evaluation determines
whether the blast radius of Option A is genuinely safe.

**Verify**: no files modified yet.
`git diff --name-only HEAD` → no output.

---

### Step 3: Assess Option C (separate check-in entity)

**What to do**: Determine whether there is a concrete feature need for a
separate `checkin` entity distinct from the movement ledger.

Investigate whether a separate check-in entity is warranted:

1. Does Neko have any planned feature (in `specs/`, `plans/`, or `CONTEXT.md`)
   that requires a check-in entity as a _ritual gesture_ (explicit daily
   confirmation) separate from ledger entries? Search:
   ```
   grep -rn "checkin\|check.in\|ritual" specs/ docs/
   ```
2. Would a future "daily ritual" UI (person manually confirms today's spend)
   require a separate table, or could it write a `transaction` row? The method
   call is: a Diário expense IS a transaction row. If the person is already
   importing from Sheets, the transactions are already there. A separate
   check-in entity is only needed to distinguish "I confirmed this day" from
   "I recorded a transaction on this day."
3. Is there any spec that calls for a separate daily-projection table distinct
   from `daily_budget`?

Document the findings. If no spec or plan calls for a separate checkin ritual
entity, Option C is not justified and should be ruled out with a rationale
note.

**Verify**: `ls specs/024-daily-checkin-fate/` → directory does not exist yet
(the spike record is created in Step 4, not here).

---

### Step 4: Write the spike decision record

**What to do**: Create the directory and file:
`specs/024-daily-checkin-fate/spike.md`

The file must contain the following sections:

**1. Summary** (3–5 sentences): what this spike decided, which option was
chosen, and what the implementation plan must do.

**2. Blast radius** (a table matching the one in Step 1 above, populated with
the live grep results). Annotate each row with "safe to remove" / "needs
replacement" / "keep."

**3. Read-path analysis**: for each of the three production read-paths, a
one-paragraph evaluation of what breaks if it is removed (conclusions from
Step 2). Be explicit: "in production, this branch never fires because no
writer exists."

**4. Option evaluation**:

For each option, state:

- What changes (files + migration).
- What the observable production effect is (during the transition and after).
- Whether any existing test must be replaced and with what.
- The primary risk.

```
Option A — Retire entirely:
  Changes: new migration DROP TABLE daily_checkin; remove 3 read-paths in
  forecast_cmds.rs; remove credit_spend branch from load_cashflow_events;
  simplify has_credit to 2 conditions; update 2 regression tests (replace
  check-in INSERT with transaction INSERT of type='expense',
  payment_method='credit'); remove schema-verification entry in lib.rs.
  Production effect: zero observable change (table was always empty in
  production). Test replacement: dashboard_daily_spend_no_double_count and
  dashboard_credit_lump_at_due_day rewritten against transactions.
  Risk: if a future writer is added before the drop lands (e.g., a Mia
  copilot feature), data loss. Mitigate: the implementation plan must land
  before any writer is added.

Option B — Retire credit_spend only:
  Changes: new migration ALTER TABLE daily_checkin DROP COLUMN credit_spend
  (SQLite 3.35+; check Tauri's bundled SQLite version); remove credit_spend
  from the SELECT in load_cashflow_events, remove credit_spend ELSE fallback
  in dashboard_summary, remove third OR clause in has_credit; update
  dashboard_credit_lump_at_due_day test.
  Keeps daily_spend and the table for a potential future check-in ritual UI.
  Risk: daily_spend still has no writer and retains the double-count hazard
  if one is added naively. Preserves a half-orphaned table.

Option C — Separate check-in entity:
  Bigger scope; only if a concrete spec demands a ritual checkin entity.
  Based on Step 3 findings, state whether it is justified or ruled out.
```

**5. Recommendation**: state which option the spike recommends, with a one-
paragraph rationale. The recommendation must cite the confirmed method model
(credit = lump on due date, no per-day accumulator) and the fact that
`daily_spend` already comes from transactions via the forecast engine's
`EventKind::Daily` path.

**6. ADR-0001 revision required**: state that the implementation plan must add
an "Amended" section to `docs/adr/0001-dual-tracking-daily-credit.md` marking
the "Régua 2 / credit_spend" half as superseded and pointing to ADR-0003
(credit folds into a Saída lump at the due date) and to the method ground truth
(credit is a lump, not a daily accumulator).

**7. CONTEXT.md revision required**: state which vocabulary entries must change:

- If Option A: remove the `daily_checkin` entry and the "Régua 2" /
  "Crédito/Fatura track" entries, or rewrite them to describe the correct model
  (credit transactions with `payment_method='credit'` fold into `FixedOut` at
  the due date).
- If Option B: remove the `credit_spend` sub-bullet from the `daily_checkin`
  entry and the "Régua 2" vocabulary entry.

**8. Open questions** (if any are unresolved by this spike — list them
explicitly so the implementation plan does not inherit ambiguity):

- OQ-1: What is the minimum SQLite version bundled with Tauri that supports
  `DROP COLUMN` (3.35.0, released 2021)? The implementation plan must verify
  this before choosing Option A or B migration strategy.
- OQ-2: Does the "Mia copilot" (planned AI assistant feature) require an
  explicit check-in row as a ritual confirmation separate from a transaction?
  If yes, Option B or C may be preferred. This spike cannot answer this
  without a Mia spec.
- OQ-3: Does plan 019 (first-class invoice entity) — which deferred
  `credit_spend` deprecation to "FT-6" — now point to Option A or B as FT-6?
  The implementation plan for this spike should update plan 019's maintenance
  notes.

**9. Follow-up tasks** (for the implementation plan that cites this spike):

- FT-1: Write a new migration (timestamp > `20240612000010`) that implements
  the chosen option's schema change.
- FT-2: Remove or simplify the three production read-paths in
  `forecast_cmds.rs` per the option chosen.
- FT-3: Replace `dashboard_daily_spend_no_double_count_checkin_and_txn` with a
  test that uses only transactions (if Option A) or that only tests the
  `daily_spend` fallback (if Option B).
- FT-4: Replace `dashboard_credit_lump_at_due_day` with a test that seeds a
  `payment_method='credit'` transaction and verifies the lump lands on the due
  date via the `classify()` → `FixedOut` path already in the engine.
- FT-5: Add an "Amended" section to `docs/adr/0001-dual-tracking-daily-credit.md`.
- FT-6: Update the `daily_checkin` and Régua-2 entries in `CONTEXT.md`.
- FT-7: Update plan 019's maintenance notes to cross-reference this spike's
  decision for `credit_spend` deprecation (FT-6 in plan 019).

**Verify**: the file exists and is substantive.
`ls specs/024-daily-checkin-fate/spike.md` → file exists.
`wc -l specs/024-daily-checkin-fate/spike.md` → more than 60 lines.

---

### Step 5: Final gate

**What to do**: Confirm the working tree contains only the one new file and
that all checks pass.

```
git diff --name-only HEAD
```

Expected output:

```
specs/024-daily-checkin-fate/spike.md
```

No other files should be modified. If any source file appears, stop and report.

**Verify**: `npm run check` → exit 0.

If frontend or Rust checks fail for reasons pre-existing and unrelated to this
spike (no source files were modified), document the failure verbatim. Do NOT
fix pre-existing failures as part of this spike.

## Test plan

This is a design/spike plan — no new production tests are written. The spike
record (Step 4) specifies the replacement tests that the _implementation plan_
must write (FT-3 and FT-4 above). Those tests are:

- `dashboard_daily_spend_no_double_count_checkin_and_txn` replacement: same
  scenario but without any `daily_checkin` INSERT; verify that two
  `expense` transactions on the same day sum correctly (or that only the
  first one is used, per whatever the chosen logic is).
- `dashboard_credit_lump_at_due_day` replacement: seed a
  `payment_method='credit'` transaction for March 15 on a card with
  `closing_day=20`, `due_day=10`; verify that the projected balance as of
  March 10 is unchanged (the lump falls after the forecast horizon) and that
  the credit lump appears in `load_cashflow_events` at the April 10 due date.
  This test exercises the `forecast::classify()` → `EventKind::FixedOut` path,
  which is the _only_ production code path that needs to survive.

The structural pattern for replacement tests: use `fixture_pool()` (already
used in `mod.rs` tests) and the `insert_realized()` helper defined in
`mod.rs`.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `ls specs/024-daily-checkin-fate/spike.md` → file exists
- [ ] `wc -l specs/024-daily-checkin-fate/spike.md` → output is ≥ 60
- [ ] `git diff --name-only HEAD` → only `specs/024-daily-checkin-fate/spike.md`
      (no source files modified)
- [ ] `grep -rn "INSERT INTO daily_checkin" src-tauri/` → still exactly 3 matches
      (spike is read-only on source)
- [ ] `npm run check` exits 0 (full gate — no regression from this spike since
      no source was modified)
- [ ] `plans/README.md` status row for plan 024 updated to DONE

## STOP conditions

Stop and report (do not improvise) if:

1. **Drift**: the code at any location cited in "Current state" does not match
   the excerpts (the file changed since commit `51afe33`). Report the delta and
   wait for an updated plan.

2. **Additional writers found**: `grep -rn "INSERT INTO daily_checkin" src-tauri/`
   returns more than 3 matches. A production writer exists — this invalidates
   the "table is vestigial" premise. Report all writers found and halt.

3. **Frontend references found**: `grep -rn "daily_checkin\|credit_spend"
src/ --include="*.ts" --include="*.tsx"` returns any matches. The blast
   radius extends to the frontend; the plan does not cover that. Report and halt.

4. **A spec or plan calls for a `daily_checkin` writer**: if Step 3 finds that
   a Mia feature spec or any other spec explicitly requires a check-in ritual
   entity separate from transactions, report it. Option C may be preferred;
   this spike's recommendation must be revisited.

5. **`npm run check` fails for a reason introduced by this spike**: impossible
   since no source is modified, but if it fails, report the verbatim error to
   confirm the gate was already broken before this spike.

6. **Step 4 verification fails**: `wc -l` < 60, meaning the spike record is a
   stub. Do not mark the plan done with a stub. Expand the document.

## Maintenance notes

- **This spike is the prerequisite gate for retirement**: no schema migration
  should touch `daily_checkin` until this spike's decision record exists and
  has been reviewed. Plan 019's "FT-6" deferred credit_spend deprecation to
  after the invoice entity — this spike supersedes that deferral and provides
  the concrete decision.

- **ADR-0001 revision is load-bearing**: the ADR currently describes
  `credit_spend` as active design. Any executor reading ADR-0001 without this
  spike's context would re-implement the accumulator. The implementation plan
  that follows must add an "Amended" section to ADR-0001 as its first step,
  before any code change, so the history is clear.

- **Reviewer should scrutinize**:
  - Whether OQ-2 (Mia copilot ritual check-in) is answered by the time the
    implementation plan is dispatched. If Mia needs a ritual entity, the
    implementation plan must choose Option B or C and add a `note` column
    writer for the ritual confirmation.
  - The replacement test for `dashboard_credit_lump_at_due_day` (FT-4): it
    must exercise the `classify()` → `FixedOut` path, not re-introduce a
    check-in row. The test in plan 019 (Step 3) exercises a similar pattern
    and can be used as the structural model.
  - SQLite `DROP COLUMN` support (OQ-1): if the bundled SQLite is < 3.35,
    Option A requires `RECREATE TABLE` (copy data, drop, rename) instead of
    a simple `DROP TABLE` or `ALTER TABLE DROP COLUMN`. The implementation
    plan must inspect `SELECT sqlite_version()` in a test before choosing the
    migration strategy.

- **Plan 019 cross-reference**: plan 019's maintenance notes (final bullet)
  say "`daily_checkin.credit_spend` deprecation (FT-6) is a multi-step
  migration risk — deprecate only after FT-1 and FT-3 are validated." This
  spike short-circuits that: if Option A is chosen, the table goes away
  entirely (no FT-1 and FT-3 of plan 019 are prerequisites for this spike's
  implementation plan, because the credit lump already works via
  `payment_method='credit'` transactions and `classify()` → `FixedOut`). The
  implementation plan for this spike should note this in plan 019's status row.
