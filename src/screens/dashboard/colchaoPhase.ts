import type { DashboardSummary, Forecast } from "../../lib/api";
import type { Phase } from "../../design-system/components/PhaseBadge";
import { SAVINGS_MIN_BPS } from "../totaisStatus";

/**
 * Fase de adaptação ao método derivada dos dados (não mais fixa em "calibrate"):
 * - "map": ainda mapeando — poucos lançamentos (<30) ou nenhum mês realizado.
 * - "operate": operando — economizado anual ≥ SAVINGS_MIN_BPS (20%) E reserva ≥ 6 meses.
 * - "calibrate": no meio do caminho (o caso comum enquanto se ajusta o diário).
 *
 * Em módulo próprio (não no arquivo do componente) para não quebrar o Fast Refresh
 * (`only-export-components`).
 */

/** Mínimo de meses de reserva para a fase "operate" (método: piso de 6 meses). */
export const RESERVE_MIN_MONTHS = 6;

export function colchaoPhase(
  summary: DashboardSummary | null,
  forecast: Forecast,
): Phase {
  const txns = summary?.transaction_count ?? 0;
  const income = forecast.annual_savings.realized_income_cents;
  if (txns < 30 || income === 0) return "map";
  const economia = forecast.annual_savings.registered_economia_cents;
  const rateOk = economia * 10_000 >= income * SAVINGS_MIN_BPS;
  const reserveOk = (summary?.reserve_months ?? 0) >= RESERVE_MIN_MONTHS;
  return rateOk && reserveOk ? "operate" : "calibrate";
}
