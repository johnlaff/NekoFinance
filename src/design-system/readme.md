# Neko Finance — Design System

A design system for **Neko Finance**: a local-first, private personal-finance desktop app (Tauri 2 + React 19 + TypeScript, SQLite + Google Sheets import) with an AI copilot named **Mia** who reads your sheets, explains your finances, separates ownership/responsibility, and proposes _human-approved_ sheet edits only.

The system is **"Midnight Purr"** — the systematization of the "Conversa com a Mia" direction. Fonts (Geist, Geist Mono, Newsreader) are self-hosted; lucide-react is the production icon set (`strokeWidth={1.75}`).

> **Primary theme:** Dark is the default and presented-first theme (zinc, zero-chroma). A light theme ships as `[data-theme="light"]` with full parity — status, diff, and badge text use dedicated dark on-tint colors in light mode (not just inverted tokens). The shell exposes a light/dark toggle persisted to `localStorage` (`neko-theme`) and a 6-palette brand-accent selector persisted as `neko-accent` (`data-accent` on `<html>`; jade default = no attribute).

---

## 1 · Brand principles & adjectives

**Adjectives:** calm, precise, tactile, premium, private, grounded.

1. **Truth over flourish.** Every number is sourced and reconcilable. Decoration never competes with data.
2. **Calm under load.** Dense financial data is presented quietly — generous hierarchy, restrained color, no visual noise.
3. **Private by default.** Local-first. Human-approved writes and connection state are first-class UI, never buried.
4. **Quiet warmth.** Mia's feline calm shows in a few restrained cues — never childish, never a mascot.
5. **Ownership is explicit.** Personal / partner / shared, payer vs. beneficiary vs. responsible owner — always legible.

### The visual identity

**Midnight Purr** is a zinc-neutral night cockpit with one configurable brand accent:

- **Neutrals (zinc, zero-chroma):** `#09090B` bg · `#18181B` surface · `#27272A` border/elevated · `#FAFAFA` ink (dark); `#FAFAFA` bg · `#FFFFFF` surface · `#18181B` ink (light). No warm tint, no green cast — the color belongs to the data and the accent.
- **Accent = brand, one at a time:** the pair `--accent`/`--accent-ink` is atomic and user-selectable — jade (default) `#3FBF8F`, lima `#95FF48`, violeta `#C084FC`, âmbar `#FBBF24`, céu `#38BDF8`, rosa `#FB7185` — with darker AA-as-text values in light mode. The accent paints chrome, CTAs and selection. Nothing else.
- **Status = method, fixed:** paz/entrada green, atenção orange, dinheiro pos/neg, info sky are fixed per theme and NEVER follow the accent. A user switching palettes must never see "paz" change color.
- **Type:** Geist everywhere, including money (bold + `tabular-nums`); Geist Mono for parcels ("2/10"), citations and code; Newsreader strictly editorial.
- **Geometry:** pill-dominant — radii 6/10/14/18/22, pills for chips and the active mobile tab.

---

## 2 · Sources & provenance

- **Implemented in the live app.** This system shipped into the Tauri + React 19 codebase; the production tokens/components live under `src/design-system/` and the screens under `src/screens/`. Reconcile changes against the running app, not a standalone proposal.
- Product surface (9 telas): Hoje, Lançamentos, Este mês, O ano, Calendário, Horizonte, Tags, Mia, Configurações.
- All logos and the Mia avatar are original SVGs authored here (`assets/`).

---

## 3 · Files & index (root manifest)

