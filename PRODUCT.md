# Product

## Register

product

## Users

A solo user managing personal finances — a software/AI engineer using the app in a home office at night under low ambient light. The primary workflow: updating daily expenses, checking future projections, diagnosing financial health, and planning savings and investments. A partner (additional card) is tracked but does not use the app directly.

## Product Purpose

Neko Finance is a local-first desktop finance tool that connects to Google Sheets and provides an AI copilot (Mia) for financial diagnosis. Its job: replace the friction of manual spreadsheet tracking with a disciplined daily check-in flow, forward-looking projections, and methodology-driven insights — all without sending financial data to the cloud.

Success means the user spends less time managing spreadsheets and more time making informed financial decisions. The bar: a daily check-in under 30 seconds that surfaces exactly what matters.

## Brand Personality

**Precisa, discreta, confiável, amigável** (precise, discreet, trustworthy, friendly).

The app is a capable financial assistant that never shows off. It speaks with quiet authority — numbers are self-evident, explanations are clear but never preachy. The tone is warm but not casual; professional but not cold. Think: a skilled accountant who actually respects you.

## Anti-references

- **Bancos tradicionais** — cluttered interfaces, cross-selling products, hidden fees in fine print.
- **Apps gamificados** — confetti, mascots, achievements, streaks. Money is serious; don't infantilize it.
- **Dashboards corporativos** — dense tables, complex charts, ERP aesthetics. This is personal, not enterprise.
- **Neobanks coloridos** — neon purple/pink, cartoon illustrations, teen-banking energy.

## Design System

The visual design system is **"Midnight Purr"** (dark-first), the systematization of the
"Conversa com a Mia" direction. Key tokens:

- **Neutrals**: zinc, zero-chroma — `#09090B` bg / `#18181B` surface / `#27272A` border (dark); `#FAFAFA` / `#FFFFFF` (light)
- **Accent (brand)**: user-configurable palette — jade (default) `#3FBF8F`, lima, violeta, âmbar, céu, rosa — each with an atomic `--accent`/`--accent-ink` pair; set via `data-accent` on `<html>`
- **Status (method)**: fixed per theme, never follow the accent — paz/entrada green, atenção orange, dinheiro pos/neg. Brand color and method-status color are hard-separated.
- **Typography**: Geist (UI and money, `tabular-nums`), Geist Mono (parcels/citations/code), Newsreader (editorial)
- **Corners**: pill-dominant — 6-10-14-18-22px scale, pills for chips/tabs
- **Shadows**: discreet; dark leans on borders + `--lift`, light on soft ambient shadows
- **Motion**: 130-480ms, `cubic-bezier(0.2, 0, 0, 1)`, reduced-motion respected; money values never animate
- **Shell**: per-viewport chrome — fixed sidebar with the primary CTA on desktop, icon rail on tablet, blurred appbar + floating tab bar with embedded FAB on mobile

Full design system at `src/design-system/`. Agent skill at `.agents/skills/neko-finance-design/SKILL.md`.

## Design Principles

1. **Night-first** — designed for low-light use. Dark mode is the default, not an afterthought. Light surfaces are the minority.
2. **Precision without noise** — every number earns its place. No decorative elements near financial data. Clarity beats density.
3. **Discreet confidence** — the tool knows what it's doing and doesn't need to prove it. Restraint over flash; capability over decoration.
4. **Friendly guidance** — Mia is warm but not chatty. Advice is specific, actionable, and never moralizing. The user is the decision-maker.
5. **Data-first, chrome-second** — the numbers own the screen. UI chrome (nav, headers, controls) recedes until needed.
6. **Didactics behind a question** — the app teaches the method, but fixed conceptual copy never occupies the screen as a permanent paragraph. Variable data and CTAs stay inline; explanation opens on demand from a tappable question. Hard rules in `docs/ui-standards.md`.
7. **Calm-clean surfaces** — separation comes from whitespace and surface contrast, not from boxes and borders; hierarchy is typographic (size, weight, ink shade), and accent color is spent only where it carries meaning. The owner's standing aesthetic preference. Hard rules in `docs/ui-standards.md` ("Calm density").

## Accessibility & Inclusion

- **WCAG AA** minimum — all text ≥ 4.5:1 contrast against its background.
- **Dark mode as default** — primary design target given nighttime use. Light mode as secondary theme.
- **Reduced motion respected** — all animations wrapped in `@media (prefers-reduced-motion: reduce)`.
- **Keyboard navigable** — all interactive elements reachable via Tab; focus rings visible.
- **Screen reader compatible** — semantic HTML, ARIA labels on financial data, meaningful alt text.
