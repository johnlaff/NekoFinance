import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";
import type { TransactionRow } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

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
    expect(screen.getByText("itens não batem")).toBeInTheDocument();
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
