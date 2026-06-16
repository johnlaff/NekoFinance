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
  credit_spend_month: 0,
  has_credit: false,
  reserve_months: 3,
  reserve_trend: "flat",
  transaction_count: 10,
};

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

  it("registra um Diário realizado de hoje e dispara onLogged", async () => {
    const user = userEvent.setup();
    mockCommands({ create_transaction: "id" });
    const onLogged = vi.fn();
    render(<DailyCheckinCard summary={SUMMARY} onLogged={onLogged} />);

    await user.type(
      screen.getByLabelText("Gasto de hoje no débito, PIX ou dinheiro"),
      "12,30",
    );
    await user.click(screen.getByRole("button", { name: "Registrar" }));

    await waitFor(() => expect(onLogged).toHaveBeenCalledTimes(1));
    const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
    expect(call?.[1]).toMatchObject({
      txnType: "expense",
      amountCents: 1230,
      isFixed: false,
      paymentMethod: "debit",
      recurrence: null,
    });
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
});