```
styles.css                ← consumer entry point (@import only)
tokens/
  fonts.css               self-hosted @font-face (variable TTFs in assets/fonts)
  colors.css              base + semantic color tokens, dark :root + light scope
  typography.css          families, scale, weights, line-heights, helper classes
  spacing.css             4px spacing scale, radii, layout constants, density
  elevation.css           shadow + inset + focus tokens
  motion.css              durations, easings, reduced-motion
assets/
  neko-mark.svg           brand mark (friendly geometric cat head)
  mia-avatar.svg          copilot avatar
guidelines/*.card.html    foundation specimen cards (Design System tab)
components/core/          Badge, Button, Input, SegmentedControl, Switch, Disclosure,
                          InfoPopover, MonthNav, NekoMark
components/finance/       ApprovalDiffCard, BalanceTrajectory, HealthBadge, LineItemEditor,
                          MetricTile, Money, MovBadge, OwnerChip, PhaseBadge, ProvBadge,
                          TransactionRow
components/copilot/       ChatBubble, Citation, EmptyState, MiaAvatar
ui_kits/dashboard/        Dashboard ("Quanto posso gastar hoje") + AppShell + shared icons
ui_kits/transactions/     Lançamentos — historical ledger
ui_kits/totais/           Totais — month calculations (performance, custo de vida, economizado)
ui_kits/anuais/           Anual — yearly view, all months at a glance
ui_kits/ano-inteiro/      Ano inteiro — month-by-month daily grid with balance heatmap
ui_kits/economia-compare/ Economia comparada — two years side by side
ui_kits/horizonte/        Horizonte de saldos — month-by-month balance projection
ui_kits/tags/             Tags — monthly tag totals with include/ignore
ui_kits/settings/         Configurações e privacidade
ui_kits/methodology/      Ajuda — method-principle help cards
ui_kits/copilot/          Mia — copilot stub ("Em desenvolvimento") + known method facts
SKILL.md                  Agent Skill manifest
```

> The `_ds_bundle.js`, `_ds_manifest.json`, `_adherence.oxlintrc.json` files are generated by the compiler — never edit by hand.

---

## 4 · Content fundamentals (voice & copy)

Neko's voice is **calm, plain, and exact** — a competent partner, not a cheerleader or a mascot.

- **Person:** Address the user as **you**; the product/Mia speaks as **I** sparingly ("I found 3 charges…", "I'd make this change"). Never royal "we" for the app.
- **Casing:** Sentence case everywhere — buttons, titles, menus ("Approve & write", not "Approve & Write"). UPPERCASE only for tiny eyebrow/section labels with letter-spacing.
- **Numbers are sacred:** Always show currency symbol + tabular figures + 2 decimals for money ("R$ 642,18"). Percentages get one place when meaningful ("6,1%"). Never approximate a figure Mia could compute exactly.
- **Tone:** Declarative and reassuring. Lead with the fact, then the nuance. e.g. _"Você está R$ 1.678 à frente neste mês. Os gastos estão 6% abaixo da sua média."_
- **Trust language:** Be explicit about privacy and control — "runs locally", "read-only", "needs your approval", "nothing leaves your machine". These phrases are features, not fine print.
- **Mia's warmth:** A light, dry calm — _"Want me to set up a rule for that?"_ Never cat puns, never emoji, never exclamation-spam. The feline character is in restraint, not decoration.
- **Errors:** Name what happened + the fix, no blame — _"Couldn't reach that sheet. Check it's shared with your connected account."_
- **Empty states:** One sentence of what + one of how — _"Nenhum lançamento ainda. Conecte uma planilha do Google e a Mia importa sua atividade."_
- **Emoji:** None. **Cat puns:** None.

**Examples** (copy de produção é PT-BR; tags são para CLASSIFICAR, não "categorizar" — categorias foram rebaixadas a tags na spec 015)

| Don't                          | Do                                                     |
| ------------------------------ | ------------------------------------------------------ |
| "Opa! Algo deu errado 😿"      | "Não consegui acessar essa planilha."                  |
| "Seus Gastos Estão Ótimos!"    | "Os gastos estão 6% abaixo da sua média."              |
| "Categorizamos tudo para você" | "Classifiquei 3 gastos com tags — aprove para salvar." |

---

## 5 · Visual foundations

**Overall feel:** a private financial cockpit — dark, quiet, tactile, dense but never cramped.

