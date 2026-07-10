import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";
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

/** ISO date in the CURRENT month, so rows land in the default "Por mês" view regardless of clock. */
function currentMonthISO(day = 15): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

describe("TransactionsScreen (Lançamentos)", () => {
  beforeEach(() => {
    // A tela abre no "Por mês" do MÊS CORRENTE; congela o relógio em junho/2026 para
    // alinhar com as fixtures datadas (2026-06-…) em qualquer data real de execução.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-20T12:00:00-03:00"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the ledger with the loaded transactions", async () => {
    // Datas no mês corrente para o default "Por mês" (independe do relógio).
    const rows = TXNS.map((t, i) => ({ ...t, date: currentMonthISO(10 + i) }));
    mockCommands({ get_recent_transactions: rows });
    renderLedger();
    expect(await screen.findByText(rows[2]!.description)).toBeInTheDocument();
  });

  it("opens in Por mês view by default and lists it first", async () => {
    mockCommands({ get_recent_transactions: TXNS });
    renderLedger();

    const month = await screen.findByRole("radio", { name: "Por mês" });
    const timeline = screen.getByRole("radio", { name: "Linha do tempo" });

    expect(month).toHaveAttribute("aria-checked", "true");
    expect(timeline).toHaveAttribute("aria-checked", "false");
    expect(
      month.compareDocumentPosition(timeline) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("shows kind badges and a quiet divergence marker for itemized rows", async () => {
    const txns = [
      {
        ...TXNS[1]!,
        id: "itemized-061",
        amount: 10_000,
        description: "Despesa itemizada",
        date: currentMonthISO(15),
        payment_method: "debit",
        is_fixed: true,
        line_items: [
          {
            id: "li-card",
            transaction_id: "itemized-061",
            amount_cents: 4_000,
            description: "Compra crédito",
            position: 0,
            kind: "cartao",
          },
          {
            id: "li-saida",
            transaction_id: "itemized-061",
            amount_cents: 3_000,
            description: "Conta fixa",
            position: 1,
            kind: "saida",
          },
        ],
      },
    ];
    mockCommands({ get_recent_transactions: txns });
    renderLedger();

    // userEvent + fake timers: os delays internos precisam avançar o relógio falso.
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const row = await screen.findByRole("button", { name: "Despesa itemizada" });
    await user.click(row);

    expect(screen.getByLabelText("Item classificado como Cartão")).toBeInTheDocument();
    expect(screen.getByLabelText("Item classificado como Saída")).toBeInTheDocument();
    expect(screen.getByText("Itens não batem")).toBeInTheDocument();
  });

  it("items of an income row carry the Entrada badge, never Saída", async () => {
    const txns = [
      {
        ...TXNS[2]!,
        id: "income-itemized",
        amount: 300_264,
        description: "Entrada itemizada",
        date: currentMonthISO(12),
        line_items: [
          {
            id: "li-inc-1",
            transaction_id: "income-itemized",
            amount_cents: 257_764,
            description: "salário",
            position: 0,
            kind: "entrada",
          },
          {
            id: "li-inc-2",
            transaction_id: "income-itemized",
            amount_cents: 42_500,
            description: "reembolso",
            position: 1,
            kind: "entrada",
          },
        ],
      },
    ];
    mockCommands({ get_recent_transactions: txns });
    renderLedger();

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const row = await screen.findByRole("button", { name: "Entrada itemizada" });
    await user.click(row);

    expect(screen.getAllByLabelText("Item classificado como Entrada")).toHaveLength(2);
    expect(
      screen.queryByLabelText("Item classificado como Saída"),
    ).not.toBeInTheDocument();
  });

  it("Por mês: cabeçalho do dia mostra o Saldo encadeado, colorido pela banda", async () => {
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

    // O grupo do dia agrupa os lançamentos e o cabeçalho carrega o Saldo do fim
    // do dia com rótulo acessível; o rótulo do dia é "weekday, dia" (03/06 = Qua).
    expect(await screen.findByLabelText(/Saldo do dia.*380,00/)).toBeInTheDocument();
    expect(screen.getByText("Qua, 3")).toBeInTheDocument();
  });

  it("Por mês: cabeçalho do dia de hoje traz o chip 'Hoje' (sem selo redundante na linha)", async () => {
    // Relógio congelado em 2026-06-20 → hoje.
    const todayIso = currentMonthISO(20);
    const rows = [
      { ...TXNS[1]!, id: "hoje-1", description: "Compra de hoje", date: todayIso },
    ];
    mockCommands({
      get_recent_transactions: rows,
      get_month_grid: [
        {
          date: todayIso,
          day: 20,
          income_cents: 0,
          fixed_out_cents: 5_000,
          daily_out_cents: 0,
          balance_cents: 300_000,
        },
      ],
    });
    renderLedger();

    // O marcador de hoje é um chip textual "Hoje" no cabeçalho do dia — e é o
    // ÚNICO "Hoje" na tela (o selo redundante da linha foi removido).
    expect(await screen.findByText("Hoje")).toBeInTheDocument();
    expect(screen.getAllByText("Hoje")).toHaveLength(1);
  });

  it("Por mês: dia sem Saldo no grid não renderiza a pílula", async () => {
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

    await screen.findByRole("button", { name: "Aluguel" });
    expect(screen.queryByLabelText(/Saldo do dia/)).not.toBeInTheDocument();
  });
});

// Feature 3: apagar uma série recorrente com escopo (só esta / em diante / toda a série).
describe("TransactionsScreen — apagar série recorrente", () => {
  // Uma ocorrência de série no mês corrente (aparece na visão "Por mês" padrão).
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
    const row = await screen.findByRole("button", { name: "Aluguel" });
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
