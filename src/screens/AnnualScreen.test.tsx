import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { AnnualScreen } from "./AnnualScreen";
import type { AnnualMetrics, MonthMetric } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mk = (month: number, perf: number, cost: number): MonthMetric => ({
  year: 2026,
  month,
  income_cents: perf + cost,
  performance_cents: perf,
  cost_of_living_cents: cost,
  real_daily_avg_cents: 0,
  economia_cents: 0,
  savings_rate_bps: month === 3 ? 2200 : 0,
});

const ANNUAL: AnnualMetrics = {
  year: 2026,
  months: Array.from({ length: 12 }, (_, i) =>
    mk(i + 1, i + 1 === 3 ? 445700 : 0, i + 1 === 3 ? 254300 : 0),
  ),
};

describe("AnnualScreen", () => {
  it("renderiza a tabela anual das 4 métricas por mês", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_annual_metrics: ANNUAL });
    render(<AnnualScreen />);

    await waitFor(() => expect(screen.getByText("Visão anual")).toBeInTheDocument());
    // Cabeçalhos das 4 métricas.
    expect(screen.getByText("Performance")).toBeInTheDocument();
    expect(screen.getByText("Custo de vida")).toBeInTheDocument();
    expect(screen.getByText("Economizado")).toBeInTheDocument();
    expect(screen.getByText("Diário médio")).toBeInTheDocument();
    // 12 meses (linhas do corpo).
    expect(screen.getByText("Jan")).toBeInTheDocument();
    expect(screen.getByText("Mar")).toBeInTheDocument();
    expect(screen.getByText("Dez")).toBeInTheDocument();
    // O Economizado de março (22%).
    expect(screen.getByText("22%")).toBeInTheDocument();
  });

  it("linha TOTAL: Economizado% anual é ΣEconomia/ΣEntradas (ponderado, não média das taxas)", async () => {
    mockInvoke.mockReset();
    const base = (month: number): MonthMetric => ({
      year: 2026,
      month,
      income_cents: 0,
      performance_cents: 0,
      cost_of_living_cents: 0,
      real_daily_avg_cents: 0,
      economia_cents: 0,
      savings_rate_bps: 0,
    });
    const months = Array.from({ length: 12 }, (_, i) => base(i + 1));
    // Jan: 30% (30k/100k). Fev: 10% (30k/300k). Média simples = 20%, mas o anual ponderado
    // = 60k/400k = 15%.
    months[0] = {
      ...base(1),
      income_cents: 100_000,
      economia_cents: 30_000,
      savings_rate_bps: 3000,
    };
    months[1] = {
      ...base(2),
      income_cents: 300_000,
      economia_cents: 30_000,
      savings_rate_bps: 1000,
    };
    mockCommands({ get_annual_metrics: { year: 2026, months } });
    render(<AnnualScreen />);

    await waitFor(() => expect(screen.getByText("Total")).toBeInTheDocument());
    expect(screen.getByText("15%")).toBeInTheDocument(); // anual ponderado
    expect(screen.queryByText("20%")).not.toBeInTheDocument(); // não é a média simples
  });
});
