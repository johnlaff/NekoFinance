import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    exclude_from_performance: false,
    exclude_from_cost_of_living: false,
    exclude_from_savings: false,
    exclude_from_daily_avg: false,
  },
  {
    id: "pagar",
    name: "! Pagar",
    color: "#a83",
    emoji: null,
    is_special: true,
    exclude_from_performance: false,
    exclude_from_cost_of_living: false,
    exclude_from_savings: false,
    exclude_from_daily_avg: false,
  },
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

  it("vencimento (plano 045): só aparece em Saída/Cartão e viaja no create", async () => {
    const user = userEvent.setup();
    mockCommands({ list_tags_cmd: [], create_transaction: "due-id" });
    render(<NewTransactionForm />);

    // Diário (padrão) não mostra o campo de vencimento.
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_tags_cmd"));
    expect(screen.queryByLabelText("Vencimento (opcional)")).not.toBeInTheDocument();

    // Saída revela o campo; preenchê-lo o envia como dueDate.
    await user.click(screen.getByRole("button", { name: /Saída/ }));
    const due = screen.getByLabelText("Vencimento (opcional)");
    expect(due).toBeInTheDocument();
    await user.type(screen.getByLabelText("Valor"), "1.200,00");
    await user.type(due, "2026-08-10");
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
      expect(call?.[1]).toMatchObject({
        txnType: "expense",
        isFixed: true,
        amountCents: 120000,
        dueDate: "2026-08-10",
      });
    });
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

  describe("line items (plano 036)", () => {
    it("adicionar duas partes torna o Valor somente-leitura e mostra a soma", async () => {
      const user = userEvent.setup();
      mockCommands({ list_tags_cmd: [], create_transaction: "new-id" });
      render(<NewTransactionForm onCreated={vi.fn()} />);

      await user.click(screen.getByRole("button", { name: "+ Adicionar item" }));
      await user.type(screen.getByLabelText("Valor do item 1"), "50,00");
      await user.click(screen.getByRole("button", { name: "+ Adicionar item" }));
      await user.type(screen.getByLabelText("Valor do item 2"), "75,00");

      // O campo Valor do form agora reflete a SOMA e fica somente-leitura.
      const amountField = screen.getByLabelText("Valor");
      expect(amountField).toHaveValue("125,00");
      expect(amountField).toHaveAttribute("readonly");
    });

    it("remover a última parte reabilita o campo Valor", async () => {
      const user = userEvent.setup();
      mockCommands({ list_tags_cmd: [], create_transaction: "new-id" });
      render(<NewTransactionForm onCreated={vi.fn()} />);

      await user.click(screen.getByRole("button", { name: "+ Adicionar item" }));
      await user.type(screen.getByLabelText("Valor do item 1"), "30,00");
      const amountField = screen.getByLabelText("Valor");
      expect(amountField).toHaveAttribute("readonly");

      await user.click(screen.getByRole("button", { name: "Remover item 1" }));
      expect(screen.getByLabelText("Valor")).not.toHaveAttribute("readonly");
    });

    it("ao enviar com partes, chama update_transaction_items_cmd após create_transaction", async () => {
      const user = userEvent.setup();
      mockCommands({
        list_tags_cmd: [],
        create_transaction: "new-id",
        update_transaction_items_cmd: null,
      });
      const onCreated = vi.fn();
      render(<NewTransactionForm onCreated={onCreated} />);

      await user.click(screen.getByRole("button", { name: "+ Adicionar item" }));
      await user.type(screen.getByLabelText("Valor do item 1"), "50,00");
      await user.type(screen.getByLabelText("Descrição do item 1"), "Parte A");
      await user.click(screen.getByRole("button", { name: "+ Adicionar item" }));
      await user.type(screen.getByLabelText("Valor do item 2"), "75,00");
      await user.click(screen.getByRole("button", { name: "Lançar" }));

      await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));

      const createIdx = mockInvoke.mock.calls.findIndex(
        (c) => c[0] === "create_transaction",
      );
      const itemsIdx = mockInvoke.mock.calls.findIndex(
        (c) => c[0] === "update_transaction_items_cmd",
      );
      expect(createIdx).toBeGreaterThanOrEqual(0);
      expect(itemsIdx).toBeGreaterThan(createIdx); // itens DEPOIS do create
      const itemsCall = mockInvoke.mock.calls[itemsIdx];
      expect(itemsCall?.[1]).toMatchObject({
        transactionId: "new-id",
        items: [
          { amount_cents: 5000, description: "Parte A", position: 0 },
          { amount_cents: 7500, description: "", position: 1 },
        ],
      });
      // O total enviado ao create é a SOMA das partes (aritmética, não string).
      const createCall = mockInvoke.mock.calls[createIdx];
      expect(createCall?.[1]).toMatchObject({ amountCents: 12500 });
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

  it.each([
    ["vazia", ""],
    ["não numérica", "abc"],
  ])("mantém as parcelas com entrada %s", async (_descricao, value) => {
    const user = userEvent.setup();
    mockCommands({
      list_tags_cmd: [],
      list_cards: [
        {
          id: "card-001",
          name: "Cartão demo",
          institution: null,
          owner_name: "Pessoa demo",
          linked_account_id: null,
          closing_day: 10,
          due_day: 20,
          credit_limit_cents: null,
          aliases: [],
          open_invoice: null,
          next_due: null,
        },
      ],
      create_card_series: "series-id",
    });
    render(<NewTransactionForm />);

    await user.click(screen.getByRole("button", { name: /Cartão/ }));
    await waitFor(() =>
      expect(screen.getByLabelText("Cartão")).toHaveValue("card-001"),
    );
    await user.click(screen.getByRole("button", { name: "Parcelado em N" }));

    const installments = screen.getByLabelText("Número de parcelas");
    fireEvent.change(installments, { target: { value: "12" } });
    fireEvent.change(installments, { target: { value } });

    expect(installments).toHaveValue(12);

    await user.type(screen.getByLabelText("Valor"), "100");
    await user.click(screen.getByRole("button", { name: "Lançar" }));

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === "create_card_series");
      expect(call?.[1]).toMatchObject({ count: 12 });
    });
  });
});
