import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { HorizonteScreen } from "./HorizonteScreen";
import { stripScenarioMarker, addMonthsISO } from "../lib/scenarioHelpers";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";
import type { ScenarioCompareDto } from "../lib/api";

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
    scenario_month_end: [{ year: 2026, month: 12, balance_cents: 350_000 }],
    scenario_deepest_deficit: { date: "2026-07-01", balance_cents: -50_000 },
    scenario_performance_cents: 150_000,
    scenario_safe_to_spend_today_cents: 8_000,
    scenario_binding_guardrail: "cash",
    scenario_cost_of_living_cents: 350_000,
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
    expect(await screen.findByText("Empréstimo parcela 1/3")).toBeInTheDocument();
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
    // Buraco do futuro: real R$1.000,00 → cenário −R$500,00.
    expect(within(surface).getByText("R$ 1.000,00")).toBeInTheDocument();
    expect(within(surface).getByText("−R$ 500,00")).toBeInTheDocument();
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

    expect(await screen.findByText("↔ alterou")).toBeInTheDocument();
    expect(screen.getByText("R$ 1.500,00 → R$ 1.200,00")).toBeInTheDocument();
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
