---
name: neko-finance-design
description: Use this skill to generate well-branded interfaces and assets for Neko Finance — a local-first, private personal-finance desktop app with the calm AI copilot "Mia" — for production or throwaway prototypes/mocks. Contains design guidelines, color/type/spacing tokens, fonts, brand assets, reusable components, and full-screen UI kits.
user-invocable: true
---

Read `readme.md` in this skill first — it covers brand principles, voice, visual foundations, iconography, the token system, the chart language, accessibility rules, and a component inventory. Then explore the other files.

**Design context**
- **Register**: product (app UI / dashboard)
- **User**: solo engineer in home office at night, low ambient light
- **Brand**: precise, discreet, trustworthy, friendly
- **Mode**: dark-first (nighttime use), WCAG AA minimum
- **References**: YNAB (discipline), Notion (clean minimal), Stripe Dashboard (clear data)
- **Anti-references**: traditional banks, gamified apps, corporate ERPs, neon neobanks, SaaS cream
- **Design principles**: night-first, precision without noise, discreet confidence, friendly guidance, data-first / chrome-second

**Layout**
- `styles.css` — the entry point; link it to get every token + font. It only `@import`s `tokens/*.css`.
- `tokens/` — colors, typography, spacing, elevation, motion (CSS custom properties; dark `:root` + `[data-theme="light"]`).
- `assets/` — `neko-mark.svg` (brand mark), `mia-avatar.svg` (copilot).
- `guidelines/*.card.html` — foundation specimens (colors, type, spacing, brand).
- `components/<group>/` — reusable React primitives (`<Name>.jsx` + `.d.ts` + `.prompt.md`). Core, Finance, Copilot.
- `ui_kits/` — full-screen recreations: Dashboard, Transactions/import review, Copilot approval flow. See `ui_kits/README.md`.

**Working rules (lift these from the system)**
- Money & all figures: Geist with `tabular-nums` (`--font-money`), right-aligned in columns, currency + 2 decimals; a money value never animates.
- Status, owner, and confidence are *never* color-only — always pair with a word/icon/shape.
- Writes to a user's data are a first-class approval surface — show a before→after diff, never auto-apply.
- Voice: calm, plain, sentence-case, "you" for the user, no emoji, no cat puns.
- Color: zinc neutrals + ONE configurable brand accent (`--accent`/`--accent-ink`; jade default via `data-accent` on `<html>`); method-status colors are fixed and never follow the accent; charts use the fixed `--chart-1..6` data palette.
- Didactics behind a question: fixed conceptual copy collapses into a tappable "Como funciona?" (`InfoPopover`); inline stays for variable data, one sentence of context and CTAs. Every ruler/progress bar is the DS `Meter`. Full screen-level rules: `docs/ui-standards.md` in the app repo — mandatory before any screen work.

**If creating visual artifacts** (slides, mocks, throwaway prototypes): copy the assets and tokens you need into your output folder and build static/standalone HTML. For component-driven mocks, you can reuse the patterns in `ui_kits/` (read components from the compiled bundle, or copy a component's JSX and inline it).

**If working in production code**: read the token files and `.d.ts`/`.prompt.md` contracts and design to the CSS custom properties. Replace the Google-Fonts `@import` with self-hosted WOFF2, and prefer official `lucide-react` (strokeWidth 1.75) over the bundled icon set.

If invoked without guidance, ask what the user wants to build, ask a few focused questions (surface, fidelity, light/dark, variations), then act as an expert Neko Finance designer and output HTML artifacts or production code as needed.
