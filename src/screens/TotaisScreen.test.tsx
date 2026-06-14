import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import {
  TotaisScreen,
  currentMonthMetric,
  performanceStatus,
  economizadoStatus,
  custoVidaStatus,
} from "./TotaisScreen";
import type { MonthMetric } from "../lib/api";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

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
    mockCommands({ get_forecast: FORECAST });
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
});
