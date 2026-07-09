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

  it("saves Saída with the canonical debit payment method (kindToFields)", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_pockets: EMPTY_POCKETS,
      create_transaction: "txn-saida",
    });

    render(
      <Compose
        open
        options={{ mode: "new", type: "saida", date: "2026-06-23" }}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Valor único"), "300,00");
    await user.click(screen.getByRole("button", { name: "Salvar lançamento" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "create_transaction",
        expect.objectContaining({
          txnType: "expense",
          amountCents: 30000,
          date: "2026-06-23",
          paymentMethod: "debit",
          isFixed: true,
        }),
      );
    });
  });

  it("falha ao salvar mostra erro e não fecha o drawer", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onSaved = vi.fn();

    mockCommands({
      get_pockets: EMPTY_POCKETS,
      create_transaction: () => Promise.reject(new Error("database is locked")),
    });

    render(
      <Compose
        open
        options={{ mode: "new", type: "saida", date: "2026-06-23" }}
        onClose={onClose}
        onSaved={onSaved}
      />,
    );

    await user.type(screen.getByLabelText("Valor único"), "10,00");
    await user.click(screen.getByRole("button", { name: "Salvar lançamento" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/ocupado/i);
    expect(onClose).not.toHaveBeenCalled();
    expect(onSaved).not.toHaveBeenCalled();
  });

  it("retry após falha limpa o erro e salva", async () => {
    const user = userEvent.setup();
    const onSaved = vi.fn();
    let calls = 0;

    mockCommands({
      get_pockets: EMPTY_POCKETS,
      create_transaction: () => {
        calls += 1;
        return calls === 1
          ? Promise.reject(new Error("database is locked"))
          : Promise.resolve("txn-ok");
      },
    });

    render(
      <Compose
        open
        options={{ mode: "new", type: "saida", date: "2026-06-23" }}
        onClose={vi.fn()}
        onSaved={onSaved}
      />,
    );

    await user.type(screen.getByLabelText("Valor único"), "10,00");
    await user.click(screen.getByRole("button", { name: "Salvar lançamento" }));

    await screen.findByRole("alert");
    await user.click(screen.getByRole("button", { name: "Salvar lançamento" }));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
