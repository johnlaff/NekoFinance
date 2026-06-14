import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import {
  HorizonteScreen,
  saldoBand,
  groupByMonth,
} from "./HorizonteScreen";
import type { ForecastDay } from "../lib/api";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("saldoBand — faixas do heatmap (fronteiras)", () => {
  it.each([
    [-50001, "critical"],
    [-50000, "negative"],
    [-1, "negative"],
    [0, "tight"],
    [99999, "tight"],
    [100000, "ok"],
    [199999, "ok"],
    [200000, "comfortable"],
  ] as const)("saldo %d → %s", (cents, band) => {
    expect(saldoBand(cents)).toBe(band);
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
