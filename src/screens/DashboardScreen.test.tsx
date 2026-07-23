import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import { NekoAppProvider } from "../shell/appContext";
import type { UpcomingInvoice } from "../lib/api";
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

function invoiceFixture(overrides: Partial<UpcomingInvoice>): UpcomingInvoice {
  return {
    account_id: "card-1",
    card_name: "Cartão",
    due_date: "2026-06-20",
    amount_cents: 100_00,
    status: "aberta",
    owner_name: "Eu",
    has_refund_expectation: false,
    ...overrides,
  };
}

describe("DashboardScreen (Hoje)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    app.navigate.mockReset();
  });

  it("herói: saudação, veredito com o guardrail que morde e curadoria da Mia", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    const { container } = renderHoje();

    expect(await screen.findByText(/Pode gastar hoje/)).toBeInTheDocument();
    // O fixture prende no guardrail de poupança — a segunda linha nomeia ELE.
    expect(
      screen.getByText("Sem tocar na economia planejada do ano."),
    ).toBeInTheDocument();
    expect(screen.getByText(/A Mia separou o que importa hoje/)).toBeInTheDocument();
    // Primeiro adotante da coordenação large-title do shell.
    expect(container.querySelector("[data-large-title]")).not.toBeNull();
  });

  it("o registro inline morreu: o bloco do dia é leitura; registrar vive no shell", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    await screen.findByText(/Pode gastar hoje/);
    expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Registrar" })).not.toBeInTheDocument();
  });

  it("modo débito: check-in do teto com barra e valor do teto", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Diário de hoje")).toBeInTheDocument();
    // O número exibido é só o guardrail; o teto é apresentado como SEGUNDO limite,
    // nunca como componente do número (o motor não o inclui no min).
    expect(screen.getByText(/segundo limite do dia/)).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: /Diário de hoje em \d+% do teto/ }),
    ).toBeInTheDocument();
  });

  it("teto estimado: número com selo de estimativa, nunca veredito silencioso", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 10000,
        daily_ceiling_source: "estimate",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText(/Pode gastar hoje/)).toBeInTheDocument();
    expect(screen.getAllByText("Estimativa").length).toBeGreaterThanOrEqual(1);
  });

  it("sem teto + proposta pendente: convite único de revisão levando à cerimônia", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 0,
        daily_ceiling_source: "none",
        ceiling_proposal_pending: true,
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    const review = await screen.findByRole("button", {
      name: "A planilha propõe um teto — revisar.",
    });
    // Com proposta pendente o convite é ÚNICO na tela inteira: nenhum "estipular"
    // sobra (nem no herói, nem no bloco do dia).
    expect(screen.queryByText(/estipular/i)).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Proposta do teto aguardando — revisar" }),
    ).toBeInTheDocument();
    await user.click(review);
    expect(app.navigate).toHaveBeenCalledWith("teto");
  });

  it("modo cartão: faturas em aberto agrupadas por vencimento são o corpo do bloco", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        card_spend_today_cents: 0,
        upcoming_invoices: [
          invoiceFixture({
            account_id: "itau",
            card_name: "Itaú",
            amount_cents: 1_747_39,
            due_date: "2026-06-20",
          }),
          invoiceFixture({
            account_id: "amazon",
            card_name: "Amazon",
            amount_cents: 195_62,
            due_date: "2026-06-20",
          }),
          invoiceFixture({
            account_id: "gio",
            card_name: "Bradesco Gio",
            amount_cents: 987_70,
            due_date: "2026-06-22",
            owner_name: "Gio",
            has_refund_expectation: true,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [
        { id: "itau", name: "Itaú" },
        { id: "amazon", name: "Amazon" },
        { id: "gio", name: "Bradesco Gio" },
        { id: "inter", name: "Inter" },
        { id: "bb", name: "BB" },
      ],
    });
    renderHoje();

    // Cabeçalhos por vencimento, com concordância singular/plural.
    expect(await screen.findByText("Vencem em 20 de junho")).toBeInTheDocument();
    expect(screen.getByText("Vence em 22 de junho")).toBeInTheDocument();
    // Dentro do grupo, maior primeiro; a maior de todas leva o contexto de destaque.
    expect(screen.getByText("A maior fatura em aberto")).toBeInTheDocument();
    // Reembolso vinculado vira etiqueta de status; o dono aparece quando há mais de um.
    expect(screen.getByText("Reembolso")).toBeInTheDocument();
    expect(screen.getByText(/De Gio/)).toBeInTheDocument();
    // Cartão parado nunca some em silêncio.
    expect(
      screen.getByText(/Inter e BB estão sem fatura em aberto/),
    ).toBeInTheDocument();
    // A régua de Diário sai de cena no modo cartão.
    expect(screen.queryByText("Diário de hoje")).not.toBeInTheDocument();
    expect(screen.getByText(/nada somado à fatura hoje/)).toBeInTheDocument();
  });

  it("modo cartão sem fatura em aberto: recai no Cartão do mês, nunca lista vazia muda", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        cartao_month_cents: 260000,
        next_fatura_date: "2026-06-20",
        next_fatura_amount_cents: 140000,
        upcoming_invoices: [],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [
        { id: "inter", name: "Inter" },
        { id: "bb", name: "BB" },
      ],
    });
    renderHoje();

    expect(await screen.findByText("Cartão do mês")).toBeInTheDocument();
    expect(screen.getByText(/Próxima fatura:/)).toBeInTheDocument();
    expect(screen.getByText("Modo cartão")).toBeInTheDocument();
    // Cartão parado nunca some em silêncio — mesmo sem NENHUMA fatura em aberto.
    expect(
      screen.getByText(/Inter e BB estão sem fatura em aberto/),
    ).toBeInTheDocument();
  });

  it("insight da Mia: fechamento na faixa do termômetro + ponto mais apertado + entrada", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    const insight = await screen.findByLabelText("Leitura da Mia");
    expect(insight).toHaveTextContent(/junho termina em Folga/);
    expect(insight).toHaveTextContent(/O ponto mais apertado do mês é dia 15/);
    expect(insight).toHaveTextContent(/a próxima entrada chega dia 25/);
    expect(insight).toHaveTextContent(/Nenhum dia no vermelho à vista/);
  });

  it("próximos movimentos: contas e a próxima entrada, em ordem de data", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [
        {
          id: "bill-1",
          description: "Aulas de inglês",
          amount: 23200,
          due_date: "2026-06-30",
          is_projection: true,
        },
      ],
      list_cards: [],
    });
    renderHoje();

    const list = await screen.findByRole("list", { name: "Próximos movimentos" });
    const items = [...list.querySelectorAll("li")].map((li) => li.textContent ?? "");
    // Entrada de 25/06 vem antes da conta de 30/06.
    expect(items[0]).toContain("Entrada prevista");
    expect(items[1]).toContain("Aulas de inglês");
  });

  it("mostra estado de erro com retry quando o fetch falha — nunca R$ 0,00 como dado real", async () => {
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: new Error("db offline"),
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
    // Sem dados reais, o herói e qualquer "R$ 0,00" fabricado não podem aparecer.
    expect(screen.queryByText(/Pode gastar hoje/)).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });

  it("estado de erro também quando SÓ uma das fontes falha no primeiro load (sem zeros fabricados)", async () => {
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Pode gastar hoje/)).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });

  it("reserva zerada: palavra dedicada em vez de alarme numérico", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_months: 0,
        reserve_state: "zero",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem reserva")).toBeInTheDocument();
    expect(screen.queryByText(/0,0 meses/)).not.toBeInTheDocument();
  });

  it("reserva sem registro: travessão + CTA de mapear levando a Configurações", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_months: 0,
        reserve_state: "no_record",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem registro")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mapear" }));
    expect(app.navigate).toHaveBeenCalledWith("config");
  });
});
