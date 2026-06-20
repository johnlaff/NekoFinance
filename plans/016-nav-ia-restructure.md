# Plan 016: Navigation/IA restructure + dashboard de-duplication

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
> git diff --stat d183bbf..HEAD -- src/shell/AppShell.tsx src/screens/DashboardScreen.tsx src/screens/dashboard/MonthLedgerCard.tsx src/screens/TransactionsScreen.tsx src/screens/CopilotScreen.tsx src/screens/MethodologyScreen.tsx src/App.tsx src/App.navigation.test.tsx
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
- **Category**: direction
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The sidebar presents 8 equally-weighted items under a single "Finanças" heading, meaning the user
has no visual cue about which items belong to the primary daily check-in flow (Dashboard,
Lançamentos) vs. deeper analysis tools (Anual, Horizonte) vs. informational pages (Metodologia)
vs. an unbuilt feature stub (Mia/Copilot). This conflicts directly with the product goal of a
<30s nightly check-in and the "data-first, chrome-second" principle from `PRODUCT.md`. On the
dashboard, the four-tile `MetricBar` (lines 176–204 of `DashboardScreen.tsx`) duplicates numbers
already visible in the hero section above and in cards below, adding cognitive load with no
additional signal. The Transactions screen hosts a second search input (lines 261–269) that is
wired to the same `query` prop as the global ⌘K header search, creating two competing affordances
for the same action. The empty state at lines 293–303 names "Configurações" as text but provides
no navigation affordance. These compound into a UX that over-complicates a simple daily ritual.

## Current state

### In-scope files and their roles

- `src/shell/AppShell.tsx` — sidebar shell: `Screen` type union, `NAV_ITEMS` array (line 44),
  nav render loop (line 101), global ⌘K search form (lines 165–185).
- `src/App.tsx` — top-level router; wires `onNavigate={setScreen}` and renders each screen
  behind a `{screen === "..."}`guard. Houses `handleSearch` (line 43) which pushes to
  `"transactions"` on ⌘K submit.
- `src/screens/DashboardScreen.tsx` — dashboard; contains the 4-tile `MetricBar` grid
  (class `dash-grid4`, lines 176–204), the hero section (lines 107–174), and the
  `onAskMia` button CTA (lines 136–144).
- `src/screens/dashboard/MonthLedgerCard.tsx` — the "Dia a dia" card with a `<tfoot>` that
  shows a "Performance" row (lines 195–215). The `title` attribute on the `<th>` at line 196
  already acknowledges the distinction: _"Distinta da Performance do método em Totais"_.
- `src/screens/TransactionsScreen.tsx` — transactions list; has a second `<label class="ak-search">` search input (lines 261–269) and an empty-state paragraph that mentions "Configurações" with no `action` prop (lines 293–303).
- `src/screens/CopilotScreen.tsx` — Mia stub; has a "Em desenvolvimento" badge (line 67) and a
  deterministic facts panel (lines 77–131) that is the real value today.
- `src/screens/MethodologyScreen.tsx` — static 7-card doc page; currently a top-level nav item
  with no contextual entry point.
- `src/App.navigation.test.tsx` — RTL integration tests for all nav items; tests for "Mia",
  "Metodologia", "Lançamentos", and ⌘K search (lines 38–104). These tests MUST stay green and
  **must be updated** to match the new nav labels/grouping this plan produces.

### Key excerpts (verify these match before making any change)

**AppShell.tsx — Screen union and NAV_ITEMS (lines 21–53)**:

```ts
// line 21
export type Screen =
  | "dashboard"
  | "totais"
  | "anuais"
  | "horizonte"
  | "transactions"
  | "tags"
  | "copilot"
  | "methodology"
  | "settings";

// line 44 — single flat array, 8 items
const NAV_ITEMS: { key: Screen; label: string; icon: typeof LayoutDashboard }[] = [
  { key: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { key: "totais", label: "Totais", icon: Calculator },
  { key: "anuais", label: "Anual", icon: TrendingUp },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "transactions", label: "Lançamentos", icon: Receipt },
  { key: "tags", label: "Tags", icon: TagsIcon },
  { key: "copilot", label: "Mia", icon: Sparkles },
  { key: "methodology", label: "Metodologia", icon: BookOpen },
];
```

**AppShell.tsx — nav render (lines 101–126)**:

