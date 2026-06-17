import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";
import {
  EMPTY_FORECAST,
  EMPTY_SUMMARY,
  FORECAST,
  MONTH_GRID,
  SUMMARY,
  mockCommands,
  mockInvoke,
} from "./test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("App (dashboard)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders dashboard with loaded data", async () => {
    mockCommands({
      check_auth_status: "disconnected",
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });

    expect(screen.getAllByText(/8\.420/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Saldo projetado")).toBeInTheDocument();
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();
    // "Lançamentos" agora aparece na nav lateral E no stat do hero (nomenclatura unificada).
    expect(screen.getAllByText("Lançamentos").length).toBeGreaterThanOrEqual(1);
  });

  it("shows loading state", () => {
    mockInvoke.mockReturnValue(new Promise<unknown>(() => undefined));
    render(<App />);
    expect(screen.getByText("Neko")).toBeInTheDocument();
  });

  it("shows error state with retry button", async () => {
    mockCommands({
      check_auth_status: "disconnected",
      get_dashboard_summary: new Error("database not found"),
      get_forecast: FORECAST,
    });
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/Não foi possível concluir a ação/)).toBeInTheDocument();
    });
    expect(screen.getByText("Tentar novamente")).toBeInTheDocument();
  });

  it("shows empty state when no transactions", async () => {
    mockCommands({
      check_auth_status: "disconnected",
      get_dashboard_summary: EMPTY_SUMMARY,
      get_forecast: EMPTY_FORECAST,
      get_month_grid: [],
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Mês sem lançamentos")).toBeInTheDocument();
    });
  });
});
