import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TetoScreen } from "./TetoScreen";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const EMPTY_BUDGET = { per_day_cents: 0, divisor_days: null, categories: [] };
const CEREMONY_BUDGET = {
  per_day_cents: 4033,
  divisor_days: 31,
  categories: [
    { id: "c1", name: "Alimentação", amount_cents: 100000, position: 0 },
    { id: "c2", name: "Transporte", amount_cents: 25000, position: 1 },
  ],
};
const PROPOSAL = {
  id: "cp-1",
  per_day_cents: 4033,
  divisor_days: 31,
  source_month: "2026-05",
  items: [
    { name: "Alimentação", amount_cents: 100000 },
    { name: "Transporte", amount_cents: 25000 },
  ],
};

describe("TetoScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("cerimônia guiada: sem teto, apresenta a didática e salva itens ÷ divisor", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    expect(await screen.findByText(/Você ainda não estipulou um teto/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Adicionar categoria" }));
    await user.type(screen.getByLabelText("Nome da categoria 1"), "Alimentação");
    await user.type(screen.getByLabelText("Valor mensal da categoria 1"), "1.250,00");
    await user.clear(screen.getByLabelText("Divisor de dias"));
    await user.type(screen.getByLabelText("Divisor de dias"), "31");
    await user.click(screen.getByRole("button", { name: "Salvar teto" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("upsert_daily_budget_with_categories_cmd", {
        amountCents: 4032, // piso de 125000 ÷ 31
        categories: [{ name: "Alimentação", amount_cents: 125000, position: 0 }],
        divisorDays: 31,
      });
    });
  });

  it("teto existente: mostra itens, divisor e o teto/dia derivado ao vivo", async () => {
    mockCommands({
      get_daily_budget_cmd: CEREMONY_BUDGET,
      get_ceiling_proposal_cmd: null,
    });
    render(<TetoScreen />);

    expect(await screen.findByDisplayValue("Alimentação")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Transporte")).toBeInTheDocument();
    expect(screen.getByLabelText("Divisor de dias")).toHaveValue("31");
    // 125000 ÷ 31 = 4032 (piso) — derivação exibida ao vivo.
    expect(
      screen.getByText(
        (_, el) =>
          el?.tagName === "SPAN" &&
          /Teto:\s*R\$\s?40,32 por dia/.test((el.textContent ?? "").replace(/\s+/g, " ")),
      ),
    ).toBeInTheDocument();
  });

  it("valor direto: salva o teto por dia sem itens nem divisor", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("radio", { name: "Valor direto" }));
    await user.type(screen.getByLabelText("Teto por dia (R$)"), "50,00");
    await user.click(screen.getByRole("button", { name: "Salvar teto" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("upsert_daily_budget_with_categories_cmd", {
        amountCents: 5000,
        categories: [],
        divisorDays: null,
      });
    });
  });

  it("validação: cerimônia sem categorias não grava e explica o porquê", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: null,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Salvar teto" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/ao menos uma categoria/i);
    expect(mockInvoke).not.toHaveBeenCalledWith(
      "upsert_daily_budget_with_categories_cmd",
      expect.anything(),
    );
  });

  it("proposta da planilha: mostra valor, itens e origem; aceitar confirma explicitamente", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: PROPOSAL,
      accept_ceiling_proposal_cmd: undefined,
    });
    render(<TetoScreen />);

    expect(await screen.findByText("Proposta da sua planilha")).toBeInTheDocument();
    expect(screen.getByText(/2026-05/)).toBeInTheDocument();
    expect(screen.getByText(/Alimentação —/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Usar este teto" }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("accept_ceiling_proposal_cmd", {
        proposalId: "cp-1",
      });
    });
  });

  it("dispensar a proposta chama o comando de dismiss", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_daily_budget_cmd: EMPTY_BUDGET,
      get_ceiling_proposal_cmd: PROPOSAL,
      dismiss_ceiling_proposal_cmd: undefined,
    });
    render(<TetoScreen />);

    await user.click(await screen.findByRole("button", { name: "Agora não" }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("dismiss_ceiling_proposal_cmd", {
        proposalId: "cp-1",
      });
    });
  });
});
