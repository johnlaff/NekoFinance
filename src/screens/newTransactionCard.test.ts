import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { submitCardPurchase } from "./newTransactionCard";

describe("submitCardPurchase", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    window.localStorage.clear();
  });

  it("registra compra e reembolso em um único comando sem consultar faturas", async () => {
    mockCommands({
      register_card_purchase: "purchase-id",
      list_invoices: [
        {
          id: "wrong-invoice",
          cycle_month: "2026-12",
          closing_date: "2026-11-20",
          due_date: "2026-12-10",
          status: "aberta",
          stated_total_cents: 0,
          purchases_sum_cents: 0,
          effective_total_cents: 0,
          reconciliation_delta_cents: null,
        },
      ],
      create_refund_expectation: "refund-id",
    });

    await submitCardPurchase({
      cardId: "visa",
      cardRepeat: "never",
      installments: 1,
      refundAmount: "8,00",
      amountCents: 2_500,
      description: "Compra compartilhada",
      date: "2026-01-15",
      tagIds: ["pessoal"],
    });

    expect(mockInvoke).toHaveBeenCalledWith("register_card_purchase", {
      cardAccountId: "visa",
      amountCents: 2_500,
      description: "Compra compartilhada",
      date: "2026-01-15",
      refundCents: 800,
      tagIds: ["pessoal"],
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("list_invoices", expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith(
      "create_refund_expectation",
      expect.anything(),
    );
  });
});
