import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { HorizonteScreen } from "./HorizonteScreen";
import {
  stripScenarioMarker,
  addMonthsISO,
  placeChartEndLabels,
  CHART_LABEL_MIN_GAP,
} from "../lib/scenarioHelpers";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";
import type { ScenarioCompareDto } from "../lib/api";
import { fmtBRL, fmtCompactBRL, saldoBand } from "../lib/nkFormat";
import { performanceStatus, custoVidaStatus } from "./totaisStatus";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function baseCompare(overrides: Partial<ScenarioCompareDto> = {}): ScenarioCompareDto {
  return {
    scenario_id: "scn-1",
    scenario_name: "E se eu financiar um carro",
    real_today: "2026-06-10",
    real_horizon_end: "2026-12-31",
    real_month_end: [{ year: 2026, month: 12, balance_cents: 500_000 }],
    real_deepest_deficit: { date: "2026-07-01", balance_cents: 100_000 },
    real_performance_cents: 200_000,
    real_safe_to_spend_today_cents: 15_000,
    real_binding_guardrail: "cash",
    real_cost_of_living_cents: 300_000,
    real_income_cents: 500_000,
    scenario_month_end: [{ year: 2026, month: 12, balance_cents: 350_000 }],
    scenario_deepest_deficit: { date: "2026-07-01", balance_cents: -50_000 },
    scenario_performance_cents: 150_000,
    scenario_safe_to_spend_today_cents: 8_000,
    scenario_binding_guardrail: "cash",
    scenario_cost_of_living_cents: 350_000,
    scenario_income_cents: 450_000,
    month_end: [
      {
        year: 2026,
        month: 6,
        real_balance_cents: 500_000,
        scenario_balance_cents: 480_000,
        delta_cents: -20_000,
      },
      {
        year: 2026,
        month: 12,
        real_balance_cents: 500_000,
        scenario_balance_cents: 350_000,
        delta_cents: -150_000,
      },
    ],
    deepest_deficit_delta_cents: -150_000,
    performance_delta_cents: -50_000,
    safe_to_spend_delta_cents: -7_000,
    cost_of_living_delta_cents: 50_000,
    changes: [],
    loan: null,
    ...overrides,
  };
}

describe("cenários 'e se' — helpers puros", () => {
  it("stripScenarioMarker remove os sufixos #loan e #repl do fim da descrição", () => {
    expect(stripScenarioMarker("Empréstimo #loan:abc-123:250")).toBe("Empréstimo");
    expect(stripScenarioMarker("Substituição #repl:ov-1")).toBe("Substituição");
    expect(stripScenarioMarker("Aluguel")).toBe("Aluguel");
  });

  it("stripScenarioMarker preserva um '#loan:' literal no MEIO do texto (dado do usuário)", () => {
    // Ancorado ao FIM, como o parser do backend: só o sufixo de sistema é removido.
    expect(stripScenarioMarker("Pagamento #loan:xyz do consórcio")).toBe(
      "Pagamento #loan:xyz do consórcio",
    );
    expect(stripScenarioMarker("Pagamento #loan:xyz do consórcio #loan:abc:250")).toBe(
      "Pagamento #loan:xyz do consórcio",
    );
  });

  it("addMonthsISO soma meses preservando o dia (com saturação no fim do mês)", () => {
    expect(addMonthsISO("2026-06-15", 1)).toBe("2026-07-15");
    expect(addMonthsISO("2026-01-31", 1)).toBe("2026-02-28");
  });

  describe("placeChartEndLabels — rótulos de fim de linha nunca colidem (clamp do PAR)", () => {
    // Limites reais do DualLineChart: minY = padTop+8 = 28, maxY = H−6 = 194.
    const MIN_Y = 28;
    const MAX_Y = 194;

    function gap(p: { realLabelY: number; scenarioLabelY: number }): number {
      return Math.abs(p.realLabelY - p.scenarioLabelY);
    }

    it("traços convergindo perto do TOPO: o clamp não come o vão (caso da revisão)", () => {
      // realY=22, scenarioY=20 — no clamp por-rótulo o superior era empurrado a 28 e o
      // inferior ficava em 36 (vão de 8px = colisão a fontSize 11 + halo 3px).
      const p = placeChartEndLabels(22, 20, MIN_Y, MAX_Y);
      expect(gap(p)).toBeGreaterThanOrEqual(CHART_LABEL_MIN_GAP);
      // Cenário termina mais alto → é o rótulo de cima, clampado ao teto; o real deriva dele.
      expect(p.scenarioLabelY).toBe(28);
      expect(p.realLabelY).toBe(42);
    });

    it("espelho do caso da revisão com o REAL mais alto: mesmo vão, papéis trocados", () => {
      const p = placeChartEndLabels(20, 22, MIN_Y, MAX_Y);
      expect(gap(p)).toBeGreaterThanOrEqual(CHART_LABEL_MIN_GAP);
      expect(p.realLabelY).toBe(28);
      expect(p.scenarioLabelY).toBe(42);
    });

    it("traços convergindo perto do FUNDO: o par sobe junto, o vão não comprime", () => {
      const p = placeChartEndLabels(190, 192, MIN_Y, MAX_Y);
      expect(gap(p)).toBeGreaterThanOrEqual(CHART_LABEL_MIN_GAP);
      expect(p.scenarioLabelY).toBeLessThanOrEqual(MAX_Y);
      expect(p.realLabelY).toBeLessThanOrEqual(MAX_Y);
      expect(p.realLabelY).toBeGreaterThanOrEqual(MIN_Y);
    });

    it("janela menor que o vão: os 14px vencem o limite de baixo (fundir é pior que vazar)", () => {
      const p = placeChartEndLabels(30, 30, 28, 35);
      expect(gap(p)).toBe(CHART_LABEL_MIN_GAP);
      expect(Math.min(p.realLabelY, p.scenarioLabelY)).toBe(28);
    });

    it("traços bem separados: colocação natural intacta (acima do mais alto, abaixo do mais baixo)", () => {
      const p = placeChartEndLabels(50, 120, MIN_Y, MAX_Y);
      expect(p.realLabelY).toBe(42); // 50 − 8
      expect(p.scenarioLabelY).toBe(134); // 120 + 14
      const inverted = placeChartEndLabels(120, 50, MIN_Y, MAX_Y);
      expect(inverted.scenarioLabelY).toBe(42);
      expect(inverted.realLabelY).toBe(134);
    });
  });
});

