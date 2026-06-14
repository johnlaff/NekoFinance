import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { CardChip } from "./CardChip";

describe("CardChip", () => {
  const base = { mono: "NUBANK", last4: "1234", nick: "Roxinho", total: -50000 };

  it("renderiza apelido, final e nome impresso", () => {
    render(<CardChip {...base} />);
    expect(screen.getByText("Roxinho")).toBeInTheDocument();
    expect(screen.getByText("NUBANK")).toBeInTheDocument();
    expect(screen.getByText(/1234/)).toBeInTheDocument();
  });

  it("aria-label descreve cartão + fatura", () => {
    render(<CardChip {...base} />);
    expect(
      screen.getByRole("button", { name: /Roxinho, final 1234, fatura/ }),
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
    render(<CardChip {...base} additional ownerLabel="Gio" />);
    expect(screen.getByText("paga Gio")).toBeInTheDocument();
  });

  it('mostra "adicional" sem ownerLabel', () => {
    render(<CardChip {...base} additional />);
    expect(screen.getByText("adicional")).toBeInTheDocument();
  });
});
