import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { holder } = vi.hoisted(() => ({
  holder: {
    id: "holder",
    name: "Cartão principal",
    institution: null,
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
    open_invoice: {
      id: "inv-ago",
      cycle_month: "2026-08",
      closing_date: "2026-07-20",
      due_date: "2026-08-10",
      status: "aberta" as const,
      stated_total_cents: 100_000,
      purchases_sum_cents: 0,
      effective_total_cents: 100_000,
      reconciliation_delta_cents: null,
    },
    next_due: null,
  },
}));

vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
  getDashboardSummary: vi.fn().mockResolvedValue({
    card_gate_economy: "alive",
    card_gate_reserve: "alive",
  }),
  listCardProposals: vi.fn().mockResolvedValue([]),
  listCards: vi.fn().mockResolvedValue([holder]),
  // Faturas nunca respondem: carregando não pode virar "Sem fatura registrada".
  listInvoices: vi.fn(() => new Promise(() => undefined)),
  getInvoice: vi.fn().mockRejectedValue(new Error("io")),
}));

import { CartoesScreen } from "./CartoesScreen";

describe("Cartões — faturas no modo desktop (Tauri)", () => {
  it("carregando não é ausência: skeleton no lugar de 'Sem fatura registrada'", async () => {
    render(<CartoesScreen />);
    expect(await screen.findAllByRole("status")).not.toHaveLength(0);
    expect(screen.queryByText(/Sem fatura registrada/)).not.toBeInTheDocument();
  });
});
