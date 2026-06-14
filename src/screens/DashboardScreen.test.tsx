import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import {
  DEFICIT_FORECAST,
  EMPTY_POCKETS,
  FORECAST,
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
    mockCommands({ get_dashboard_summary: SUMMARY, get_forecast: FORECAST });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.getByText("R$ 350,00")).toBeInTheDocument();
  });

  it("renders the daily projection table with today marked and dashes for zero flows", async () => {
    mockCommands({ get_dashboard_summary: SUMMARY, get_forecast: FORECAST });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Previsão diária — junho/)).toBeInTheDocument();
    });

    // "hoje" aparece no sufixo do herói e como marcador na tabela diária.
    expect(screen.getAllByText("hoje").length).toBeGreaterThanOrEqual(1);
    // Income day shows the inflow and the new chained balance.
    expect(screen.getByText("R$ 7.000,00")).toBeInTheDocument();
    const balances = screen.getAllByText("R$ 12.877,00");
    expect(balances.length).toBeGreaterThanOrEqual(2); // 25th and 30th carry the same saldo
    // Zero flows render as em-dashes, not R$ 0,00.
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });

  it("shows the deficit warning only when the projection goes negative", async () => {
    mockCommands({ get_dashboard_summary: SUMMARY, get_forecast: FORECAST });
    const { unmount } = render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.queryByText(/Buraco previsto/)).not.toBeInTheDocument();
    unmount();

    mockCommands({ get_dashboard_summary: SUMMARY, get_forecast: DEFICIT_FORECAST });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/Buraco previsto/)).toBeInTheDocument();
    });
    // Appears in the warning AND in the table's negative saldo row.
    expect(screen.getAllByText("-R$ 420,00")).toHaveLength(2);
    expect(screen.getByText("28/06")).toBeInTheDocument();
  });

  it("shows liquidity-grouped pockets and the net worth (spec 007)", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
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

  it("uses the forecast month in the hero tile sublabel", async () => {
    mockCommands({ get_dashboard_summary: SUMMARY, get_forecast: FORECAST });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Fim de junho")).toBeInTheDocument();
    });
  });

  it('credit tile shows "—" / "Sem cartão rastreado" when no credit is tracked', async () => {
    mockCommands({
      get_dashboard_summary: { ...SUMMARY, has_credit: false },
      get_forecast: FORECAST,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Sem cartão rastreado")).toBeInTheDocument();
    });
    expect(screen.getByText("Crédito no mês")).toBeInTheDocument();
  });
});
