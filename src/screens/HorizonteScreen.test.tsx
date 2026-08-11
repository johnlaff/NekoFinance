import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { HorizonteScreen } from "./HorizonteScreen";
import { mockCommands, mockInvoke } from "../test/commands";
import type { Forecast, MonthEnd, TransactionRow } from "./horizonteView";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// A régua do saldo (faixas absolutas) é testada em `lib/saldoHeatmap.test.ts`; a composição do
// veredito/estrada/grade/compromissos, em `horizonteView.test.ts`. Aqui provamos que a tela
// monta os dados reais e mostra as peças-chave.

const ann = {
  realized_income_cents: 0,
  realized_savings_cents: 0,
  realized_rate_bps: 0,
  registered_economia_cents: 0,
  patrimonio_cents: 0,
  economia_ruler_cents: 0,
  economia_ruler_rate_bps: 0,
  economia_state: "no_record" as const,
  projected_income_cents: 0,
  projected_savings_cents: 0,
  projected_rate_bps: 0,
};

const MONTH_END: MonthEnd[] = [
  { year: 2026, month: 7, balance_cents: 1299520 },
  { year: 2026, month: 8, balance_cents: 1468037 },
  { year: 2026, month: 9, balance_cents: 1825323 },
  { year: 2026, month: 12, balance_cents: 2997711 },
];

const FORECAST: Forecast = {
  today: "2026-07-22",
  horizon_end: "2026-12-31",
  annual_savings: ann,
  coverage: [
    {
      year: 2026,
      month: 9,
      projected_outflow_cents: 383522,
      baseline_outflow_cents: 1112126,
      coverage_bps: 3300,
      is_complete: false,
      estimated_missing_cents: 728604,
    },
  ],
  baseline_outflow_cents: 1112126,
  trusted_through_month: "2026-08",
  total_missing_cents: 728604,
  safe_to_spend_today_cents: 20000,
  cash_headroom_cents: 752086,
  savings_headroom_cents: 20000,
  binding_guardrail: "savings",
  savings_band_verdict: "in_band",
  savings_band: { floor_bps: 2000, ceiling_bps: 3000 },
  savings_band_scope_lived: true,
  deepest_deficit: null,
  daily: [
    {
      date: "2026-07-22",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 756830,
    },
    {
      date: "2026-07-26",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 752086,
    },
    {
      date: "2026-09-10",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 1825323,
    },
    {
      date: "2026-12-31",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 2997711,
    },
  ],
  month_end: MONTH_END,
  months: [],
};

function tx(
  over: Partial<TransactionRow> & { id: string; date: string },
): TransactionRow {
  return {
    type: "expense",
    amount: 10000,
    description: "",
    payment_method: "debit",
    is_projection: true,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "projetado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    has_refund_link: false,
    ...over,
  };
}

const AGOSTO: TransactionRow[] = [
  tx({
    id: "a1",
    date: "2026-08-07",
    description: "Financiamento do carro",
    amount: 100651,
    installment_index: 11,
    installment_total: 36,
  }),
  tx({
    id: "a2",
    date: "2026-08-27",
    description: "Salário",
    amount: 601273,
    type: "income",
    is_fixed: true,
  }),
];

function mockLivre() {
  mockInvoke.mockReset();
  mockCommands({
    get_forecast: FORECAST,
    last_sync_at: "2026-07-22 22:14:00",
    scenario_forecast: null,
    get_recent_transactions: (args) =>
      (args?.["month"] as string) === "2026-08" ? AGOSTO : [],
  });
}

