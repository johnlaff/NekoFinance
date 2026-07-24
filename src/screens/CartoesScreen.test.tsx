import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: false,
}));
import { CartoesScreen } from "./CartoesScreen";
import { shiftCycleMonth, validateCardCycle } from "../lib/cardCycle";

describe("Cartões", () => {
  it("mostra veredito, proposta, gate e a lista no fallback web", () => {
    render(<CartoesScreen />);
    expect(
      screen.getByRole("heading", { name: "A próxima fatura vence 10 de ago." }),
    ).toBeInTheDocument();
    expect(screen.getByText("Cartão de viagens")).toBeInTheDocument();
    expect(screen.getByText(/Economia viva/)).toBeInTheDocument();
    expect(screen.getByText("Cartão principal")).toBeInTheDocument();
    expect(screen.getByText("Cartão reserva")).toBeInTheDocument();
  });

  it("mostra a matemática do gate — percentual de economia e meses de reserva atuais, não só 'falta'", () => {
    render(<CartoesScreen />);
    expect(screen.getByText(/24%/)).toBeInTheDocument();
    expect(screen.getByText(/4,2 meses/)).toBeInTheDocument();
  });

  it("expõe o drill com herói honesto, reconciliação e leitura líquida marcada", () => {
    render(<CartoesScreen />);
    expect(
      screen.getByText("Total declarado — autoridade da planilha"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Não itemizado/)).toBeInTheDocument();
    expect(screen.getByText(/Líquido de reembolsos/)).toBeInTheDocument();
    expect(screen.getByText("Conferência")).toBeInTheDocument();
  });

  it("seleciona a fatura aberta por padrão e nomeia os ciclos com o status", () => {
    render(<CartoesScreen />);
    const selected = screen.getByRole("radio", { name: "Ago · Aberta" });
    expect(selected).toBeChecked();
    expect(screen.getByRole("radio", { name: "Jul · Fechada" })).toBeInTheDocument();
  });

  it("carrega o histórico em barras com equivalente textual", () => {
    render(<CartoesScreen />);
    const bars = screen.getByRole("img", { name: /Faturas por ciclo/ });
    expect(bars.getAttribute("aria-label")).toContain("Ago");
  });

  it("deriva progresso de parcela e cadência de assinatura nas séries", () => {
    render(<CartoesScreen />);
    expect(screen.getByRole("heading", { name: "Séries" })).toBeInTheDocument();
    expect(screen.getByText(/Parcela 2 de 5/)).toBeInTheDocument();
    expect(screen.getByText(/Todo mês, dia 15/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Editar Notebook" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Cancelar Streaming a partir deste ciclo" }),
    ).toBeInTheDocument();
  });

  it("prefill do ajuste preserva centavos", async () => {
    const user = userEvent.setup();
    render(<CartoesScreen />);
    await user.click(screen.getByRole("button", { name: "Ajustar total declarado" }));
    expect(screen.getByLabelText("Total declarado")).toHaveValue("4289,00");
  });

  it("valida os dias do ciclo", () => {
    expect(validateCardCycle("29", "10")).toMatch(/1 e 28/);
    expect(validateCardCycle("20", "32")).toMatch(/1 e 31/);
    expect(validateCardCycle("20", "10")).toBeNull();
  });

  it("desloca ciclos ao atravessar o ano", () => {
    expect(shiftCycleMonth("2026-01", -1)).toBe("2025-12");
    expect(shiftCycleMonth("2026-12", 1)).toBe("2027-01");
  });
});
