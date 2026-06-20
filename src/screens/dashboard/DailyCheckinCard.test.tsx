import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DailyCheckinCard } from "./DailyCheckinCard";
import type { DashboardSummary } from "../../lib/api";
import { mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const SUMMARY: DashboardSummary = {
  balance: 500000,
  daily_budget: 5000,
  daily_spend_today: 2000,
  reserve_months: 3,
  reserve_trend: "flat",
  transaction_count: 10,
  last_real_tx_date: "2026-06-19",
};

/** Última chamada ao backend de criação de lançamento, para inspecionar os campos derivados. */
function lastCreateCall() {
  return mockInvoke.mock.calls.findLast((c) => c[0] === "create_transaction");
}

describe("DailyCheckinCard", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra o diário de hoje contra o teto e o disponível", () => {
    mockCommands({});
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
    // R$ 20,00 de R$ 50,00 → R$ 30,00 disponível.
    expect(screen.getByText("Diário de hoje")).toBeInTheDocument();
    expect(screen.getByText(/R\$\s?30,00 disponível/)).toBeInTheDocument();
  });

  it("registra um Diário realizado de hoje (padrão) e dispara onLogged", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    const onLogged = vi.fn();
    render(<DailyCheckinCard summary={SUMMARY} onLogged={onLogged} />);

    await user.type(screen.getByLabelText(/Valor do lançamento/), "12,30");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(onLogged).toHaveBeenCalledTimes(1));
    expect(lastCreateCall()?.[1]).toMatchObject({
      txnType: "expense",
      amountCents: 1230,
      isFixed: false,
      paymentMethod: "debit",
      recurrence: null,
    });
  });

  it("envia a descrição digitada no submit", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    await user.type(screen.getByLabelText("Descrição (opcional)"), "mercado");
    await user.type(screen.getByLabelText(/Valor do lançamento/), "50,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(lastCreateCall()).toBeTruthy());
    expect(lastCreateCall()?.[1]).toMatchObject({ description: "mercado" });
  });

  it("descrição vazia envia null", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    await user.type(screen.getByLabelText(/Valor do lançamento/), "10,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(lastCreateCall()).toBeTruthy());
    expect(lastCreateCall()?.[1]).toMatchObject({ description: null });
  });

  it("o chip Saída deriva isFixed:true e paymentMethod:debit via kindToFields", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /Saída/ }));
    await user.type(screen.getByLabelText(/Valor do lançamento/), "100,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(lastCreateCall()).toBeTruthy());
    expect(lastCreateCall()?.[1]).toMatchObject({
      txnType: "expense",
      isFixed: true,
      paymentMethod: "debit",
    });
  });

  it("o chip Cartão deriva paymentMethod:credit e isFixed:false via kindToFields", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /Cartão/ }));
    await user.type(screen.getByLabelText(/Valor do lançamento/), "80,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(lastCreateCall()).toBeTruthy());
    expect(lastCreateCall()?.[1]).toMatchObject({
      txnType: "expense",
      isFixed: false,
      paymentMethod: "credit",
    });
  });

  it("o chip Economia fica desabilitado (precisa de conta-destino no form completo)", () => {
    mockCommands({});
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
    expect(screen.getByRole("radio", { name: /Economia/ })).toBeDisabled();
  });

  it("valor e descrição limpam após o submit; o tipo é mantido", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    // Seleciona Cartão para provar que o tipo persiste após o submit.
    await user.click(screen.getByRole("radio", { name: /Cartão/ }));
    const desc = screen.getByLabelText("Descrição (opcional)");
    const amount = screen.getByLabelText(/Valor do lançamento/);
    await user.type(desc, "ifood");
    await user.type(amount, "33,00");
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(amount).toHaveValue(""));
    expect(desc).toHaveValue("");
    // O tipo Cartão continua selecionado para o próximo lançamento em sequência.
    expect(screen.getByRole("radio", { name: /Cartão/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("Enter na descrição move o foco para o campo de valor", async () => {
    const user = userEvent.setup();
    mockCommands({});
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);

    const desc = screen.getByLabelText("Descrição (opcional)");
    await user.click(desc);
    await user.keyboard("{Enter}");
    expect(screen.getByLabelText(/Valor do lançamento/)).toHaveFocus();
  });

  it("sinaliza estouro do teto", () => {
    mockCommands({});
    render(
      <DailyCheckinCard
        summary={{ ...SUMMARY, daily_spend_today: 7000 }}
        onLogged={vi.fn()}
      />,
    );
    expect(screen.getByText(/R\$\s?20,00 acima do teto/)).toBeInTheDocument();
  });

  it("desabilita o botão sem valor", () => {
    mockCommands({});
    render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Registrar" })).toBeDisabled();
  });

  it("entrega o ref do campo de valor via onAmountRef após o mount (atalho N)", () => {
    mockCommands({});
    const onAmountRef = vi.fn();
    render(
      <DailyCheckinCard
        summary={SUMMARY}
        onLogged={vi.fn()}
        onAmountRef={onAmountRef}
      />,
    );
    expect(onAmountRef).toHaveBeenCalledTimes(1);
    expect(onAmountRef.mock.calls[0]?.[0]).toBeInstanceOf(HTMLInputElement);
  });
});
