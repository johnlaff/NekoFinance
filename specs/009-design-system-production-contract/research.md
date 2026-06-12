# Research 009 - Design System Production Contract

## Sources Checked

### Accessibility

- WCAG 2.2: https://www.w3.org/TR/WCAG22/
- Error Prevention for Legal, Financial, Data: https://www.w3.org/WAI/WCAG22/Understanding/error-prevention-legal-financial-data.html
- Focus Not Obscured: https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html
- Target Size Minimum: https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
- Accessible Authentication: https://www.w3.org/WAI/WCAG22/Understanding/accessible-authentication-minimum.html
- WAI-ARIA Authoring Practices Guide: https://www.w3.org/WAI/ARIA/apg/

### Design Tokens And CSS

- W3C Design Tokens Community Group: https://www.w3.org/community/design-tokens/
- DTCG format module: https://design-tokens.github.io/community-group/format/
- MDN `@layer`: https://developer.mozilla.org/en-US/docs/Web/CSS/@layer
- MDN container queries: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_containment/Container_queries
- MDN `:has()`: https://developer.mozilla.org/en-US/docs/Web/CSS/:has
- MDN `oklch()`: https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/oklch
- MDN `light-dark()`: https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/light-dark
- Baseline: https://web.dev/baseline

### React And Component Primitives

- React 19: https://react.dev/blog/2024/12/05/react-19
- React Aria: https://react-spectrum.adobe.com/react-aria/
- Radix Primitives: https://www.radix-ui.com/primitives
- Ark UI: https://ark-ui.com/
- Open UI: https://open-ui.org/

### AI UX And Provenance

- Microsoft Guidelines for Human-AI Interaction: https://www.microsoft.com/en-us/research/project/guidelines-for-human-ai-interaction/
- Google PAIR Guidebook: https://pair.withgoogle.com/guidebook/
- NIST AI Risk Management Framework: https://www.nist.gov/itl/ai-risk-management-framework
- C2PA technical specifications: https://spec.c2pa.org/specifications/specifications/2.4/index.html

### Data Visualization

- Financial Times Visual Vocabulary: https://raw.githubusercontent.com/Financial-Times/chart-doctor/main/visual-vocabulary/README.md
- Observable Plot: https://observablehq.com/plot/
- Vega-Lite: https://vega.github.io/vega-lite/
- WAI complex images tutorial: https://www.w3.org/WAI/tutorials/images/complex/
- Chartability: https://chartability.fizz.studio/
- Data Viz Project: https://datavizproject.com/

### Local-First UX

- Local-first software, Ink & Switch: https://www.inkandswitch.com/essay/local-first/

## Decisions From Research

### WCAG 2.2 AA Is The Floor

Finance workflows need more than color contrast. WCAG 2.2 error-prevention criteria apply directly
to Google Sheets write-back and any data-changing approval surface. Neko should require review,
confirmation, validation, and rollback/recovery visibility before material writes.

### Tokens Need Semantic State Layers

The current token set covers brand, text, surfaces, money, owners, motion, spacing, and charts. The
next state layer should cover review/confidence, diff/write-back, rollback, privacy/local/cloud,
connection freshness, and table density. These states should be semantic tokens, not hard-coded
component colors.

### CSS Should Stay Platform-First

Use CSS custom properties as runtime token transport. Prefer cascade layers, container queries,
logical properties, `color-scheme`, `@media (prefers-reduced-motion)`, and accessible native HTML
before adding JS behavior. Because this is Tauri, browser support should be checked against the
target WebView, not only evergreen desktop browsers.

### Headless Primitives Are For Complex Widgets

Native HTML should remain first choice. React Aria/Radix/Ark-style primitives become useful for
combobox, select, popover, menu, dialog, and other complex behaviors where keyboard and screen-reader
details are easy to get wrong.

### React 19 Supports Simpler Contracts

React 19 reduces some component ceremony (`ref` as prop) and improves form/action pending/error
handling. For finance UI, optimistic state must be constrained: a write proposal can appear quickly,
but a material write is not final until validation and explicit approval succeed.

### AI UX Must Preserve Human Control

Microsoft, Google PAIR, NIST AI RMF, and C2PA all point toward expectation-setting, provenance,
uncertainty, correction paths, and clear control. For Neko, that means deterministic tool output and
source citations appear separately from Mia's prose; model text never becomes an implicit approval.

### Charts Need Data Equivalents

Charts are useful for trend, deviation, and credit/reserve pressure, but finance users need exact
numbers. Every chart should include textual summary and table equivalent, and no chart should encode
critical meaning by color alone or hover-only details.

### Local-First Needs Visible Boundaries

Local-first UX should make ownership legible: this device, data location, offline readiness, sync
freshness, export/backup, and data-leaves-device moments. Connection errors should be clear without
implying local data is lost.

## Repo-Specific Findings

- `PRODUCT.md` already names Midnight Ledger, dark-first use, token families, and WCAG AA baseline.
- `CONTEXT.md` already defines the domain language needed by component contracts.
- `docs/architecture.md` already states the copilot cannot write material changes directly and all
  writes require structured diff, validation, and approval.
- `src/design-system/tokens/colors.css` already has primitive and semantic color tokens, owner
  accents, chart colors, and dark/light themes.
- `src/design-system/components/*.tsx` has a small production component set with tests, but several
  components currently use inline style objects and string-to-style parsing. Future hardening should
  move variants toward explicit class/token styles.
- `src/screens/DashboardScreen.tsx`, `TransactionsScreen.tsx`, `SettingsScreen.tsx`, and
  `CopilotScreen.tsx` contain screen-local card/table/status patterns that can migrate gradually as
  production components are introduced.

## Open Questions For Implementation

- Should high-contrast mode be a first-class app setting in addition to OS `forced-colors`, or only
  OS-driven for now?
- Should chart rendering use a lightweight in-house SVG pattern first, or adopt Observable Plot once
  the first chart-heavy screen lands?
- Should complex primitives come from React Aria, Radix, Ark UI, or remain custom until a concrete
  widget requires them?
- Which screen should be the first migration target after core component hardening: Dashboard,
  Transacoes, Ajustes/privacidade, or the future Caixa de revisao?
