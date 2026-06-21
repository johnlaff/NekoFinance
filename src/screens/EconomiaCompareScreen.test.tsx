import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { EconomiaCompareScreen } from "./EconomiaCompareScreen";
import type { AnnualMetrics, MonthMetric } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Anos que a tela mostra por padrão (base = ano atual − 1; direita = ano atual).
const THIS_YEAR = new Date().getFullYear();
const YEAR_A = THIS_YEAR - 1;
const YEAR_B = THIS_YEAR;

// Helper: MonthMetric com só os campos relevantes p/ Economia populados.
function mk(month: number, income: number, economia: number, bps: number): MonthMetric {
  return {
    year: YEAR_A,
    month,
    income_cents: income,
    performance_cents: income - economia,
    cost_of_living_cents: income - economia,
    fixed_out_cents: income - economia,
    daily_out_cents: 0,
    real_daily_avg_cents: 0,
    economia_cents: economia,
    savings_rate_bps: bps,
  };
}

const METRICS: AnnualMetrics = {
  year: YEAR_A,
  months: [
    mk(1, 500_000, 100_000, 2000),
    ...Array.from({ length: 11 }, (_, i) => mk(i + 2, 0, 0, 0)),
  ],
};

describe("EconomiaCompareScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders both year columns in the header", async () => {
    mockCommands({ get_annual_metrics: METRICS });
    render(<EconomiaCompareScreen />);
    await waitFor(() =>
      expect(screen.getByText(`Economia: ${YEAR_A} vs ${YEAR_B}`)).toBeInTheDocument(),
    );
    // Ambos os rótulos de ano aparecem nos cabeçalhos de coluna.
    expect(screen.getAllByText(String(YEAR_A)).length).toBeGreaterThan(0);
    expect(screen.getAllByText(String(YEAR_B)).length).toBeGreaterThan(0);
  });

  it("renders 12 month rows", async () => {
    mockCommands({ get_annual_metrics: METRICS });
    render(<EconomiaCompareScreen />);
    await waitFor(() => expect(screen.getByText("Jan")).toBeInTheDocument());
    expect(screen.getByText("Dez")).toBeInTheDocument();
    expect(screen.getAllByText("Jan").length).toBeGreaterThanOrEqual(1);
  });

  it("total Economizado% row is weighted (ΣEconomia/ΣEntradas), not average of monthly rates", async () => {
    // Jan: 30% (30k/100k), Fev: 10% (30k/300k). Média simples = 20%.
    // Ponderado correto = 60k/400k = 15%.
    const two: AnnualMetrics = {
      year: YEAR_A,
      months: [
        mk(1, 100_000, 30_000, 3000), // 30%
        mk(2, 300_000, 30_000, 1000), // 10%
        ...Array.from({ length: 10 }, (_, i) => mk(i + 3, 0, 0, 0)),
      ],
    };
    mockCommands({ get_annual_metrics: two });
    render(<EconomiaCompareScreen />);
    await waitFor(() => expect(screen.getAllByText("Total").length).toBeGreaterThan(0));
    expect(screen.getAllByText("15%").length).toBeGreaterThan(0); // ponderado
    expect(screen.queryAllByText("20%").length).toBe(0); // NÃO a média simples
  });

  it("shows empty state when no data at all", async () => {
    const empty: AnnualMetrics = {
      year: YEAR_A,
      months: Array.from({ length: 12 }, (_, i) => mk(i + 1, 0, 0, 0)),
    };
    mockCommands({ get_annual_metrics: empty });
    render(<EconomiaCompareScreen />);
    await waitFor(() =>
      expect(screen.getByText(/Sem dados de Economia/)).toBeInTheDocument(),
    );
  });
});
