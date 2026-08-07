import type { PocketType } from "./pocketsView";

/** PT-BR labels for the pocket types accepted by `create_account`. */
export const POCKET_TYPE_LABELS: Record<PocketType, string> = {
  bank: "Conta corrente",
  wallet: "Carteira",
  business: "Conta PJ",
  savings: "Poupança / reserva",
  meal_voucher: "Vale alimentação/refeição",
  pension: "Previdência privada",
  fgts: "FGTS",
};

export const LIQUIDITY_LABELS: Record<string, string> = {
  liquid: "Caixa",
  reserve: "Reserva",
  restricted: "Restrito",
  illiquid: "Ilíquido",
};
