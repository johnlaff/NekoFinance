import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import { NekoAppProvider } from "../shell/appContext";
import { FORECAST, SUMMARY, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderHoje() {
  return render(
    <NekoAppProvider value={app}>
      <DashboardScreen />
    </NekoAppProvider>,
  );
}

describe("DashboardScreen (Hoje)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the can-spend hero, check-in and upcoming bills", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });
    renderHoje();
    expect(await screen.findByText("Pode gastar hoje")).toBeInTheDocument();
    expect(screen.getByText("Check-in de hoje")).toBeInTheDocument();
    expect(screen.getByText("A pagar em breve")).toBeInTheDocument();
  });

  it("registers Cartão check-in with the engine payment method literal", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills: [],
      create_transaction: "txn-checkin-cartao",
    });

    renderHoje();
    await screen.findByText("Pode gastar hoje");

    await user.click(screen.getByRole("radio", { name: "Cartão" }));
    await user.type(screen.getByLabelText("Valor"), "75,90");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "create_transaction",
        expect.objectContaining({
          txnType: "expense",
          amountCents: 7590,
          date: FORECAST.today,
          paymentMethod: "credit",
          isFixed: false,
        }),
      );
    });
  });

  it("registers Saída check-in with the canonical debit payment method (kindToFields)", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills: [],
      create_transaction: "txn-checkin-saida",
    });

    renderHoje();
    await screen.findByText("Pode gastar hoje");

    await user.click(screen.getByRole("radio", { name: "Saída" }));
    await user.type(screen.getByLabelText("Valor"), "300,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "create_transaction",
        expect.objectContaining({
          txnType: "expense",
          amountCents: 30000,
          date: FORECAST.today,
          paymentMethod: "debit",
          isFixed: true,
        }),
      );
    });
  });

  it("mostra estado de erro com retry quando o fetch falha — nunca R$ 0,00 como dado real", async () => {
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: new Error("db offline"),
      get_upcoming_bills: [],
    });

    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
    // Sem dados reais, o herói e qualquer "R$ 0,00" fabricado não podem aparecer.
    expect(screen.queryByText("Pode gastar hoje")).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });

  it("estado de erro também quando SÓ uma das fontes falha no primeiro load (sem zeros fabricados)", async () => {
    // Com summary falhando e forecast OK, a tela renderizava o
    // herói com teto/saldo/reserva = 0 e um banner mentindo "últimos dados carregados".
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });

    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Pode gastar hoje")).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });
});
