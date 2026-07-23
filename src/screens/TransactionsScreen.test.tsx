import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";
import { crumbOverridesSnapshot } from "../shell/crumbStore";
import type { TransactionRow } from "../lib/api";
import type * as FormatModule from "../lib/format";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// `TODAY` no componente é const module-level (avaliado no import, antes do
// setSystemTime do beforeEach). Fixar todayISO alinha "hoje" ao relógio
// congelado (2026-06-20) e mata a armadilha de time-bomb nos testes de hoje.
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

/** ISO date in the CURRENT month, so rows land in the month view regardless of clock. */
function currentMonthISO(day = 15): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

describe("TransactionsScreen (Lançamentos)", () => {
  beforeEach(() => {
    // A tela abre no mês corrente; congela o relógio em junho/2026 para alinhar
    // com as fixtures datadas (2026-06-…) em qualquer data real de execução.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-20T12:00:00-03:00"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the ledger with the loaded transactions", async () => {
    const rows = TXNS.map((t, i) => ({ ...t, date: currentMonthISO(10 + i) }));
    mockCommands({ get_recent_transactions: rows });
    renderLedger();
    expect(await screen.findByText(rows[0]!.description)).toBeInTheDocument();
  });

  it("publica o mês visto no crumb da appbar e limpa ao desmontar", async () => {
    mockCommands({ get_recent_transactions: [] });
    const { unmount } = renderLedger();
    await waitFor(() =>
      expect(crumbOverridesSnapshot()).toEqual({ lancamentos: "Junho de 2026" }),
    );
    unmount();
    expect(crumbOverridesSnapshot()).toEqual({});
  });

  it("explode a nota em linhas de item sob o cabeçalho de célula", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "cel-12",
        amount: 810_158,
        description: "Saída",
        date: currentMonthISO(12),
        payment_method: "debit",
        is_fixed: true,
        line_items: [
          {
            id: "li-1",
            transaction_id: "cel-12",
            amount_cents: 400_066,
            description: "Bradesco João",
            position: 0,
            kind: "cartao",
            section: "CARTÕES |",
          },
          {
            id: "li-2",
            transaction_id: "cel-12",
            amount_cents: 407_764,
            description: "Bradesco Gio",
            position: 1,
            kind: "cartao",
            section: null,
          },
          {
            id: "li-3",
            transaction_id: "cel-12",
            amount_cents: 2_298,
            description: "Inter",
            position: 2,
            kind: "cartao",
            section: null,
          },
        ],
      },
    ];
    mockCommands({ get_recent_transactions: txns });
    renderLedger();

    // Os itens são linhas de primeira classe (não escondidos atrás de expansão).
    expect(
      await screen.findByRole("button", { name: /^Bradesco João/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Bradesco Gio/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Inter/ })).toBeInTheDocument();
    // O cabeçalho de célula declara a coluna e carrega o total como autoridade.
    expect(screen.getByText("Saída — Total da célula")).toBeInTheDocument();
    expect(screen.getByText(/8\.101,58/)).toBeInTheDocument();
  });

  it("acusa a diferença célula×nota com selo e linha sintética — nunca item", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "cel-dif",
        amount: 810_158,
        description: "Saída",
        date: currentMonthISO(12),
        payment_method: "debit",
        is_fixed: true,
        line_items: [
          {
            id: "li-a",
            transaction_id: "cel-dif",
            amount_cents: 810_128,
            description: "Único item",
            position: 0,
            kind: "cartao",
            section: null,
          },
        ],
      },
    ];
    mockCommands({ get_recent_transactions: txns });
    renderLedger();

    expect(await screen.findByText("Com diferença")).toBeInTheDocument();
    const dif = screen.getByText("Diferença no detalhamento");
    expect(dif).toBeInTheDocument();
    // A linha sintética não é interativa (nunca um item da lista de ações).
    expect(dif.closest("li")).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText(/0,30/)).toBeInTheDocument();
  });

  it("busca ativa esconde a reconciliação (subconjunto não compara com a célula)", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "cel-dif",
        amount: 810_158,
        description: "Saída",
        date: currentMonthISO(12),
        payment_method: "debit",
        is_fixed: true,
        line_items: [
          {
            id: "li-a",
            transaction_id: "cel-dif",
            amount_cents: 400_066,
            description: "Bradesco João",
            position: 0,
            kind: "cartao",
            section: null,
          },
          {
            id: "li-b",
            transaction_id: "cel-dif",
            amount_cents: 407_764,
            description: "Bradesco Gio",
            position: 1,
            kind: "cartao",
            section: null,
          },
        ],
      },
    ];
    mockCommands({ get_recent_transactions: txns });
    renderLedger();
    await screen.findByText("Com diferença");

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const search = screen.getAllByLabelText("Buscar lançamento")[0]!;
    await user.type(search, "Gio");

    expect(screen.queryByText("Com diferença")).not.toBeInTheDocument();
    expect(screen.queryByText("Diferença no detalhamento")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Bradesco Gio/ })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^Bradesco João/ }),
    ).not.toBeInTheDocument();
  });

  it("pílulas de metadado moram junto do nome: parcela, reembolso e previsto", async () => {
    const rows: TransactionRow[] = [
      {
        ...TXNS[1]!,
        id: "rec-1:9",
        description: "Financiamento",
        date: currentMonthISO(7),
        installment_index: 10,
        installment_total: 36,
        has_refund_link: true,
      },
      {
        ...TXNS[1]!,
        id: "fut-1",
        description: "Conta futura",
        date: currentMonthISO(28),
        is_projection: true,
      },
    ];
    mockCommands({ get_recent_transactions: rows });
    renderLedger();

    const row = await screen.findByRole("button", { name: /^Financiamento/ });
    expect(row).toHaveTextContent("10/36");
    expect(row).toHaveTextContent("Reembolso");
    // O lançamento futuro vive no disclosure do mês corrente.
    expect(screen.getByText("O que ainda vem neste mês")).toBeInTheDocument();
  });

  it("filtro por tipo seleciona células inteiras", async () => {
    const rows = [
      {
        ...TXNS[1]!,
        id: "fix-1",
        description: "Aluguel",
        date: currentMonthISO(10),
        payment_method: "debit",
        is_fixed: true,
      },
      {
        ...TXNS[1]!,
        id: "card-1",
        description: "Compra no crédito",
        date: currentMonthISO(11),
        payment_method: "credit",
      },
    ];
    mockCommands({ get_recent_transactions: rows });
    renderLedger();
    await screen.findByRole("button", { name: /^Aluguel/ });

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await user.click(screen.getByRole("button", { name: "Cartão" }));

    expect(screen.queryByRole("button", { name: /^Aluguel/ })).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^Compra no crédito/ }),
    ).toBeInTheDocument();
  });

  it("vazio com busca cita o termo e o mês", async () => {
    mockCommands({ get_recent_transactions: [] });
    renderLedger();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const search = (await screen.findAllByLabelText("Buscar lançamento"))[0]!;
    await user.type(search, "pix");
    expect(
      screen.getByText('Nada em junho para "pix". Limpe a busca ou troque o filtro.'),
    ).toBeInTheDocument();
  });

  it("daymark carrega o Saldo encadeado, colorido pela banda", async () => {
    const rows = [
      { ...TXNS[1]!, id: "saldo-1", description: "Aluguel", date: currentMonthISO(3) },
    ];
    mockCommands({
      get_recent_transactions: rows,
      get_month_grid: [
        {
          date: currentMonthISO(3),
          day: 3,
          income_cents: 0,
          fixed_out_cents: 180_000,
          daily_out_cents: 0,
          balance_cents: 38_000, // R$ 380,00 → banda "apertado"
        },
      ],
    });
    renderLedger();

    expect(await screen.findByLabelText(/Saldo do dia.*380,00/)).toBeInTheDocument();
    expect(screen.getByText(/3 de junho/)).toBeInTheDocument();
  });

  it("daymark de hoje traz o chip 'Hoje'", async () => {
    const todayIso = currentMonthISO(20);
    const rows = [
      { ...TXNS[1]!, id: "hoje-1", description: "Compra de hoje", date: todayIso },
    ];
    mockCommands({ get_recent_transactions: rows, get_month_grid: [] });
    renderLedger();

    expect(await screen.findByText("Hoje")).toBeInTheDocument();
    expect(screen.getAllByText("Hoje")).toHaveLength(1);
  });

  it("dia sem Saldo no grid não renderiza a pílula", async () => {
    const rows = [
      { ...TXNS[1]!, id: "saldo-2", description: "Aluguel", date: currentMonthISO(3) },
    ];
    mockCommands({
      get_recent_transactions: rows,
      get_month_grid: [
        {
          date: currentMonthISO(3),
          day: 3,
          income_cents: 0,
          fixed_out_cents: 180_000,
          daily_out_cents: 0,
          balance_cents: null,
        },
      ],
    });
    renderLedger();

    await screen.findByRole("button", { name: /^Aluguel/ });
    expect(screen.queryByLabelText(/Saldo do dia/)).not.toBeInTheDocument();
  });
});

