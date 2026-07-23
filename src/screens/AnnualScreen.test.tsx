import { render, screen, waitFor, within, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AnnualScreen } from "./AnnualScreen";
import type { AnnualMetrics, MonthMetric } from "../lib/api";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mk = (month: number, perf: number, cost: number): MonthMetric => ({
  year: 2026,
  month,
  income_cents: perf + cost,
  income_performance_cents: perf + cost,
  performance_cents: perf,
  cost_of_living_cents: cost,
  fixed_out_cents: cost,
  daily_out_cents: 0,
  daily_avg_out_cents: 0,
  daily_projected_cents: 0,
  cartao_cents: 0,
  real_daily_avg_cents: 0,
  economia_cents: 0,
  patrimonio_cents: 0,
  savings_rate_bps: 0,
});

const ANNUAL: AnnualMetrics = {
  year: 2026,
  months: Array.from({ length: 12 }, (_, i) =>
    mk(i + 1, i + 1 === 3 ? 450000 : 0, i + 1 === 3 ? 250000 : 0),
  ),
};

const monthGridHandler = (args?: Record<string, unknown>) => {
  const month = Number(args?.["month"]);
  const mm = String(month).padStart(2, "0");
  return [
    {
      date: `2026-${mm}-28`,
      day: 28,
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      daily_projected_cents: 0,
      balance_cents: month === 1 ? 111000 : month === 5 ? 555000 : null,
    },
  ];
};

describe("AnnualScreen", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renderiza a tabela anual (redesign) com as colunas e 12 meses", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_annual_metrics: ANNUAL,
      get_forecast: FORECAST,
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    // Espera a tabela carregar — cabeçalho único "Saldo fim" como âncora.
    await waitFor(() => expect(screen.getByText("Saldo fim")).toBeInTheDocument());
    // Cabeçalhos do redesign — "Custo de vida" aparece no KPI e no cabeçalho, por isso getAllByText.
    expect(screen.getAllByText("Custo de vida").length).toBeGreaterThan(0);
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
      income_performance_cents: 0,
      performance_cents: 0,
      cost_of_living_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      daily_avg_out_cents: 0,
      daily_projected_cents: 0,
      cartao_cents: 0,
      real_daily_avg_cents: 0,
      economia_cents: 0,
      patrimonio_cents: 0,
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
    mockCommands({
      get_annual_metrics: { year: 2026, months },
      get_forecast: FORECAST,
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    // Aguarda o rodapé "Realizado" aparecer (substitui o antigo "Total").
    await waitFor(() => expect(screen.getByText("Realizado")).toBeInTheDocument());
    // Economizado% ponderado no tfoot: ΣEconomia/ΣEntradas = 60k/400k = 15%.
    // Pode aparecer também no KPI card, então verificamos que existe ao menos uma ocorrência.
    expect(screen.getAllByText("15%").length).toBeGreaterThan(0);
    // Não é a média simples de 30%+10%=20%.
    expect(screen.queryByText("20%")).not.toBeInTheDocument();
  });

  it("Comparar anos: mostra Economizado% por mês e resumo ponderado (absoluto maior ≠ taxa maior)", async () => {
    mockInvoke.mockReset();
    const base = (year: number, month: number): MonthMetric => ({
      year,
      month,
      income_cents: 0,
      income_performance_cents: 0,
      performance_cents: 0,
      cost_of_living_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      daily_avg_out_cents: 0,
      daily_projected_cents: 0,
      cartao_cents: 0,
      real_daily_avg_cents: 0,
      economia_cents: 0,
      patrimonio_cents: 0,
      savings_rate_bps: 0,
    });
    // yearA=2025: Jan 30k/100k = 30%. yearB=2026: Jan 40k/400k = 10%.
    // Economia absoluta MAIOR em B (40k>30k), mas a TAXA é MENOR (10%<30%) —
    // exatamente o que a feature existe para expor. Dado só em janeiro (mês
    // decorrido no ano corrente, cutoff = junho no relógio congelado).
    const yearMonths = (
      year: number,
      income: number,
      economia: number,
      bps: number,
    ) => {
      const arr = Array.from({ length: 12 }, (_, i) => base(year, i + 1));
      arr[0] = {
        ...base(year, 1),
        income_cents: income,
        economia_cents: economia,
        savings_rate_bps: bps,
      };
      return arr;
    };
    mockCommands({
      get_annual_metrics: (args?: Record<string, unknown>) => ({
        year: Number(args?.["year"]),
        months:
          Number(args?.["year"]) === 2025
            ? yearMonths(2025, 100_000, 30_000, 3000)
            : yearMonths(2026, 400_000, 40_000, 1000),
      }),
      get_forecast: FORECAST,
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    // Troca para a aba "Comparar anos".
    await waitFor(() =>
      expect(screen.getByRole("radio", { name: "Comparar anos" })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("radio", { name: "Comparar anos" }));

    // Taxa por mês: 30% (2025) e 10% (2026) presentes; a média simples 20% NÃO aparece.
    await waitFor(() => expect(screen.getAllByText("30%").length).toBeGreaterThan(0));
    expect(screen.getAllByText("10%").length).toBeGreaterThan(0);
    expect(screen.queryByText("20%")).not.toBeInTheDocument();

    // Resumo por ano: Entradas + Economizado% ponderado do ano corrente (só Jan).
    expect(screen.getByText(/Economizado 30%/)).toBeInTheDocument();
    expect(screen.getByText(/Economizado 10%/)).toBeInTheDocument();
  });

  it("KPI Performance acum. soma performance_cents do motor (mês com economia > 0)", async () => {
    mockInvoke.mockReset();
    const months = Array.from({ length: 12 }, (_, i) => mk(i + 1, 0, 0));
    // Março: Entradas 7.000, Saída total 2.500, Economia 2.000 → Performance do motor = 2.500.
    // A re-derivação local (Entradas − Saída total = 4.500) ignoraria a Economia.
    months[2] = {
      ...mk(3, 250_000, 250_000),
      income_cents: 700_000,
      economia_cents: 200_000,
    };
    mockCommands({
      get_annual_metrics: { year: 2026, months },
      get_forecast: FORECAST,
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    await waitFor(() =>
      expect(screen.getByText("Performance acum.")).toBeInTheDocument(),
    );
    const kpi = screen.getByText("Performance acum.").closest(".ano-kpi")!;
    expect(within(kpi as HTMLElement).getByText("R$ 2,5 mil")).toBeInTheDocument();
    expect(
      within(kpi as HTMLElement).queryByText("R$ 4,5 mil"),
    ).not.toBeInTheDocument();
  });

  it("preenche Saldo fim de meses passados a partir do month-grid realizado", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_annual_metrics: ANNUAL,
      get_forecast: FORECAST,
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    await waitFor(() => expect(screen.getByText("Saldo fim")).toBeInTheDocument());
    expect(screen.getByText(/1\.110,00/)).toBeInTheDocument();
    expect(screen.getByText(/5\.550,00/)).toBeInTheDocument();
  });

  it("mostra Saldo fim histórico quando o ano exibido é anterior ao forecast", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_annual_metrics: ANNUAL,
      get_forecast: { ...FORECAST, today: "2027-06-10", month_end: [] },
      get_month_grid: monthGridHandler,
    });
    render(<AnnualScreen />);

    await waitFor(() => expect(screen.getByText("Saldo fim")).toBeInTheDocument());
    expect(screen.getByText(/1\.110,00/)).toBeInTheDocument();
    expect(screen.getByText(/5\.550,00/)).toBeInTheDocument();
  });
});
