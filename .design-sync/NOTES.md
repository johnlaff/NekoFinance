# design-sync — NOTES

Repo-specific gotchas for syncing the Neko Finance design system to claude.ai/design.

## Shape: off-script (hand-authored bundle already in upload format)

- The DS already ships in claude.ai/design's hand-authored format under `src/design-system/`:
  `@dsCard`-marked `*.card.html`, `_ds_bundle.js` (format 3, namespace `NekoFinanceDesignSystem_9bd1cd`),
  `_ds_manifest.json`, `styles.css` → `tokens/*.css`, `assets/**`, `components/{core,finance,copilot}/**`,
  `ui_kits/**`, `guidelines/*.card.html`. The standard package/storybook converter is NOT used.
- **No build script** for `_ds_bundle.js` in the repo. Any component/screen change requires regenerating the
  bundle. Plan: write a deterministic assembler that transpiles each `components/*/*.jsx` (+ ui_kits) and
  rewrites the `@ds-bundle` header `sourceHashes`.
- Cards load React 18 + Babel standalone from `unpkg.com` CDN, then `../../_ds_bundle.js`, then render from
  `window.NekoFinanceDesignSystem_9bd1cd.*`. Relative paths verified (`../../styles.css`, `../styles.css`).

## Upload include-set (bundle only)

INCLUDE: `_ds_bundle.js`, `_ds_manifest.json`, `styles.css`, `readme.md`, `tokens/*.css`, `assets/**`,
`components/{core,finance,copilot}/**`, `ui_kits/**`, `guidelines/*.card.html`.

EXCLUDE (do NOT upload): `components/*.tsx` + `components/*.test.tsx` (these are the app's PRODUCTION source,
which lives mixed into `src/design-system/components/`), `srOnly.ts`, `_adherence.oxlintrc.json`, `SKILL.md`,
`index.html`, and every `*:Zone.Identifier` (Windows download cruft).

## Drift vs current app (audited 2026-06-21)

- Production components MISSING from the DS bundle (11): BalanceTrajectory, Disclosure, InfoPopover,
  LineItemEditor, MiaAvatar, Money, MonthNav, MovBadge, NekoMark, PhaseBadge, ProvBadge.
