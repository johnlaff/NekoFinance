import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { NewTransactionForm } from "./NewTransactionForm";
import type { Tag } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const TAGS: Tag[] = [
  {
    id: "demo-a",
    name: "Categoria demo A",
    color: "#3aa",
    emoji: null,
    is_special: false,
  },
  { id: "pagar", name: "! Pagar", color: "#a83", emoji: null, is_special: true },
];

describe("NewTransactionForm", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("lança um diário variável com tag e dispara onCreated", async () => {
    const user = userEvent.setup();
    mockCommands({ list_tags_cmd: TAGS, create_transaction: "new-id" });
    const onCreated = vi.fn();
    render(<NewTransactionForm onCreated={onCreated} />);

    // Tags carregadas no mount.
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /Categoria demo A/ }),
      ).toBeInTheDocument(),
    );

    await user.type(screen.getByLabelText("Valor"), "42,50");
    await user.type(screen.getByLabelText("Descrição"), "Despesa demo");
    await user.click(screen.getByRole("button", { name: /Categoria demo A/ }));
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      txnType: "expense",
      amountCents: 4250,
      isFixed: false,
      paymentMethod: "debit",
      description: "Despesa demo",
      tagIds: ["demo-a"],
      recurrence: null,
    });
  });

  it("mapeia Entrada para income sem método de pagamento", async () => {
    const user = userEvent.setup();
    mockCommands({ list_tags_cmd: [], create_transaction: "id" });
    render(<NewTransactionForm />);

    await user.click(screen.getByRole("button", { name: /Entrada/ }));
    await user.type(screen.getByLabelText("Valor"), "5000");
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
      expect(call?.[1]).toMatchObject({
        txnType: "income",
        amountCents: 500000,
        paymentMethod: null,
      });
    });
  });

  it("envia a série quando Repetir está ativo", async () => {
    const user = userEvent.setup();
    mockCommands({ list_tags_cmd: [], create_transaction: "rec" });
    render(<NewTransactionForm />);

    await user.click(screen.getByRole("button", { name: /Saída/ }));
    await user.type(screen.getByLabelText("Valor"), "2.300,00");
    await user.click(screen.getByLabelText("Repetir"));
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
      expect(call?.[1]).toMatchObject({
        txnType: "expense",
        isFixed: true,
        amountCents: 230000,
        recurrence: { frequency: "mensal", repetitions: 12 },
      });
    });
  });

  it("não envia com valor vazio (botão desabilitado)", async () => {
    mockCommands({ list_tags_cmd: [] });
    render(<NewTransactionForm />);
    expect(screen.getByRole("button", { name: "Lançar" })).toBeDisabled();
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_tags_cmd"));
  });
});