- **Color usage:** Zinc ink ground (`--bg #09090B`), one brand accent for action/selection (user-picked palette, jade default), fixed method-status colors for meaning. Color is _always_ paired with a label or icon — never the only signal.
- **Backgrounds:** Flat solids. No photographic imagery, no decorative gradients. The only gradients are functional: a faint jade area-fill under the cashflow line, and an 80%-opacity blurred topbar (`backdrop-filter: blur(8px)`) for a subtle glass edge while scrolling.
- **Cards:** `--surface #18181B`, 1px `--border`, `--radius-md (14px)`, `--shadow-1` (a low 1px ambient). Quiet, not floaty. Inside a card, separation is `--lift`, not shadow. Elevated/overlay surfaces step up to `--surface-elevated` + larger shadows. No colored left-border accent cards.
- **Corners:** 6 (chips) → 10 (inputs/buttons/rows) → 14 (cards) → 18 (panels/dialogs) → 22 (sheets/large surfaces). Pills for owner chips, switches and the active mobile tab. Fully rounded only for avatars, toggles and pills.
- **Borders & dividers:** 1px "whisker" hairlines (`--border`) separate rows and sections. Dashed hairlines inside diff cards. Active nav/selection is an accent-tint pill (`--surface-selected`), not a side rail.
- **Typography motifs:** Geist for everything UI **and money** (`--font-money` = Geist + `tabular-nums`; a money value never wraps and never animates); Geist Mono for parcels, code, citations and sheet references; serif (Newsreader) reserved strictly for the methodology/insights editorial voice.
- **Shadows:** Cool and low-spread (dark theme leans on borders, not glow). Light theme uses soft ambient shadows + a white inset top-highlight.
- **Motion:** Composed, never bouncy. 130ms hovers, 200ms most transitions, 320–480ms for panels/approvals. Easing `cubic-bezier(0.2,0,0,1)`. The only ambient motion is Mia's slow reading-dot blink and the three-dot "reading" indicator — both gated on `prefers-reduced-motion`.
- **Hover / press:** Hover lightens surface (`--surface-hover`) and lifts text from muted→full; primary buttons go a step lighter. Press nudges `translateY(0.5px) scale(0.992)` — a tiny tactile dip, no color flip beyond `--primary-press`.
- **Transparency / blur:** Used sparingly — the glass topbar and tint washes (`--*-tint`, ~12–15% alpha) behind badges/health states. Never frosted everything.
- **Imagery vibe:** There is essentially no imagery; the brand is typographic + data-viz. Where an avatar is needed (Mia), it's a flat geometric jade mark, cool-toned to match the ink ground.
- **Density:** Comfortable-dense. 44px default table rows (36px compact), 14–16px gutters inside cards, 22px screen padding, 8px/12px control gaps. Whitespace does the separating; rules are thin.

---

## 6 · Iconography

