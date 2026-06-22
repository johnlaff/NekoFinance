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
  fixed_out_cents: cost,
  daily_out_cents: 0,
  real_daily_avg_cents: 0,
  economia_cents: 0,
  savings_rate_bps: 0,
});

const ANNUAL: AnnualMetrics = {
  year: 2026,
  months: Array.from({ length: 12 }, (_, i) =>
    mk(i + 1, i + 1 === 3 ? 450000 : 0, i + 1 === 3 ? 250000 : 0),
  ),
};

describe("AnnualScreen", () => {
  it("renderiza a tabela anual (redesign) com as colunas e 12 meses", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_annual_metrics: ANNUAL });
    render(<AnnualScreen />);

    // Espera a tabela carregar — cabeçalho único "Saldo fim" como âncora.
    await waitFor(() => expect(screen.getByText("Saldo fim")).toBeInTheDocument());
    // Cabeçalhos do redesign — "Saída total" aparece no KPI e no cabeçalho, por isso getAllByText.
    expect(screen.getAllByText("Saída total").length).toBeGreaterThan(0);
    expect(screen.getByText("Resultado")).toBeInTheDocument();
    expect(screen.getByText("Diário")).toBeInTheDocument();
    expect(screen.getByText("Economia")).toBeInTheDocument();
    expect(screen.getByText("Entradas")).toBeInTheDocument();
    // 12 meses (linhas do corpo).
    expect(screen.getByText("Janeiro")).toBeInTheDocument();
    expect(screen.getByText("Março")).toBeInTheDocument();
    expect(screen.getByText("Dezembro")).toBeInTheDocument();
    // Rodapé "Realizado" (antigo "Total").
    expect(screen.getByText("Realizado")).toBeInTheDocument();
  });

  it("linha TOTAL: Economizado% anual é ΣEconomia/ΣEntradas (ponderado, não média das taxas)", async () => {
    mockInvoke.mockReset();
    const base = (month: number): MonthMetric => ({
      year: 2026,
      month,
      income_cents: 0,
      performance_cents: 0,
      cost_of_living_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
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

    // Aguarda o rodapé "Realizado" aparecer (substitui o antigo "Total").
    await waitFor(() => expect(screen.getByText("Realizado")).toBeInTheDocument());
    // Economizado% ponderado no tfoot: ΣEconomia/ΣEntradas = 60k/400k = 15%.
    // Pode aparecer também no KPI card, então verificamos que existe ao menos uma ocorrência.
    expect(screen.getAllByText("15%").length).toBeGreaterThan(0);
    // Não é a média simples de 30%+10%=20%.
    expect(screen.queryByText("20%")).not.toBeInTheDocument();
  });
});