// Apagar uma série recorrente com escopo (só esta / em diante / toda a série).
describe("TransactionsScreen — apagar série recorrente", () => {
  // Uma ocorrência de série no mês corrente (aparece na visão padrão).
  function seriesRow(): TransactionRow {
    const d = new Date();
    const iso = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-15`;
    return {
      ...TXNS[1]!,
      id: "rec-abc:2",
      description: "Aluguel",
      date: iso,
      provenance: "manual",
      installment_index: 3,
      installment_total: 12,
    };
  }

  async function openRowActions() {
    const row = await screen.findByRole("button", { name: /^Aluguel/ });
    await userEvent.click(row);
    return screen.getByRole("button", { name: "Apagar da série" });
  }

  beforeEach(() => {
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("labels the delete action 'Apagar da série' on a recurring occurrence", async () => {
    mockCommands({ get_recent_transactions: [seriesRow()], list_tags: [] });
    renderLedger();
    expect(await openRowActions()).toBeInTheDocument();
  });

  it("OK no 1º confirm apaga TODA a série (recurrence_id derivado do id)", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    mockCommands({
      get_recent_transactions: [seriesRow()],
      list_tags: [],
      delete_series_all_cmd: 10,
    });
    renderLedger();
    await userEvent.click(await openRowActions());

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("delete_series_all_cmd", {
        recurrenceId: "rec-abc",
      }),
    );
  });

  it("2º confirm OK apaga esta e as futuras (delete_series_from)", async () => {
    vi.spyOn(window, "confirm")
      .mockReturnValueOnce(false) // não é "toda a série"
      .mockReturnValueOnce(true); // sim, "esta e as futuras"
    mockCommands({
      get_recent_transactions: [seriesRow()],
      list_tags: [],
      delete_series_from_cmd: 8,
    });
    renderLedger();
    await userEvent.click(await openRowActions());

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("delete_series_from_cmd", {
        transactionId: "rec-abc:2",
      }),
    );
  });

  it("ambos confirms cancelados apagam só esta ocorrência", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    mockCommands({
      get_recent_transactions: [seriesRow()],
      list_tags: [],
      delete_transaction_cmd: null,
    });
    renderLedger();
    await userEvent.click(await openRowActions());

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("delete_transaction_cmd", {
        id: "rec-abc:2",
      }),
    );
  });
});
