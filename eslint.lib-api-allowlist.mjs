/**
 * Legacy allowlist for the `lib/api` funnel gate (see docs/adr/0006-lib-api-funnel-gate.md).
 *
 * Every path here still imports `src/lib/api.ts` directly instead of through its screen's
 * `*View.ts`. The list only shrinks as each importer migrates — `scripts/check-lib-api-allowlist.mjs`
 * fails the gate if an entry stops importing `lib/api` (dead entry, remove it) or if the list grows
 * past LIB_API_ALLOWLIST_CEILING (new direct import, route it through the view instead).
 */
export const LIB_API_ALLOWLIST = [
  "src/App.tsx",
  "src/design-system/components/BalanceTrajectory.test.tsx",
  "src/design-system/components/BalanceTrajectory.tsx",
  "src/design-system/components/LineItemEditor.tsx",
  "src/features/onboarding/OnboardingFlow.tsx",
  "src/features/pockets/PocketsCard.tsx",
  "src/features/pockets/PocketsManager.tsx",
  "src/features/pockets/pocketLabels.ts",
  "src/features/reconcile/ConflictGate.test.tsx",
  "src/features/reconcile/ConflictGate.tsx",
  "src/features/sheets/GoogleSheetsPanel.test.tsx",
  "src/features/sheets/GoogleSheetsPanel.tsx",
  "src/features/sheets/LocalXlsxImport.tsx",
  "src/features/sheets/WriteBackPreview.test.tsx",
  "src/features/sheets/WriteBackPreview.tsx",
  "src/features/sheets/writeBack.ts",
  "src/lib/useShowReceipt.ts",
  "src/screens/DashboardScreen.test.tsx",
  "src/screens/DashboardScreen.tsx",
  "src/screens/LoanGroupItem.tsx",
  "src/screens/NewTransactionForm.test.tsx",
  "src/screens/NewTransactionForm.tsx",
  "src/screens/ObligationsPanel.tsx",
  "src/screens/SettingsScreen.test.tsx",
  "src/screens/SettingsScreen.tsx",
  "src/screens/TetoScreen.test.tsx",
  "src/screens/TetoScreen.tsx",
  "src/screens/TransactionsScreen.test.tsx",
  "src/screens/TransactionsScreen.tsx",
  "src/screens/dashboard/WriteBackPending.test.tsx",
  "src/screens/dashboard/WriteBackPending.tsx",
  "src/screens/newTransactionCard.ts",
  "src/screens/newTransactionOptions.ts",
  "src/screens/scenarios.test.tsx",
  "src/screens/scenarios.tsx",
  "src/shell/AppShell.tsx",
  "src/shell/Compose.tsx",
];

// Snapshot at the gate's introduction (#326). Lower this as entries migrate; never raise it to
// paper over a new direct import.
export const LIB_API_ALLOWLIST_CEILING = LIB_API_ALLOWLIST.length;
