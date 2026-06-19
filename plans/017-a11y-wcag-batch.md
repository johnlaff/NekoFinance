# Plan 017: Accessibility WCAG batch (contrast + landmarks)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src/design-system/tokens/colors.css src/App.css src/shell/AppShell.tsx src/design-system/components/MetricTile.tsx src/screens/AnnualScreen.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: a11y
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

Eight WCAG 2.1 violations were identified across contrast (WCAG 1.4.11 for
non-text UI components, threshold ≥3:1) and landmark/ARIA structure (WCAG
1.3.1, 2.4.3). Users who rely on keyboard focus indicators, scrollbars as
spatial cues, or assistive technology (screen readers, magnification) are
currently impaired on the light theme and in both themes for structure.
Fixing them closes the gap to WCAG AA without any visual redesign — values
are all within the existing "Midnight Ledger" palette.

## Current state

### Files and their roles

- `src/design-system/tokens/colors.css` — CSS custom properties for both
  dark (`:root`) and light (`[data-theme="light"]`) themes. The focus-ring
  color lives here.
- `src/App.css` — global resets, scrollbar styling, and component styles
  including the `.gs-toggle--off` class.
- `src/shell/AppShell.tsx` — app chrome: sidebar `<nav>`, main content
  wrapper `<div class="ak-main">`, topbar `<header>`.
- `src/design-system/components/MetricTile.tsx` — `<article>` cards shown
  on the dashboard.
- `src/screens/AnnualScreen.tsx` — annual screen; contains
  `EconomizadoSparkline`, a `<div>` that bears `aria-label` without
  `role="img"`.

### Repo conventions that apply here

- **React Compiler is enabled** — do NOT add `memo`, `useMemo`, or
  `useCallback` manually; the compiler handles all memoization.
- **Design System tokens**: use tokens from `src/design-system/tokens/`
  for all colors. Raw hex values are acceptable inside CSS token
  definitions themselves (they are the token values) but not in component
  styles.
- **Functional-core style**: `AppShell.tsx` and `MetricTile.tsx` are
  pure-render shell components. Keep changes minimal — add one prop or one
  HTML attribute; do not restructure the component.
- **CONTEXT.md vocabulary** (relevant terms): no domain-logic impact here;
  this plan only touches HTML structure and CSS values.

### Finding 1 — Light-theme focus ring: 1.73:1, needs ≥3:1

`src/design-system/tokens/colors.css`, line 193 (light theme block):

```css
/* current */
--focus-ring: rgba(34, 135, 100, 0.45);
```

