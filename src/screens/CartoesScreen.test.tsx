import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: false,
}));
import { CartoesScreen, shiftCycleMonth, validateCardCycle } from "./CartoesScreen";

describe("Cartões", () => {
  it("mostra proposta, gate e a lista no fallback web", () => {
    render(<CartoesScreen />);
    expect(screen.getByText("Cartão de viagens")).toBeInTheDocument();
    expect(screen.getByText("Economia viva")).toBeInTheDocument();
    expect(screen.getByText("Cartão principal")).toBeInTheDocument();
  });

  it("abre o drill com reconciliação e leitura líquida marcada", async () => {
    const user = userEvent.setup();
    render(<CartoesScreen />);
    await user.click(screen.getByRole("button", { name: /Faturas/ }));
    expect(screen.getByText(/Não itemizado/)).toBeInTheDocument();
    expect(screen.getByText(/Líquido de reembolsos/)).toBeInTheDocument();
    expect(screen.getByText("Conferência")).toBeInTheDocument();
  });

  it("agrupa séries e oferece seus gestos no drill", async () => {
    const user = userEvent.setup();
    render(<CartoesScreen />);

    await user.click(screen.getByRole("button", { name: /Faturas/ }));

    expect(screen.getByRole("heading", { name: "Séries" })).toBeInTheDocument();
    expect(screen.getAllByText("Assinatura").length).toBeGreaterThan(1);
    expect(screen.getAllByRole("button", { name: "Editar" }).length).toBeGreaterThan(1);
    expect(
      screen.getByRole("button", { name: "Cancelar a partir deste ciclo" }),
    ).toBeInTheDocument();
  });

  it("prefill do ajuste preserva centavos", async () => {
    const user = userEvent.setup();
    render(<CartoesScreen />);

    await user.click(screen.getByRole("button", { name: /Faturas/ }));
    await user.click(screen.getByRole("button", { name: "Ajustar total" }));

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