```tsx
// line 101
<nav className="ak-nav">
  <div className="ak-navh">Finanças</div>
  {NAV_ITEMS.map((n) => (
    <button key={n.key} ... onClick={() => onNavigate(n.key)}>
      ...
    </button>
  ))}
  <div className="ak-navh">Sistema</div>
  <button ... onClick={() => onNavigate("settings")}>
    Configurações e privacidade
  </button>
</nav>
```

**DashboardScreen.tsx — 4-tile MetricBar (lines 176–204)**:

```tsx
// line 176
<div className="dash-grid4">
  <MetricTile label="Saldo projetado" value={summary ? fmtBRL(summary.balance) : "—"}
    icon={<TrendingUp size={15} strokeWidth={1.75} />}
    sublabel={forecast ? `Fim de ${monthNamePtBR(forecast.today)}` : "Fim do mês"}
  />
  <MetricTile label="Diário de hoje" value={summary ? fmtBRL(summary.daily_spend_today) : "—"}
    sublabel={summary ? `de ${fmtBRL(summary.daily_budget)}` : ""}
  />
  <MetricTile label="Crédito no mês" value={summary?.has_credit ? fmtBRL(summary.credit_spend_month) : "—"}
    icon={<TrendingDown size={15} strokeWidth={1.75} />}
    sublabel={...}
  />
  <MetricTile label="Reserva" value={summary ? `${summary.reserve_months.toFixed(1)} meses` : "—"}
    icon={reserveTrendIcon}
    sublabel="Mín. 6 · paz 12+"
  />
</div>
```

**MonthLedgerCard.tsx — "Performance" footer row (lines 195–215)**:

```tsx
// line 195
<tr className="fc-foot">
  <th
    scope="row"
    title="Performance do mês na planilha (entradas − saída total). Distinta da Performance do método em Totais, que também desconta economia e a previsão do diário restante."
  >
    Performance
  </th>
  <td className="money" colSpan={3}>
    <span style={{ color: "var(--text-faint)", fontSize: "var(--fs-micro)" }}>
      entradas − saída total (do mês)
    </span>
  </td>
  <td className="money">
    <Money cents={foot.performance} size="sm" sign="auto" />
  </td>
</tr>
```

**TransactionsScreen.tsx — second search input (lines 261–269)**:

```tsx
// line 261
<label className="ak-search txs-tools__search">
  <Search size={15} strokeWidth={1.75} />
  <input
    aria-label="Filtrar por descrição"
    placeholder="Filtrar por descrição…"
    type="search"
    value={query}
    onChange={(e) => onQueryChange(e.target.value)}
  />
</label>
```

**TransactionsScreen.tsx — empty-state without navigation action (lines 293–303)**:

```tsx
// line 293
{visible.length === 0 ? (
  <EmptyState
    variant="empty"
    title="Nenhum lançamento encontrado"
    description={
      transactions.length === 0
        ? "Importe sua planilha em Configurações para começar."
        : "Nenhum resultado para o filtro atual."
    }
  />
```

**CopilotScreen.tsx — "Em desenvolvimento" badge (line 67)**:

```tsx
// line 67
<span className="cop-panel__badge">
  <Badge tone="warning">Em desenvolvimento</Badge>
</span>
```

**App.navigation.test.tsx — tests that reference current nav labels (lines 38–104)**:
The test at line 53 clicks `{ name: "Metodologia" }` directly. The test at line 62 clicks
`{ name: "Mia" }`. Both must be updated if labels or nav placement change.

### Conventions

- **No manual `memo`/`useMemo`/`useCallback`** — React Compiler is enabled. See top-level
  comment in `src/App.tsx` (line 43): _"React Compiler memoizes; no manual useCallback needed."_
- **Icons**: lucide-react, always `size={N} strokeWidth={1.75}`.
- **Design system tokens**: Only tokens defined in `src/design-system/` — do NOT invent tokens.
  `--surface-1` does not exist; use `--surface`.
- **Domain vocabulary** (from `CONTEXT.md`): Transaction types are `income | expense | transfer`.
  Five movement kinds: Entrada, Saída (fixed), Diário, Economia (transfer), Cartão (credit).
  "Performance" in the spreadsheet footer = `income − (fixed_out + daily_out)`. "Performance"
  in the method/Totais screen = `income − (saída + diário + economia + previsão diário restante)`.
- **InfoPopover**: available at `src/design-system/components/InfoPopover.tsx`; accepts a `term`
  key from the `GLOSSARY` record or an inline `{ title?, body }` object. Use it for contextual
  help; do NOT add a separate help route.
- **EmptyState `action` prop**: the `EmptyState` component accepts an `action?: ReactNode` prop
  (visible in `DashboardScreen.tsx` line 63 — `action={<Button ...>Tentar novamente</Button>}`).
