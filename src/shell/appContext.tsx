import { createContext, useContext } from "react";
import type { Screen } from "./AppShell";
import type { MovementType } from "../lib/nkFormat";

export interface ComposeOptions {
  mode?: "new" | "edit";
  type?: MovementType;
  date?: string;
  transactionId?: string;
}

export interface NekoApp {
  /** Navega entre telas (equivale ao window.__nav do protótipo). */
  navigate: (screen: Screen) => void;
  /** Abre o compositor de lançamento (equivale ao window.openCompose do protótipo). */
  openCompose: (opts?: ComposeOptions) => void;
}

const NekoAppContext = createContext<NekoApp | null>(null);

export const NekoAppProvider = NekoAppContext.Provider;

/** Hook de navegação/compose para as telas. Lança se usado fora do provider. */
export function useNekoApp(): NekoApp {
  const ctx = useContext(NekoAppContext);
  if (!ctx) throw new Error("useNekoApp must be used within NekoAppProvider");
  return ctx;
}