- **System:** A **Lucide-style** line set (ISC license), normalized to a **1.75px stroke**, round caps/joins, 24×24 grid. This stroke weight (slightly under Lucide's default 2) reads more refined at small sizes for a finance tool. Shipped as `ui_kits/shared/icons.jsx` → `window.Icon` (`<Icon name="wallet" size={18} />`), drawing `currentColor` so icons inherit theme + state color.
  > **Substitution flag (ui_kit prototype only):** these are hand-normalized Lucide-equivalent paths. The production app already uses `lucide-react` with `strokeWidth={1.75}`; the names used here (dashboard, receipt, sparkles, wallet, creditCard, table, lock, shield, etc.) map 1:1 to Lucide.
- **Sizes:** 15px inside dense labels/metric tiles, 16–18px in nav and buttons, 20–22px for state/empty icons. Never below 14px.
- **Color:** Icons are `--text-muted`/`--text-faint` at rest, inherit accent color in active/semantic contexts (jade nav, status tints). Avatars and the Neko mark use `--primary`.
- **The Neko mark:** a friendly geometric cat head with rounded ears, round eyes and a small nose, drawn as a single `currentColor` silhouette with negative-space features (`assets/neko-mark.svg`). It is the _only_ place the literal cat geometry appears; never decorate UI with ears/whiskers literally — the "whisker" cue is expressed abstractly as 1px hairlines.
- **Mia avatar:** `assets/mia-avatar.svg` — the Neko cat in a calm dark chip (jade head, negative-space eyes); also embedded as a data-URI inside `ChatBubble`.
- **Emoji / unicode as icons:** Not used in product UI. A few unicode glyphs (→ ↳ ▲▼) appear in data contexts (diffs, splits, deltas) where they read as typographic marks, not icons.

---

## 7 · Type system (sizes, weights, usage)

| Token                    | px          | Weight  | Line-height | Use                                            |
| ------------------------ | ----------- | ------- | ----------- | ---------------------------------------------- |
| `--fs-display`           | 40          | 700     | 1.1         | hero numbers, landing titles                   |
| `--fs-h1`                | 28          | 700     | 1.25        | screen titles                                  |
| `--fs-h2`                | 22          | 600     | 1.25        | section heads                                  |
| `--fs-h3`                | 18          | 600     | 1.25        | sub-section                                    |
| `--fs-title`             | 16          | 600/700 | 1.25        | card titles                                    |
| `--fs-body`              | 14          | 400     | 1.45        | default UI/body                                |
| `--fs-sm`                | 13          | 400     | 1.45        | dense tables, secondary                        |
| `--fs-label`             | 12          | 500     | 1.2         | field labels                                   |
| `--fs-micro`             | 11          | 400/600 | 1.2         | annotations, axis ticks, eyebrows              |
| `--fs-money-xl/lg/md/sm` | 34/22/15/13 | 600/500 | 1.05        | money — always `--font-money` + `tabular-nums` |

Rules: money & all figures → **Geist, `tabular-nums`, right-aligned in columns**; headings → sans, tight tracking; methodology long-form → serif. Micro-uppercase eyebrows are not shell vocabulary — prefer plain sentences for labels.

---

## 8 · Chart language

- **Series colors:** `--chart-1..6` (fixed data palette — emerald, amber, sky, violet, rose, teal), assigned in that order; categories keep a stable color across the app and do NOT follow the brand accent. Money sign uses `--money-pos/--money-neg`, never a chart color.
- **Line/area:** 2.5px line, round joins; area fill is a single-hue vertical gradient from 22%→0% alpha. Data points are 3px hollow dots (bg-filled, colored stroke).
- **Bars:** rounded-top (3px radius), 14px wide, 55% opacity when paired behind a line (spending behind income).
- **Donut:** 16px ring, no gaps between segments, center holds the total in mono. Legend lists name · amount · %.
- **Gridlines:** horizontal only, `--chart-grid` (~16% alpha). No vertical grid, no chart borders, no 3D, no drop shadows on data.
- **Axis/labels:** mono `--fs-micro` in `--chart-axis`. X labels under the plot; Y implied by gridlines + tile values rather than a heavy axis.
- **Thresholds & annotations:** budget/target shown as a 1px dashed `--warning` line with a small inline label; anomalies get a single colored dot + tooltip, never a cluttered callout. Annotations appear on hover/focus, not permanently.

---

## 9 · Data-density rules

- **Dashboards:** 4-up KPI grid (collapses to 2-up < 1080px), 12–16px card padding, charts ≤ 200px tall. One hero (health) + one primary chart row + supporting cards.
- **Tables:** default row 44px (compact 36px), 14px horizontal padding, 14px inter-column gap. Right-align amounts; left-align text; owner/status columns hug the amount. Sticky header in `--bg-subtle`. Selected row = jade rail + `--surface-selected`; attention row = warning rail.
- **Master/detail:** table (fluid) + 384px detail panel; stacks under 1180px. Detail panel is the only place per-row editing happens.
- **Truncate, don't wrap** merchant/label cells (ellipsis); never truncate a money value.

---

## 10 · Accessibility constraints

- **Contrast:** body text `--text` on `--bg`/`--surface` ≥ 7:1; muted text ≥ 4.5:1; primary button text `--text-on-primary` on jade ≥ 4.5:1. Status text uses the lighter `*-400` on dark tints to hold ≥ 4.5:1.
- **Status ≠ color alone:** every status/owner/confidence pairs color with a word, icon, or shape (badge label, confidence bar count, owner initials/split avatar).
- **Focus:** visible ring on all interactive elements — `0 0 0 2px var(--bg), 0 0 0 4px var(--focus-ring)` (2px offset + jade halo). Never remove outlines without a replacement.
- **Keyboard:** full tab order; table rows are buttons (Enter/Space select); segmented controls are `role=tablist`; dialogs trap focus and restore on close.
- **Hit targets:** ≥ 36px desktop dense, ≥ 44px touch/mobile.
- **Reduced motion:** `prefers-reduced-motion: reduce` zeroes durations and stops the pupil blink, reading dots, spinner spin, and skeleton shimmer.

---

## 11 · Component inventory

**Core** — `Badge` (tones, dot/square), `Button` (primary/secondary/ghost/danger × sm/md/lg), `Input` (label/affix/money/error), `SegmentedControl`, `Switch`, `Disclosure` (collapsible, card/warn), `InfoPopover` (method-glossary tooltips), `MonthNav` (prev/next + "Hoje"), `NekoMark` (brand mark).
**Finance** — `ApprovalDiffCard` (before→after, pending/approved/rejected), `BalanceTrajectory` (area chart, full/compact, zero-line band), `HealthBadge` (strong/steady/watch/risk + ring), `LineItemEditor` ("detalhar em partes"), `MetricTile` (KPI + delta + spark), `Money` (R$ tabular mono, sign colors), `MovBadge` (entrada/saída/diário/economia/cartão), `OwnerChip` (personal/partner/shared via `who`, `note` role), `PhaseBadge` (Mapear/Calibrar/Operar), `ProvBadge` (importado/manual/projetado), `TransactionRow` (`desc`/`provenance`/`future`/`lump`/`passthrough`).
**Copilot** — `ChatBubble` (mia/user, thinking), `Citation` (inline chip + deterministic CALC block), `EmptyState` (empty/loading/skeleton/error), `MiaAvatar`.

Each lives in `components/<group>/<Name>.{jsx,d.ts,prompt.md}` with a directory `*.card.html` specimen. Consume via `const { Name } = window.NekoFinanceDesignSystem_9bd1cd`.

---

## 12 · Implementation handoff & first-build checklist

**CSS custom properties** are the contract — link `styles.css` and use the semantic aliases (`--bg`, `--surface`, `--surface-elevated`, `--border`, `--text`, `--text-muted`, `--primary`, `--secondary`, `--success/warning/danger/info`, `--money-pos/neg`, `--owner-personal/partner/shared`, `--chart-1..6`, `--radius-*`, `--space-*`, `--shadow-*`, `--dur-*`, `--ease-*`). Theme by toggling `data-theme="light"` on `<html>`; brand accent by `data-accent="lima|violeta|ambar|ceu|rosa"` (absent = jade).

**Engineer, build in this order:**

1. **Tokens + theme switch** — ship `styles.css`, wire `data-theme`, confirm dark↔light swap.
2. **Fonts** — self-hosted via `@font-face` (variable TTFs in `assets/fonts`: Geist, Geist Mono, Newsreader). Optionally subset to WOFF2 for production.
3. **Core primitives** — `Button`, `Input`, `SegmentedControl`, `Badge`, `Switch` (+ focus rings, disabled, reduced-motion).
4. **Money & finance primitives** — `MetricTile`, `OwnerChip`, `TransactionRow`, `HealthBadge` with real tabular-nums formatting.
5. **AppShell** — per-viewport chrome: fixed sidebar with the primary CTA (desktop), icon rail (tablet 701–900px), blurred appbar + floating tab bar with embedded FAB (mobile ≤700px); connection/theme footer.
6. **Dashboard** — KPI grid + cashflow/donut charts (start with the SVG approach here, or a charting lib themed to `--chart-*`).
7. **Transactions/import** — master table + detail panel + Sheets column-mapping with confidence states.
8. **Copilot + ApprovalDiffCard** — chat, deterministic tool-result blocks, and the human-approval write gate (the diff must be the _only_ path to a sheet write).
9. **States** — wire `EmptyState` variants for every async surface.
10. **A11y pass** — contrast, keyboard, focus, reduced-motion, status-with-label audit.

> **Open items to confirm with the team:** final brand mark — the included friendly-cat mark is a proposal. Fonts are self-hosted (Geist, Geist Mono, Newsreader as variable TTFs); consider subsetting to WOFF2 for production weight.