- **`Screen` type is exported** from `AppShell.tsx` and imported by `App.tsx`. Changing the type
  union requires updating both files and the navigation test.
- **Commit style**: conventional commits with scope, e.g. `feat(shell): ...`, `fix(dashboard): ...`.
  Match the style in `git log` (`fix:`, `chore:`, `feat:` with colon-space).
- **Method-neutral language**: this repo is public. Never reference external brand names, apps,
  courses, or reverse-engineering in code comments, strings, or this file.

## Commands you will need

| Purpose          | Command                | Expected on success                      |
| ---------------- | ---------------------- | ---------------------------------------- |
| Typecheck        | `npm run typecheck`    | exit 0, no TS errors                     |
| Lint             | `npm run lint`         | exit 0, no ESLint errors                 |
| Unit tests       | `npm run test:run`     | all pass                                 |
| Full gate        | `npm run check`        | exit 0                                   |
| E2E visual smoke | `npm run e2e`          | all pass; screenshots in `test-results/` |
| Privacy scan     | `npm run privacy:scan` | exit 0                                   |

## Suggested executor toolkit

- Use the `impeccable` or `neko-finance-design` skill if available when making
  sidebar copy decisions (the "Midnight Ledger" design system governs label tone).
- Read `PRODUCT.md` lines 13–17 ("daily check-in under 30 seconds") before writing
  any nav copy; the principle is the filter.

## Scope

**In scope** (the only files you should modify):

- `src/shell/AppShell.tsx`
- `src/screens/DashboardScreen.tsx`
- `src/screens/dashboard/MonthLedgerCard.tsx`
- `src/screens/TransactionsScreen.tsx`
- `src/screens/CopilotScreen.tsx` — nav presence only (label/placement); do NOT change the facts panel or roadmap
- `src/screens/MethodologyScreen.tsx` — only if removing it from top-level nav requires adjusting its own header
- `src/App.tsx` — only if `Screen` type changes require import updates
- `src/App.navigation.test.tsx` — must be updated to match new nav labels/groups
- `src/screens/DashboardScreen.test.tsx` — may need updating if `dash-grid4` is removed

**Out of scope** (do NOT touch):

- `src/screens/TotaisScreen.tsx`, `src/screens/AnnualScreen.tsx`,
  `src/screens/HorizonteScreen.tsx`, `src/screens/TagsScreen.tsx` — not involved in
  this IA restructure.
- `src/screens/SettingsScreen.tsx` — settings content/flow is out of scope; only a CTA
  label change in the Transactions empty state touches it indirectly.
- `src/design-system/` — no new components; use existing `InfoPopover`, `EmptyState`, `Button`.
- Any Rust/Tauri source under `src-tauri/` — this plan is frontend-only.
- Any new screen or route — do NOT add a new `Screen` value; reduce, do not expand.
- `plans/README.md` — update the status row when done, but do not touch other rows.

## Git workflow

- Branch: `advisor/016-nav-ia-restructure`
- Commit one logical unit at a time (one commit per step group is fine; never let the
  build be broken between commits).
- Message style — match repo convention: `fix(shell): ...` / `feat(dashboard): ...` /
  `refactor(transactions): ...` with imperative present-tense description.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Drift check

Before touching any file, run the drift check from the header:

```
git diff --stat d183bbf..HEAD -- src/shell/AppShell.tsx src/screens/DashboardScreen.tsx src/screens/dashboard/MonthLedgerCard.tsx src/screens/TransactionsScreen.tsx src/screens/CopilotScreen.tsx src/screens/MethodologyScreen.tsx src/App.tsx src/App.navigation.test.tsx
```

Open each in-scope file and confirm the excerpts in "Current state" match the live code at
the cited line numbers (±2 lines is acceptable for blank-line drift; structural mismatch is
a STOP condition).

**Verify**: `npm run typecheck` → exit 0 (baseline green before any change).

---

### Step 2: Restructure the sidebar into two groups

**Target IA** (what the nav must look like after this step):

```
── [brand mark] Neko · Local ──────────────

  Início
  ○ Dashboard          (LayoutDashboard)
  ○ Lançamentos        (Receipt)

  Análise
  ○ Totais             (Calculator)
  ○ Anual              (TrendingUp)
  ○ Horizonte          (CalendarRange)
  ○ Tags               (Tags)

  Sistema
  ○ Configurações e privacidade  (Settings)
  ○ Ajuda              (HelpCircle)   ← new; navigates to "methodology" screen
  ○ Mia                (Sparkles)     ← demoted here; stub honest label

──────────────────────────────────────────
```

