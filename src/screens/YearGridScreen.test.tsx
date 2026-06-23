import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import {
  ANNUAL_METRICS,
  FORECAST,
  MONTH_GRID,
  mockCommands,
  mockInvoke,
} from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("YearGridScreen (Calendário)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the calendar with the saldo thermometer legend", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_annual_metrics: ANNUAL_METRICS,
    });
    render(<YearGridScreen />);
    expect(await screen.findByText("Folga")).toBeInTheDocument();
  });

  it("uses month-grid balances for past days in the displayed month", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_annual_metrics: ANNUAL_METRICS,
    });
    render(<YearGridScreen />);

    expect(await screen.findByText("R$ 9,1 mil")).toBeInTheDocument();
  });
});
