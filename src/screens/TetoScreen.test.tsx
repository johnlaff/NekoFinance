import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TetoScreen } from "./TetoScreen";
import { SUMMARY, mockCommands, mockInvoke } from "../test/commands";
import type { CeilingProposal, DailyBudget } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const NOTA_REAL = "Mensal  R$ 1250,00  Variável\nR$ 1250,00 / 31 Dias = R$ 40,33";

const EMPTY_BUDGET: DailyBudget = {
  per_day_cents: 0,
  divisor_days: null,
  ceremony_month: null,
  source_note: null,
  categories: [],
};

const CEREMONY_BUDGET: DailyBudget = {
  per_day_cents: 4033,
  divisor_days: 31,
  ceremony_month: "2025-09",
  source_note: NOTA_REAL,
  categories: [
    { id: "c1", name: "Alimentação", amount_cents: 100000, position: 0 },
    { id: "c2", name: "Transporte", amount_cents: 25000, position: 1 },
  ],
};

const PROPOSAL: CeilingProposal = {
  id: "cp-1",
  per_day_cents: 4033,
  divisor_days: 31,
  source_month: "2025-09",
  raw_note: NOTA_REAL,
  items: [
    { name: "Alimentação", amount_cents: 100000 },
    { name: "Transporte", amount_cents: 25000 },
  ],
};

function flat(el: Element | null): string {
  return (el?.textContent ?? "").replace(/\s+/g, " ");
}