Rationale:

- "Início" contains the items a user touches in every nightly session (Dashboard + Lançamentos).
  These are the only two items in the primary group.
- "Análise" holds the deeper views (Totais, Anual, Horizonte, Tags) used weekly or diagnostically.
- "Metodologia" is removed as a standalone nav item. Its content becomes reachable via an "Ajuda"
  entry at the bottom of the Sistema section (same nav slot, different label and icon). This
  preserves the `methodology` screen and its URL-equivalent while removing the misleading top-level
  billing of a static doc page.
- "Mia" moves to Sistema (below Ajuda) with honest billing. It no longer occupies an
  "Análise"-level position before the chat feature exists.

**Changes to make in `src/shell/AppShell.tsx`**:

1. Add `HelpCircle` to the lucide-react import (line 2).
2. Replace the flat `NAV_ITEMS` array (lines 44–53) with two typed arrays:

   ```ts
   const NAV_PRIMARY: { key: Screen; label: string; icon: typeof LayoutDashboard }[] = [
     { key: "dashboard", label: "Dashboard", icon: LayoutDashboard },
     { key: "transactions", label: "Lançamentos", icon: Receipt },
   ];

   const NAV_ANALYSIS: { key: Screen; label: string; icon: typeof LayoutDashboard }[] =
     [
       { key: "totais", label: "Totais", icon: Calculator },
       { key: "anuais", label: "Anual", icon: TrendingUp },
       { key: "horizonte", label: "Horizonte", icon: CalendarRange },
       { key: "tags", label: "Tags", icon: TagsIcon },
     ];
   ```

3. Replace the nav render block (lines 101–126) with:

   ```tsx
   <nav className="ak-nav">
     <div className="ak-navh">Início</div>
     {NAV_PRIMARY.map((n) => (
       <button
         key={n.key}
         type="button"
         className={`ak-item ${active === n.key ? "ak-item--active" : ""}`}
         aria-current={active === n.key ? "page" : undefined}
         onClick={() => onNavigate(n.key)}
       >
         <n.icon size={18} strokeWidth={1.75} className="ak-item__ic" />
         <span>{n.label}</span>
       </button>
     ))}

     <div className="ak-navh">Análise</div>
     {NAV_ANALYSIS.map((n) => (
       <button
         key={n.key}
         type="button"
         className={`ak-item ${active === n.key ? "ak-item--active" : ""}`}
         aria-current={active === n.key ? "page" : undefined}
         onClick={() => onNavigate(n.key)}
       >
         <n.icon size={18} strokeWidth={1.75} className="ak-item__ic" />
         <span>{n.label}</span>
       </button>
     ))}

     <div className="ak-navh">Sistema</div>
     <button
       type="button"
       className={`ak-item ${active === "settings" ? "ak-item--active" : ""}`}
       aria-current={active === "settings" ? "page" : undefined}
       onClick={() => onNavigate("settings")}
     >
       <Settings size={18} strokeWidth={1.75} className="ak-item__ic" />
       <span>Configurações e privacidade</span>
     </button>
     <button
       type="button"
       className={`ak-item ${active === "methodology" ? "ak-item--active" : ""}`}
       aria-current={active === "methodology" ? "page" : undefined}
       onClick={() => onNavigate("methodology")}
     >
       <HelpCircle size={18} strokeWidth={1.75} className="ak-item__ic" />
       <span>Ajuda</span>
     </button>
     <button
       type="button"
       className={`ak-item ${active === "copilot" ? "ak-item--active" : ""}`}
       aria-current={active === "copilot" ? "page" : undefined}
       onClick={() => onNavigate("copilot")}
     >
       <Sparkles size={18} strokeWidth={1.75} className="ak-item__ic" />
       <span>Mia</span>
     </button>
   </nav>
   ```

4. The `BookOpen` import (line 4 of AppShell.tsx) becomes unused; remove it from the import list.
   Keep `Sparkles`, `Settings`, `Calculator`, `CalendarRange`, `Receipt`, `TagsIcon`, `TrendingUp`,
   `LayoutDashboard`, `Lock`, `Search`, `Table2`, `Unlink`. Add `HelpCircle`.

5. Update `SCREEN_META` (line 32) entry for `"methodology"` to match the new nav label:
   Change `crumb: "Como o Neko calcula"` → `crumb: "Como o Neko calcula"` (no change needed
   for the screen title/crumb; only the nav label changes from "Metodologia" to "Ajuda").

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0.

