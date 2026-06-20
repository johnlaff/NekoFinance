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

  it("lança Economia como transfer para conta reserva", async () => {
    const user = userEvent.setup();
    mockCommands({
      list_tags_cmd: [],
      get_pockets: {
        liquid_cents: 0,
        reserve_cents: 1500000,
        restricted_cents: 0,
        illiquid_cents: 0,
        net_worth_cents: 1500000,
        accounts: [
          {
            id: "reserve-001",
            name: "Poupança",
            type: "savings",
            liquidity: "reserve",
            balance: 1500000,
            institution: null,
          },
        ],
      },
      create_transaction: "tx-economia-id",
    });
    render(<NewTransactionForm />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Economia/ })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /Economia/ }));

    // Conta-destino carregada (reserve/illiquid).
    await waitFor(() =>
      expect(screen.getByLabelText("Conta-destino (reserva)")).toBeInTheDocument(),
    );

    await user.type(screen.getByLabelText("Valor"), "1.000,00");
    await user.selectOptions(
      screen.getByLabelText("Conta-destino (reserva)"),
      "reserve-001",
    );
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
      expect(call?.[1]).toMatchObject({
        txnType: "transfer",
        amountCents: 100000,
        paymentMethod: null,
        isFixed: false,
        toAccountId: "reserve-001",
        recurrence: null,
      });
    });
  });

  it("desabilita Lançar sem conta reserva disponível", async () => {
    const user = userEvent.setup();
    mockCommands({
      list_tags_cmd: [],
      get_pockets: {
        liquid_cents: 842000,
        reserve_cents: 0,
        restricted_cents: 0,
        illiquid_cents: 0,
        net_worth_cents: 842000,
        accounts: [
          {
            id: "bank-001",
            name: "Conta corrente",
            type: "bank",
            liquidity: "liquid",
            balance: 842000,
            institution: null,
          },
        ],
      },
      create_transaction: "never-called",
    });
    render(<NewTransactionForm />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Economia/ })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /Economia/ }));
    await user.type(screen.getByLabelText("Valor"), "500,00");

    // Sem conta reserve/illiquid → toAccountId fica vazio → botão desabilitado.
    expect(screen.getByRole("button", { name: "Lançar" })).toBeDisabled();
  });
});
