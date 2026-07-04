import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import { NekoAppProvider } from "../shell/appContext";
import {
  ANNUAL_METRICS,
  FORECAST,
  MONTH_GRID,
  mockCommands,
  mockInvoke,
} from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderGrid() {
  return render(
    <NekoAppProvider value={app}>
      <YearGridScreen />
    </NekoAppProvider>,
  );
}

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

/** Junho com um dia passado (05) carregando movimentos, para os testes de tooltip/clique. */
const monthGridWithMovements = (args?: Record<string, unknown>) => {
  const month = Number(args?.["month"]);
  if (month === 6)
    return [
      {
        date: "2026-06-05",
        day: 5,
        income_cents: 50_000, // R$ 500,00
        fixed_out_cents: 20_000, // R$ 200,00
        daily_out_cents: 4_500, // R$ 45,00
        balance_cents: 900_000,
      },
    ];
  return [];
};

describe("YearGridScreen (Calendário)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"));
    mockInvoke.mockReset();
    app.navigate.mockReset();
    app.openCompose.mockReset();
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
    renderGrid();
    expect(await screen.findByText("Folga")).toBeInTheDocument();
  });

  it("uses month-grid balances for past days in the displayed month", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_annual_metrics: ANNUAL_METRICS,
    });
    renderGrid();

    expect(await screen.findByText("R$ 9,1 mil")).toBeInTheDocument();
  });

  it("Ano inteiro: célula de mês passado recebe o saldo realizado do grid daquele mês", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: monthGridByMonth,
      get_annual_metrics: ANNUAL_METRICS,
    });
    renderGrid();
    await screen.findByText("Folga");

    await user.click(screen.getByRole("radio", { name: "Ano inteiro" }));

    // 15/01 (mês passado ≠ mês corrente) deixa de ser "—" e mostra o saldo real.
    const janCell = await screen.findByTitle(
      (title: string) => title.startsWith("15/01") && title.includes("1.234,00"),
    );
    expect(janCell).toBeInTheDocument();
  });

  it("célula do dia expõe os movimentos no nome acessível (tooltip + aria)", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: monthGridWithMovements,
      get_annual_metrics: ANNUAL_METRICS,
    });
    renderGrid();

    // findByTitle aguarda o month-grid assíncrono assentar (padrão do arquivo).
    const cell = await screen.findByTitle(
      (t: string) => t.includes("Entrada") && t.includes("500,00"),
    );
    // A célula é um botão nativo e carrega os movimentos no nome acessível (aria).
    expect(cell.tagName).toBe("BUTTON");
    expect(cell).toHaveAccessibleName(/Saldo.*9\.000,00/);
    expect(cell).toHaveAccessibleName(/Entrada.*500,00/);
    expect(cell).toHaveAccessibleName(/Saída fixa.*200,00/);
    expect(cell).toHaveAccessibleName(/Diário.*45,00/);
  });

  it("clicar numa célula do calendário navega para o Livro-razão", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCommands({
      get_forecast: FORECAST,
      get_month_grid: monthGridWithMovements,
      get_annual_metrics: ANNUAL_METRICS,
    });
    renderGrid();

    const cell = await screen.findByTitle(
      (t: string) => t.includes("Entrada") && t.includes("500,00"),
    );
    await user.click(cell);
    expect(app.navigate).toHaveBeenCalledWith("lancamentos");
  });
});
