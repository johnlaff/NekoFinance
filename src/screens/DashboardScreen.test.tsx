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

  it("modo cartão: o check-in re-roteia para as faturas e o Diário zerado não finge régua", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 4033,
        daily_ceiling_source: "chosen",
        daily_spend_today: 0,
        spending_mode: "card",
        card_gate: "below",
        cartao_month_cents: 260000,
        next_fatura_date: "2026-06-20",
        next_fatura_amount_cents: 140000,
      },
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });
    renderHoje();

    expect(await screen.findByText("Cartão do mês")).toBeInTheDocument();
    expect(screen.getByText("Modo cartão")).toBeInTheDocument();
    // Gate do método: a economia abaixo do piso aparece com palavra, não só cor.
    expect(screen.getByText("Economia abaixo do piso")).toBeInTheDocument();
    // A régua de Diário (progresso gasto/teto) sai de cena no modo cartão.
    expect(screen.queryByText("Diário de hoje")).not.toBeInTheDocument();
    // Próxima fatura com data + teto estipulado como referência.
    expect(screen.getByText(/Próxima fatura:/)).toBeInTheDocument();
    expect(screen.getByText(/Teto estipulado de/)).toBeInTheDocument();
  });

  it("teto estimado: número com selo de estimativa, nunca veredito silencioso", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 10000,
        daily_ceiling_source: "estimate",
      },
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });
    renderHoje();

    expect(await screen.findByText("Pode gastar hoje")).toBeInTheDocument();
    // Selo no herói e no check-in (2 ocorrências).
    expect(screen.getAllByText("Estimativa").length).toBeGreaterThanOrEqual(1);
  });

  it("sem teto: travessão + CTA que leva à cerimônia (nunca R$ 0,00 fabricado)", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 0,
        daily_ceiling_source: "none",
        ceiling_proposal_pending: true,
      },
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem registro")).toBeInTheDocument();
    // Com proposta pendente o convite é ÚNICO (revisar a proposta) — sem "Estipular" duplicado.
    expect(screen.queryByRole("button", { name: "Estipular" })).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Proposta da planilha aguardando — revisar" }),
    );
    expect(app.navigate).toHaveBeenCalledWith("teto");
  });

  it("reserva zerada: palavra dedicada em vez de alarme numérico", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_months: 0,
        reserve_state: "zero",
      },
      get_forecast: FORECAST,
      get_upcoming_bills: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem reserva")).toBeInTheDocument();
    expect(screen.queryByText(/0,0 meses/)).not.toBeInTheDocument();
  });
});