describe("HorizonteScreen — side-sheet 'Simular cenário'", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("abre o side-sheet ao clicar em 'Simular cenário'", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));

    expect(
      await screen.findByRole("dialog", { name: "Simular cenário" }),
    ).toBeInTheDocument();
  });

  it("cria um cenário e entra em modo comparação", async () => {
    let created: unknown = null;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [],
      create_scenario_cmd: (args) => {
        created = args;
        return { id: "scn-novo", name: "E se eu financiar um carro", person_id: "p1" };
      },
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));

    await user.type(
      screen.getByLabelText("Novo cenário"),
      "E se eu financiar um carro",
    );
    await user.click(screen.getByRole("button", { name: "Criar cenário" }));

    await waitFor(() => {
      expect(created).toMatchObject({ name: "E se eu financiar um carro" });
    });

    // Entra em modo comparação: a superfície aparece com o nome do cenário.
    expect(
      await screen.findByText("Cenário: E se eu financiar um carro"),
    ).toBeInTheDocument();
  });

  it("adicionar um lançamento hipotético chama a API e refaz o fetch da lista", async () => {
    let addArgs: unknown = null;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: (): unknown[] =>
        addArgs
          ? [
              {
                id: "hipo-1",
                type: "expense",
                amount: 50000,
                description: "Reforma da cozinha",
                date: "2026-08-01",
              },
            ]
          : [],
      list_obligations_cmd: [],
      add_scenario_transaction_cmd: (args) => {
        addArgs = args;
        return "hipo-1";
      },
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const addSection = screen.getByRole("region", { name: "Adicionar lançamento" });
    await user.type(
      within(addSection).getByLabelText("Descrição"),
      "Reforma da cozinha",
    );
    await user.type(within(addSection).getByLabelText("Valor/mês"), "500,00");
    await user.click(within(addSection).getByRole("button", { name: "Adicionar" }));

    await waitFor(() => {
      expect(addArgs).toMatchObject({
        scenarioId: "scn-1",
        description: "Reforma da cozinha",
        amountCents: 50000,
      });
    });

    expect(await screen.findByText("Reforma da cozinha")).toBeInTheDocument();
  });

  it("parseia valor/mês com separador de milhar pt-BR (1.200,00 → 120000 centavos)", async () => {
    let addArgs: unknown = null;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: (): unknown[] =>
        addArgs
          ? [
              {
                id: "hipo-1",
                type: "expense",
                amount: 120000,
                description: "Novo carro",
                date: "2026-08-01",
              },
            ]
          : [],
      list_obligations_cmd: [],
      add_scenario_transaction_cmd: (args) => {
        addArgs = args;
        return "hipo-1";
      },
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const addSection = screen.getByRole("region", { name: "Adicionar lançamento" });
    await user.type(within(addSection).getByLabelText("Descrição"), "Novo carro");
    await user.type(within(addSection).getByLabelText("Valor/mês"), "1.200,00");
    await user.click(within(addSection).getByRole("button", { name: "Adicionar" }));

    await waitFor(() => {
      expect(addArgs).toMatchObject({
        scenarioId: "scn-1",
        description: "Novo carro",
        amountCents: 120000,
      });
    });

    expect(await screen.findByText("Novo carro")).toBeInTheDocument();
  });

  it("mostra a rejeição de data anterior ao mês corrente vinda do backend", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      add_scenario_transaction_cmd: () =>
        Promise.reject(
          new Error("data anterior ao mês corrente não entra na projeção do cenário"),
        ),
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const addSection = screen.getByRole("region", { name: "Adicionar lançamento" });
    await user.type(within(addSection).getByLabelText("Descrição"), "Linha antiga");
    await user.type(within(addSection).getByLabelText("Valor/mês"), "10,00");
    await user.click(within(addSection).getByRole("button", { name: "Adicionar" }));

    expect(
      await screen.findByText(
        "data anterior ao mês corrente não entra na projeção do cenário",
      ),
    ).toBeInTheDocument();
  });

  it("mostra a rejeição de override duplicado vinda do backend", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [
        {
          id: "ob-1",
          person_id: "p1",
          name: "Aluguel",
          match_desc: "aluguel",
          match_section: null,
          kind: "saida",
        },
      ],
      obligation_items_cmd: [],
      set_scenario_override_cmd: () =>
        Promise.reject(
          new Error("já existe uma alteração para esta obrigação neste cenário"),
        ),
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    await user.selectOptions(screen.getByLabelText("Obrigação recorrente"), "ob-1");
    await user.click(
      await screen.findByRole("button", { name: "Confirmar alteração" }),
    );

    expect(
      await screen.findByText(
        "já existe uma alteração para esta obrigação neste cenário",
      ),
    ).toBeInTheDocument();
  });

  it("parseia novo valor/mês de override com separador de milhar pt-BR (1.234,56 → 123456 centavos)", async () => {
    let overrideArgs: unknown = null;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [
        {
          id: "ob-1",
          person_id: "p1",
          name: "Aluguel",
          match_desc: "aluguel",
          match_section: null,
          kind: "saida",
        },
      ],
      obligation_items_cmd: [],
      set_scenario_override_cmd: (args) => {
        overrideArgs = args;
        return "ov-1";
      },
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    await user.selectOptions(screen.getByLabelText("Obrigação recorrente"), "ob-1");
    await user.selectOptions(screen.getByLabelText("Ação"), "replace");
    await user.type(screen.getByLabelText("Novo valor/mês"), "1.234,56");
    await user.click(
      await screen.findByRole("button", { name: "Confirmar alteração" }),
    );

    await waitFor(() => {
      expect(overrideArgs).toMatchObject({
        scenarioId: "scn-1",
        op: "replace",
        obligationId: "ob-1",
        replacement: { amount_cents: 123456 },
      });
    });
  });

  it("empréstimo com falha no meio: erro diz quantas parcelas entraram e a lista refetch mostra as órfãs", async () => {
    let addCalls = 0;
    let failed = false;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_obligations_cmd: [],
      price_installment_cmd: 35000,
      // Ordem sequencial: 1º call = principal (OK), 2º = parcela 1 (OK), 3º = parcela 2 REJEITA.
      add_scenario_transaction_cmd: () => {
        addCalls += 1;
        if (addCalls === 3) {
          failed = true;
          return Promise.reject(new Error("database is locked"));
        }
        return `txn-${addCalls}`;
      },
      // Depois da falha, a lista reflete o grupo parcial persistido (principal + 1 parcela).
      list_scenario_transactions_cmd: (): unknown[] =>
        failed
          ? [
              {
                id: "txn-1",
                type: "income",
                amount: 100000,
                description: "Empréstimo #loan:g1:200",
                date: "2026-07-08",
              },
              {
                id: "txn-2",
                type: "expense",
                amount: 35000,
                description: "Empréstimo parcela 1/3 #loan:g1:200",
                date: "2026-08-08",
              },
            ]
          : [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const loanSection = screen.getByRole("region", {
      name: "Dimensionar um empréstimo",
    });
    await user.type(within(loanSection).getByLabelText("Valor"), "1000,00");
    const termInput = within(loanSection).getByLabelText("Nº parcelas");
    await user.clear(termInput);
    await user.type(termInput, "3");
    await user.click(
      await within(loanSection).findByRole("button", {
        name: "Adicionar empréstimo ao cenário",
      }),
    );

    // O erro nomeia exatamente o estado parcial: 1 de 3 parcelas criadas.
    const alert = await within(loanSection).findByRole("alert");
    expect(alert.textContent).toMatch(/1 de 3 parcelas criadas/);
    // E o catch invalida: a lista refetch já mostra as linhas órfãs (com o marcador removido)
    // para o usuário poder excluí-las antes de tentar de novo.
    expect(await screen.findByText("Parcela 1/3")).toBeInTheDocument();
  });

  it("parseia valor do empréstimo com separador de milhar pt-BR (10.000,00 → 1000000 centavos)", async () => {
    const calls: unknown[] = [];
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_obligations_cmd: [],
      price_installment_cmd: 35000,
      add_scenario_transaction_cmd: (args) => {
        calls.push(args);
        return `txn-${calls.length}`;
      },
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const loanSection = screen.getByRole("region", {
      name: "Dimensionar um empréstimo",
    });
    await user.type(within(loanSection).getByLabelText("Valor"), "10.000,00");
    const termInput = within(loanSection).getByLabelText("Nº parcelas");
    await user.clear(termInput);
    await user.type(termInput, "3");
    await user.click(
      await within(loanSection).findByRole("button", {
        name: "Adicionar empréstimo ao cenário",
      }),
    );

    await waitFor(() => {
      expect(calls.length).toBeGreaterThanOrEqual(1);
    });
    expect(calls[0]).toMatchObject({
      scenarioId: "scn-1",
      txnType: "income",
      amountCents: 1000000,
    });
  });

  async function openLoanSection() {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      price_installment_cmd: 35000,
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));
    const loanSection = await screen.findByRole("region", {
      name: "Dimensionar um empréstimo",
    });
    // Principal e prazo válidos: isola a TAXA como única causa de (in)validez do formulário.
    await user.type(within(loanSection).getByLabelText("Valor"), "1000,00");
    return { user, loanSection };
  }

  it("taxa não-numérica ('abc') desabilita o botão e mostra 'Taxa inválida' (nunca vira 0%)", async () => {
    const { user, loanSection } = await openLoanSection();
    const rate = within(loanSection).getByLabelText("Juros a.m. (%)");
    await user.clear(rate);
    await user.type(rate, "abc");

    expect(within(loanSection).getByText(/Taxa inválida/)).toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(loanSection).getByRole("button", {
          name: "Adicionar empréstimo ao cenário",
        }),
      ).toBeDisabled(),
    );
  });

  it("taxa negativa ('-2') desabilita o botão (nunca cria empréstimo com bps negativo)", async () => {
    const { user, loanSection } = await openLoanSection();
    const rate = within(loanSection).getByLabelText("Juros a.m. (%)");
    await user.clear(rate);
    await user.type(rate, "-2");

    await waitFor(() =>
      expect(
        within(loanSection).getByRole("button", {
          name: "Adicionar empréstimo ao cenário",
        }),
      ).toBeDisabled(),
    );
  });

  it("taxa com vírgula decimal ('1,8') habilita o botão (parse pt-BR funciona)", async () => {
    const { user, loanSection } = await openLoanSection();
    const rate = within(loanSection).getByLabelText("Juros a.m. (%)");
    await user.clear(rate);
    await user.type(rate, "1,8");

    const btn = within(loanSection).getByRole("button", {
      name: "Adicionar empréstimo ao cenário",
    });
    await waitFor(() => expect(btn).toBeEnabled());
    expect(within(loanSection).queryByText(/Taxa inválida/)).not.toBeInTheDocument();
  });

  it("prévia do override falha: erro visível, Confirmar desabilitado e sem '0 ocorrências'", async () => {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: baseCompare(),
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [
        {
          id: "ob-1",
          person_id: "p1",
          name: "Aluguel",
          match_desc: "aluguel",
          match_section: null,
          kind: "saida",
        },
      ],
      obligation_items_cmd: () => Promise.reject(new Error("falha ao ler ocorrências")),
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    await user.selectOptions(screen.getByLabelText("Obrigação recorrente"), "ob-1");

    // Erro de leitura vira alerta com retry — nunca "afeta 0 ocorrências" (isso pareceria
    // "não muda nada" e o usuário salvaria às cegas).
    expect(
      await screen.findByText("Não foi possível carregar as ocorrências afetadas."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Confirmar alteração" })).toBeDisabled();
    expect(screen.queryByText(/Isto afeta/)).not.toBeInTheDocument();
  });
});

