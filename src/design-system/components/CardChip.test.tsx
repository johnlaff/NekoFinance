import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { CardChip } from "./CardChip";

describe("CardChip", () => {
  const base = { mono: "BANCO", last4: "1234", nick: "Meu cartão", total: -50000 };

  it("renderiza apelido, final e nome impresso", () => {
    render(<CardChip {...base} />);
    expect(screen.getByText("Meu cartão")).toBeInTheDocument();
    expect(screen.getByText("BANCO")).toBeInTheDocument();
    expect(screen.getByText(/1234/)).toBeInTheDocument();
  });

  it("aria-label descreve cartão + fatura", () => {
    render(<CardChip {...base} />);
    expect(
      screen.getByRole("button", { name: /Meu cartão, final 1234, fatura/ }),
    ).toBeInTheDocument();
  });

  it("dispara onClick e reflete active em aria-pressed", async () => {
    const onClick = vi.fn();
    render(<CardChip {...base} active onClick={onClick} />);
    const btn = screen.getByRole("button");
    expect(btn).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(btn);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('mostra "paga {titular}" em cartão adicional', () => {
    render(<CardChip {...base} additional ownerLabel="Pessoa A" />);
    expect(screen.getByText("paga Pessoa A")).toBeInTheDocument();
  });

  it('mostra "adicional" sem ownerLabel', () => {
    render(<CardChip {...base} additional />);
    expect(screen.getByText("adicional")).toBeInTheDocument();
  });
});
