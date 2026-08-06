import {
  checkAuthStatus,
  getAppSetting,
  lastSyncAt,
  type AuthStatus,
} from "../lib/api";

// View de sessão do shell (ADR-0008): App.tsx e AppShell.tsx compartilham estado transversal —
// status da conta Google, preferência local de onboarding, recência de sincronização — que não
// pertence a nenhuma tela específica, então não cabe numa `*View.ts` de `src/screens/`. Segue o
// mesmo contrato de ADR-0007 num nível diferente: tipos reexportados e fetchers estáveis do
// `useCommand`; nenhum comando de escrita mora aqui porque nenhuma tela do shell grava direto.

// Tipos do shim reexportados pela view — App.tsx e AppShell.tsx leem daqui.
export type { AuthStatus };

export function fetchAuthStatus(): Promise<AuthStatus> {
  return checkAuthStatus();
}

export function fetchAppSetting(key: string): Promise<string | null> {
  return getAppSetting(key);
}

export function fetchLastSyncAt(): Promise<string | null> {
  return lastSyncAt();
}
