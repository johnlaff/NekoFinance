import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { HorizonteScreen, saldoBand, groupByMonth } from "./HorizonteScreen";
import type { ForecastDay } from "../lib/api";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("saldoBand — faixas RELATIVAS à escala do usuário", () => {
  const BASE = 300_000; // gasto mensal típico: R$3.000
  it.each([
    [-300_001, "critical"], // < -1 mês de gasto no vermelho
    [-300_000, "negative"],
    [-1, "negative"],
    [0, "tight"],
    [299_999, "tight"], // < 1 mês de folga
    [300_000, "ok"],
    [599_999, "ok"], // < 2 meses
    [600_000, "comfortable"], // >= 2 meses
  ] as const)("saldo %d (base 300k) → %s", (cents, band) => {
    expect(saldoBand(cents, BASE)).toBe(band);
  });

  it("é relativo à escala: o mesmo saldo muda de faixa conforme o baseline", () => {
    // R$1.500 é folga (ok) p/ quem gasta 1k/mês, mas apertado p/ quem gasta 3k/mês.
    expect(saldoBand(150_000, 100_000)).toBe("ok");
    expect(saldoBand(150_000, 300_000)).toBe("tight");
  });

  it("sem baseline (usuário novo) classifica só pelo sinal", () => {
    expect(saldoBand(-1, 0)).toBe("negative");
    expect(saldoBand(0, 0)).toBe("ok");
    expect(saldoBand(999_999, 0)).toBe("ok");
  });
});

describe("groupByMonth", () => {
  it("agrupa a série diária em colunas por mês", () => {
    const daily = [
      { date: "2026-06-29", balance_cents: 100 },
      { date: "2026-06-30", balance_cents: 200 },
      { date: "2026-07-01", balance_cents: 300 },
    ] as ForecastDay[];
    const cols = groupByMonth(daily, "2026-06-29");
    expect(cols.map((c) => c.ym)).toEqual(["2026-06", "2026-07"]);
    expect(cols[0]!.days).toHaveLength(2);
    expect(cols[0]!.days[0]).toMatchObject({ day: 29, balance: 100, isToday: true });
    expect(cols[1]!.days[0]).toMatchObject({ day: 1, balance: 300 });
  });
});

describe("HorizonteScreen (render)", () => {
  it("renderiza o heatmap com a coluna do mês", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST });
    render(<HorizonteScreen />);
    await waitFor(() => {
      expect(screen.getByText("Horizonte de saldos")).toBeInTheDocument();
    });
    // FORECAST.daily está em junho/2026 → coluna "Junho".
    expect(screen.getByText("Junho")).toBeInTheDocument();
  });
});