describe("TetoScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("veredito escolhido no modo cartão: o teto, o recorte do modo e a prova do número", async () => {
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: { ...SUMMARY, spending_mode: "card" },
    });
    render(<TetoScreen />);

    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: /Seu teto é R\$\s?40,33 por dia/,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("Estipulado em setembro de 2025")).toBeInTheDocument();
    expect(screen.getByText(/O dia é medido pelas faturas/)).toBeInTheDocument();

    // A prova recalcula a partir dos itens: 125.000 ÷ 31 = 4.033 (resto para cima).
    const proof = screen.getByRole("region", { name: "A prova do número" });
    // O "i" entre o rótulo e o número é o marcador do popover didático do termo.
    expect(flat(proof)).toMatch(/Total do mês variáveliR\$\s?1\.250,00/);
    expect(flat(proof)).toMatch(/Dividido por31 dias/);
    expect(flat(proof)).toMatch(/Teto por diaR\$\s?40,33/);
  });

  it("a nota original da planilha é reproduzida atrás do disclosure", async () => {
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
    });
    render(<TetoScreen />);

    const summary = await screen.findByText("Ver a nota original da planilha");
    expect(summary).toBeInTheDocument();
    // O texto vai cru, com as quebras do dono preservadas.
    expect(screen.getByText(/R\$ 1250,00 \/ 31 Dias = R\$ 40,33/)).toBeInTheDocument();
  });

  it("sem nota da planilha, o disclosure não existe", async () => {
    mockCommands({
      get_daily_budget_cmd: { ...CEREMONY_BUDGET, source_note: null },
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
    });
    render(<TetoScreen />);

    await screen.findByRole("region", { name: "A prova do número" });
    expect(
      screen.queryByText("Ver a nota original da planilha"),
    ).not.toBeInTheDocument();
  });

  it("a idade da cerimônia convida a recalibrar quando passa da cadência do método", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-23T10:00:00"));
    try {
      mockCommands({
        get_daily_budget_cmd: CEREMONY_BUDGET,
        get_ceiling_proposal_cmd: null,
        get_dashboard_summary: SUMMARY,
      });
      render(<TetoScreen />);
      await vi.waitFor(() =>
        expect(screen.getByRole("region", { name: /cerimônia/i })).toBeInTheDocument(),
      );
      expect(flat(screen.getByRole("region", { name: /cerimônia/i }))).toMatch(
        /A cerimônia fez dez meses/,
      );
      expect(
        screen.getByRole("button", { name: "Recalibrar o teto" }),
      ).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o rito grava itens ÷ divisor com o arredondamento para cima", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Recalibrar o teto" }));

    // Batida 1: os itens gravados abrem o rito prontos para revisão.
    expect(screen.getByDisplayValue("Alimentação")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Valor mensal da categoria 1"));
    await user.type(screen.getByLabelText("Valor mensal da categoria 1"), "1.100,00");
    await user.click(screen.getByRole("button", { name: "Definir os dias" }));

    // Batida 2: o divisor.
    await user.clear(screen.getByLabelText("Divisor de dias"));
    await user.type(screen.getByLabelText("Divisor de dias"), "31");
    await user.click(screen.getByRole("button", { name: "Ver o teto novo" }));

    // Batida 3: o aceite (1.350,00 ÷ 31 = 43,55 — resto para cima) grava o número exibido.
    expect(
      screen.getByRole("group", {
        name: /Teto sai de R\$\s?40,33 para R\$\s?43,55 por dia, válido daqui para frente/,
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Usar este teto" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "upsert_daily_budget_with_categories_cmd",
        {
          amountCents: 4355,
          categories: [
            { name: "Alimentação", amount_cents: 110000, position: 0 },
            { name: "Transporte", amount_cents: 25000, position: 1 },
          ],
          divisorDays: 31,
        },
      );
    });
  });

  it("divisor vazio recusa com calma: erro inline e avanço bloqueado", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Recalibrar o teto" }));
    await user.click(screen.getByRole("button", { name: "Definir os dias" }));
    await user.clear(screen.getByLabelText("Divisor de dias"));

    expect(await screen.findByRole("alert")).toHaveTextContent(/pelo menos 1 dia/);
    expect(screen.getByRole("button", { name: "Ver o teto novo" })).toBeDisabled();
  });

  it("a guarda do vença o dia intercepta quem baixa o teto, e libera a escolha", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Recalibrar o teto" }));
    await user.clear(screen.getByLabelText("Valor mensal da categoria 1"));
    await user.type(screen.getByLabelText("Valor mensal da categoria 1"), "750,00");
    await user.click(screen.getByRole("button", { name: "Definir os dias" }));
    await user.click(screen.getByRole("button", { name: "Ver o teto novo" }));

    // 1.000,00 ÷ 31 = 32,26 — abaixo do teto vigente: a guarda aparece antes do aceite.
    expect(
      screen.getByRole("heading", {
        name: /Antes de baixar o teto, vença o dia primeiro/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Usar este teto" }),
    ).not.toBeInTheDocument();

    // Ensina e LIBERA: seguir mostra o aceite normalmente.
    await user.click(screen.getByRole("button", { name: "Baixar assim mesmo" }));
    await user.click(screen.getByRole("button", { name: "Usar este teto" }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "upsert_daily_budget_with_categories_cmd",
        expect.objectContaining({ amountCents: 3226 }),
      );
    });
  });

  it("dispensar a guarda não é crédito permanente: mexer no número a traz de volta", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: SUMMARY,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Recalibrar o teto" }));
    await user.clear(screen.getByLabelText("Valor mensal da categoria 1"));
    await user.type(screen.getByLabelText("Valor mensal da categoria 1"), "750,00");
    await user.click(screen.getByRole("button", { name: "Definir os dias" }));
    await user.click(screen.getByRole("button", { name: "Ver o teto novo" }));
    await user.click(screen.getByRole("button", { name: "Baixar assim mesmo" }));

    // Voltar ao divisor e refazer a conta reabre o julgamento da guarda.
    await user.click(screen.getByRole("button", { name: "Voltar" }));
    await user.click(screen.getByRole("button", { name: "Ver o teto novo" }));
    expect(
      screen.getByRole("heading", {
        name: /Antes de baixar o teto, vença o dia primeiro/,
      }),
    ).toBeInTheDocument();
  });

  it("sem teto: a cerimônia guiada abre nas cinco perguntas do método", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: {
        ...SUMMARY,
        daily_ceiling_source: "none",
        daily_budget: 0,
      },
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "Você ainda não tem um teto.",
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Estipular o teto" }));

    expect(
      screen.getByRole("heading", { name: "Quanto você gasta por mês com comida?" }),
    ).toBeInTheDocument();
    await user.type(screen.getByLabelText("Comida por mês (R$)"), "600,00");
    await user.click(screen.getByRole("button", { name: "Próxima pergunta" }));
    expect(
      screen.getByRole("heading", { name: "E com transporte?" }),
    ).toBeInTheDocument();
  });

  it("estimativa: a média do histórico se declara e não vira veredito", async () => {
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: {
        ...SUMMARY,
        daily_ceiling_source: "estimate",
        daily_budget: 4600,
      },
    });
    render(<TetoScreen />);

    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: /Cerca de R\$\s?46 por dia, pelo seu histórico/,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("Estimativa")).toBeInTheDocument();
    expect(
      screen.queryByRole("region", { name: "A prova do número" }),
    ).not.toBeInTheDocument();
  });

  // A estimativa IMPRIME a conta em vez de descrevê-la: a frase antiga falava em "meses"
  // com registro, e o motor divide o gasto de UM mês pelos dias dele.
  it("estimativa: a conta vem impressa com os operandos do motor", async () => {
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: {
        ...SUMMARY,
        daily_ceiling_source: "estimate",
        daily_budget: 2000,
        daily_ceiling_estimate: { variable_cents: 62000, days: 31, month: "2026-05" },
      },
    });
    render(<TetoScreen />);

    expect(
      await screen.findByText("Gasto variável de maio de 2026"),
    ).toBeInTheDocument();
    expect(screen.getByText("Dias de maio de 2026")).toBeInTheDocument();
    expect(screen.getByText("31")).toBeInTheDocument();
    expect(screen.getByText("Cerca de, por dia")).toBeInTheDocument();
    // A prosa que descrevia a fórmula saiu: quem descreve agora é a própria conta.
    expect(screen.queryByText(/média do gasto variável dos seus meses/)).toBeNull();
  });

  // Sem operandos do motor, a tela fica sem conta — nunca com uma conta reconstruída.
  it("estimativa sem base do motor não imprime conta nenhuma", async () => {
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: {
        ...SUMMARY,
        daily_ceiling_source: "estimate",
        daily_budget: 4600,
      },
    });
    render(<TetoScreen />);

    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByText(/^Gasto variável de/)).toBeNull();
  });

  it("proposta da planilha: manda na manchete, confronta o teto vigente e só grava no aceite", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: { ...CEREMONY_BUDGET, per_day_cents: 3500 },
      get_ceiling_proposal_cmd: PROPOSAL,
      get_dashboard_summary: SUMMARY,
      accept_ceiling_proposal_cmd: undefined,
    });
    render(<TetoScreen />);

    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: /Sua planilha propõe R\$\s?40,33 por dia/,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/escrita em setembro de 2025/)).toBeInTheDocument();
    expect(screen.getByText(/substitui o atual de/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Usar este teto" }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("accept_ceiling_proposal_cmd", {
        proposalId: "cp-1",
      });
    });
  });

  it("dispensar a proposta chama o comando de dismiss", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: PROPOSAL,
      get_dashboard_summary: SUMMARY,
      dismiss_ceiling_proposal_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Agora não" }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("dismiss_ceiling_proposal_cmd", {
        proposalId: "cp-1",
      });
    });
  });

  it("falha de carga vira estado de erro anunciado, com caminho de volta", async () => {
    mockInvoke.mockRejectedValue(new Error("sem conexão"));
    render(<TetoScreen />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/Não foi possível carregar o teto/);
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
  });

  it("valor direto: grava o teto por dia sem itens nem divisor", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      get_dashboard_summary: {
        ...SUMMARY,
        daily_ceiling_source: "none",
        daily_budget: 0,
      },
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Já sei meu teto" }));
    await user.type(screen.getByLabelText("Teto por dia (R$)"), "50,00");
    await user.click(screen.getByRole("button", { name: "Usar este teto" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "upsert_daily_budget_with_categories_cmd",
        {
          amountCents: 5000,
          categories: [],
          divisorDays: null,
        },
      );
    });
  });
});
