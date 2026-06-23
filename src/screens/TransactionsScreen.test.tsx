import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";

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

describe("TransactionsScreen (Lançamentos)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the ledger with the loaded transactions", async () => {
    mockCommands({ get_recent_transactions: TXNS });
    renderLedger();
    // O default "Por mês" mostra o mês atual do fixture.
    expect(await screen.findByText(TXNS[2]!.description)).toBeInTheDocument();
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
        date: "2026-06-15",
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

    const row = await screen.findByRole("button", { name: "Despesa itemizada" });
    await userEvent.click(row);

    expect(screen.getByLabelText("Item classificado como Cartão")).toBeInTheDocument();
    expect(screen.getByLabelText("Item classificado como Saída")).toBeInTheDocument();
    expect(screen.getByText("itens não batem")).toBeInTheDocument();
  });
});
