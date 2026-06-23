import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { TotaisScreen } from "./TotaisScreen";
import {
  currentMonthMetric,
  performanceStatus,
  economizadoStatus,
  custoVidaStatus,
  SAVINGS_MIN_BPS,
} from "./totaisStatus";
import type { MonthMetric } from "../lib/api";
import {
  ANNUAL_METRICS,
  FORECAST,
  OWNER_TOTALS,
  mockCommands,
  mockInvoke,
} from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("TotaisScreen — regras do método (puro)", () => {
  it("Performance: >=0 Sobrou, <0 Faltou", () => {
    expect(performanceStatus(100).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(0).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(-1).label).toBe("Faltou dinheiro");
    expect(performanceStatus(-1).level).toBe("risk");
  });
  it("Economizado: >=20% (2000bps) dentro do ideal", () => {
    expect(economizadoStatus(2500).label).toBe("Dentro do ideal");
    expect(economizadoStatus(2000).label).toBe("Dentro do ideal");
    expect(economizadoStatus(1900).label).toBe("Abaixo do ideal");
  });
  it("SAVINGS_MIN_BPS é a constante canônica de 20% (compartilhada entre as telas)", () => {
    // Guarda o piso canônico: AnnualScreen e colchaoPhase importam esta mesma constante,
    // então um rename/mudança de valor falha aqui em vez de silenciosamente divergir.
    expect(SAVINGS_MIN_BPS).toBe(2000);
    // Confirma que o badge mensal usa exatamente este limiar.
    expect(economizadoStatus(SAVINGS_MIN_BPS).label).toBe("Dentro do ideal");
    expect(economizadoStatus(SAVINGS_MIN_BPS - 1).label).toBe("Abaixo do ideal");
  });
  it("Custo de vida: custo<=renda dentro da renda", () => {
    expect(custoVidaStatus(500, 1000).label).toBe("Dentro da renda");
    expect(custoVidaStatus(1200, 1000).label).toBe("Acima da renda");
  });
  it("currentMonthMetric acha o mês do `today`", () => {
    const months = [
      { year: 2026, month: 5 },
      { year: 2026, month: 6 },
    ] as MonthMetric[];
    expect(currentMonthMetric(months, "2026-06-13")?.month).toBe(6);
    expect(currentMonthMetric(months, "2026-07-01")).toBeNull();
  });
});

describe("TotaisScreen (render)", () => {
  it("mostra as 4 métricas-herói e o status do mês corrente", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Performance")).toBeInTheDocument();
    });
    expect(screen.getByText("Custo de vida")).toBeInTheDocument();
    expect(screen.getByText("Economizado")).toBeInTheDocument();
    expect(screen.getByText("Diário médio")).toBeInTheDocument();
    // Status do método aparece (performance positiva no mock → "Sobrou dinheiro").
    expect(screen.getByText("Sobrou dinheiro")).toBeInTheDocument();
  });

  it("seletor de mês: avança para o próximo mês e volta ao hoje", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
    render(<TotaisScreen />);
    await waitFor(() => expect(screen.getByText("Performance")).toBeInTheDocument());
    // Começa no mês corrente (junho) → sem botão "Hoje".
    expect(screen.queryByRole("button", { name: "Hoje" })).not.toBeInTheDocument();
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Próximo mês" }));
    expect(screen.getByText(/Julho de 2026/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hoje" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Hoje" }));
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();
  });

  it("mostra totais por titular quando há 2+ titulares", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      owner_totals_for_month_cmd: OWNER_TOTALS,
    });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByRole("region", { name: "Por titular" })).toBeInTheDocument();
    });
    expect(screen.getByText("Titular A")).toBeInTheDocument();
    expect(screen.getByText("Titular B")).toBeInTheDocument();
    // Os valores (R$ 3.200,00 e R$ 1.800,00) aparecem como Money.
    expect(screen.getByText(/3\.200,00/)).toBeInTheDocument();
    expect(screen.getByText(/1\.800,00/)).toBeInTheDocument();
  });

  it("não mostra a seção por titular quando não há split (lista vazia)", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Performance")).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("region", { name: "Por titular" }),
    ).not.toBeInTheDocument();
  });

  it("Economizado mostra badge de status (Dentro do ideal quando >= 20%)", async () => {
    mockInvoke.mockReset();
    // FORECAST tem savings_rate_bps: 2500 em junho → "Dentro do ideal".
    mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Economizado")).toBeInTheDocument();
    });
    expect(screen.getByText("Dentro do ideal")).toBeInTheDocument();
  });

  it("trend inclui meses realizados anteriores vindos do annual metrics", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    const trend = await screen.findByRole("region", {
      name: "Resultado nos últimos meses",
    });
    expect(within(trend).getByText("Mai")).toBeInTheDocument();
    expect(within(trend).getByText("Jun")).toBeInTheDocument();
  });

  it("Custo de vida sublabel menciona cartão", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Custo de vida")).toBeInTheDocument();
    });
    // O sublabel da métrica passa a mencionar "incl. cartão", alinhado ao hint do rodapé —
    // por isso há 2+ ocorrências (sublabel + rodapé). A presença do sublabel é o que importa.
    expect(
      screen.getByText("= Saída Total (saídas incl. cartão + diário)"),
    ).toBeInTheDocument();
  });
});