---

### Step 3: Update navigation tests for new grouping

Open `src/App.navigation.test.tsx`. The tests that will break after Step 2:

- Line 53: `screen.getByRole("button", { name: "Metodologia" })` — the sidebar button is now
  labelled "Ajuda". Update the selector to `{ name: "Ajuda" }`.
- Line 62: `screen.getByRole("button", { name: "Mia" })` — label unchanged; verify this test
  still passes as-is.
- Line 78: `screen.getByRole("button", { name: "Configurações e privacidade" })` — unchanged.

Add one new test after the existing ones to assert the new group headings exist:

```ts
it("sidebar has Início and Análise group headings", async () => {
  render(<App />);
  expect(screen.getByText("Início")).toBeInTheDocument();
  expect(screen.getByText("Análise")).toBeInTheDocument();
});
```

**Verify**: `npm run test:run` → all pass. The test that previously found "Metodologia" in the
nav now finds "Ajuda". The "Mia" nav test and "Configurações e privacidade" test still pass.

---

### Step 4: Remove the redundant 4-tile MetricBar from the dashboard

Open `src/screens/DashboardScreen.tsx`. Delete the entire `<div className="dash-grid4">` block
(lines 176–204 in the current file). This removes four `MetricTile` components:
"Saldo projetado", "Diário de hoje", "Crédito no mês", and "Reserva".

The `TrendingDown` import (line 2) and the `MetricTile` import (line 6) may become unused
after this removal. Check: `TrendingDown` is also used in the `reserveTrendIcon` expression
at lines 73–78 (via `summary?.reserve_trend`). Verify:

```ts
// line 71 — still used after removal of the grid?
const reserveTrendIcon =
  summary?.reserve_trend === "up" ? (
    <TrendingUp size={15} strokeWidth={1.75} />
  ) : summary?.reserve_trend === "down" ? (
    <TrendingDown size={15} strokeWidth={1.75} />
  ) : (
    <Minus size={15} strokeWidth={1.75} />
  );
```

`reserveTrendIcon` is only consumed inside the deleted `MetricTile` "Reserva". After deletion:

- Remove the `reserveTrendIcon` const (lines 71–78) since it is now unused.
- Remove `TrendingDown` from the lucide-react import line.
- `TrendingUp` is still used in the hero's `<dl>` — check; if not, remove it too.
  (At current code, `TrendingUp` is used only in the deleted `MetricTile`. Remove it.)
- `MetricTile` is used only in the `dash-grid4` block. Remove its import (line 6).

After removal:

- The hero section (lines 107–174 unchanged) already shows "Reserva" and "Lançamentos" in the
  `<dl>` inside `dash-hero__stats` (lines 124–135). No hero change needed.
- The `monthDailyAvgCents` local variable (lines 90–93) is passed to `DailyCheckinCard`. Verify
  it is still used; if yes, keep it.

**Verify**: `npm run typecheck` → exit 0. `npm run lint` → exit 0 (no unused imports).

---

### Step 5: Update dashboard test for removed MetricBar

Open `src/screens/DashboardScreen.test.tsx`. Search for any assertion that queries
`"Saldo projetado"`, `"Diário de hoje"`, `"Crédito no mês"`, `"Reserva"`, or
`"dash-grid4"` as text/selector directly tied to the four `MetricTile` items.

If any such assertion exists, remove it (those tiles no longer render). Do not remove assertions
about the hero or other cards.

**Verify**: `npm run test:run` → all pass.

---

### Step 6: Disambiguate the "Performance" label in MonthLedgerCard

The `<tfoot>` row labelled "Performance" in `MonthLedgerCard.tsx` (lines 195–215) computes
`income − (fixed_out + daily_out)` — which matches the spreadsheet's raw footer but differs
from the method's "Performance" concept defined in `MethodologyScreen.tsx` (which subtracts
Economia and projected remaining daily). The `title` attribute at line 196 already contains the
correct clarification but the visible label misleads.

Rename the visible `<th>` label from "Performance" to "Resultado do mês" and update the
subtitle span from "entradas − saída total (do mês)" to "entradas − saída total":

```tsx
// Replace lines 196–215 with:
<tr className="fc-foot">
  <th
    scope="row"
    title="Resultado contábil do mês na grade: entradas menos saída total (fixas + diário). Diferente da Performance do método (Totais), que também desconta Economia e a projeção do diário restante."
  >
    Resultado do mês
  </th>
  <td className="money" colSpan={3}>
    <span
      style={{
        color: "var(--text-faint)",
        fontSize: "var(--fs-micro)",
      }}
    >
      entradas − saída total
    </span>
  </td>
  <td className="money">
    <Money cents={foot.performance} size="sm" sign="auto" />
  </td>
</tr>
```

