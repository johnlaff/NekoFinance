# ADR-0013: Didactics collapse, data stays

The daily-reading screens open with permanent didactic prose — method explanations, metaphors,
hypothetical walkthroughs — identical on every visit. A copy audit measured the cost against rule
1's own reference: Hoje carried ~127 words of permanent prose, O ano ~147, against a ~10-word
ceiling on the genre's best screens, while the same audit found zero prose blocks before the first
datum on any of them — density of data was never the defect.

## Decision

**Fixed text (identical on every visit) collapses behind a tap; text that varies with the user's
data stays inline.** Three tests decide the fate of each _clause_, not each sentence:

1. **Notation test** — if the clause reduces to notation without loss, it is a calculation
   caption: stays.
2. **Variation test** — if the clause changes when the underlying datum changes, it stays; if it
   reads identically on every visit, it is didactic: collapses.
3. **Veteran-reader test** — a returning user still checks operands; metaphor and explanation,
   they no longer need to reread.

A mixed sentence (fixed didactic skeleton + interpolated operand) splits at the clause: the
conceptual clause collapses behind "Como funciona?", the operand survives as a short caption next
to the number it describes. This is the same boundary `docs/ui-standards.md` rule 1 already states
— the three tests are how a screen author applies it without re-deriving it from scratch each time;
rule 41 (each datum once per screen) then decides whether a surviving caption also duplicates a
block that already exists elsewhere on the screen.

Two terms enter the vocabulary because they are exactly the cases the tests would otherwise flag
by mistake:

- **Selo do veredito** — the single line of body copy under a screen's headline that changes with
  the verdict's state (rule 42 allows exactly one). It reads as prose but passes the variation
  test: stays.
- **Legenda de cálculo** — the caption naming the operands of the number printed just above it.
  Rule 3 already protects it; the tests must never mistake it for a permanent didactic paragraph.

## Why

The measured defect is **familiarity**, not information density: the same paragraph paid every
day, forever, by a reader who already knows it. A veteran and a first-time reader need different
things from the same block, and the collapsed form (`InfoPopover`, "Como funciona?") already is
the method's canonical teaching style — question-first disclosure. The three tests exist because
"is this didactic?" is not obvious at sentence granularity: a sentence can carry one clause that
never changes and one operand that does, and only a per-clause test catches that split correctly.

## Scope

This ADR documents the criterion; it does not itself move any copy. It governs any screen that
opens with fixed prose, not just the ones a given wave happens to touch. Out of scope: empty
states (rule 3's CTA already protects them) and any guided flow where the text being read is the
product, not a paragraph to skip.

## Consequences

- Rule 1 of `docs/ui-standards.md` carries the three tests as its operational criterion; no other
  rule changes.
- `CONTEXT.md` gains **selo do veredito** and **legenda de cálculo** as glossary entries, so a
  future screen author does not need to rediscover why these two forms of inline prose survive the
  tests.
- Applying the criterion to specific screens is separate, screen-scoped work; this ADR carries no
  code change.
