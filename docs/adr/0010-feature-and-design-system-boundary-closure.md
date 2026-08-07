# ADR-0010: Pockets, reconcile, and onboarding get named views; the design system stops importing DTOs

ADR-0006 fenced `src/lib/api.ts` behind exception zones and a shrinking legacy allowlist. Three
features (`features/pockets`, `features/reconcile`, `features/onboarding`) and two design-system
components (`BalanceTrajectory`, `LineItemEditor`) were still on that allowlist, each for a
different reason, so each needed its own explicit call.

## Decision

**Pockets** (`PocketsManager.tsx`, `PocketsCard.tsx`, `pocketLabels.ts`) gets `features/pockets/
pocketsView.ts`: a small read (`fetchPockets`) plus one write wrapper (`createAccountCmd`), and the
re-exported `Pockets`/`PocketType` types `pocketLabels.ts` needs for its exhaustive label maps. This
is the same shape as `configView.ts` — a feature-local view, not a screen's, per the precedent
`features/sheets/sheetsView.ts` already set for non-screen domains.

**Reconcile** (`ConflictGate.tsx`) gets `features/reconcile/reconcileView.ts`: the conflict list read,
the resolve write, and a `listenSyncDone` wrapper around the generic `listenEvent`/`SYNC_DONE_EVENT`
pair — the gate is the one consumer left needing the live event after `useWriteBackPending.ts`
(a `src/hooks/**` exception zone) covers the dashboard's own subscription.

**Onboarding** (`OnboardingFlow.tsx`) gets `features/onboarding/onboardingView.ts`: one wrapper,
`markOnboardingDone`, plus the `ONBOARDING_KEY` constant it and `App.tsx` share. `App.tsx` already
reads shell state through `shellView.ts` (ADR-0008); it now imports the onboarding key from
`onboardingView.ts` instead of re-exporting it through the component module.

**Design system stops importing DTOs, even as types.** `BalanceTrajectory` took `ForecastDay[]` and
`LineItemEditor` took `LineItemDraft[]` directly from the shim — a type-only import ADR-0006 already
forbids outside a named zone, and a real coupling: either DTO's shape can now only change together
with the design-system component that renders it. Both components declare a local, structural
interface instead — `BalanceTrajectoryPoint { date, balance_cents }` and `LineItemEditorItem
{ amount_cents, description, position }` — matching only the fields each component actually reads or
writes. TypeScript's structural typing means every existing domain type shaped that way (`ForecastDay`,
`LineItemDraft` from `lancamentosView.ts`) still satisfies the prop without a cast; the design system
just no longer names the wire type to do it. Both components' tests build fixtures against the new
local type instead of the shim's.

All nine allowlist entries this touches (five component files, `pocketLabels.ts`, `App.tsx`'s import
of `ONBOARDING_KEY`, and both `BalanceTrajectory`/`ConflictGate` test files) come off
`eslint.lib-api-allowlist.mjs` in the same change that migrates them.

## Why

A feature reaching past its own view for one field starts a second, undocumented reading of the same
DTO — the failure mode ADR-0006 already named for screens applies identically to features. For the
design system specifically: a reusable component that imports a feature DTO (even type-only) is no
longer feature-agnostic — it silently depends on `src/lib/api.ts`'s wire shape, and a backend rename
would break a component that has no business knowing the backend exists.

## Consequences

- `features/pockets`, `features/reconcile`, `features/onboarding`, and `src/design-system` (and
  their tests) have no more `lib/api` importers; a future one is a plain diff regression, not a
  pre-existing allowlist entry.
- A future design-system component takes domain data through a local, structural prop type — never a
  type imported from `lib/api` or a screen/feature view. The prop shape documents exactly what the
  component reads; the caller's domain type satisfies it for free when the fields line up.
