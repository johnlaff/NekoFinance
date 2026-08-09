# ADR-0006: `lib/api` is the shim; the screen's View is its gate

`src/lib/api.ts` is a thin wrapper over the Tauri `invoke` calls the Rust backend exposes — it has
no domain logic of its own. Any file in `src/` could import it directly, and most historically did:
components, features and screens all reached across the shim's raw DTOs instead of the domain types
their screen's `*View.ts` already derives from them.

## Decision

**Outside a named exception zone, `src/**` may not import `src/lib/api.ts`, in any form** —
`**/lib/api`, `./api` (from inside `src/lib/` itself, e.g. `useCommand.ts`), or any other relative
depth. The `no-restricted-imports` rule in `eslint.config.js` enforces this with **no
`allowTypeImports`**: a type-only import of a raw DTO is still a violation, since the point is that
domain-shaped types come from the view, not from the wire format underneath it.

**Exception zones** (no friction, no `eslint-disable`):

- A screen's `*View.ts` and its `*View.test.ts` — the view is the funnel's gate. It is the one place
  allowed to translate the shim's DTOs into the domain shapes the screen renders.
- A feature's domain `*View.ts` and its tests (`src/features/<feature>/<feature>View.ts` — sheets,
  pockets, reconcile, onboarding, updater) — the same gate role as a screen's view, for domains that
  live under `src/features/` instead of `src/screens/`.
- The Mia runtime on the real path — `miaRuntime.ts`, `miaSession.ts`, and their tests — which drive
  the live conversation loop directly against the shim.
- `src/hooks/**` — cross-screen hooks sit at the same funnel depth as a view.
- `src/test/commands.ts` — the IPC mock infrastructure every test's `invoke` stub is built from; it
  has to speak the shim's vocabulary to fake it.

**Legacy allowlist.** Every other current importer is listed by exact path in
`eslint.lib-api-allowlist.mjs`, so the gate is green on day one without touching a single importer.
`scripts/check-lib-api-allowlist.mjs` (wired into `npm run check` as `lint:lib-api-allowlist`) fails
the gate if an entry stops importing `lib/api` (a dead entry — remove it, the migration already
happened) or if the list grows past its recorded ceiling (a new direct import that should have gone
through a view instead). The list only shrinks.

## Why

A screen that reaches past its view for one field starts a second, undocumented reading of the same
DTO — the same failure mode ADR-0005 closed for the annual ruler, one layer up the stack. The
inventory that motivated this gate (`npx eslint` with the rule on and no allowlist, not `grep` —
grep both false-positived on a comment in `nkFormat.ts` and missed `App.tsx`) found 78 current
importers: 27 already sit inside a named zone, 51 do not and form the initial allowlist.

## Consequences

- New domain logic for a screen has one legal entry point to the backend: its `*View.ts`. A reviewer
  who sees `from "../lib/api"` outside a zone in a diff knows it is either the view file itself or a
  regression.
- The 51-file allowlist is a punch list, not a policy. Migrating one of its files means deleting its
  line from `eslint.lib-api-allowlist.mjs`, not adding an `eslint-disable` comment — the anti-rot
  check would immediately flag a disabled rule as a dead entry once the underlying import moves into
  the view, so there is no cheaper way through the gate.
