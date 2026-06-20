import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import {
  DEFICIT_FORECAST,
  EMPTY_POCKETS,
  FORECAST,
  MONTH_GRID,
  POCKETS,
  SUMMARY,
  mockCommands,
  mockInvoke,
} from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("DashboardScreen (forecast view)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("shows the safe-to-spend callout", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.getByText("R$ 350,00")).toBeInTheDocument();
  });

  it("renders the daily month grid with today marked and dashes for zero flows", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    // Espera o CORPO da grade carregar (o título do card aparece antes dos dados).
    await waitFor(() => {
      expect(screen.getAllByText("R$ 12.877,00").length).toBeGreaterThanOrEqual(2);
    });
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();

    // "hoje" aparece no sufixo do herói e como marcador na grade do mês.
    expect(screen.getAllByText("hoje").length).toBeGreaterThanOrEqual(1);
    // Income day shows the inflow (também aparece no total do rodapé).
    expect(screen.getAllByText("R$ 7.000,00").length).toBeGreaterThanOrEqual(1);
    // Zero flows render as em-dashes, not R$ 0,00.
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });

  it("shows the deficit warning only when the projection goes negative", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    const { unmount } = render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.queryByText(/Buraco previsto/)).not.toBeInTheDocument();
    unmount();

    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: DEFICIT_FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/Buraco previsto/)).toBeInTheDocument();
    });
    // O buraco aparece ao menos no alerta e no "pode faltar" do herói (a grade do mês é
    // testada à parte). Money usa o minus real (U+2212).
    expect(screen.getAllByText("−R$ 420,00").length).toBeGreaterThanOrEqual(2);
  });

  it("shows liquidity-grouped pockets and the net worth (spec 007)", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: POCKETS,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Caixa")).toBeInTheDocument();
    });
    expect(screen.getByText("Bolsos & patrimônio")).toBeInTheDocument();
    expect(screen.getByText("R$ 15.000,00")).toBeInTheDocument(); // reserva
    expect(screen.getByText("R$ 420,00")).toBeInTheDocument(); // vale
    expect(screen.getByText("R$ 12.000,00")).toBeInTheDocument(); // ilíquido
    expect(screen.getByText("R$ 35.420,00")).toBeInTheDocument(); // patrimônio
  });

  it("hints at Ajustes when no pocket exists yet", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: EMPTY_POCKETS,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Nenhum bolso cadastrado/)).toBeInTheDocument();
    });
  });

  it("shows an explicit error in the pockets card instead of the empty state", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: new Error("db locked"),
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(
        screen.getByText(/Não foi possível carregar os bolsos/),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText(/Nenhum bolso cadastrado/)).not.toBeInTheDocument();
  });

  it("names the projected month in the hero forecast head", async () => {
    // O metric tile redundante ("Saldo projetado · Fim de junho") foi removido; o mês projetado
    // continua nomeado no cabeçalho do herói ("Saldo no fim de junho").
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/Saldo no fim de junho/)).toBeInTheDocument();
    });
  });

  it("does not render the redundant 4-tile metric bar", async () => {
    // Os números desses tiles já vivem no herói (saldo projetado, pode-gastar) e nos cards abaixo.
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    const { container } = render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(container.querySelector(".dash-grid4")).toBeNull();
    expect(screen.queryByRole("article", { name: "Saldo projetado" })).toBeNull();
    expect(screen.queryByRole("article", { name: "Crédito no mês" })).toBeNull();
  });
});
