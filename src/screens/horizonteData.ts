import type { ForecastDay } from "../lib/api";
import { monthNamePtBR } from "../lib/format";

export interface DayCell {
  day: number;
  balance: number;
  isToday: boolean;
}

export interface MonthCol {
  ym: string;
  label: string;
  days: DayCell[];
}

/** Agrupa a série diária do forecast por ano-mês (uma coluna por mês). */
export function groupByMonth(daily: ForecastDay[], today: string): MonthCol[] {
  const cols: MonthCol[] = [];
  const byYm = new Map<string, MonthCol>();
  for (const d of daily) {
    const ym = d.date.slice(0, 7);
    let col = byYm.get(ym);
    if (!col) {
      const label = monthNamePtBR(`${ym}-01`);
      col = { ym, label: label.charAt(0).toUpperCase() + label.slice(1), days: [] };
      byYm.set(ym, col);
      cols.push(col);
    }
    col.days.push({
      day: Number(d.date.slice(8, 10)),
      balance: d.balance_cents,
      isToday: d.date === today,
    });
  }
  return cols;
}