Note: the variable name `foot.performance` in the `footerOf` function (line 26) can remain
unchanged — it is a local computation variable, not user-visible text. Only the rendered label
changes.

**Verify**: `npm run typecheck` → exit 0. `npm run test:run` → all pass (no test asserts the
old "Performance" label in MonthLedgerCard specifically; if one does, update the assertion to
"Resultado do mês").

---

### Step 7: Collapse the duplicate search affordance in TransactionsScreen

The `TransactionsScreen` currently has its own `<label className="ak-search txs-tools__search">`
with an `<input aria-label="Filtrar por descrição">` (lines 261–269) that is wired to the same
`query` prop driving the global ⌘K header search. This creates two affordances for the same
filter and visual noise in the toolbar.

**What to do**: Remove the standalone search `<label>` element (lines 261–269) from
`TransactionsScreen.tsx`. The header ⌘K search already writes to the same `query` prop via
`App.tsx`'s `handleSearch` (line 43) and `setSearchQuery`. The `SegmentedControl` scope filter,
the count badge, and the "Novo lançamento" button remain.

After removal, the `Search` icon import (line 2, `import { Plus, Search, Tag as TagIcon }`)
becomes unused. Remove `Search` from that import.

The `onQueryChange` prop on `TransactionsScreen` still receives updates from the header search,
so clearing the query via the header still works. The `query` prop is still used by
`filterTransactions` (line 203). No functional change to filtering logic.

**Verify**:

1. `npm run typecheck` → exit 0.
2. `npm run lint` → exit 0.
3. `npm run test:run` → all pass. The test at `TransactionsScreen.test.tsx` line 50
   (`expect(screen.getByLabelText("Filtrar por descrição")).toBeInTheDocument()`) will now FAIL
   because the input was removed. Update that test assertion to instead verify the header search
   is the sole affordance:

   ```ts
   // In App.navigation.test.tsx line 50 (the "navigates to Lançamentos" test)
   // Replace: expect(screen.getByLabelText("Filtrar por descrição")).toBeInTheDocument();
   // With:    expect(screen.getByLabelText("Buscar lançamentos")).toBeInTheDocument();
   ```

   Check `TransactionsScreen.test.tsx` for any test that explicitly targets
   `"Filtrar por descrição"` as an `aria-label`. If found, update to match the header search
   label `"Buscar lançamentos"` or remove that specific assertion if it tested local-filter
   behavior (the filter logic itself is exercised separately via `filterTransactions` unit tests
   which do not depend on the input element).

**Verify**: `npm run test:run` → all pass.

---

### Step 8: Make the Transactions empty-state CTA navigate to Settings

When `transactions.length === 0`, the empty state at `TransactionsScreen.tsx` lines 293–303
displays the description "Importe sua planilha em Configurações para começar." but offers no
navigation action. The user must find Settings themselves.

`TransactionsScreen` currently receives `{ query, onQueryChange }` props. To navigate to
Settings it needs an `onGoToSettings` callback. Add the prop and wire it.

**Changes**:

1. Add `onGoToSettings: () => void` to the `TransactionsScreen` props interface (line 147):

   ```ts
   export function TransactionsScreen({
     query,
     onQueryChange,
     onGoToSettings,
   }: {
     query: string;
     onQueryChange: (query: string) => void;
     onGoToSettings: () => void;
   }) {
   ```

2. Update the `EmptyState` invocation (lines 293–303) to add an `action` only when there are
   no transactions at all (not when a filter has 0 results):

   ```tsx
   <EmptyState
     variant="empty"
     title="Nenhum lançamento encontrado"
     description={
       transactions.length === 0
         ? "Importe sua planilha em Configurações para começar."
         : "Nenhum resultado para o filtro atual."
     }
     action={
       transactions.length === 0 ? (
         <Button variant="secondary" size="sm" onClick={onGoToSettings}>
           Ir para Configurações
         </Button>
       ) : undefined
     }
   />
   ```

3. In `src/App.tsx`, update the `TransactionsScreen` usage (line 74) to pass the new prop:

   ```tsx
   {
     screen === "transactions" && (
       <TransactionsScreen
         query={searchQuery}
         onQueryChange={setSearchQuery}
         onGoToSettings={() => setScreen("settings")}
       />
     );
   }
   ```