- In the DS bundle with NO production `.tsx` counterpart (verify they aren't stale): Input, Switch,
  ChatBubble, Citation. (Copilot chat may be inlined in `src/screens/CopilotScreen.tsx`.)
- In both, recreated independently (may have drifted): ApprovalDiffCard, Badge, Button, EmptyState,
  HealthBadge, MetricTile, OwnerChip, SegmentedControl, TransactionRow.
- Screens: app has 9 product screens; ui_kits cover only 5 (dashboard, transactions, methodology, copilot,
  settings). MISSING ui_kits: Totais, Anual, Horizonte, Tags. Existing 5 likely drifted.
- Tokens under `src/design-system/tokens/` are the app's REAL tokens (same dir as production) → current.

## claude.ai/design

- Only existing project: "ArchTime Design System" (UUID 7cff719a-…, unrelated). No Neko project yet.
- First sync → create a new project.

## Build pipeline (NEW — fills the missing build script)

- `.ds-sync/build-bundle.mjs` regenerates `src/design-system/_ds_bundle.js` from sources (babel classic,
  JSX→createElement). Discovers `components/{copilot,core,finance}/*.jsx` (alpha within group) + `ui_kits/**/*.jsx`.
  Components → `Object.assign(__ds_scope,{X})` + final `__ds_ns.X`; ui_kits self-register on `window.*`.
  Header `sourceHashes` = sha256(file)[:12]. Updates `_ds_manifest.json.components`. `--check` = dry run.
  Component sources MUST import only React (assembler throws `[IMPORT]` otherwise) → keep recreations self-contained.
- `.ds-sync/render-card.mjs <card.html> [shot.png]` renders a card in repo chromium, reports
  `__errors`/exports/console, waits past FOUC (component CSS is injected via useEffect). Run from repo root.
- VERIFIED: regenerated bundle renders pixel-identical to the original `core.card.html` (primary/danger/error/
  selected/switch-on all correct), 0 bundle errors, 13 exports. Assembler is faithful.

## Screen/nav drift (refined 2026-06-21, from src/shell/AppShell.tsx)

Current app navigation (NOT the readme's stale "9 telas"):

- Daily screens (8): dashboard, transactions(Lançamentos), totais(Totais), anuais(Anual),
  ano-inteiro(YearGridScreen "Ano inteiro"), economia-compare(EconomiaCompareScreen "Economia comparada"),
  horizonte(Horizonte), tags(Tags).
- Secondary: settings("Configurações e privacidade"), methodology DEMOTED to "Ajuda" (static help doc — the
  full MethodologyScreen ui_kit is now STALE/over-built), copilot "Mia" is still a STUB ("Em desenvolvimento").
- DS ui_kits today recreate only 5 (dashboard, transactions, methodology, copilot, settings).
- MISSING ui_kits (6): totais, anuais, ano-inteiro, economia-compare, horizonte, tags.
- STALE ui_kits: methodology (now "Ajuda" static), copilot (app Mia is a stub, not a full chat).
- Production screen sizes: Transactions 904, Settings 883, NewTransactionForm 778, Tags 376, Anual 369,
  EconomiaCompare 351, Horizonte 312, YearGrid 265, Dashboard 230, Copilot 156, Methodology 74.
- DECISION NEEDED with user: 6 missing (not 4); how to handle Mia-stub + Methodology→Ajuda in the DS.

## Uploaded state (2026-06-21)

- Project "Neko Finance Design System" created: projectId `77a60fba-0912-49d6-99a0-d86480d05bda`
  (https://claude.ai/design/p/77a60fba-0912-49d6-99a0-d86480d05bda). Recorded in config.json.
- Uploaded 131 files (24 components ×3 + 3 group cards + 14 guidelines + 11 screens ×2 + AppShell + icons +
  README + bundle + manifest + styles + 7 tokens + 6 assets) + `_ds_needs_recompile` sentinel.
- Inventory now: 24 components (core 9, finance 11, copilot 4) + 11 screen ui_kits + 14 guidelines.
- Every component card + screen + guideline rendered with 0 bundle errors + 0 console errors before upload.
- No `_ds_sync.json` (off-script) → next sync re-verifies everything (correct/honest).

## Re-sync (how to redo)

1. Edit sources under `src/design-system/` (components/<group>/\*.jsx + .d.ts + .prompt.md, ui_kits/\*\*, cards).
2. `node .ds-sync/build-bundle.mjs && node .ds-sync/build-manifest.mjs` (regenerates bundle + manifest).
3. Verify: `cd src/design-system && python3 -m http.server 8799 &` then
   `node .ds-sync/render-card.mjs http://localhost:8799/ui_kits/<key>/index.html shot.png` (http for screens;
   file:// works for component/guideline cards). Confirm 0 bundle/console errors. Watch FOUC (render-card waits).
4. config.json has the pinned projectId → finalize_plan + write_files the changed files (see uploadIncludeGlobs/
   uploadExclude). The `.ds-sync/` dir + node_modules are gitignored; reinstall with
   `cd .ds-sync && npm i @babel/core @babel/preset-react` if missing.

## Re-sync risks

- The bundle assembler transpiles with babel (object-spread, not the original `_extends`) — functionally
  equivalent, byte-different from the historical bundle. That's fine; don't chase byte-parity.
- ui_kits + component recreations are hand-authored from `src/**` — they silently rot when the app changes.
  They use realistic DEMO data (some via Math.random → non-deterministic screenshots), not live data.
- DS-only primitives (Input, Switch, ChatBubble, Citation) have no production .tsx; reconciled vs app usage.
  Citation is speculative (chat not shipped — Mia is a stub). Re-check if the Mia chat ever ships.

## Reverse flow — redesign → app (2026-06-22)

The user redesigned the app in claude.ai/design (cleaner, less cluttered) under `redesign/` in project
`77a60fba-0912-49d6-99a0-d86480d05bda` and asked to make the app identical to it. This is the REVERSE of the
sync above (pull the design DOWN into the app, not push the DS up).

- **Connector used:** the `claude_design` MCP connector is the `DesignSync` tool (endpoint
  api.anthropic.com/v1/design/mcp), authorized via `/design-login` (scopes user:design:read/write). There is
  NO separate `claude_design` MCP server in this session — DesignSync IS that connector.
- **Import mechanism:** `DesignSync get_project` (confirm connected/authorized) + `list_files` +
  `get_file redesign/Neko.html` (the canonical entry the user named) and the screen sources
  (`Hoje.jsx`, `Lancamentos.jsx`, `Mes.jsx`, `Ano.jsx`, `Calendario.jsx`, `Horizonte.jsx`, `Compose.jsx`,
  `Shell.jsx`, `data.jsx`, `Extras.jsx`). Implemented into `src/` (shell + 9 screens + Compose + redesign.css).
- **Canonical source of truth = `redesign/Neko.html`** in the connector. Its `SCREENS` map (9: hoje,
  lancamentos, mes, ano, calendario, horizonte, tags, mia, config) + titles/crumbs were verified 1:1 against
  `src/shell/AppShell.tsx` SCREEN_META (exact match). `NekoShell`→AppShell, `ComposeHost`→Compose. The
  "Polimento 2026" `#app` CSS overrides were ported into `src/redesign.css` (`.neko-app` scope). `TweaksPanel`
  is prototype-only (font/density/anim switcher) — intentionally NOT in the app.
- To re-verify drift later: `DesignSync get_file redesign/Neko.html` and diff its SCREENS/crumbs vs AppShell.
- The app self-compiles `_ds_manifest.json` from `@dsCard` markers on upload; the uploaded manifest is a
  best-effort copy. Keep `@dsCard` (line 1 of \*.card.html; line 2 of ui_kit index.html) + `@startingPoint` intact.
