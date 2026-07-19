import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

const { acceptCardProposal, holder } = vi.hoisted(() => ({
  acceptCardProposal: vi.fn().mockResolvedValue("additional-card"),
  holder: {
    id: "holder",
    name: "Titular",
    institution: null,
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
    open_invoice: null,
    next_due: null,
  },
}));

vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
  acceptCardProposal,
  getDashboardSummary: vi.fn().mockResolvedValue({
    card_gate_economy: "alive",
    card_gate_reserve: "below",
  }),
  listCardProposals: vi.fn().mockResolvedValue([
    {
      id: "proposal",
      alias: "adicional",
      display_name: "Cartão adicional",
      source_month: "2026-07",
      status: "pending",
    },
  ]),
  listCards: vi.fn().mockResolvedValue([holder]),
  listInvoices: vi.fn().mockResolvedValue([]),
}));

import { CartoesScreen } from "./CartoesScreen";

describe("aceite de proposta como cartão adicional", () => {
  it("envia o ciclo nulo para o backend", async () => {
    const user = userEvent.setup();
    render(<CartoesScreen />);

    await user.click(await screen.findByRole("button", { name: "Cadastrar cartão" }));
    await user.selectOptions(screen.getByLabelText("Cartão adicional de"), "holder");
    await user.click(screen.getByRole("button", { name: "Salvar cartão" }));

    expect(acceptCardProposal).toHaveBeenCalledWith({
      proposalId: "proposal",
      closingDay: null,
      dueDay: null,
      ownerPersonName: "Eu",
      linkedAccountId: "holder",
    });
  });
});
