import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

const { acceptCardProposal, attachCardProposal, holder } = vi.hoisted(() => ({
  acceptCardProposal: vi.fn().mockResolvedValue("additional-card"),
  attachCardProposal: vi.fn().mockResolvedValue(undefined),
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

vi.mock("../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
}));

vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal()),
  acceptCardProposal,
  attachCardProposal,
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
      aliases: ["adicional"],
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

  it("resolve a proposta como apelido de um cartão que já existe", async () => {
    const user = userEvent.setup();
    acceptCardProposal.mockClear();
    render(<CartoesScreen />);

    // O vínculo mora atrás de um convite: o cadastro continua sendo a ação de abertura.
    await user.click(
      await screen.findByRole("button", { name: "É um cartão que já tenho" }),
    );
    await user.selectOptions(screen.getByLabelText("Cartão que já tenho"), "holder");
    await user.click(screen.getByRole("button", { name: "Usar como apelido" }));

    expect(attachCardProposal).toHaveBeenCalledWith({
      proposalId: "proposal",
      accountId: "holder",
    });
    expect(acceptCardProposal).not.toHaveBeenCalled();
  });
});
