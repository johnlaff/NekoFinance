import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

/** get_month_grid por mês: janeiro tem saldo realizado próprio, junho usa o fixture padrão. */
const monthGridByMonth = (args?: Record<string, unknown>) => {
  const month = Number(args?.["month"]);
  if (month === 1)
    return [
      {
        date: "2026-01-15",
        day: 15,
        income_cents: 0,
        fixed_out_cents: 0,
        daily_out_cents: 0,
        balance_cents: 123400,
      },
    ];
  if (month === 6) return MONTH_GRID;
  return [];
};

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

  it("Ano inteiro: célula de mês passado recebe o saldo realizado do grid daquele mês", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: monthGridByMonth,
      get_annual_metrics: ANNUAL_METRICS,
    });
    render(<YearGridScreen />);
    await screen.findByText("Folga");

    await user.click(screen.getByRole("radio", { name: "Ano inteiro" }));

    // 15/01 (mês passado ≠ mês corrente) deixa de ser "—" e mostra o saldo real.
    const janCell = await screen.findByTitle(
      (title: string) => title.startsWith("15/01") && title.includes("1.234,00"),
    );
    expect(janCell).toBeInTheDocument();
  });
});
