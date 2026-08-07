/**
 * Legacy allowlist for the `lib/api` funnel gate (see docs/adr/0006-lib-api-funnel-gate.md).
 *
 * Empty since #336: every importer that started here migrated behind a named exception zone or
 * its screen's `*View.ts`. It stays declared (rather than deleted) because
 * `scripts/check-lib-api-allowlist.mjs` still runs in `npm run check` as the anti-rot guard — any
 * future entry added here must actually import `lib/api` (dead entry, remove it) and the list may
 * never grow past LIB_API_ALLOWLIST_CEILING (new direct import belongs in a view, not here).
 */
export const LIB_API_ALLOWLIST = [];

// Ceiling is 0 since #336 — the funnel closed. Never raise it to paper over a new direct import.
export const LIB_API_ALLOWLIST_CEILING = 0;