describe("ScenarioCompare — superfície de comparação", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  async function renderCompare(compare: ScenarioCompareDto) {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [
        { id: "scn-1", name: compare.scenario_name, person_id: "p1" },
      ],
      get_scenario_forecast_cmd: compare,
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(
      await screen.findByRole("button", { name: compare.scenario_name }),
    );
    await screen.findByText(`Cenário: ${compare.scenario_name}`);
    return user;
  }

  it("renderiza os 5 cartões de KPI com real → cenário e o delta", async () => {
    await renderCompare(baseCompare());

    const surface = screen.getByLabelText("Comparação real × cenário");
    for (const label of [
      "Buraco do futuro",
      "Saldo no fim do horizonte",
      "Custo de vida",
      "Performance · mês atual",
      "Pode gastar hoje",
    ]) {
      expect(within(surface).getByText(label)).toBeInTheDocument();
    }
    // Buraco do futuro: real R$1.000,00 → cenário −R$500,00 (escopado ao card: o valor
    // do delta de Performance é coincidentemente igual, então uma busca solta ambiguaria).
    // A linha de evidência mantém os DOIS lados em precisão cheia (nunca só o cenário).
    const deficitCard = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    expect(within(deficitCard).getByText("R$ 1.000,00")).toBeInTheDocument();
    expect(within(deficitCard).getByText("−R$ 500,00")).toBeInTheDocument();
    // A manchete (fatia A, anatomia nova) mostra só o valor do CENÁRIO, no formato compacto —
    // nunca duas leituras de precisão cheia lado a lado em tamanho de destaque.
    expect(deficitCard.querySelector(".scn-kpi__headline")?.textContent).toBe(
      fmtCompactBRL(-50_000),
    );
    // Fonte única para o leitor de tela: o aria-label do article carrega o ESTADO do método
    // (Nível 2, plano 074/fatia B) + real e cenário em precisão cheia; estado, manchete E linha
    // de evidência são visual-only (aria-hidden) — sem isso o leitor ouviria tudo em dobro.
    // Real R$1.000,00 cai na faixa "Apertado" do Termômetro; cenário −R$500,00 cai em
    // "Negativo" — bandas DIFERENTES → transição anunciada como "novo (antes velho)".
    expect(deficitCard).toHaveAttribute(
      "aria-label",
      // fmtBRL usa NBSP após "R$" — montar a expectativa com o próprio formatador evita
      // acoplar o teste ao byte exato do espaço.
      `Buraco do futuro: Negativo (antes Apertado), real ${fmtBRL(100_000)}, cenário ${fmtBRL(-50_000)}`,
    );
    expect(
      deficitCard.querySelector(".scn-kpi__evidence")?.getAttribute("aria-hidden"),
    ).toBe("true");
    expect(
      deficitCard.querySelector(".scn-kpi__headline")?.getAttribute("aria-hidden"),
    ).toBe("true");
  });

  it("disciplina do delta: material usa ícone de melhor/pior (nunca o sinal cru); diferença ≤R$1 vira texto quieto sem pill", async () => {
    await renderCompare(
      baseCompare({
        performance_delta_cents: 50_000, // higher-better, positivo → melhor
        deepest_deficit_delta_cents: -50_000, // higher-better, negativo → pior
        cost_of_living_delta_cents: -30_000, // lower-better: custo CAIU → melhor
        safe_to_spend_delta_cents: 50, // ≤R$1 de diferença → ruído, não resultado
        month_end: [
          {
            year: 2026,
            month: 12,
            real_balance_cents: 500_000,
            scenario_balance_cents: 500_090,
            delta_cents: 90, // ≤R$1 → texto quieto no "Saldo no fim do horizonte"
          },
        ],
      }),
    );

    const surface = screen.getByLabelText("Comparação real × cenário");
    // Nunca mais o glifo cru de seta/ponto — só ícone lucide (better/worse) ou texto quieto.
    expect(surface.textContent).not.toMatch(/[▲▼•]/);

    const perfCard = within(surface)
      .getByRole("button", { name: "Performance · mês atual" })
      .closest("article")!;
    expect(perfCard.querySelector(".lucide-trending-up")).toBeInTheDocument();
    expect(perfCard.querySelector(".lucide-trending-down")).not.toBeInTheDocument();

    const deficitCard = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    expect(deficitCard.querySelector(".lucide-trending-down")).toBeInTheDocument();
    expect(deficitCard.querySelector(".lucide-trending-up")).not.toBeInTheDocument();

    // Custo de vida é "menor é melhor": o custo CAIU (delta negativo) mas o ícone tem que
    // ser TrendingUp (melhor) — provando que o glifo vem do sentido, não do sinal cru.
    const costCard = within(surface)
      .getByRole("button", { name: "Custo de vida" })
      .closest("article")!;
    expect(costCard.querySelector(".lucide-trending-up")).toBeInTheDocument();
    expect(costCard.querySelector(".lucide-trending-down")).not.toBeInTheDocument();

    const spendCard = within(surface)
      .getByRole("button", { name: "Pode gastar hoje" })
      .closest("article")!;
    expect(within(spendCard).getByText("≈ Sem mudança")).toBeInTheDocument();
    expect(
      spendCard.querySelector(".lucide-trending-up, .lucide-trending-down"),
    ).not.toBeInTheDocument();

    const endCard = within(surface)
      .getByRole("button", { name: "Saldo no fim do horizonte" })
      .closest("article")!;
    expect(within(endCard).getByText("≈ Sem mudança")).toBeInTheDocument();
  });

  it("funde replace numa única entrada 'o que mudou' (velho → novo)", async () => {
    await renderCompare(
      baseCompare({
        changes: [
          {
            op: "replace",
            description: "Aluguel",
            from_date: "2026-07-01",
            old_amount_cents: 150_000,
            new_amount_cents: 120_000,
          },
        ],
      }),
    );

    expect(await screen.findByText("↔ Alterado")).toBeInTheDocument();
    // O valor antigo/novo agora renderiza em dois <Money> (a11y), então o texto fica
    // partido em nós diferentes — comparamos o textContent normalizado da linha inteira.
    const amt = document.querySelector(".scn-change-row__amt");
    expect(amt?.textContent?.replace(/\s+/g, " ").trim()).toBe(
      "R$ 1.500,00 → R$ 1.200,00",
    );
  });

  it("remove os sufixos de marca (#loan/#repl) da lista de mudanças", async () => {
    await renderCompare(
      baseCompare({
        changes: [
          {
            op: "add",
            description: "Empréstimo parcela 1/12 #loan:abc:250",
            from_date: "2026-07-01",
            old_amount_cents: null,
            new_amount_cents: 90_000,
          },
        ],
      }),
    );

    expect(await screen.findByText("Empréstimo parcela 1/12")).toBeInTheDocument();
    expect(screen.queryByText(/#loan/)).not.toBeInTheDocument();
  });

  it("o popover didático de um termo abre ao focar via teclado (a11y)", async () => {
    const user = await renderCompare(baseCompare());

    const term = screen.getByRole("button", { name: "Buraco do futuro" });
    term.focus();
    await user.keyboard("{Enter}");

    expect(
      await screen.findByText(/O menor saldo que sua projeção alcança/),
    ).toBeInTheDocument();
  });

  it("a região aria-live muda de texto a cada recomputo (não só na troca de cenário)", async () => {
    const compare1 = baseCompare(); // saldo final delta = −R$ 1.500,00
    const compare2 = baseCompare({
      month_end: [
        {
          year: 2026,
          month: 12,
          real_balance_cents: 500_000,
          scenario_balance_cents: 200_000,
          delta_cents: -300_000, // recomputo após uma edição: delta diferente
        },
      ],
    });
    let forecastCalls = 0;
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [{ id: "scn-1", name: "Cenário A", person_id: "p1" }],
      get_scenario_forecast_cmd: () => {
        forecastCalls += 1;
        return forecastCalls === 1 ? compare1 : compare2;
      },
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
      add_scenario_transaction_cmd: "txn-novo",
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(await screen.findByRole("button", { name: "Cenário A" }));

    const live = await screen.findByTestId("scn-live-region");
    await waitFor(() => {
      // \s cobre o espaço não separável que o formatador BRL usa após "R$".
      expect(live.textContent).toMatch(/saldo final −R\$\s1\.500,00/);
    });
    const before = live.textContent;

    // Uma edição qualquer (adicionar lançamento) invalida e refaz o compare → o texto anunciado
    // TEM que mudar, senão o leitor de tela fica mudo (região live só fala em mutação de texto).
    const addSection = screen.getByRole("region", { name: "Adicionar lançamento" });
    await user.type(within(addSection).getByLabelText("Descrição"), "Streaming novo");
    await user.type(within(addSection).getByLabelText("Valor/mês"), "50,00");
    await user.click(within(addSection).getByRole("button", { name: "Adicionar" }));

    await waitFor(() => {
      expect(live.textContent).toMatch(/saldo final −R\$\s3\.000,00/);
    });
    expect(live.textContent).not.toBe(before);
  });
});

describe("ScenarioCompare — camada didática (plano 074, fatia B: veredito + estados do método)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  async function renderCompare(compare: ScenarioCompareDto) {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [
        { id: "scn-1", name: compare.scenario_name, person_id: "p1" },
      ],
      get_scenario_forecast_cmd: compare,
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(
      await screen.findByRole("button", { name: compare.scenario_name }),
    );
    await screen.findByText(`Cenário: ${compare.scenario_name}`);
    return user;
  }

  // --- Veredito (Nível 1) ---

  it("veredito de risco: cenário fura o caixa mostra mês + falta compacta, ícone+palavra+cor (nunca só cor)", async () => {
    // Consistência com o card: banda negativa do Termômetro ⇒ nível de risco do banner.
    expect(saldoBand(-50_000).key).toBe("negative");
    const compare = baseCompare({
      scenario_deepest_deficit: { date: "2026-07-01", balance_cents: -50_000 },
    });
    await renderCompare(compare);

    const banner = document.querySelector(".scn-verdict");
    expect(banner).not.toBeNull();
    expect(banner).toHaveClass("scn-verdict--risk");
    expect(banner!.textContent).toContain(
      `Fura o caixa em julho — faltam ${fmtCompactBRL(50_000)}.`,
    );
    // GPS, não ameaça: a subline sugere uma ação.
    expect(banner!.querySelector(".scn-verdict__subline")?.textContent).toMatch(
      /antecipe|reduza|cubra/i,
    );
    // Nunca só cor: o ícone (lucide) acompanha a palavra/cor sempre.
    expect(banner!.querySelector(".lucide-triangle-alert")).toBeInTheDocument();
  });

  it("veredito ok: cenário não fura o caixa mostra o menor saldo + a banda do Termômetro (saldoBand verbatim)", async () => {
    // Consistência com o card: banda "ok" (>R$1.000) ⇒ nível verde do banner.
    expect(saldoBand(150_000).key).toBe("ok");
    const compare = baseCompare({
      scenario_deepest_deficit: { date: "2026-07-01", balance_cents: 150_000 },
    });
    await renderCompare(compare);

    const banner = document.querySelector(".scn-verdict");
    expect(banner).toHaveClass("scn-verdict--ok");
    expect(banner!.textContent).toContain("Este cenário se mantém no azul o ano todo.");
    const band = saldoBand(150_000);
    expect(banner!.querySelector(".scn-verdict__subline")?.textContent).toBe(
      `Menor saldo no período: ${fmtBRL(150_000)} — ${band.label}.`,
    );
    expect(banner!.querySelector(".lucide-circle-check")).toBeInTheDocument();
  });

  it("veredito intermediário (âmbar) em R$0: banda 'apertado' nunca vira 'no azul' — banner e card concordam", async () => {
    // R$0 exato cai na banda "apertado" do Termômetro (fronteira inferior). O veredito de dois
    // níveis dizia "se mantém no azul o ano todo" (verde) enquanto o card "Buraco do futuro"
    // mostrava "Apertado" (âmbar) sobre o MESMO número — o nível intermediário fecha isso.
    expect(saldoBand(0).key).toBe("tight");
    const compare = baseCompare({
      scenario_deepest_deficit: { date: "2026-09-01", balance_cents: 0 },
    });
    await renderCompare(compare);

    const banner = document.querySelector(".scn-verdict");
    expect(banner).toHaveClass("scn-verdict--tight");
    expect(banner!.textContent).not.toContain("no azul o ano todo");
    expect(banner!.textContent).toContain(
      `Fica apertado em setembro — menor saldo ${fmtCompactBRL(0)}.`,
    );
    // GPS, não ameaça: a subline sugere uma ação.
    expect(banner!.querySelector(".scn-verdict__subline")?.textContent).toMatch(
      /segure|reforce/i,
    );
    expect(banner!.querySelector(".lucide-triangle-alert")).toBeInTheDocument();
    // O card logo abaixo classifica o MESMO número na MESMA banda (âmbar) — sem contradição.
    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      saldoBand(0).label,
    );
  });

  it("veredito intermediário na fronteira superior (R$1.000 exato ainda é 'apertado')", async () => {
    expect(saldoBand(100_000).key).toBe("tight");
    const compare = baseCompare({
      scenario_deepest_deficit: { date: "2026-08-01", balance_cents: 100_000 },
    });
    await renderCompare(compare);

    const banner = document.querySelector(".scn-verdict");
    expect(banner).toHaveClass("scn-verdict--tight");
    expect(banner!.textContent).toContain(
      `Fica apertado em agosto — menor saldo ${fmtCompactBRL(100_000)}.`,
    );
  });

  it("veredito com deepest_deficit nulo cai no mínimo mensal do cenário (mesma resolução do gráfico)", async () => {
    const compare = baseCompare({
      scenario_deepest_deficit: null,
      deepest_deficit_delta_cents: null,
      scenario_month_end: [
        { year: 2026, month: 8, balance_cents: 90_000 },
        { year: 2026, month: 12, balance_cents: 350_000 },
      ],
    });
    await renderCompare(compare);

    // Mínimo mensal 90.000 → banda "apertado" → nível intermediário, com o mês do mínimo.
    expect(saldoBand(90_000).key).toBe("tight");
    const banner = document.querySelector(".scn-verdict");
    expect(banner).toHaveClass("scn-verdict--tight");
    expect(banner!.textContent).toContain(
      `Fica apertado em agosto — menor saldo ${fmtCompactBRL(90_000)}.`,
    );

    // O CARD "Buraco do futuro" cai no MESMO mínimo mensal (fatia C: `scenarioDeepestPoint`
    // é a fonte única de banner e card) — nunca o "cenário R$ 0,00" do `?? 0` cru sobre o
    // deficit diário nulo, que fazia banner e card discordarem sobre o mesmo dado.
    const surface = screen.getByLabelText("Comparação real × cenário");
    const deficitCard = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    // Estado do Termômetro sobre o mínimo mensal (90.000 → "Apertado"), igual ao banner.
    expect(deficitCard.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      saldoBand(90_000).label,
    );
    // Manchete e evidência mostram o mínimo mensal, não um R$ 0 fabricado.
    expect(deficitCard.querySelector(".scn-kpi__headline")?.textContent).toBe(
      fmtCompactBRL(90_000),
    );
    expect(within(deficitCard).getByText("R$ 900,00")).toBeInTheDocument();
    expect(within(deficitCard).queryByText("R$ 0,00")).not.toBeInTheDocument();
    // Delta derivado dos mesmos números da evidência (90.000 − 100.000 = −R$ 100,00): o
    // backend não manda delta quando o deficit diário falta num dos ramos.
    expect(within(deficitCard).getByText("−R$ 100,00")).toBeInTheDocument();
    expect(deficitCard.querySelector(".lucide-trending-down")).toBeInTheDocument();
    expect(deficitCard).toHaveAttribute(
      "aria-label",
      `Buraco do futuro: Apertado, real ${fmtBRL(100_000)}, cenário ${fmtBRL(90_000)}`,
    );
  });

  it("veredito sem projeção nenhuma (deficit nulo + month_end vazio) não inventa menor saldo", async () => {
    const compare = baseCompare({
      scenario_deepest_deficit: null,
      deepest_deficit_delta_cents: null,
      scenario_month_end: [],
    });
    await renderCompare(compare);

    const banner = document.querySelector(".scn-verdict");
    expect(banner).toHaveClass("scn-verdict--ok");
    expect(banner!.querySelector(".scn-verdict__subline")?.textContent).toBe(
      "Sem pontos de projeção no horizonte para apontar um menor saldo.",
    );
  });

  it("veredito e o primeiro card (Buraco do futuro) não repetem a mesma frase literal", async () => {
    const compare = baseCompare({
      scenario_deepest_deficit: { date: "2026-07-01", balance_cents: -50_000 },
    });
    await renderCompare(compare);

    const headline = document.querySelector(".scn-verdict__headline")!.textContent;
    const surface = screen.getByLabelText("Comparação real × cenário");
    const firstCard = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    expect(firstCard.textContent).not.toContain(headline);
  });

  // --- Estados do método (Nível 2) ---

  it("'Buraco do futuro' e 'Saldo no fim' usam saldoBand (Termômetro) verbatim — sem transição quando a banda é igual", async () => {
    const compare = baseCompare({
      real_deepest_deficit: { date: "2026-07-01", balance_cents: 300_000 },
      scenario_deepest_deficit: { date: "2026-07-01", balance_cents: 250_000 },
      deepest_deficit_delta_cents: -50_000,
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;
    const realBand = saldoBand(300_000);
    const scenarioBand = saldoBand(250_000);
    expect(realBand.key).toBe(scenarioBand.key); // mesma banda ("Folga") — sem transição
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      scenarioBand.label,
    );
    expect(card.querySelector(".scn-kpi__state-origin")).not.toBeInTheDocument();
    // O bloco de estado é visual-only: a fonte única para o leitor de tela é o aria-label
    // do article (já testado noutro describe) — nunca dobrar o anúncio.
    expect(card.querySelector(".scn-kpi__state")?.getAttribute("aria-hidden")).toBe(
      "true",
    );
  });

  it("Performance sem transição: mesmo estado (performanceStatus verbatim) nos dois ramos não mostra 'Antes:'", async () => {
    const compare = baseCompare({
      real_performance_cents: 100_000,
      scenario_performance_cents: 80_000,
      performance_delta_cents: -20_000,
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Performance · mês atual" })
      .closest("article")!;
    expect(performanceStatus(100_000).label).toBe(performanceStatus(80_000).label);
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      performanceStatus(80_000).label,
    );
    expect(card.querySelector(".scn-kpi__state-origin")).not.toBeInTheDocument();
  });

  it("Custo de vida: 'Faltou'/'Acima da renda' são quebras reais — estado em vermelho cheio (não âmbar)", async () => {
    const compare = baseCompare({
      real_performance_cents: -10_000,
      real_cost_of_living_cents: 450_000,
      real_income_cents: 400_000,
      scenario_performance_cents: -10_000,
      scenario_cost_of_living_cents: 450_000,
      scenario_income_cents: 400_000,
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    expect(custoVidaStatus(450_000, 400_000).label).toBe("Acima da renda");
    const custoCard = within(surface)
      .getByRole("button", { name: "Custo de vida" })
      .closest("article")!;
    expect(custoCard.querySelector(".scn-kpi__state")).toHaveStyle({
      color: "var(--danger-400)",
    });

    expect(performanceStatus(-10_000).label).toBe("Faltou dinheiro");
    const perfCard = within(surface)
      .getByRole("button", { name: "Performance · mês atual" })
      .closest("article")!;
    expect(perfCard.querySelector(".scn-kpi__state")).toHaveStyle({
      color: "var(--danger-400)",
    });
  });

  it("Custo de vida: transição real↔cenário renderiza a hero NOVA com 'Antes: …' empilhado (nunca inline)", async () => {
    const compare = baseCompare({
      real_cost_of_living_cents: 300_000,
      real_income_cents: 400_000,
      scenario_cost_of_living_cents: 450_000,
      scenario_income_cents: 400_000,
    });
    await renderCompare(compare);

    expect(custoVidaStatus(300_000, 400_000).label).toBe("Dentro da renda");
    expect(custoVidaStatus(450_000, 400_000).label).toBe("Acima da renda");

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Custo de vida" })
      .closest("article")!;
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      "Acima da renda",
    );
    const origin = card.querySelector(".scn-kpi__state-origin");
    expect(origin?.textContent).toBe("Antes: Dentro da renda");
    // Empilhado: dois elementos DOM distintos, nunca "Dentro da renda → Acima da renda" num só nó.
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).not.toContain("→");
    expect(origin?.parentElement).toBe(
      card.querySelector(".scn-kpi__state")!.parentElement,
    );
  });

  it("'Pode gastar hoje' > 0 mostra 'Livre até {compacto}'", async () => {
    const compare = baseCompare({
      real_safe_to_spend_today_cents: 15_000,
      scenario_safe_to_spend_today_cents: 42_000,
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Pode gastar hoje" })
      .closest("article")!;
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe(
      `Livre até ${fmtCompactBRL(42_000)}`,
    );
    expect(card.querySelector(".scn-kpi__state-line")).not.toBeInTheDocument();
  });

  it("'Pode gastar hoje' == 0 e limitado pela poupança mostra 'Segure hoje' + linha derivada do guardrail", async () => {
    const compare = baseCompare({
      scenario_safe_to_spend_today_cents: 0,
      scenario_binding_guardrail: "savings",
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Pode gastar hoje" })
      .closest("article")!;
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe("Segure hoje");
    expect(card.querySelector(".scn-kpi__state-line")?.textContent).toBe(
      "Limitado pela régua de poupança (20–30% ao ano), não pelo caixa.",
    );
    // O porquê chega ao leitor de tela: a linha visual é aria-hidden, então a razão do
    // guardrail PRECISA estar no aria-label do card — "Segure hoje" sem o porquê é mudo.
    expect(card.getAttribute("aria-label")).toContain(
      "Limitado pela régua de poupança (20–30% ao ano), não pelo caixa.",
    );
  });

  it("'Pode gastar hoje' == 0 e limitado pelo caixa mostra a linha derivada do caixa (não da poupança)", async () => {
    const compare = baseCompare({
      scenario_safe_to_spend_today_cents: 0,
      scenario_binding_guardrail: "cash",
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const card = within(surface)
      .getByRole("button", { name: "Pode gastar hoje" })
      .closest("article")!;
    expect(card.querySelector(".scn-kpi__state-word")?.textContent).toBe("Segure hoje");
    expect(card.querySelector(".scn-kpi__state-line")?.textContent).toBe(
      "Limitado pelo caixa do mês, não pela régua de poupança.",
    );
    // Mesmo requisito de a11y do caso "savings": a razão viaja no aria-label.
    expect(card.getAttribute("aria-label")).toContain(
      "Limitado pelo caixa do mês, não pela régua de poupança.",
    );
  });

  it("ordena os cards por prioridade de decisão: Buraco, Saldo no fim, Pode gastar, Performance, Custo de vida", async () => {
    await renderCompare(baseCompare());

    const surface = screen.getByLabelText("Comparação real × cenário");
    const labels = Array.from(surface.querySelectorAll(".scn-kpi")).map((el) =>
      el.querySelector(".scn-kpi__label")?.textContent?.trim(),
    );
    expect(labels).toEqual([
      "Buraco do futuro",
      "Saldo no fim do horizonte",
      "Pode gastar hoje",
      "Performance · mês atual",
      "Custo de vida",
    ]);
  });
});

describe("ScenarioCompare — polimento (plano 074, fatia C)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  async function renderCompare(compare: ScenarioCompareDto) {
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_scenarios_cmd: [
        { id: "scn-1", name: compare.scenario_name, person_id: "p1" },
      ],
      get_scenario_forecast_cmd: compare,
      list_scenario_transactions_cmd: [],
      list_obligations_cmd: [],
    });
    render(<HorizonteScreen />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Simular cenário" }));
    await user.click(
      await screen.findByRole("button", { name: compare.scenario_name }),
    );
    await screen.findByText(`Cenário: ${compare.scenario_name}`);
    return user;
  }

  function loanFixture(reserveMonths: number | null) {
    return {
      loan_principal_cents: 1_000_000,
      loan_installment_cents: 50_000,
      loan_term_months: 24,
      loan_monthly_rate_bps: 150,
      loan_total_paid_cents: 1_200_000,
      loan_total_cost_cents: 200_000,
      reserve_months_after_financing: reserveMonths,
    };
  }

  // --- Semáforo da reserva pós-financiamento (item 2) ---

  it.each([
    [5.9, "Abaixo do mínimo", "lucide-triangle-alert"],
    [6, "Zona amarela", "lucide-triangle-alert"],
    [8, "Zona amarela", "lucide-triangle-alert"],
    [8.1, "Confortável", "lucide-circle-check"],
    [12, "Confortável", "lucide-circle-check"],
    [12.1, "Paz", "lucide-circle-check"],
  ] as const)(
    "%s meses de reserva pós-financiamento → '%s' (ícone %s)",
    async (months, label, iconClass) => {
      await renderCompare(baseCompare({ loan: loanFixture(months) }));
      const badge = document.querySelector(".scn-loan-summary__reserve");
      expect(badge).not.toBeNull();
      expect(badge!.textContent).toContain(label);
      expect(badge!.querySelector(`.${iconClass}`)).toBeInTheDocument();
    },
  );

  it("reserve_months_after_financing nulo não renderiza nada novo", async () => {
    await renderCompare(baseCompare({ loan: loanFixture(null) }));
    expect(
      document.querySelector(".scn-loan-summary__reserve"),
    ).not.toBeInTheDocument();
    // O resto do resumo do empréstimo continua normal.
    expect(screen.getByText("Custo do crédito")).toBeInTheDocument();
  });

  // --- "Buraco do futuro" sem projeção nenhuma (item 5, residual da fatia B) ---

  it("'Buraco do futuro' sem nenhum ponto de projeção do cenário mostra vazio neutro, nunca 'Apertado R$ 0' fake", async () => {
    const compare = baseCompare({
      scenario_deepest_deficit: null,
      deepest_deficit_delta_cents: null,
      scenario_month_end: [],
      month_end: [],
    });
    await renderCompare(compare);

    const surface = screen.getByLabelText("Comparação real × cenário");
    const deficitCard = within(surface)
      .getByRole("button", { name: "Buraco do futuro" })
      .closest("article")!;

    expect(within(deficitCard).queryByText(/Apertado/)).not.toBeInTheDocument();
    expect(deficitCard.querySelector(".scn-kpi__state-word")?.textContent).toBe("—");
    expect(deficitCard.querySelector(".scn-kpi__headline")?.textContent).toBe("—");
    // Ícone neutro (nunca alerta/check fingindo um estado que não existe) + cor faint (nunca
    // cor de estado).
    expect(deficitCard.querySelector(".lucide-minus")).toBeInTheDocument();
    expect(deficitCard.querySelector<HTMLElement>(".scn-kpi__state")!.style.color).toBe(
      "var(--text-faint)",
    );
    // Sem chip de delta (nada pra comparar) nem linha "Antes:" (não é uma transição de estado).
    expect(deficitCard.querySelector(".scn-kpi__delta")).not.toBeInTheDocument();
    expect(deficitCard.querySelector(".scn-kpi__state-origin")).not.toBeInTheDocument();
    expect(deficitCard).toHaveAttribute(
      "aria-label",
      `Buraco do futuro: sem dados de projeção do cenário, real ${fmtBRL(100_000)}`,
    );
  });

  // --- Nota do pior mês em formato compacto (item 3) ---

  it("nota do pior mês do DiffSparkline usa o formato compacto (nunca a precisão cheia)", async () => {
    const compare = baseCompare({
      month_end: [
        {
          year: 2026,
          month: 6,
          real_balance_cents: 500_000,
          scenario_balance_cents: 480_000,
          delta_cents: -20_000,
        },
        {
          year: 2026,
          month: 9,
          real_balance_cents: 500_000,
          scenario_balance_cents: -1_050_000,
          delta_cents: -1_550_000,
        },
      ],
    });
    await renderCompare(compare);

    // Mesmo padrão de título do DualLineChart logo acima (fatia C, item 3).
    expect(document.querySelector(".scn-diffchart__head")).not.toBeNull();
    const note = document.querySelector(".scn-worst-note");
    expect(note?.textContent).toBe(`Pior mês: Setembro ${fmtCompactBRL(-1_550_000)}`);
    // Nunca a precisão cheia sem quebra na nota visível (ela mora só no aria-label do SVG).
    expect(note?.textContent).not.toMatch(/15\.500,00/);
  });
});