describe("HorizonteScreen (render)", () => {
  it("veredito livre: manchete com o mês de confiança", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText(/Caminho livre até o fim de agosto/)).toBeInTheDocument(),
    );
  });

  // Regra 1 (ADR-0013): a regra dos 60% de lastro é prosa fixa, idêntica em toda visita —
  // já vive integralmente no popover de "gasto típico"; o parágrafo inline morre. A
  // fronteira de réguas (que morava no mesmo parágrafo) recolhe para o popover do semáforo.
  // O operando (o valor do gasto típico) é dado que varia por usuário — numa frase mista,
  // a cláusula didática recolhe e o operando sobrevive como legenda curta (regra 3).
  it("a regra dos 60% de lastro não aparece mais em parágrafo fixo, mas o operando fica", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText("A estrada até dezembro")).toBeInTheDocument(),
    );
    expect(
      screen.queryByText(/Um mês à frente só sustenta o veredito/),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(/A régua do ano \(Economizado%\) mora em O ano/),
    ).not.toBeInTheDocument();
    const typicalTrigger = screen.getByRole("button", { name: "Gasto típico" });
    expect(typicalTrigger.parentElement?.parentElement).toHaveTextContent(
      /Gasto típico.*R\$ 11\.121,26\/mês/,
    );
  });

  // A fronteira entre as réguas (ano × caixa) recolhe para o popover do semáforo — a grade
  // ganha um gatilho "Como funciona?" próprio, e o texto duplicado do bloco some da superfície.
  it("a grade recolhe o semáforo atrás de 'Como funciona?' com a fronteira de réguas dentro", async () => {
    const user = userEvent.setup();
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText("Os próximos 12 meses")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Cada mês abre no Calendário/)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/Mês sem lastro não ganha cor de aprovação/),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Como funciona? — os próximos 12 meses" }),
    );
    const tooltip = await screen.findByRole("tooltip");
    expect(tooltip).toHaveTextContent(/faixas fixas da sua planilha/);
    expect(tooltip).toHaveTextContent(
      /A régua do ano \(Economizado%\) mora em O ano — aqui o juiz é o caixa/,
    );
  });

  // A didática do bloco de cenários recolhe atrás de "Como funciona?"; o CTA de simular
  // continua sempre visível (regra 3) em qualquer estado da tela.
  it("cenários: a didática recolhe atrás de 'Como funciona?' e o CTA de simular fica visível", async () => {
    const user = userEvent.setup();
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() => expect(screen.getByText("E se?")).toBeInTheDocument());
    expect(
      screen.queryByText(/Teste uma compra, um financiamento ou uma troca de plano/),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Simular cenário/ })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Como funciona? — E se?" }));
    const tooltip = await screen.findByRole("tooltip");
    expect(tooltip).toHaveTextContent(/a reserva continua com 6 meses ou mais/);
    expect(tooltip).toHaveTextContent(/A economia de 20–30% segue viva/);
  });

  it("desenha a estrada com rótulo acessível", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByRole("img", { name: /Saldo projetado/ })).toBeInTheDocument(),
    );
  });

  it("mostra a grade dos 12 meses e o card de compromissos com parcela", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText("Os próximos 12 meses")).toBeInTheDocument(),
    );
    // Os compromissos vêm de uma segunda query (meses derivados do forecast) — findByText espera.
    expect(await screen.findByText("O que já está marcado")).toBeInTheDocument();
    expect(screen.getByText("Financiamento do carro")).toBeInTheDocument();
    // Parcela n/N em mono.
    expect(screen.getByText("11/36")).toBeInTheDocument();
  });

  it("aperto: a manchete constata o mês e o valor que falta, e devolve a pergunta", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: {
        ...FORECAST,
        deepest_deficit: { date: "2026-09-12", balance_cents: -124000 },
      },
      last_sync_at: null,
      scenario_forecast: null,
      get_recent_transactions: () => [],
    });
    render(<HorizonteScreen />);
    expect(await screen.findByText(/O caminho aperta em setembro/)).toHaveTextContent(
      "O caminho aperta em setembro — faltam R$ 1.240,00. O que dá para mover antes?",
    );
    // A receita fixa de travessia (antecipar/adiar/cruzar com a reserva) morre da manchete —
    // já vive integralmente no popover do "buraco do futuro", termo do glossário do método.
    expect(
      screen.queryByText(/Do jeito que está lançado, o saldo passa por/),
    ).not.toBeInTheDocument();
  });

  it("aperto: a receita de travessia continua acessível atrás de 'Como funciona?'", async () => {
    const user = userEvent.setup();
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: {
        ...FORECAST,
        deepest_deficit: { date: "2026-09-12", balance_cents: -124000 },
      },
      last_sync_at: null,
      scenario_forecast: null,
      get_recent_transactions: () => [],
    });
    render(<HorizonteScreen />);
    await user.click(
      await screen.findByRole("button", { name: "Como funciona? — o caminho aperta" }),
    );
    const tooltip = await screen.findByRole("tooltip");
    expect(tooltip).toHaveTextContent(/antecipar uma entrada/);
    expect(tooltip).toHaveTextContent(/cruzar com a reserva/);
  });

  it("estado de erro oferece tentar novamente", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: new Error("boom"),
      last_sync_at: null,
      get_recent_transactions: () => [],
    });
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(
        screen.getByText("Não foi possível carregar o horizonte"),
      ).toBeInTheDocument(),
    );
  });
});
