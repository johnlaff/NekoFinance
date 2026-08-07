//! Porta do domínio Bolsos: `PocketsManager.tsx`, `PocketsCard.tsx` e `pocketLabels.ts`
//! importam só daqui — nunca de `lib/api`.

import {
  createAccount,
  getPockets,
  type Pockets,
  type PocketType,
} from "../../lib/api";

export type { Pockets, PocketType };

// --- Leitura -----------------------------------------------------------------------------

export function fetchPockets(): Promise<Pockets> {
  return getPockets();
}

// --- Escrita -------------------------------------------------------------------------------

export function createAccountCmd(
  name: string,
  accountType: PocketType,
  balanceCents: number,
  institution?: string,
): Promise<string> {
  return createAccount(name, accountType, balanceCents, institution);
}
