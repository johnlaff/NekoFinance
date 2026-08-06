import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockCommands, mockInvoke } from "../test/commands";

const card = {
  id: "holder",
  name: "Cartão principal",
  institution: null,
  owner_name: "Eu",
  linked_account_id: null,
  closing_day: 20,
  due_day: 10,
  credit_limit_cents: null,
  aliases: [],
  open_invoice: null,
  next_due: null,
};

const additions = [
  {
    id: "additional-a",
    name: "Adicional A",
    institution: null,
    owner_name: "Bia",
    linked_account_id: "holder",
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
    open_invoice: null,
    next_due: null,
  },
  {
    id: "additional-b",
    name: "Adicional B",
    institution: null,
    owner_name: "Caio",
    linked_account_id: "holder",
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
    open_invoice: null,
    next_due: null,
  },
];

vi.mock("../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { CartoesScreen } from "./CartoesScreen";

describe("Cartões durante o carregamento do gate", () => {
  it("não inventa um veredito antes do resumo chegar", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_dashboard_summary: () => new Promise(() => undefined),
      list_card_proposals: [],
      list_cards: [card, ...additions],
      list_invoices: [],
    });
    render(<CartoesScreen />);

    expect(await screen.findAllByText("Carregando…")).toHaveLength(2);
    expect(screen.queryByText("Economia viva")).not.toBeInTheDocument();
    expect(screen.queryByText("Reserva de 6 meses — falta")).not.toBeInTheDocument();
    expect(screen.getByText("Adicional A")).toBeInTheDocument();
    expect(screen.getByText("Adicional B")).toBeInTheDocument();
  });
});
