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
});