**Verify**: `npm run typecheck` → exit 0.

---

### Step 9: Update tests for the new TransactionsScreen prop

Open `src/screens/TransactionsScreen.test.tsx`. All `render(<TransactionsScreen ...>)` calls
must now pass `onGoToSettings`. Add `onGoToSettings={vi.fn()}` to every render call in that
file (use find-and-replace within the file — do not change test logic).

Add one new test asserting the CTA appears and can be clicked:

```ts
it("shows a Settings CTA when there are no transactions", async () => {
  const onGoToSettings = vi.fn();
  mockCommands({ get_recent_transactions: [], list_tags: [] });
  render(
    <TransactionsScreen query="" onQueryChange={vi.fn()} onGoToSettings={onGoToSettings} />,
  );
  await waitFor(() => {
    expect(screen.getByText("Ir para Configurações")).toBeInTheDocument();
  });
  await userEvent.setup().click(screen.getByText("Ir para Configurações"));
  expect(onGoToSettings).toHaveBeenCalledOnce();
});
```

Model this test after the existing `"lists transactions and updates the shown count"` test
(line 57 in `TransactionsScreen.test.tsx`) for import/mock patterns.

Also check `src/App.navigation.test.tsx` for any `TransactionsScreen` render call that must
receive the new prop. The navigation tests render `<App />` (not `TransactionsScreen` directly),
so they are not affected — but confirm by reading the file.

**Verify**: `npm run test:run` → all pass, including the new CTA test.

---

### Step 10: Final gate — full check + e2e smoke

Run the full quality gate:

```
npm run check
```

If `npm run check` passes, run the Playwright visual smoke:

```
npm run e2e
```

After e2e, inspect `test-results/` screenshots (or the Playwright report via
`npm run e2e:report`) and confirm:

1. The sidebar shows three group headings: "Início", "Análise", "Sistema".
2. "Dashboard" and "Lançamentos" are under "Início".
3. "Metodologia" no longer appears as a nav label; "Ajuda" appears under "Sistema".
4. "Mia" appears under "Sistema" after "Ajuda".
5. The dashboard has no `dash-grid4` four-tile row between the hero and the deficit banner.
6. The "Dia a dia" card footer shows "Resultado do mês" (not "Performance").
7. The Transactions screen toolbar has no second search box (only the scope segmented control,
   count badge, and "Novo lançamento" button).
8. An empty Transactions state shows the "Ir para Configurações" button.

**Verify**: `npm run check` → exit 0. `npm run e2e` → all pass (or no regressions vs baseline).

---

### Step 11: Commit

Stage only the in-scope files modified during steps 2–9:

```
git add src/shell/AppShell.tsx \
        src/screens/DashboardScreen.tsx \
        src/screens/dashboard/MonthLedgerCard.tsx \
        src/screens/TransactionsScreen.tsx \
        src/screens/CopilotScreen.tsx \
        src/screens/MethodologyScreen.tsx \
        src/App.tsx \
        src/App.navigation.test.tsx \
        src/screens/DashboardScreen.test.tsx \
        src/screens/TransactionsScreen.test.tsx
```

Commit message:

```
feat(shell): restructure nav IA, dedupe dashboard, collapse search

- Sidebar: Início (Dashboard, Lançamentos) / Análise / Sistema groups
- Methodology promoted to "Ajuda" in Sistema; Mia demoted below it
- Remove redundant 4-tile MetricBar from dashboard (data already in hero)
- Rename MonthLedgerCard footer "Performance" → "Resultado do mês"
- Remove duplicate search input from TransactionsScreen toolbar
- Add "Ir para Configurações" CTA to empty Transactions state
```

**Verify**: `git status` → only in-scope files modified; working tree clean after commit.

---

## Test plan

### Tests to update (existing)

| File                                      | Test description                                                                                 | Change needed                                    |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------ |
| `src/App.navigation.test.tsx:53`          | clicks "Metodologia" nav button                                                                  | Change selector to `{ name: "Ajuda" }`           |
| `src/App.navigation.test.tsx:50`          | asserts `"Filtrar por descrição"` input                                                          | Change to `"Buscar lançamentos"` (header search) |
| `src/screens/TransactionsScreen.test.tsx` | all `render(<TransactionsScreen ...>)` calls                                                     | Add `onGoToSettings={vi.fn()}` prop              |
| `src/screens/DashboardScreen.test.tsx`    | any assertion on "Saldo projetado" / "Diário de hoje" / "Crédito no mês" / "Reserva" MetricTiles | Remove those assertions                          |

