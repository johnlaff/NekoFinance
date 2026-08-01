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
2. **Entry points are named by the user's question and preview the answer.** A door
   to an instrument (a report, a drill-down, a "see all") leads with the intention
   it serves ("Como o mês está indo?") and carries the live answer as its
   subtitle — the current number, scoped to its period. Instruments (charts,
   tables) live behind the door, never as the door: nobody should have to
   interpret a chart to discover which question it answers. Two boundaries: primary
   navigation keeps noun names — the question lives in the crumb and in the door's
   preview, never as a renamed tab; and the questions are the method's own — a
   question the method rejects (spend-by-category is the classic) does not earn a
   door by being familiar.
3. **Inline stays for: variable data, one sentence of context, and CTAs.** Numbers,
   dates and percentages are the content — they never collapse. An action is never
   hidden inside a popover: empty/missing states keep their CTA visible.
4. **One invitation per state.** When a state has a call to action (e.g. a pending
   proposal), every surface that mentions it uses the _same phrase_ — never two
   different invitations for the same act. Punctuation may differ by context (prose
   takes a final period; inline labels don't).
5. **CTAs are verb + object** ("Estipular o teto", "Registrar lançamento"), sentence-case.
   **No rendered copy starts lowercase — ever.** Every string that opens an element or a
   visual line (label, pill, tail, section note, summary, status caption) begins with a
   capital letter or a non-letter (digit, "R$", "—"). Lowercase is only legal
   mid-sentence, continuing a line another string started (a money suffix, a crumb after
   "·"). Prototype copy routinely arrives lowercase — capitalize at the implementation
   boundary, don't wait for review to catch it.
6. **Copy that describes a formula must match the engine.** Before shipping a sentence
   like "the smaller of two limits", read the function that computes the number and
   verify the sentence is literally true. A sentence that claims a `min` the engine
   does not compute is a fabricated number in prose form.
7. **Empty-state copy claims only what the data confirms.** A sentence like "no
   movement — the balance stayed put" speaks for every source it summarizes, not
   just the list that happens to be empty: a day with zero itemized rows can still
   carry movement in its aggregates. Check each surface the phrase makes a claim
   about before rendering it, and fall back to a narrower sentence when only part
   of the claim is true.
8. **Same behavior in both viewports.** Copy rules are universal, not per-breakpoint —
   mobile inherits the space win, desktop wins consistency. Never fork copy by viewport
   without a structural reason (and then via CSS visibility, never divergent DOM text).

## Layout

9. **Cards of uneven mass compose in independent columns.** Row-aligned grids are
   unsalvageable with disparate card heights: `align-items: start` opens holes between
   cards; `stretch` inflates short surfaces over empty tint (a voice note must never
   stretch). Use per-column wrappers (flex column, uniform gap) that dissolve on mobile
   via `display: contents`. Native masonry (`grid-lanes`) is not a baseline until it
   ships cross-browser. Grid items default to `min-width: auto`, and `display: contents`
   promotes the cards themselves to grid items: set `min-width: 0` on them, or one row of
   fixed-width fields drags the whole screen past the 390px viewport.
10. **DOM order is reading order — always.** Screen readers and tab order follow the
    DOM, not `order` or visual placement. Layout variants may only change _where the
    column break falls_, never the sequence of children. No `order` property to fake a
    reading sequence.
11. **Roving tabindex follows programmatic focus.** Calling `el.focus()` without
    moving the roving state leaves `tabIndex="0"` on the previous cell, so the next
    arrow key navigates from the wrong place. Every programmatic focus move (month
    switch, page jump) also updates the state that renders `tabIndex`.
12. **Check token values before mapping a prototype.** The spacing scale is 4px-based
    and non-linear (`--space-3` = 6px, `--space-5` = 12px, `--space-6` = 16px). Mapping
    prototype pixels by token _name_ instead of value compresses or inflates every
    internal spacing at once.
    Radius follows the token contract: `--radius-md` for cards and metric tiles,
    `--radius-lg`/`--radius-xl` only for panels, sheets and large surfaces.
13. **Screen classes are namespaced** (`.hoje__*`, `.cartoes__*`). Short generic class
    names collide with shell globals (`.sh` is the app shell root). Screen CSS lives in
    a sibling file (`src/screens/<screen>.css`); shared chrome stays in `redesign.css`.
14. **Grid gaps are uniform** (one token both axes, `--space-6` default). Height slack
    is anchored _inside_ cards (`margin-top: auto` on the footer element), never left
    as holes between them.

## Components over reimplementation

15. **Every progress/ruler bar is the DS `Meter`.** Track + pill radius + width fill is
    one component (decorative by default, `role="img"` with a full text equivalent when
    named). Handwritten track+fill markup drifts into divergent tags and radii across a
    single screen.
16. **Loading and error states use `EmptyState`** — it announces to screen readers
    (`role="status"` skeleton/loading, `role="alert"` error). Never a silent bespoke
    skeleton, and never a fabricated `R$ 0,00` while data is missing (epistemic-state
    primitives: `EstimateMark`, `NoRecordDash`, `ModeChip`).
17. **Repeated button labels get distinct accessible names.** Two "Ver tudo ›" on one
    screen need `aria-label`s that name their subject. A card's "see all" navigates to
    the _subject of the card_ (invoices → Cartões, not the generic ledger).

## Per-environment ergonomics

18. **The mobile first screen belongs to primary content.** What a thumb-user opens the
    app for (the verdict and today's data) must be visible without scrolling on a
    390×844 viewport; didactics and secondary detail come after or behind disclosure.
    Measure it on the real breakpoint before shipping.
19. **Thumb vs mouse density.** Horizontal carousels use `scroll-snap-type: x mandatory`
    and at most 6 items on mobile; the desktop list form may show more (CSS hides the
    surplus on the small breakpoint). Touch targets ≥ 44px — grow the hit area with
    padding + negative margin, never by inflating the layout. Programmatic scroll
    targets (drill tops, anchors) that live under the fixed appbar carry
    `scroll-margin-top` its size — `scrollIntoView` aligns to the scrollport and the
    bar covers the target; DOM visibility checks (`toBeVisible`) do not detect
    occlusion, only an inspected screenshot does.
20. **Desktop earns its density.** Hover states (+`transition`) on every inline link
    and interactive row; reading widths capped (`ch`-based); the content ceiling rises
    one step on ultrawide (≥ 1700px) instead of letting side voids grow.
21. **An anchored element floats over the content until the scroll ends.** A screen
    whose composer/toolbar is `position: sticky; bottom: 0` must scroll the **scroller**
    to its end (`scrollTo({top: scrollHeight})`), not itself: `scrollIntoView({block:
"end"})` aligns the screen's end to the scrollport's end and stops while padding
    remains, leaving the new content under the anchored bar. Sticky bottom also honors
    the scroller's own `padding-bottom` — on mobile that padding is exactly the room the
    floating dock needs, so the composer lands above it for free.

## Calm density (clean)

The reference look is the current fintech clean standard: generous whitespace, soft
surfaces, typographic hierarchy, near-invisible structure. These rules keep every wave
honest to it.

22. **Whitespace separates; borders are the exception.** Separation comes from spacing
    and surface contrast first. A hairline divider is a list's rhythm marker, not a
    default wrapper — when everything sits in a bordered box, the screen reads as a
    form, not a ledger.
23. **Hierarchy is typographic.** Size, weight and ink shade (strong/base/muted/faint)
    carry the ranking; money is the typographic hero of its surface. A screen that
    needs a colored box to rank content has a type-scale problem, not a color problem.
24. **Accent is spent, not sprayed.** Brand accent and method-status colors appear only
    where they carry meaning (primary action, judged state, the datum itself);
    everything else stays neutral. Decorative color dilutes the signal of status color
    everywhere else in the app.
25. **Metadata earns its pill.** A pill/badge on a row exists for state the user acts
    on or must not miss (Previsto, parcela, reembolso, tag). If every row at rest
    carries one, none of them is information — fold the constant ones into the context
    column.

## Honest numbers (reinforcing the DS working rules)

26. **Text and `aria` tell the true value; only the bar saturates.** Never clamp a
    displayed or announced percentage at 100% — a bar may cap its width, the number
    never lies.
27. **Display sign derives from the movement type, never from the raw amount.**
    `TransactionRow.amount` carries magnitude; its stored sign is not a contract
    (imported and manual rows differ, and the backend compares by absolute value).
    Render money through the ledger rule — entrada positive, everything else
    negative — or an expense renders as `+R$ 43,00`.
28. Money is tabular and never animates; method-status colors never follow the brand
    accent; missing data never renders as zero. (Contracts of the design system —
    restated here because every wave touches them.)

## Signatures

The four proprietary moments are listed in `PRODUCT.md`. These are the rules that keep them
from drifting back into one implementation per screen.

32. **Verdict-first has one contract and exactly two scales.** `VerdictHero` (opens a screen)
    and `HealthBadge` (marks a block). A third form — a tinted box with an icon — is not a
    third scale, it is rule 23 being violated: a screen that needs a colored box to rank
    content has a type-scale problem. State words come from one closed vocabulary app-wide,
    so the user learns it once.
33. **A derived hero number carries a receipt; a typed number does not.** The receipt prints
    the operands and **replaces** the prose that described the formula — never stacks on it.
    One per screen, on the hero. The operands come from the engine: a screen that lacks them
    renders no receipt rather than reconstructing them, because a reconstructed sum is a
    fabricated number wearing the costume of proof.
34. **Two doors, declared boundary.** "Como funciona?" (`InfoPopover`) answers _what this
    concept means in the method_ — invariable. "Ver a conta" (`Receipt`) answers _where this
    number came from today_ — arithmetic. Content that does not clearly fall in one becomes
    neither. Both are governed by their own preference; "Conta sempre à mostra" hides
    arithmetic, never the state of the data — the epistemic mark survives collapsing.
35. **One glyph per role.** `NekoMark` is the product mark and appears only in the shell;
    `MiaAvatar` marks Mia's voice wherever the app interprets rather than reports. The cat
    never appears decoratively — a mascot with no meaning is the gamified anti-reference.
    Empty states earn it only when the emptiness is a method gap with something to teach;
    a load failure is an error state (rule 16), not a gap, and never carries the cat.

## Motion

36. **One entrance budget: ~400ms from the first element to the end of the last.** Choreography
    durations and delays come from `--dur-*` tokens (`--dur-base` for the step,
    `--dur-stagger-step` for the offset, capped at five steps) and **never live inside a media
    query**. `prefers-reduced-motion` is not the only trigger: the "Animações" toggle lives in
    `[data-motion]` on `<html>`, and a media query cannot see an attribute — a hardcoded
    duration under `@media (prefers-reduced-motion: no-preference)` means the setting neither
    turns motion off nor turns it back on. Token collapse covers both triggers and honors the
    user who deliberately turns motion on.
37. **Uppercase is for abbreviations, never for labels.** `text-transform: uppercase` is legal
    only where the content is a conventionally-uppercase abbreviation set (weekdays, months).
    Caps erase the word's silhouette and charge letter-by-letter reading in an element that is
    context, not content; micro-label caps are the corporate-dashboard idiom the brand declares
    as an anti-reference. Caps tracking (`--ls-label`/`--ls-caps`) goes with it — kept on
    sentence case it just loosens the word.

## Verification

38. **Visual baselines regenerate from scratch** (`rm -rf` the snapshot dirs, run the
    suite twice — record then verify) whenever a screen changes intentionally.
    `--update-snapshots` alone does not rewrite sub-threshold drift, and its default
    `changed` mode has been observed passing a rewritten screen against its stale
    baseline — pass `--update-snapshots=all` when regenerating deliberately.
39. **A shared e2e fixture feeds many screens.** Changing `tauri-mock` data
    re-records baselines beyond the screen under work; inspect every consumer's
    regenerated baseline before committing — a richer fixture must read as richer,
    not different.
40. Every wave passes: `npm run check`, e2e visual smoke with inspected screenshots,
    React Doctor with no new findings, and the impeccable audit + critique gates before
    it is considered done.
