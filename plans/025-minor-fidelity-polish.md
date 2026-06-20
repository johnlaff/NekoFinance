# Plan 025: Minor fidelity polish: patrimônio de-emphasis, dead category kit, reserve ADR reconcile

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
> git diff --stat 51afe33..HEAD -- \
>   src/features/pockets/PocketsCard.tsx \
>   src/App.css \
>   src/design-system/components/finance/TransactionRow.d.ts \
>   src/design-system/components/finance/TransactionRow.jsx \
>   src/design-system/_ds_manifest.json \
>   src/design-system/_ds_bundle.js \
>   src/design-system/_adherence.oxlintrc.json \
>   docs/adr/0002-reserve-as-first-class-entity.md \
>   CONTEXT.md
> ```
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `51afe33`, 2026-06-20

## Why this matters

The method treats patrimônio (net worth) as a secondary concept — something
you look at after liquid cash and the reserve are healthy. The current
`PocketsCard` header reads "Bolsos & patrimônio" at full `dash-card__title`
weight, which visually promotes net worth to hero status alongside the liquid
and reserve groups. This contradicts the method hierarchy (caixa and reserva
first; patrimônio is the whole balance-sheet picture, shown after).

A dead DS-kit file (`finance/TransactionRow.d.ts` and its companion `.jsx`)
carries a `category` prop that the active production component
(`design-system/components/TransactionRow.tsx`) and the DB schema no longer
use (categories were demoted to tags in migrations 20240612000003/04). The
kit files have zero production-code imports; they only exist in DS preview
tooling. Removing the dead type declaration removes a misleading signal for
any future reader.

ADR-0002 says `reserve.current_months` is a field tracked in the `reserve`
table, but production never writes it — `reserve_months` is derived live
(reserve-account balance ÷ `realized_monthly_baseline`). `CONTEXT.md` line 105
records this fact but ADR-0002 does not. The ADR needs a reconciliation note
so the next engineer doesn't write a writer for `reserve.current_months`
thinking they are completing unfinished work.

## Current state

### Finding 1 — Patrimônio prominence in `PocketsCard`

**File**: `src/features/pockets/PocketsCard.tsx` (full file, 57 lines)

Relevant lines (live at time of writing):

```tsx
// PocketsCard.tsx:19–29
  return (
    <div className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title">
          <Landmark size={16} strokeWidth={1.75} className="dash-card__ic" />
          Bolsos &amp; patrimônio
        </span>
        {pockets && pockets.accounts.length > 0 && (
          <span className="pockets-networth">
            Patrimônio <Money cents={pockets.net_worth_cents} size="sm" sign="auto" />
          </span>
        )}
      </div>
```

The `dash-card__title` class applies `font-size: var(--fs-sm); font-weight: var(--fw-bold); color: var(--text-strong)` (see `src/App.css:573–579`). "Patrimônio" appears in the card title itself and again as a badge in the header — double-exposing it at full emphasis weight.

The `.pockets-networth` class in `src/App.css:2387–2395` renders it at `var(--fs-sm)` / `color: var(--text-muted)` — already muted — but its presence in the header row puts it at the same visual level as the Caixa/Reserva groups.

The method intent (confirmed in `specs/007-pockets-liquidity/spec.md:33`): "vejo no Dashboard um cartão **Bolsos & patrimônio** com os grupos (caixa, reserva, restrito, ilíquido) e o patrimônio total." Patrimônio is the footer/total, not a hero. The spec names the 4 liquidity groups first; net worth is the aggregate.

Target hierarchy: card title = "Bolsos" (or keep "Bolsos & patrimônio" if concision is preferred), net worth moves below the pockets grid as a quiet footer line, not a header badge.

### Finding 2 — Dead category-kit files

**Files** (design-system kit only, not the production component):

- `src/design-system/components/finance/TransactionRow.d.ts` (31 lines)
- `src/design-system/components/finance/TransactionRow.jsx` (107 lines)

These are the design-system preview-kit versions of TransactionRow. The
production component is `src/design-system/components/TransactionRow.tsx`
(316 lines), which has no `category` prop at all.

The kit `.d.ts` (lines 8–10):

```ts
  /** Category label. */
  category?: string;
  /** Category dot color (a chart-series var). */
  categoryColor?: string;
```

The kit `.jsx` (lines 48–81) renders a `nk-txn__cat` span when `category` is
truthy, including a colored dot — a UI pattern the active production row no
longer exposes.

**Production imports**: zero. Verified with:

```
grep -rn "from.*finance/TransactionRow\|require.*finance/TransactionRow" src/features/ src/screens/ src/lib/ src/shell/
```

(returned no output at plan time)

The kit files ARE referenced in DS preview tooling only:

- `src/design-system/_ds_manifest.json:22` — lists the kit component for the preview gallery
- `src/design-system/_ds_bundle.js:1` — generated bundle includes the kit
- `src/design-system/ui_kits/dashboard/DashboardScreen.jsx:8,491–520` — imports kit `TransactionRow` for the preview screen (not production)
- `src/design-system/components/finance/finance.card.html:65,108–128` — preview card
- `src/design-system/_adherence.oxlintrc.json:134` — adherence rule anchored to the kit's prop list (includes `category`)

The executor must update all five referencing files when removing `category`/`categoryColor` from the kit. Do NOT delete the kit files — they are the DS preview component, actively shown in the gallery. Only remove the `category` and `categoryColor` props (and their render logic) from the kit to align with the active production component.

### Finding 3 — ADR-0002 vs live production

**File**: `docs/adr/0002-reserve-as-first-class-entity.md` (17 lines, full content at plan time):

```markdown
# ADR-0002: Reserve as a First-Class Entity

...

## Decision

Model the emergency reserve as its own entity (`reserve` table) rather than a
field on `account` or a derived query. Include `target_months`, `current_months`,
`trend`, and a separate `reserve_snapshot` table for monthly history to detect
direction.
```

**`CONTEXT.md` line 105** (the reconciling fact, already in the codebase):

> **Reserve months** (dashboard): derived live as reserve-account balance ÷
> monthly cost of living (`realized_monthly_baseline`); the
> `reserve.current_months` column has no production writer.

The ADR says "Include `current_months`" in the reserve table; CONTEXT.md says
the column has no production writer and the value is derived live. The ADR
does not acknowledge this gap. A new engineer reading only the ADR would
attempt to write a background job to populate `current_months`, duplicating
the live derivation and potentially creating a stale-vs-live conflict.

The reconciliation note should be added to ADR-0002 as a "## Production
reality (2026)" section: `current_months` and `reserve_snapshot` exist in the
schema but have no production writer; the dashboard derives `reserve_months`
live. If a writer is ever added, it must stay consistent with the live formula
in `get_forecast`.

## Commands you will need

| Purpose        | Command              | Expected on success        |
| -------------- | -------------------- | -------------------------- |
| Full gate      | `npm run check`      | exit 0                     |
| Typecheck only | `npm run typecheck`  | exit 0, no errors          |
| Lint only      | `npm run lint`       | exit 0                     |
| Unit tests     | `npm run test:run`   | all pass                   |
| React Doctor   | `npm run doctor`     | 0 new findings vs baseline |
| Rust checks    | `npm run rust:check` | exit 0                     |
| E2E smoke      | `npm run e2e`        | all pass                   |

## Suggested executor toolkit

- Read `docs/adr/0002-reserve-as-first-class-entity.md` and `CONTEXT.md`
  lines 73–105 before writing step 3, so the reconciliation prose is accurate.
- For step 1, reference `specs/007-pockets-liquidity/spec.md` to confirm the
  intended card hierarchy before editing the JSX.

## Scope

**In scope** (the only files you should modify):

- `src/features/pockets/PocketsCard.tsx` — step 1: demote net-worth to footer
- `src/App.css` — step 1: optional minor CSS tweak if `.pockets-networth` needs
  a separator or footer margin; do not add new class names
- `src/design-system/components/finance/TransactionRow.d.ts` — step 2: remove `category` / `categoryColor` props
- `src/design-system/components/finance/TransactionRow.jsx` — step 2: remove `category` / `categoryColor` render logic
- `src/design-system/_ds_manifest.json` — step 2: no change needed (component stays; only props change)
- `src/design-system/_ds_bundle.js` — step 2: must be regenerated or manually patched to remove category from the bundle
- `src/design-system/_adherence.oxlintrc.json` — step 2: remove `category` and `categoryColor` from the allowed-prop regex
- `src/design-system/ui_kits/dashboard/DashboardScreen.jsx` — step 2: remove any `category=` / `categoryColor=` JSX attributes on `<TransactionRow>`
- `src/design-system/components/finance/finance.card.html` — step 2: remove any `category=` / `categoryColor=` attributes used in the preview card
- `docs/adr/0002-reserve-as-first-class-entity.md` — step 3: add reconciliation section

**Out of scope** (do NOT touch):

- `src/design-system/components/TransactionRow.tsx` — the active production component; already correct, no `category` prop
- `src/design-system/components/TransactionRow.test.tsx` — production component tests; do not touch
- Any Rust source, migrations, or schema files — reserve math is out of scope
- `CONTEXT.md` — already accurate (line 105 records the live-derivation fact); no change needed
- `src/design-system/_ds_manifest.json` — manifest lists the component by name only; removing props from the kit files is sufficient
- Any file not listed above

## Git workflow

- Branch: `advisor/025-minor-fidelity-polish`
- Commit style matches the repo's recent log: `fix:`, `chore:`, `docs:` prefixes with a Portuguese or English summary
- One commit per step is fine; a single squash commit is also acceptable
- Do NOT push or open a PR unless the operator says to

## Steps

### Step 1: Demote patrimônio to a secondary footer in `PocketsCard`

Open `src/features/pockets/PocketsCard.tsx`.

Current header block (lines 20–29):

```tsx
<div className="dash-card__head">
  <span className="dash-card__title">
    <Landmark size={16} strokeWidth={1.75} className="dash-card__ic" />
    Bolsos &amp; patrimônio
  </span>
  {pockets && pockets.accounts.length > 0 && (
    <span className="pockets-networth">
      Patrimônio <Money cents={pockets.net_worth_cents} size="sm" sign="auto" />
    </span>
  )}
</div>
```

Change so that:

1. The `dash-card__title` reads "Bolsos" (or "Bolsos & liquidez" — pick whichever is cleaner; the method's first concern is liquid cash, not net worth).
2. The `pockets-networth` span moves OUT of `dash-card__head` and into `dash-card__body`, placed AFTER the `pockets-grid` — as a quiet one-line footer row showing "Patrimônio total: R$ X". It must remain conditionally rendered (only when accounts exist). It must keep the existing `pockets-networth` CSS class.
3. Do NOT remove `net_worth_cents` from the data or the `Money` component call — data stays, emphasis changes.

Target shape (illustrative; exact classnames must stay compatible with existing CSS):

```tsx
      <div className="dash-card__head">
        <span className="dash-card__title">
          <Landmark size={16} strokeWidth={1.75} className="dash-card__ic" />
          Bolsos
        </span>
      </div>
      <div className="dash-card__body">
        {/* ... error / empty states unchanged ... */}
        {pockets && pockets.accounts.length > 0 && (
          <>
            <div className="pockets-grid">
              {/* ... existing GROUPS map unchanged ... */}
            </div>
            <div className="pockets-networth">
              Patrimônio total <Money cents={pockets.net_worth_cents} size="sm" sign="auto" />
            </div>
          </>
        )}
      </div>
```

The CSS for `.pockets-networth` in `src/App.css:2387–2395` renders it at
`var(--fs-sm)` / `color: var(--text-muted)` which is already quiet — no CSS
changes are required. If you want to add a subtle top border or padding to
separate the footer line from the grid, you may add one `border-top` rule to
`.pockets-networth` in `src/App.css`, but do not introduce new class names.

**Verify**: `npm run typecheck` → exit 0, no errors

### Step 2: Remove `category` / `categoryColor` from the DS kit TransactionRow

The goal is to align the kit's public API with the active production component,
which has no `category` prop. The kit files themselves stay (they are shown in
the DS gallery); only the `category` and `categoryColor` prop surface is removed.

**2a. Edit `src/design-system/components/finance/TransactionRow.d.ts`**

Remove lines 8–10 and the jsdoc comment at line 7 ("`/** Category label. */`"),
line 9 ("`category?: string;`"), line 10 ("`/** Category dot color (a chart-series var). */`"),
and line 11 ("`categoryColor?: string;`"). The resulting interface must not
contain `category` or `categoryColor`.

Also update the JSDoc comment at line 29 — remove "category," from the list
of fields: "Carries owner, category, amount, status..." → "Carries owner, amount, status...".

**2b. Edit `src/design-system/components/finance/TransactionRow.jsx`**

Remove `category` and `categoryColor` from the destructured props (lines 48–49).
Remove the `nk-txn__cat` render block (lines 78–83):

```jsx
{
  category ? (
    <span className="nk-txn__cat">
      <span className="nk-txn__catdot" style={{ background: categoryColor }} />
      {category}
    </span>
  ) : null;
}
```

Remove also the `.nk-txn__cat` and `.nk-txn__catdot` CSS rules from the
`CSS` constant (lines 13–14 of the `CSS` string). Verify the CSS string compiles
without those two rules before removing them.

**2c. Edit `src/design-system/_adherence.oxlintrc.json`**

Find the allowed-prop regex for `<TransactionRow>` (line 134):

```
"name!=/^(?:date|merchant|category|categoryColor|owner|amount|positive|status|confidence|selected|onClick|className|key|ref|className|style|children)$/"
```

Remove `category|categoryColor|` from the alternation so it reads:

```
"name!=/^(?:date|merchant|owner|amount|positive|status|confidence|selected|onClick|className|key|ref|className|style|children)$/"
```

Also update the `message` at line 135 to remove `category, categoryColor` from
the declared-props list.

**2d. Edit `src/design-system/ui_kits/dashboard/DashboardScreen.jsx`**

Search for every `<TransactionRow` element (lines 491–520). Remove any
`category={...}` or `categoryColor={...}` attributes. If none are present,
no change needed (confirm with `grep -n "category" src/design-system/ui_kits/dashboard/DashboardScreen.jsx`).

**2e. Edit `src/design-system/components/finance/finance.card.html`**

Search for `<TransactionRow` elements (lines 108–128). Remove any
`category={...}` or `categoryColor={...}` attributes. Same grep check applies.

**2f. Patch `src/design-system/_ds_bundle.js`**

There is no generation script for this file (`grep "ds.bundle\|dsBundle" package.json`
returns nothing). It must be patched manually. The file is 5,852 lines; find
the `TransactionRow` section with:

```
grep -n "nk-txn__cat\|categoryColor\|category" src/design-system/_ds_bundle.js
```

At plan time, the relevant lines are approximately:

- Lines 1620–1621: `.nk-txn__cat{...}` and `.nk-txn__catdot{...}` CSS rules inside the `CSS` string — remove both
- Lines 1660–1661: `category,` and `categoryColor = "var(--chart-3)",` in the props destructure — remove both
- Lines 1712–1724: the `category ? React.createElement("span", { className: "nk-txn__cat" }, ...)` block — remove the entire conditional render block
- Lines 3542–3571: `category: "Groceries"`, `categoryColor: "var(--chart-2)"` etc. on `<TransactionRow>` calls in the dashboard UI-kit section — remove those prop lines (4 pairs, one per TransactionRow call)

Each removal must leave the surrounding code syntactically valid. After
patching, verify:

```
grep -n "nk-txn__cat\|categoryColor" src/design-system/_ds_bundle.js
```

→ no output (line 2095 contains "category = ∅" in a string literal — that is
a different context and must NOT be removed; only remove the TransactionRow
prop surface).

**Verify**:

```
npm run lint
```

→ exit 0 (the adherence rule now correctly excludes `category`/`categoryColor`)

```
grep -rn "category" src/design-system/components/finance/TransactionRow.d.ts
```

→ no output

```
grep -n "nk-txn__cat" src/design-system/components/finance/TransactionRow.jsx
```

→ no output

### Step 3: Add production-reality reconciliation note to ADR-0002

Open `docs/adr/0002-reserve-as-first-class-entity.md`.

Append the following section at the end of the file (after "## Why record it here"):

```markdown
## Production reality (2026-06)

`reserve.current_months` and `reserve_snapshot` exist in the schema but have
no production writer as of this reconciliation. The dashboard derives
`reserve_months` live: reserve-account balance ÷ `realized_monthly_baseline`
(see `CONTEXT.md` — "Reserve months (dashboard)"). The live-derived value is
the source of truth for the UI.

`trend` in the `reserve` table is also unwritten in production; it is not yet
surfaced in the UI.

**Implication for future work**: if a background writer for `current_months` or
`reserve_snapshot` is ever added, it must use the same formula as the live
derivation to avoid a stale-vs-live conflict. Do not add a writer unless the
motivation is historical trend tracking (the only feature that requires
persisted snapshots); the live computation is sufficient for current UI needs.
```

**Verify**:

```
grep -n "Production reality" docs/adr/0002-reserve-as-first-class-entity.md
```

→ line found

## Test plan

This plan makes no changes to business logic, finance math, or data contracts.
No new tests are required.

- Step 1 changes JSX structure only. The existing Playwright E2E smoke covers the
  dashboard render path; run `npm run e2e` and confirm no new failures.
- Step 2 removes dead prop surface from a DS kit file (not the production
  component). No production tests reference `finance/TransactionRow`. Confirm
  with: `grep -rn "finance/TransactionRow" src/test/ src/__tests__/` → no output.
- Step 3 is documentation only.

Full gate after all steps: `npm run check` → exit 0.

## Done criteria

All must hold before marking this plan DONE in `plans/README.md`:

- [ ] `npm run check` exits 0 (typecheck + lint + test:run + doctor + rust:check)
- [ ] `npm run e2e` exits 0 (no new Playwright failures)
- [ ] `grep -n "Bolsos &amp; patrimônio" src/features/pockets/PocketsCard.tsx` — returns no output (title de-emphasized or changed)
- [ ] `grep -n "pockets-networth" src/features/pockets/PocketsCard.tsx` — match is inside `dash-card__body`, not `dash-card__head`
- [ ] `grep -n "category" src/design-system/components/finance/TransactionRow.d.ts` — no output
- [ ] `grep -n "nk-txn__cat" src/design-system/components/finance/TransactionRow.jsx` — no output
- [ ] `grep -n "nk-txn__cat\|categoryColor" src/design-system/_ds_bundle.js` — no output (note: `category =` string literals inside non-TransactionRow sections may remain; only TransactionRow prop surface must be gone)
- [ ] `grep -n "category\|categoryColor" src/design-system/_adherence.oxlintrc.json` — no output (the allowed-prop regex no longer names them)
- [ ] `grep -n "Production reality" docs/adr/0002-reserve-as-first-class-entity.md` — line found
- [ ] `git diff --name-only` lists only files in the in-scope list above
- [ ] `plans/README.md` status row for plan 025 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the "Current state" excerpts doesn't match what is in the file
  (the codebase has drifted since this plan was written — run the drift check
  first).
- `grep -rn "from.*finance/TransactionRow\|require.*finance/TransactionRow" src/features/ src/screens/ src/lib/ src/shell/` returns ANY output — the kit file has acquired a production import; do NOT remove the kit files and do NOT remove the props without understanding the caller's needs.
- Removing `category` from the kit `.jsx` causes a Playwright screenshot
  regression in the DS preview gallery (finance card render).
- `npm run check` fails after step 1 for a reason unrelated to PocketsCard.
- The `_ds_bundle.js` patch leaves the file syntactically broken (e.g. a
  dangling comma, an unclosed ternary) and `npm run lint` continues to fail
  after two fix attempts — report the exact error lines rather than guessing.
- Step 3's prose conflicts with actual production code (e.g. a `reserve_snapshot`
  writer is found in `src-tauri/src/commands/` that was added after this plan
  was written) — update the note accurately instead of using the canned text.

## Maintenance notes

- **Step 1 follow-up**: if the `PocketsCard` card is later given a collapsible
  body or a "details" expansion, net worth should stay at the footer of the
  expanded section, never promoted above the Caixa/Reserva figures.
- **Step 2 follow-up**: if the import-review screen ever adds category
  auto-assignment (a future spec), the production `TransactionRow.tsx` will need
  a `tags` prop (using the method's tag model, not the deprecated category tree).
  Do not re-add `category` to the kit; add `tags: string[]` to the production
  component instead.
- **Step 3 follow-up**: if a reserve snapshot writer is implemented (for trend
  tracking), update ADR-0002 again to record the decision and the formula used.
- **Reviewer focus for PR**: confirm that the `pockets-networth` element still
  renders the correct `net_worth_cents` value after moving it to the footer; the
  data must not be lost, only re-positioned.