### New tests to write

| File                                      | Test                                                    | Happy path / regression it covers                       |
| ----------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------- |
| `src/App.navigation.test.tsx`             | `"sidebar has Início and Análise group headings"`       | Verifies new IA structure renders with correct headings |
| `src/screens/TransactionsScreen.test.tsx` | `"shows a Settings CTA when there are no transactions"` | Regression: empty-state CTA navigates; prop is called   |

### Structural pattern

Model new tests after `App.navigation.test.tsx` lines 38–50 (render `<App />`, find button by
role+name, assert screen content) and `TransactionsScreen.test.tsx` lines 57–71 (mockCommands,
render screen directly, use `waitFor`, assert visibility).

**Verify**: `npm run test:run` → all pass.

## Done criteria

All must hold before marking this plan DONE:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0 (no unused imports)
- [ ] `npm run test:run` exits 0; includes the two new tests (`"sidebar has Início and Análise group headings"` and `"shows a Settings CTA when there are no transactions"`)
- [ ] `npm run check` exits 0 (full gate: typecheck + lint + tests + privacy scan)
- [ ] `grep -n "dash-grid4" src/screens/DashboardScreen.tsx` → no matches
- [ ] `grep -n "\"Performance\"" src/screens/dashboard/MonthLedgerCard.tsx` → no matches (label is "Resultado do mês")
- [ ] `grep -n "\"Metodologia\"" src/shell/AppShell.tsx` → no matches in nav labels (the Screen key `"methodology"` may still appear in `SCREEN_META` and `Screen` type — only the nav button label must be gone)
- [ ] `grep -n "Filtrar por descrição" src/screens/TransactionsScreen.tsx` → no matches (input removed)
- [ ] No files outside the in-scope list are modified (`git diff --name-only HEAD~1` shows only the listed files)
- [ ] `plans/README.md` status row for plan 016 updated to DONE

## STOP conditions

Stop and report (do not improvise) if:

- The code at any cited line in "Current state" does not match the actual file (structural
  drift, not just blank-line shift) — the plan was written at commit `d183bbf` and may be stale.
- `MetricTile` is used anywhere other than the `dash-grid4` block in `DashboardScreen.tsx`;
  if it is, removing the block may break other screens (check before deleting the import).
- `onGoToSettings` prop injection in Step 8 requires touching a file outside the in-scope list
  (e.g. if `TransactionsScreen` is also rendered from a component not in `App.tsx`).
- A step's `npm run typecheck` or `npm run test:run` fails twice after a reasonable fix attempt.
- Removing the `Search` import from `TransactionsScreen.tsx` causes a lint error on a usage
  the plan did not account for (grep for `Search` in the file before deleting the import).
- The Playwright e2e run reveals a visual regression in a screen not touched by this plan
  (screenshot diff on a stable area); stop and report rather than suppressing the failure.
- The `EmptyState` component's `action` prop type does not accept `ReactNode` — if it only
  accepts `React.ReactElement`, adjust the value passed; do not change the `EmptyState`
  component itself (it is out of scope).

## Maintenance notes

- **Future Mia chat**: when the Mia chat feature lands, the "Em desenvolvimento" badge in
  `CopilotScreen.tsx` should be removed and Mia should move back up to the "Análise" group
  (or a dedicated group). The `"copilot"` Screen key and route are preserved by this plan.
- **"Ajuda" entry point to Methodology**: if the `MethodologyScreen` content is later expanded
  into a contextual help system (e.g. per-screen tooltips or a searchable knowledge base), the
  `"methodology"` route can be renamed or replaced without a nav-IA change — the "Ajuda" label
  already sets the right expectation.
- **Redundant `reserveTrendIcon` removal**: the variable was built for the MetricTile grid and
  holds non-trivial icon-selection logic; once removed, if the Reserva trend is later surfaced
  in the hero section, re-derive it from `summary.reserve_trend` at that callsite.
- **MonthLedgerCard `foot.performance` variable**: renamed label to "Resultado do mês" but the
  JS variable `foot.performance` in `footerOf()` is unchanged. A future plan that renames the
  method concept should also align the variable name to avoid confusion.
- **Reviewer focus in PR**: check that (1) the `SegmentedControl` filter in Transactions still
  works end-to-end (no JS error now that the inline search is gone), (2) the empty-state CTA
  actually navigates in the Tauri desktop build (test in `npm run tauri dev`), and (3) the
  Playwright screenshots confirm the nav groups visually.
