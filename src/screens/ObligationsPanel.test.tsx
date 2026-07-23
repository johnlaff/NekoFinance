import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";
import type * as FormatModule from "../lib/format";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mesmo congelamento de relógio do TransactionsScreen.test.tsx: `TODAY` é const
// module-level, então precisa casar com o "hoje" usado nas fixtures.
vi.mock("../lib/format", async (importOriginal) => {
  const actual = await importOriginal<typeof FormatModule>();
  return { ...actual, todayISO: () => "2026-06-20" };
});

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderLedger() {
  return render(
    <NekoAppProvider value={app}>
      <TransactionsScreen />
    </NekoAppProvider>,
  );
}

function currentMonthISO(day = 15): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

describe("Obrigações recorrentes (plano 069)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-20T12:00:00-03:00"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("o painel de marcação mostra a contagem da prévia antes de salvar", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "itemized-aluguel",
        amount: 150_000,
        description: "Despesa itemizada",
        date: currentMonthISO(5),
        payment_method: "debit",
        is_fixed: true,
        line_items: [
          {
            id: "li-aluguel",
            transaction_id: "itemized-aluguel",
            amount_cents: 150_000,
            description: "Aluguel",
            position: 0,
            kind: "saida",
            section: "CONTAS:",
          },
        ],
      },
    ];
    mockCommands({
      get_recent_transactions: txns,
      list_obligations_cmd: [],
      preview_obligation_matches_cmd: [
        {
          line_item_id: "a",
          transaction_id: "t1",
          amount_cents: 150_000,
          description: "Aluguel",
          date: "2026-04-05",
        },
        {
          line_item_id: "li-aluguel",
          transaction_id: "itemized-aluguel",
          amount_cents: 150_000,
          description: "Aluguel",
          date: "2026-06-05",
        },
      ],
    });
    renderLedger();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    // No Livro-razão célula×nota o item da nota É a linha; o painel dela carrega a ação.
    const row = await screen.findByRole("button", { name: /^Aluguel/ });
    await user.click(row);

    const markBtn = await screen.findByLabelText(
      'Marcar "Aluguel" como obrigação recorrente',
    );
    await user.click(markBtn);

    // Prévia confirmada obrigatória: mostra o número ANTES de qualquer botão de salvar.
    await waitFor(() => {
      expect(screen.getByText("Isto vai agrupar 2 lançamentos.")).toBeInTheDocument();
    });

    // O botão de confirmar só aparece dentro do painel de prévia — nunca some direto.
    expect(screen.getByRole("button", { name: "Confirmar" })).toBeInTheDocument();
  });

  it("confirmar cria a obrigação passando pela mesma regra mostrada na prévia", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "itemized-aluguel-2",
        amount: 150_000,
        description: "Despesa itemizada",
        date: currentMonthISO(5),
        line_items: [
          {
            id: "li-aluguel-2",
            transaction_id: "itemized-aluguel-2",
            amount_cents: 150_000,
            description: "Aluguel",
            position: 0,
            kind: "saida",
            section: "CONTAS:",
          },
        ],
      },
    ];
    let created: unknown = null;
    mockCommands({
      get_recent_transactions: txns,
      list_obligations_cmd: [],
      preview_obligation_matches_cmd: [
        {
          line_item_id: "li-aluguel-2",
          transaction_id: "itemized-aluguel-2",
          amount_cents: 150_000,
          description: "Aluguel",
          date: "2026-06-05",
        },
      ],
      create_obligation_cmd: (args) => {
        created = args;
        return "ob-1";
      },
    });
    renderLedger();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    // No Livro-razão célula×nota o item da nota É a linha; o painel dela carrega a ação.
    const row = await screen.findByRole("button", { name: /^Aluguel/ });
    await user.click(row);
    const markBtn = await screen.findByLabelText(
      'Marcar "Aluguel" como obrigação recorrente',
    );
    await user.click(markBtn);
    await waitFor(() => {
      expect(screen.getByText("Isto vai agrupar 1 lançamento.")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Confirmar" }));

    await waitFor(() => {
      expect(created).toMatchObject({
        name: "Aluguel",
        matchDesc: "Aluguel",
        matchSection: "CONTAS:",
      });
    });
  });

  it("desmarcar 'restringir à seção' envia matchSection nulo", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "itemized-aluguel-3",
        amount: 150_000,
        description: "Despesa itemizada",
        date: currentMonthISO(5),
        line_items: [
          {
            id: "li-aluguel-3",
            transaction_id: "itemized-aluguel-3",
            amount_cents: 150_000,
            description: "Aluguel",
            position: 0,
            kind: "saida",
            section: "CONTAS:",
          },
        ],
      },
    ];
    let created: unknown = null;
    mockCommands({
      get_recent_transactions: txns,
      list_obligations_cmd: [],
      preview_obligation_matches_cmd: [
        {
          line_item_id: "li-aluguel-3",
          transaction_id: "itemized-aluguel-3",
          amount_cents: 150_000,
          description: "Aluguel",
          date: "2026-06-05",
        },
      ],
      create_obligation_cmd: (args) => {
        created = args;
        return "ob-2";
      },
    });
    renderLedger();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    // No Livro-razão célula×nota o item da nota É a linha; o painel dela carrega a ação.
    const row = await screen.findByRole("button", { name: /^Aluguel/ });
    await user.click(row);
    const markBtn = await screen.findByLabelText(
      'Marcar "Aluguel" como obrigação recorrente',
    );
    await user.click(markBtn);
    await waitFor(() => {
      expect(screen.getByText("Isto vai agrupar 1 lançamento.")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("checkbox", { name: /Restringir à seção desta linha/i }),
    );
    await user.click(screen.getByRole("button", { name: "Confirmar" }));

    await waitFor(() => {
      expect(created).toMatchObject({
        name: "Aluguel",
        matchDesc: "Aluguel",
        matchSection: null,
      });
    });
  });

  it("a lista de obrigações expande o histórico mensal com média e total por mês", async () => {
    mockCommands({
      get_recent_transactions: [],
      list_obligations_cmd: [
        {
          id: "ob-1",
          person_id: "p1",
          name: "Aluguel",
          match_desc: "aluguel",
          match_section: "contas",
          kind: "saida",
        },
      ],
      obligation_history_cmd: [
        { year: 2026, month: 4, total_cents: 150_000, count: 1 },
        { year: 2026, month: 5, total_cents: 150_000, count: 1 },
      ],
    });
    renderLedger();

    expect(await screen.findByText("Aluguel")).toBeInTheDocument();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await user.click(
      screen.getByRole("button", { name: 'Ver histórico de "Aluguel"' }),
    );

    // Histórico por mês (a série que a planilha não guarda) — dois meses distintos.
    await waitFor(() => {
      expect(screen.getByText("Abr/2026")).toBeInTheDocument();
      expect(screen.getByText("Mai/2026")).toBeInTheDocument();
    });
    expect(screen.getAllByText("1 ocorrência")).toHaveLength(2);
    // Média dos meses expandidos + indicador de tendência estável (dois valores iguais).
    expect(screen.getByText("Média").parentElement).toHaveTextContent("R$ 1.500,00");
    expect(screen.getByLabelText("estável")).toBeInTheDocument();
  });

  it("obrigação sem histórico ainda mostra o estado vazio, não quebra", async () => {
    mockCommands({
      get_recent_transactions: [],
      list_obligations_cmd: [
        {
          id: "ob-2",
          person_id: "p1",
          name: "Internet",
          match_desc: "internet",
          match_section: null,
          kind: "saida",
        },
      ],
      obligation_history_cmd: [],
    });
    renderLedger();

    expect(await screen.findByText("Internet")).toBeInTheDocument();
    expect(screen.getByText("Sem ocorrências ainda")).toBeInTheDocument();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await user.click(
      screen.getByRole("button", { name: 'Ver histórico de "Internet"' }),
    );

    await waitFor(() => {
      expect(screen.getByText("Nenhuma ocorrência casada ainda.")).toBeInTheDocument();
    });
  });

  it("painel de marcação foca o nome ao abrir", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "itemized-aluguel-4",
        amount: 150_000,
        description: "Despesa itemizada",
        date: currentMonthISO(5),
        line_items: [
          {
            id: "li-aluguel-4",
            transaction_id: "itemized-aluguel-4",
            amount_cents: 150_000,
            description: "Aluguel",
            position: 0,
            kind: "saida",
            section: "CONTAS:",
          },
        ],
      },
    ];
    mockCommands({
      get_recent_transactions: txns,
      list_obligations_cmd: [],
      preview_obligation_matches_cmd: [],
    });
    renderLedger();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    // No Livro-razão célula×nota o item da nota É a linha; o painel dela carrega a ação.
    const row = await screen.findByRole("button", { name: /^Aluguel/ });
    await user.click(row);
    const markBtn = await screen.findByLabelText(
      'Marcar "Aluguel" como obrigação recorrente',
    );
    await user.click(markBtn);

    const nameInput = screen.getByRole("textbox", { name: "Nome da obrigação" });
    await waitFor(() => {
      expect(nameInput).toHaveFocus();
    });
  });
});
