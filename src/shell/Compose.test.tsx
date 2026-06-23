import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { Compose } from "./Compose";
import { EMPTY_POCKETS, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("Compose", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("saves Cartão with the engine payment method literal", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_pockets: EMPTY_POCKETS,
      create_transaction: "txn-cartao",
    });

    render(
      <Compose
        open
        options={{ mode: "new", type: "cartao", date: "2026-06-23" }}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Valor único"), "123,45");
    await user.click(screen.getByRole("button", { name: "Salvar lançamento" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "create_transaction",
        expect.objectContaining({
          txnType: "expense",
          amountCents: 12345,
          date: "2026-06-23",
          paymentMethod: "credit",
          isFixed: false,
        }),
      );
    });
  });
});
