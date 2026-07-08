# Plan 069: User-confirmed "obligation" identity (the series the spreadsheet doesn't store)

> **Executor instructions**: Follow step by step, run every verification, honor the
> STOP conditions, update `plans/README.md`. This introduces a new domain concept —
> read "Why this matters" and the CONTEXT vocabulary before touching code, and keep
> the matching **user-confirmed, never silent**.
>
> **Drift check (run first)**:
> `git diff --stat b65f0c6..HEAD -- src-tauri/src/google_sheets/import.rs src-tauri/migrations/ src/lib/api.ts CONTEXT.md`
> On any change, reconcile the "Current state" excerpts against live code first.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: none (better with 068 so `.xlsx` imports carry items too)
- **Category**: direction / correctness
- **Planned at**: commit `b65f0c6`, 2026-07-04 · **Reconciled at `381fb75`, 2026-07-05**
  (plans 068 + 070 merged): all of 069's references still hold — `classify_line_item`
  (import.rs:936), `normalize_item_section` (import.rs:917), the three `line_item`
  migrations, `person` (20240608000001), `update_transaction_items_cmd`
  (transactions.rs:509). `import.rs`/`api.ts` grew (070) so read live code for exact lines.
  The `.xlsx` path now carries line_items (068), so imported obligations resolve there too.

## Adversarial-review corrections (2026-07-04) — integrated below (changelog)

The items below are now folded into the Design / Scope / Maintenance sections; this list
is a changelog, not a separate source of truth.

1. **Exact `normalize(description) == match_desc` FAILS on the real sheet — say so up front,
   not in a STOP condition.** A real recurring loan parcela embeds a mutable **`N/36` counter
   inside its description** (changes every month), and one real line glues two obligations with
   "e". The resolver must **strip a trailing `\d+/\d+` counter** before comparing, and the
   match is always **user-confirmed via a preview** (below), never silent. Sample notes from
   ≥2 different years (grammar drifts — spec 008) and add resolver tests across that variance.
2. **`match_section` must be NORMALIZED, not raw.** `line_item.section` stores the header
   verbatim; punctuation drifts by year (`CONTAS` in 2025 vs `FATURAS:` in 2026). Normalize it
   (strip trailing colon, casefold, strip accents — reuse the existing `normalize_item_section`)
   before storing and comparing, or matching silently stops at a year boundary.
3. **Add the preview command.** The Design promises a confirm-preview but Scope lists no command
   for it. Add `preview_obligation_matches(match_desc, match_section) -> matched items/count`,
   reusing the resolver's WHERE clause without a persisted row.
