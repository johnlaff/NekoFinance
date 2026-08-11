import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import { InfoPopover } from "./InfoPopover";

describe("InfoPopover", () => {
  it("abre no clique e mostra título + corpo do glossário", async () => {
    const user = userEvent.setup();
    render(<InfoPopover term="reserva">Reserva</InfoPopover>);

    const trigger = screen.getByRole("button", { name: /Reserva/ });
    // O explicador é um tooltip (não disclosure): sem aria-expanded; o tooltip aparece ao abrir.
    expect(trigger).not.toHaveAttribute("aria-expanded");
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

    await user.click(trigger);
    const tip = screen.getByRole("tooltip");
    expect(trigger).toHaveAttribute("aria-describedby");
    expect(tip).toHaveTextContent("Reserva");
    expect(tip).toHaveTextContent(/meses de custo de vida/);
    expect(screen.getByText("Esc para fechar")).toBeInTheDocument();
  });

  it("Esc fecha e devolve o foco ao trigger", async () => {
    const user = userEvent.setup();
    render(<InfoPopover term="caixa">Caixa</InfoPopover>);
    const trigger = screen.getByRole("button", { name: /Caixa/ });
    await user.click(trigger);
    expect(screen.getByRole("tooltip")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("aceita conteúdo direto (sem chave de glossário)", async () => {
    const user = userEvent.setup();
    render(
      <InfoPopover term={{ title: "Olá", body: "Texto livre." }}>termo</InfoPopover>,
    );
    await user.click(screen.getByRole("button", { name: /termo/ }));
    expect(screen.getByRole("tooltip")).toHaveTextContent("Texto livre.");
  });

  // Esc não existe em toque, e a instrução errada ensina o gesto errado (issue #434). A
  // escolha entre as duas variantes é resolvida por CSS (media hover/pointer), nunca por
  // JS — então as DUAS ficam no DOM e o teste trava as duas por texto, não por screenshot.
  it("o hint de fechar traz as duas variantes — toque e teclado — para o CSS escolher", async () => {
    const user = userEvent.setup();
    render(<InfoPopover term="reserva">Reserva</InfoPopover>);
    await user.click(screen.getByRole("button", { name: /Reserva/ }));
    const tip = screen.getByRole("tooltip");
    expect(tip).toHaveTextContent("Toque fora para fechar");
    expect(tip).toHaveTextContent("Esc para fechar");
  });
});
