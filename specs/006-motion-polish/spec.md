# Spec: Motion & Interaction Polish

## Summary

Give the app its motion layer and close the perceived-performance gaps, strictly on the design
system's motion tokens ("calm, precise, never bouncy"). Headline: a circular-reveal theme switch
driven by the View Transitions API. Supporting cast: screen-change transitions, dashboard entrance
choreography, an SWR-style command cache that eliminates skeleton flashes on re-navigation, and a
⌘K shortcut for the header search.

## User Stories

### US1 — Circular reveal theme switch (the wow)

**As** the primary user
**I want** the dark/light switch to sweep across the screen as a circle growing from the toggle
**So that** changing themes feels like a single deliberate gesture instead of a repaint.

**Acceptance**: Clicking the theme toggle starts a View Transition; the new theme expands as a
`clip-path` circle from the click point (button center on keyboard activation) to the farthest
viewport corner, using `--dur-deliberate`/`--ease-entrance`. Browsers without
`document.startViewTransition` (e.g. WebKitGTK dev shells) and reduced-motion users get an
instant, correct swap. Theme persistence behavior is unchanged.

### US2 — Screen transitions

**As** the primary user
**I want** screens to enter with a brief fade-rise instead of popping
**So that** navigation reads as movement through one app, not page reloads.

**Acceptance**: On navigation the incoming screen animates opacity 0→1 + 4px rise over
`--dur-base`/`--ease-entrance`. No exit animation (content must never feel slow); zero duration
under reduced motion (token-driven).

### US3 — Dashboard entrance choreography

**As** the primary user
**I want** the dashboard to compose itself in one calm pass
**So that** the hero number lands with weight.

**Acceptance**: Metric tiles stagger in (≤40ms steps); the projected-balance value counts up to
its final figure over `--dur-deliberate` (tabular digits, no layout shift); the deficit warning,
when present, enters with a one-time emphasis (no looping animation). Reduced motion (or any
environment without `matchMedia`) renders final values instantly — tests assert the snap.

### US4 — No skeleton flash on re-navigation (perceived performance)

**As** the primary user
**I want** previously seen screens to render their data instantly when I return
**So that** switching tabs feels native-instant.

**Acceptance**: A small SWR-style cache (`useCommand`) returns the last response synchronously
and revalidates in the background; loading skeletons appear only on first visit. Successful
spreadsheet imports invalidate the cache so stale finance numbers never survive an import.
Hook behavior covered by unit tests (cache hit, revalidate, error, invalidation).

### US5 — ⌘K search focus

**As** the primary user
**I want** Ctrl/Cmd+K to focus the header search, with a visible kbd hint
**So that** the primary input is one keystroke away, as in every 2026 product.

## Non-functional requirements

- **Tokens only**: every duration/easing comes from `tokens/motion.css`; reduced-motion zeroing is
  inherited from the tokens, with the existing explicit guards kept.
- **Compositor-friendly**: animate `transform`/`opacity`/`clip-path` only — no layout properties.
- **Honest motion**: entrance-only; nothing loops, nothing blocks input; data is never delayed to
  show an animation.
- **Tests**: logic under test (cache hook, count-up snap, theme fallback path); visual structure
  via existing e2e (reduced-motion emulated keeps screenshots deterministic).
