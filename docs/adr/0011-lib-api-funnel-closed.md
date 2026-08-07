# ADR-0011: The `lib/api` funnel is closed — the legacy allowlist is empty

ADR-0006 opened the funnel gate with a 51-file legacy allowlist so the rule could ship green on day
one. ADR-0007 through ADR-0010 migrated every screen, shell surface, and feature onto a `*View.ts`
of its own; by #335 only `src/lib/useShowReceipt.ts` remained.

## Decision

`useShowReceipt.ts` moved to `src/hooks/useShowReceipt.ts` — it is cross-screen state (Teto and the
Mia copilot both read the same "conta sempre à mostra" preference) with no owning screen, the same
shape as `useWriteBackPending.ts`, so it belongs in the existing `src/hooks/**` exception zone
rather than earning a new named zone of its own.

`eslint.lib-api-allowlist.mjs` now exports an empty `LIB_API_ALLOWLIST` and
`LIB_API_ALLOWLIST_CEILING = 0`. The array and `scripts/check-lib-api-allowlist.mjs` stay wired into
`npm run check` as `lint:lib-api-allowlist` — the anti-rot check still guards the gate going
forward: an entry added back without actually importing `lib/api` fails as a dead entry, and the
list may never grow past zero. The `no-restricted-imports` rule itself is unchanged; only the
named zones from ADR-0006/0007/0008/0009/0010 remain as exceptions.

## Why

An allowlist that never shrinks is not a migration in progress, it is a permanent hole. Zeroing it
is the verifiable exit condition ADR-0006 promised: `npm run check` now proves, mechanically, that
every `src/lib/api.ts` read outside the named zones goes through a screen or feature's view.

## Consequences

- Any future direct `lib/api` import outside a named zone fails `npm run lint` immediately — there
  is no allowlist left to fall back on while a migration is "in progress." New cross-cutting state
  with no owning screen goes into `src/hooks/**` (if it's plain state/lifecycle) or earns its own
  named view zone (if it's a domain read/write surface), never a re-opened allowlist entry.
- `eslint.lib-api-allowlist.mjs` and `scripts/check-lib-api-allowlist.mjs` stay in the repo as the
  permanent guard rather than being deleted, since the ceiling check is what would catch a
  regression.