The shadow is `0 0 0 2px var(--bg), 0 0 0 4px var(--focus-ring)` (see
`src/design-system/tokens/elevation.css` line 16). The 4 px ring sits
directly over `--bg` (#eaeeec). When rgba(34,135,100,0.45) is composited
over #eaeeec the effective color is #90c0af, giving **1.73:1** against
#eaeeec (WCAG 1.4.11 minimum for non-text indicators: **≥3:1**).

Corrected value: `rgba(34, 135, 100, 0.85)` — effective blended color
#409678 — gives **3.07:1** against #eaeeec. The dark theme ring
(`rgba(63, 191, 143, 0.55)`, line 111 of the same file) already passes
and must NOT be changed.

### Finding 2 — Dark scrollbar thumb: 1.90:1, needs ≥3:1

`src/App.css`, lines 40–57 (scrollbar block):

```css
/* current */
scrollbar-color: var(--border-strong) transparent;
/* … */
background: var(--border-strong);
```

`--border-strong` resolves to `--ink-500` = `#3a4644` in dark mode. The
thumb appears over `transparent` track, so the actual background is the
scrollable container's fill — worst case is `--surface` (#161d1c). Ratio:
**1.90:1** vs `--bg` (#0e1413); **1.54:1** vs surface. Both fail 3:1.

Corrected value for dark scrollbar thumb: `#677870` (a bespoke muted
teal-grey that fits between `--ink-400` and `--ink-300` in the palette).
Ratios: **3.99:1** vs `--bg` (#0e1413), **3.67:1** vs `--surface`
(#161d1c). Both pass.

`--border-strong` is also used by borders/dividers throughout the app.
**Do not change `--border-strong`**. Override the scrollbar color
explicitly with a new local custom property `--scrollbar-thumb` in `:root`
and `[data-theme="light"]`, and reference it in the two scrollbar rules
(see Step 2).

### Finding 3 — Light scrollbar thumb: 1.42:1, needs ≥3:1

In `[data-theme="light"]`, `--border-strong` resolves to `#c2cbc7`.
Contrast vs `--bg` (#eaeeec): **1.42:1**; vs white surface: **1.65:1**.
Both fail.

Corrected value: `#6f7a74` — a muted ink-grey from the light palette.
Ratios: **3.81:1** vs #eaeeec, **4.46:1** vs white. Both pass. This
value goes into `[data-theme="light"] { --scrollbar-thumb: #6f7a74; }`.

### Finding 4 — Dark toggle off-state: 2.76:1, needs ≥3:1

`src/App.css`, lines 1491–1495:

```css
/* current */
.gs-toggle--off {
  background: var(--ink-400);
}
```

`--ink-400` = `#566461`. The toggle rests on `--surface` (#161d1c) in
practice. Ratio: **2.76:1** (fails 3:1 for a non-text UI component).

Corrected value: `--ink-300` = `#8e9692`. Ratios: **5.65:1** vs surface,
**6.14:1** vs `--bg`. Both comfortably pass.

The light-theme override (lines 1497–1499) uses `#828c87`. Against light
`--bg` (#eaeeec) this gives **2.97:1**, which barely fails. See Finding 5.

### Finding 5 — Light toggle off-state: 2.97:1, needs ≥3:1

`src/App.css`, lines 1497–1499:

```css
/* current */
[data-theme="light"] .gs-toggle--off {
  background: #828c87;
}
```

`#828c87` vs `--bg` #eaeeec = **2.97:1** (fails; threshold 3:1).

Corrected value: `#727c77`. Ratios: **3.69:1** vs #eaeeec, **4.32:1** vs
white surface. Both pass.

### Finding 6 — Phantom token `--lh-body` (undefined)

`src/App.css`, line 899:

```css
/* current */
line-height: var(--lh-body);
```

The token `--lh-body` is **never defined** in `src/design-system/tokens/typography.css`
or anywhere else. Defined line-height tokens are: `--lh-tight` (1.1),
`--lh-snug` (1.25), `--lh-normal` (1.45), `--lh-relaxed` (1.6). The
error message `.txs-inline-error` that uses it is body-scale text;
replace with `var(--lh-normal)` (1.45).

### Finding 7 — `<main>` landmark missing

`src/shell/AppShell.tsx`, line 158:

```tsx
/* current */
<div className="ak-main">
```

There is no `<main>` landmark in the document. The content area must be
identified as the main landmark for screen readers. Change this `<div>` to
a `<main>`.

The CSS class `ak-main` is defined in `src/App.css` (search for `.ak-main`
to find its rules). No CSS change is needed — the class name stays the
same.

### Finding 8 — Sidebar `<nav>` has no `aria-label`

`src/shell/AppShell.tsx`, line 101:

```tsx
/* current */
<nav className="ak-nav">
```

When a page has multiple navigation landmarks (or may gain them), each
`<nav>` must have an `aria-label` to distinguish it. Add
`aria-label="Navegação principal"`.

### Finding 9 — MetricTile `<article>` has no accessible name

`src/design-system/components/MetricTile.tsx`, line 45:

```tsx
/* current */
<article className={className} style={METRIC_TILE_STYLE}>
```

Each `<article>` is an independent piece of content. Without an accessible
name, screen readers announce "article" with no context. The tile's `label`
prop (type `string`, always present per the interface at line 17) is the
natural accessible name.

Add `aria-label={label}` to the `<article>` element at line 45.

### Finding 10 — `EconomizadoSparkline` div uses `aria-label` without `role="img"`

`src/screens/AnnualScreen.tsx`, line 73–74:

```tsx
/* current */
<div
  aria-label="Tendência de Economizado% por mês, com a faixa-meta de 20 a 30% sombreada"
```

`aria-label` on a plain `<div>` is only valid when the element has an
explicit ARIA role. Since this is a data visualization (not interactive),
add `role="img"` so assistive technology exposes the label.

## Commands you will need

| Purpose              | Command                                          | Expected on success           |
|----------------------|--------------------------------------------------|-------------------------------|
| Typecheck            | `npm run typecheck`                              | exit 0, no errors             |
| Lint                 | `npm run lint`                                   | exit 0                        |
| Unit tests           | `npm run test:run`                               | all pass                      |
| E2E smoke            | `npm run e2e`                                    | all tests pass                |
| Full gate            | `npm run check`                                  | exit 0                        |
| Verify token absent  | `grep -n "lh-body" src/App.css`                  | no matches                    |
| Verify main present  | `grep -n "<main" src/shell/AppShell.tsx`         | 1 match                       |
| Verify nav label     | `grep -n 'aria-label="Navegação' src/shell/AppShell.tsx` | 1 match              |
| Verify article label | `grep -n 'aria-label={label}' src/design-system/components/MetricTile.tsx` | 1 match |
| Verify role img      | `grep -n 'role="img"' src/screens/AnnualScreen.tsx` | 1 match                  |

## Scope

**In scope** (only these files may be modified):

- `src/design-system/tokens/colors.css` — fix focus-ring value in the
  light theme block only (line 193 area)
- `src/App.css` — add `--scrollbar-thumb` token + use it; fix
  `.gs-toggle--off` and its light override; replace `--lh-body`
- `src/shell/AppShell.tsx` — `<div>` → `<main>`, add `aria-label` to
  `<nav>`
- `src/design-system/components/MetricTile.tsx` — add `aria-label={label}`
  to `<article>`
- `src/screens/AnnualScreen.tsx` — add `role="img"` to sparkline `<div>`
- `tests/e2e/app-shell.spec.ts` — add one landmark-presence assertion
  (see Test plan)

**Out of scope** (do NOT touch):

- `src/design-system/tokens/elevation.css` — the `--shadow-focus` formula
  is correct; only the color token changes.
- `src/design-system/tokens/typography.css` — do NOT add `--lh-body`; the
  fix is to replace the usage in App.css with an existing token.
- `--border-strong` token values — changing them would affect dividers and
  borders app-wide; the scrollbar thumb is overridden via a new dedicated
  token instead.
- Dark-theme `--focus-ring` (`:root` block, line 111) — already passes
  (rgba(63,191,143,0.55) gives ≥3:1 in dark); do not change it.
- Any Rust or migration files.

## Git workflow

- Branch: `advisor/017-a11y-wcag-batch`
- Commit style: conventional commits, matching the repo (e.g.
  `fix: light focus-ring + scrollbar contrast + ARIA landmarks (plan 017)`).
  One commit per step, or one commit for all CSS changes + one for TSX
  changes + one for tests — whichever is cleaner.
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Fix light-theme focus ring contrast

Open `src/design-system/tokens/colors.css`.

Find the `[data-theme="light"]` block (starts at line 135). Locate the
`--focus-ring` assignment (line 193 area). Change the alpha from 0.45 to
0.85:

```css
/* before */
--focus-ring: rgba(34, 135, 100, 0.45);

/* after */
--focus-ring: rgba(34, 135, 100, 0.85);
```

Do NOT touch line 111 (the `:root` dark focus-ring).

**Verify**: `grep -n "focus-ring" src/design-system/tokens/colors.css`

Expected output (two lines, the dark value unchanged):
```
111:  --focus-ring: rgba(63, 191, 143, 0.55);
193:  --focus-ring: rgba(34, 135, 100, 0.85);
```

### Step 2: Add `--scrollbar-thumb` token and wire scrollbar rules

Open `src/design-system/tokens/colors.css`.

In the `:root` block (dark theme), add a new token near the `--focus-ring`
declaration (around line 111):

```css
  --scrollbar-thumb: #677870; /* WCAG 1.4.11: >=3:1 vs --surface (#161d1c) and --bg (#0e1413) */
```

In the `[data-theme="light"]` block, add a corresponding override near the
light `--focus-ring` (around line 193):

```css
  --scrollbar-thumb: #6f7a74; /* WCAG 1.4.11: >=3:1 vs --bg (#eaeeec) and white surface */
```

Open `src/App.css`. Find the scrollbar block (lines 40–60 area). Replace
both occurrences of `var(--border-strong)` in the scrollbar rules with
`var(--scrollbar-thumb)`:

```css
/* before */
scrollbar-color: var(--border-strong) transparent;
/* … */
background: var(--border-strong);

/* after */
scrollbar-color: var(--scrollbar-thumb) transparent;
/* … */
background: var(--scrollbar-thumb);
```

There are exactly 2 occurrences: one in the `scrollbar-color` shorthand
(line 43) and one in the `::-webkit-scrollbar-thumb { background: … }`
rule (line 53). Change both; do not change the `::-webkit-scrollbar-thumb:hover`
rule at line 59 (it uses `var(--text-faint)` which is intentional for the
hover state).

**Verify**: `grep -n "scrollbar" src/App.css | head -10`

Expected: both `scrollbar-color` and `background:` in the thumb rule now
reference `var(--scrollbar-thumb)`.

```
grep -n "border-strong" src/App.css | head -5
```

Expected: zero matches for `--border-strong` in the scrollbar block
(other uses of `--border-strong` elsewhere in App.css for non-scrollbar
purposes are fine).

### Step 3: Fix toggle off-state contrast in both themes

Open `src/App.css`.

Find `.gs-toggle--off` (line 1493 area). Change from `--ink-400` to
`--ink-300`:

```css
/* before */
.gs-toggle--off {
  background: var(--ink-400);
}

/* after */
.gs-toggle--off {
  background: var(--ink-300); /* WCAG 1.4.11: >=3:1 vs --surface (#161d1c) */
}
```

Find the light override (line 1497 area). Update the hex value:

```css
/* before */
[data-theme="light"] .gs-toggle--off {
  background: #828c87;
}

/* after */
[data-theme="light"] .gs-toggle--off {
  background: #727c77; /* WCAG 1.4.11: >=3:1 vs --bg (#eaeeec) and white surface */
}
```

**Verify**: `grep -A2 "gs-toggle--off" src/App.css`

Expected (two rule blocks):
```
.gs-toggle--off {
  background: var(--ink-300);
…
[data-theme="light"] .gs-toggle--off {
  background: #727c77;
```

### Step 4: Replace phantom `--lh-body` token

Open `src/App.css`. Find line 899 (the `.txs-inline-error` block). Replace
the undefined token:

```css
/* before */
  line-height: var(--lh-body);

/* after */
  line-height: var(--lh-normal);
```

**Verify**: `grep -n "lh-body" src/App.css`

Expected: no output (zero matches).

**Verify**: `grep -n "lh-normal" src/design-system/tokens/typography.css`

Expected: at least one match confirming the token exists (line 53: `--lh-normal: 1.45`).

### Step 5: Add `<main>` landmark and label the sidebar `<nav>`

Open `src/shell/AppShell.tsx`.

**5a.** At line 158, change the `<div className="ak-main">` opening tag to
`<main className="ak-main">` and correspondingly change the closing `</div>`
(line 189 area) to `</main>`:

```tsx
/* before */
      <div className="ak-main">
        …
      </div>

/* after */
      <main className="ak-main">
        …
      </main>
```

**5b.** At line 101, add `aria-label` to the `<nav>`:

```tsx
/* before */
        <nav className="ak-nav">

/* after */
        <nav className="ak-nav" aria-label="Navegação principal">
```

No other changes to this file.

**Verify**: `npm run typecheck` → exit 0

**Verify**:
```
grep -n "<main\|</main" src/shell/AppShell.tsx
```
Expected: 2 matches (opening and closing).

```
grep -n 'aria-label="Navegação principal"' src/shell/AppShell.tsx
```
Expected: 1 match.

### Step 6: Add accessible name to MetricTile `<article>`

Open `src/design-system/components/MetricTile.tsx`.

At line 45, add `aria-label={label}` to the `<article>` element:

```tsx
/* before */
    <article className={className} style={METRIC_TILE_STYLE}>

/* after */
    <article className={className} style={METRIC_TILE_STYLE} aria-label={label}>
```

`label` is always a non-empty string (required prop, see interface at
line 17). No other changes.

**Verify**: `grep -n 'aria-label={label}' src/design-system/components/MetricTile.tsx`

Expected: 1 match.

**Verify**: `npm run typecheck` → exit 0

### Step 7: Add `role="img"` to the EconomizadoSparkline container

Open `src/screens/AnnualScreen.tsx`.

Find the `<div` at line 73 that already carries `aria-label="Tendência…"`.
Add `role="img"` to the same element:

```tsx
/* before */
      <div
        aria-label="Tendência de Economizado% por mês, com a faixa-meta de 20 a 30% sombreada"

/* after */
      <div
        role="img"
        aria-label="Tendência de Economizado% por mês, com a faixa-meta de 20 a 30% sombreada"
```

No other changes to this file.

**Verify**: `grep -n 'role="img"' src/screens/AnnualScreen.tsx`

Expected: 1 match.

**Verify**: `npm run typecheck` → exit 0

### Step 8: Add landmark-presence assertions to the e2e suite

Open `tests/e2e/app-shell.spec.ts`.

Add a new test case inside the first `test.describe` block (after the
last existing test, before the closing `}`):

```ts
  test("page has main landmark and labelled navigation", async ({ page }) => {
    await expect(page.locator("main.ak-main")).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "Navegação principal" }),
    ).toBeVisible();
    await expect(
      page.getByRole("article", { name: "Saldo projetado" }),
    ).toBeVisible();
  });
```

The `"Saldo projetado"` article name comes from the MetricTile `label`
prop used on the dashboard hero card (confirm the exact label string by
searching: `grep -rn "Saldo projetado" src/` — it must match). If the
exact label differs in the mock data, adjust accordingly — but do not
change the mock data; read it first.

**Verify**: `npm run e2e` → all tests pass, including the new one.

### Step 9: Run the full quality gate

```sh
npm run check
```

Expected: exit 0 with no TypeScript errors, no lint errors, all unit
tests passing, and the privacy scan clean.

If `npm run e2e` is not included in `npm run check`, run it separately:

```sh
npm run e2e
```

Expected: all tests pass.

## Test plan

The new e2e test (Step 8) covers:

- **`<main>` landmark present**: `page.locator("main.ak-main")` is visible.
- **`<nav>` is labelled**: `getByRole("navigation", { name: "Navegação principal" })` resolves.
- **MetricTile has accessible name**: `getByRole("article", { name: "Saldo projetado" })` resolves.

Pattern to follow: existing tests in `tests/e2e/app-shell.spec.ts` — all
use `page.getByRole(…)` assertions with `await expect(…).toBeVisible()`.

There are no unit tests to write for CSS changes — contrast ratios are
verified by the computed values in this plan and visually confirmed via the
existing Playwright screenshots (compare `dashboard.png` before and after
to confirm the toggle and focus ring still look intentional).

## Done criteria

All of the following must hold simultaneously:

- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0, all existing tests pass
- [ ] `npm run e2e` exits 0, including the new landmark test from Step 8
- [ ] `grep -n "lh-body" src/App.css` → no output
- [ ] `grep -n "focus-ring" src/design-system/tokens/colors.css` → line
  111 = `rgba(63, 191, 143, 0.55)` (dark, unchanged); line 193 = `rgba(34, 135, 100, 0.85)` (light, updated)
- [ ] `grep -n "scrollbar-thumb" src/design-system/tokens/colors.css` → 2
  matches (one in `:root`, one in `[data-theme="light"]`)
- [ ] `grep -n "gs-toggle--off" src/App.css` → `.gs-toggle--off` uses
  `var(--ink-300)` and `[data-theme="light"]` override uses `#727c77`
- [ ] `grep -n "<main" src/shell/AppShell.tsx` → 1 match
- [ ] `grep -n 'aria-label="Navegação principal"' src/shell/AppShell.tsx` → 1 match
- [ ] `grep -n 'aria-label={label}' src/design-system/components/MetricTile.tsx` → 1 match
- [ ] `grep -n 'role="img"' src/screens/AnnualScreen.tsx` → 1 match
- [ ] `git diff --stat HEAD -- src/design-system/tokens/elevation.css src/design-system/tokens/typography.css` → no changes
- [ ] `plans/README.md` status row for plan 017 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- Any "Current state" code excerpt does not match the live file at the
  indicated location — the file has drifted since this plan was written.
- The `<main>` change causes the CSS layout to break visually (`.ak-main`
  styles may be tied to element type in some browser-specific resets). If
  so, report and do not proceed until the layout regression is understood.
- `npm run e2e` fails on existing tests after Step 8 (the new test should
  be additive and not affect the others; if existing tests break, something
  in Steps 5–7 is wrong).
- `grep -n "border-strong" src/App.css` shows `var(--border-strong)` still
  inside the scrollbar block after Step 2 — verify you changed both
  occurrences.
- Any step's verification command fails after one careful fix attempt.
- You discover the `label` prop on `MetricTile` is empty or undefined for
  any usage site (in which case `aria-label` would be set to the empty
  string, which is worse than nothing — investigate and report instead of
  applying the change blind).

## Maintenance notes

- **Scrollbar token**: `--scrollbar-thumb` is a new dedicated token. If the
  color palette is ever updated, both the dark and light values must be
  rechecked for ≥3:1 contrast against their respective surface backgrounds.
  Add a comment cross-referencing WCAG 1.4.11 to make future auditors
  aware.
- **Focus ring opacity**: the light-theme focus ring at 0.85 opacity gives
  3.07:1 — only a 2.3% margin over the 3:1 threshold. If `--bg` ever
  shifts lighter (higher luminance), this margin will shrink. Treat this
  value as contrast-sensitive and recheck after any light-theme background
  change.
- **MetricTile `aria-label`**: the label is set to the same string as the
  visible `<p>` text inside the article. This is redundant for sighted
  users but standard practice for `<article>` landmarks. If a tooltip or
  richer accessible description is added later, switch to `aria-labelledby`
  pointing to the `<p>` element's id instead.
- **`<main>` + CSS**: Tauri WebView renders on Chromium/WebKit, where
  `<main>` is a block element and is visually identical to `<div>`. If
  future browser support requires `display: block` explicitly, add it to
  `.ak-main` in `src/App.css`.
- **Deferred**: this plan does not add an axe-core automated contrast scan
  to CI (would require `@axe-core/playwright`). That is a follow-up for a
  future CI plan once the manual fixes in this plan are green.
