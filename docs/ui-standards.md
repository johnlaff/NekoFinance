# UI Standards

Hard rules for every screen and screen wave. They complement `PRODUCT.md` (brand and
design principles) and the design-system skill (`.agents/skills/neko-finance-design/`).
Each rule carries its rationale — the defect that violating it produces. Screen work
that deviates must say why in the PR.

## Copy

1. **Didactics live behind a question, never in a permanent paragraph.** Fixed
   conceptual copy (method explanations, metaphors, hypothetical scenarios — any text
   identical every day) collapses into a tappable trigger ("Como funciona?" or a
   tappable label) opening an `InfoPopover` with a custom `{title, body}` term. This is
   the method's canonical teaching style (question-first disclosure) and the current
   fintech standard (essential upfront, depth on demand). Always-visible didactic
   paragraphs consume the mobile first screen and push the primary content out of
   view.
2. **Inline stays for: variable data, one sentence of context, and CTAs.** Numbers,
   dates and percentages are the content — they never collapse. An action is never
   hidden inside a popover: empty/missing states keep their CTA visible.
3. **One invitation per state.** When a state has a call to action (e.g. a pending
   proposal), every surface that mentions it uses the _same phrase_ — never two
   different invitations for the same act. Punctuation may differ by context (prose
   takes a final period; inline labels don't).
4. **CTAs are verb + object** ("Estipular o teto", "Registrar lançamento"), sentence-case,
   first letter capitalized. No label starts lowercase.
5. **Copy that describes a formula must match the engine.** Before shipping a sentence
   like "the smaller of two limits", read the function that computes the number and
   verify the sentence is literally true. A sentence that claims a `min` the engine
   does not compute is a fabricated number in prose form.
6. **Same behavior in both viewports.** Copy rules are universal, not per-breakpoint —
   mobile inherits the space win, desktop wins consistency. Never fork copy by viewport
   without a structural reason (and then via CSS visibility, never divergent DOM text).

## Layout

7. **Cards of uneven mass compose in independent columns.** Row-aligned grids are
   unsalvageable with disparate card heights: `align-items: start` opens holes between
   cards; `stretch` inflates short surfaces over empty tint (a voice note must never
   stretch). Use per-column wrappers (flex column, uniform gap) that dissolve on mobile
   via `display: contents`. Native masonry (`grid-lanes`) is not a baseline until it
   ships cross-browser.
8. **DOM order is reading order — always.** Screen readers and tab order follow the
   DOM, not `order` or visual placement. Layout variants may only change _where the
   column break falls_, never the sequence of children. No `order` property to fake a
   reading sequence.
9. **Check token values before mapping a prototype.** The spacing scale is 4px-based
   and non-linear (`--space-3` = 6px, `--space-5` = 12px, `--space-6` = 16px). Mapping
   prototype pixels by token _name_ instead of value compresses or inflates every
   internal spacing at once.
   Radius follows the token contract: `--radius-md` for cards and metric tiles,
   `--radius-lg`/`--radius-xl` only for panels, sheets and large surfaces.
10. **Screen classes are namespaced** (`.hoje__*`, `.cartoes__*`). Short generic class
    names collide with shell globals (`.sh` is the app shell root). Screen CSS lives in
    a sibling file (`src/screens/<screen>.css`); shared chrome stays in `redesign.css`.
11. **Grid gaps are uniform** (one token both axes, `--space-6` default). Height slack
    is anchored _inside_ cards (`margin-top: auto` on the footer element), never left
    as holes between them.

## Components over reimplementation

12. **Every progress/ruler bar is the DS `Meter`.** Track + pill radius + width fill is
    one component (decorative by default, `role="img"` with a full text equivalent when
    named). Handwritten track+fill markup drifts into divergent tags and radii across a
    single screen.
13. **Loading and error states use `EmptyState`** — it announces to screen readers
    (`role="status"` skeleton/loading, `role="alert"` error). Never a silent bespoke
    skeleton, and never a fabricated `R$ 0,00` while data is missing (epistemic-state
    primitives: `EstimateMark`, `NoRecordDash`, `ModeChip`).
14. **Repeated button labels get distinct accessible names.** Two "Ver tudo ›" on one
    screen need `aria-label`s that name their subject. A card's "see all" navigates to
    the _subject of the card_ (invoices → Cartões, not the generic ledger).

## Per-environment ergonomics

15. **The mobile first screen belongs to primary content.** What a thumb-user opens the
    app for (the verdict and today's data) must be visible without scrolling on a
    390×844 viewport; didactics and secondary detail come after or behind disclosure.
    Measure it on the real breakpoint before shipping.
16. **Thumb vs mouse density.** Horizontal carousels use `scroll-snap-type: x mandatory`
    and at most 6 items on mobile; the desktop list form may show more (CSS hides the
    surplus on the small breakpoint). Touch targets ≥ 44px — grow the hit area with
    padding + negative margin, never by inflating the layout.
17. **Desktop earns its density.** Hover states (+`transition`) on every inline link
    and interactive row; reading widths capped (`ch`-based); the content ceiling rises
    one step on ultrawide (≥ 1700px) instead of letting side voids grow.

## Honest numbers (reinforcing the DS working rules)

18. **Text and `aria` tell the true value; only the bar saturates.** Never clamp a
    displayed or announced percentage at 100% — a bar may cap its width, the number
    never lies.
19. Money is tabular and never animates; method-status colors never follow the brand
    accent; missing data never renders as zero. (Contracts of the design system —
    restated here because every wave touches them.)

## Verification

20. **Visual baselines regenerate from scratch** (`rm -rf` the snapshot dirs, run the
    suite twice — record then verify) whenever a screen changes intentionally.
    `--update-snapshots` alone does not rewrite sub-threshold drift.
21. Every wave passes: `npm run check`, e2e visual smoke with inspected screenshots,
    React Doctor with no new findings, and the impeccable audit + critique gates before
    it is considered done.