4. **Give obligation matching its OWN `normalize` (do not share plan 071's).** Plan 071 is
   REJECTED, and its `normalize_item_desc` deliberately had no accent-folding; obligation
   matching needs accent-folding. Define a local `normalize` here; drop the "share with 071"
   coupling.
5. **`obligation_id` is the bridge plan 067 consumes.** 067's scenario override targets an
   `obligation_id` and subtracts the `line_item`s this resolver returns. When 067 lands, this
   resolver's query must also filter **`AND t.scenario_id IS NULL`** so an obligation never
   groups hypothetical scenario rows.
6. **`update_transaction_items_cmd` can rename a matched item** (`transactions.rs`), silently
   dropping it from an obligation. Add a regression test and document it as an accepted
   limitation (or warn on rename of a matched item).
7. **Label `obligation` explicitly as a Neko extension** in "Why this matters" (the sheet/method
   have no series concept), matching plan 067's framing convention.
8. **Fixes:** correct the migration citation to `20260620000001 / 20260620000002 /
   20260621000003` (the third is dated 2026-06-**21**); fix the itemization example to
   NEWLINE-separated items (not `/`); state whether `person_id` filters the resolver (via which
   join) or is authorship-only metadata excluded from the WHERE clause.

## Why this matters

The spreadsheet has **no concept of a recurring obligation as a first-class thing**.
A monthly rent is just one item (`R$ x - Aluguel`) inside a Saída cell, repeated —
with no id linking the twelve occurrences. `transaction.recurrence_id` only exists
for series *created in the app*; everything imported from the sheet has it `NULL`.
So today the app cannot answer "how has my rent changed over the year?", cannot
budget per obligation, and cannot cleanly power scenario what-ifs that *change* an
existing obligation (plan 067's override). This plan gives that missing identity the
only honest way it can exist for imported data: a **user-confirmed match**. The user
names a recurring item once ("Aluguel"); Neko tracks every matching line item and
shows exactly which ones — never guessing silently.

**`obligation` is a Neko convenience, not a method artifact.** The spreadsheet and the
method have no series/obligation concept (a monthly rent is twelve unlinked cells); this
layer *surpasses* the sheet without contradicting it — the same framing plan 067 uses for
its persisted scenarios and liquidity pockets. Label it that way in the doc.

## Current state

- `line_item` (migrations `20260620000001` / `20260620000002` / `20260621000003`) — `id, transaction_id,
  amount_cents, description, position, is_user_edited, section`. Items are re-derived
  from the cell note each import (`import.rs:518–621`).
- `classify_line_item(section, description) -> ItemKind` (`import.rs:936`) maps a
  section header (CONTAS/CARTÕES/DIÁRIO/…) to a money kind. Classification is by
  **section**, not description (owner decision, see `plans/README.md` Package K).
- `transaction.recurrence_id` (migration `20240612000005`) — app-created series
  only; `NULL` for imported rows.
- CONTEXT.md domain vocabulary: **Person** (not "user"); the money kinds
  Entrada/Saída/Diário/Cartão/Economia/Patrimônio. Use these exact terms.

## Design (first cut)

A new **obligation** is a user-named match rule over line items — resolved at query
time (line items are re-derived each import, so an `obligation_id` stored *on*
line_item would be wiped; store the rule, resolve on read):

```sql
CREATE TABLE obligation (
    id           TEXT PRIMARY KEY NOT NULL,
    person_id    TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,                 -- "Aluguel", "Internet"
    match_desc   TEXT NOT NULL,                 -- normalized description to match line_item.description
    match_section TEXT,                         -- OPTIONAL, stores the normalize_section'd value (e.g. "contas" — no colon/accents)
    kind         TEXT NOT NULL,                 -- the ItemKind the obligation belongs to
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- **Resolver** (pure where possible): a line item belongs to obligation O when
  `normalize_desc(line_item.description) == O.match_desc` AND (O.match_section IS NULL OR
  `normalize_section(line_item.section) == O.match_section`). **`normalize_desc` must strip
  a trailing `\d+/\d+` installment counter** — a real recurring parcela embeds a mutable
  `N/36` counter *inside* its description, so an exact match fails across months — and fold
  case + accents. **`normalize_section` reuses the existing `normalize_item_section`**
  (import.rs:917 — strips a trailing `:`, casefolds, strips accents), because section
  punctuation drifts by year (`CONTAS` in 2025 vs `FATURAS:` in 2026). Define both helpers
  HERE — do NOT share plan 071's (it is REJECTED and its helper had no accent-folding).
- **`person_id` is authorship-only**: `"transaction"` has no `person_id` column (ownership
  is only via `to_account_id → account.owner_person_id`), so the resolver does **not**
  filter by person — `obligation.person_id` records who created the obligation and is
  excluded from the match WHERE clause.
- **Known limitation (glued descriptions)**: a few real note lines glue two obligations
  onto one item with "e" (`X e Y`, sharing one amount). Exact post-normalize matching cannot
  split these — the affected-rows preview surfaces them and the user decides; do not
  auto-split.
- **User-confirmed, never silent:** creating an obligation always previews the set
  of currently-matching items ("isto vai agrupar N lançamentos — confira") before
  saving. The name and match are the user's, not inferred behind their back.
- **Divergence-safe:** an obligation is a *view/index* over line items; it never
  changes amounts or the cell-owns-total rule.

## Commands you will need

| Purpose    | Command                                                              | Expected |
|------------|---------------------------------------------------------------------|----------|
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked`          | all pass |
| Typecheck  | `npm run typecheck`                                                  | exit 0   |
| Full gate  | `npm run check`                                                      | exit 0   |

## Scope

**In scope**:
- Migration adding the `obligation` table.
- Rust: CRUD commands (`create_obligation`, `list_obligations`, `delete_obligation`);
  a resolver `obligation_items(obligation_id) -> Vec<line_item>` +
  `obligation_history(obligation_id) -> per-month totals`; and
  **`preview_obligation_matches(match_desc, match_section) -> matched items/count`** —
  the confirm-preview before `create_obligation`, reusing the resolver's WHERE clause
  without a persisted row.
- The obligation-local `normalize_desc` + `normalize_section` helpers (own, not shared).
- Frontend: a "marcar como obrigação recorrente" action on a line item, the
  confirm-preview, and a per-obligation view (history + monthly average + trend).
- Tests (resolver correctness + the preview count).

**Out of scope**:
- Auto-detecting obligations without user confirmation (explicitly not silent).
- Changing `classify_line_item` (section-based classification stays).
- Per-obligation budgets and the scenario override wiring — those are follow-ups
  (this plan provides the identity they will consume; see Maintenance notes).

## Steps

### Step 1: Migration + schema

Add `src-tauri/migrations/<timestamp>_obligation.sql` with the table above (naming
convention `<YYYYMMDDHHMMSS>_<name>.sql`; latest existing is
`20260621000004_economia_annotation.sql`).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked -- migrat`
(migrations run) → pass.

### Step 2: Resolver + CRUD (Rust)

Implement the resolver and CRUD in a new module `src-tauri/src/obligations.rs` (or
alongside `line_item` handling). Register the commands in `src-tauri/src/lib.rs`'s
`generate_handler!`. The resolver is a SQL query over `line_item` joined to
`"transaction"` for the date; keep the matching rule identical to the design.

**Verify**: `cargo test ... obligation` → resolver tests pass.

### Step 3: DTO + API binding

Add the DTOs to `src/lib/api.ts` (mirror the Rust structs; snake_case `_cents`).
Match the existing `MonthMetric`/`line_item` style.

**Verify**: `npm run typecheck` → exit 0.

### Step 4: UI — mark, confirm, view

On the itemized ledger view (`src/screens/TransactionsScreen.tsx` shows items), add
"marcar como obrigação recorrente" on a line item → a dialog that pre-fills the name
from the description, shows the **preview count** of matching items, and saves on
confirm. Add a per-obligation view (a new screen or a section) showing the monthly
history, average, and simple trend. Use design-system tokens; money via `<Money>`.

**Verify**: `npm run test:run` + `npm run e2e` smoke → green.

## Test plan

- Resolver: seed line items across 3 months with description "aluguel" under section
  header "CONTAS:" plus unrelated items → an obligation `(match_desc="aluguel",
  match_section="contas")` (note the stored value is `normalize_section`'d) resolves to
  exactly those 3, and `obligation_history` returns the 3 monthly totals.
- Normalize: "Aluguel ", "aluguel", "ALUGUEL" all match the same obligation.
- **Counter-strip**: a line item whose description carries a trailing `N/36` counter that
  changes month to month still matches the same obligation across all months.
- **Cross-year section**: items under `CONTAS` (2025, no colon) and `FATURAS:` (2026, colon)
  match their obligation via `normalize_section` — no silent break at the year boundary.
- Preview count matches the resolver (the number shown before saving == the number
  grouped after).
- Delete cascade: deleting the person removes its obligations; deleting an
  obligation leaves the line items untouched.
- Frontend: the confirm dialog shows the count; the per-obligation view renders the
  history (model after an existing screen test).

## Done criteria

- [ ] A user can name a recurring item as an obligation and see every matching
      occurrence — with a confirm-preview, never silent.
- [ ] The resolver never mutates amounts or the cell-owns-total rule.
- [ ] `npm run check` exits 0; new tests pass.
- [ ] Domain terms match CONTEXT.md (Person, the money kinds).
- [ ] `plans/README.md` row updated.

## STOP conditions

- The match would need to be inferred/auto-applied without user confirmation — it
  must not; keep it user-driven.
- `line_item.description` turns out to be too noisy to match on even after normalize
  (e.g. amounts embedded in the description) — report the real note shapes before
  designing a fuzzier matcher; do not ship silent fuzzy matching.
- Storing identity would require a column on `line_item` that survives re-import —
  it can't (items are re-derived); keep the match-rule-resolved-on-read design.

## Maintenance notes

- This is the identity layer that **plan 067's scenario override** consumes directly:
  `scenario_override.obligation_id` FKs to `obligation`, and the override subtracts the
  `line_item`s this resolver returns. **When 067 lands, this resolver's query must also
  filter `AND t.scenario_id IS NULL`** so an obligation never groups hypothetical rows.
- It also enables per-obligation budgets and "how did X change this year" — both
  read-only consumers of the resolver.
- The `normalize_desc`/`normalize_section` helpers are **local to this plan** (NOT shared
  with the REJECTED plan 071); treat them as a stable contract. `normalize_section` mirrors
  the engine's `normalize_item_section`.
- **`update_transaction_items_cmd` (transactions.rs) rewrites a transaction's items on
  edit** — renaming an item's description silently drops it from an obligation match. Add a
  regression test and document it as an accepted limitation (or warn on rename of a matched
  item).
- **`obligation.person_id`**: `"transaction"` has no `person_id` column — ownership is only
  reachable via `to_account_id → account.owner_person_id`. Decide explicitly whether the
  resolver filters by person (join through `account`) or `person_id` is authorship-only
  metadata excluded from the WHERE clause; state it in the Design.
- Section-based classification is unchanged; an obligation carries `kind` for
  display/grouping but does not re-classify anything.
