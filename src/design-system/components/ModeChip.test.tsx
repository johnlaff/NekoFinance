import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModeChip } from "./ModeChip";

describe("ModeChip", () => {
  it("modo débito: palavra do modo sem alerta de gate", () => {
    render(<ModeChip mode="debit" />);
    expect(screen.getByText("Modo débito")).toBeInTheDocument();
    expect(screen.queryByText(/abaixo do piso/i)).not.toBeInTheDocument();
  });

  it("modo cartão com economia viva: sem alerta; a didática explica a detecção", async () => {
    const user = userEvent.setup();
    render(<ModeChip mode="card" gate="alive" />);
    expect(screen.queryByText(/abaixo do piso/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /modo cartão/i }));
    expect(screen.getByRole("tooltip")).toHaveTextContent(/o dia lê as faturas/i);
  });

  it("modo cartão abaixo do piso: alerta com palavra (nunca só cor) e didática do gate", async () => {
    const user = userEvent.setup();
    render(<ModeChip mode="card" gate="below" />);
    expect(screen.getByText("Economia abaixo do piso")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /modo cartão/i }));
    expect(screen.getByRole("tooltip")).toHaveTextContent(/piso de 20%/i);
  });

  it("sem dado que sustente o modo: a didática não afirma ter detectado", async () => {
    const user = userEvent.setup();
    render(<ModeChip mode="debit" detected={false} />);
    await user.click(screen.getByRole("button", { name: /modo débito/i }));
    const tooltip = screen.getByRole("tooltip");
    expect(tooltip).not.toHaveTextContent(/detectado dos seus dados/i);
    expect(tooltip).toHaveTextContent(/ainda não/i);
  });

  it("com dado que sustenta o modo: a didática assume a detecção", async () => {
    const user = userEvent.setup();
    render(<ModeChip mode="debit" detected />);
    await user.click(screen.getByRole("button", { name: /modo débito/i }));
    expect(screen.getByRole("tooltip")).toHaveTextContent(/detectado dos seus dados/i);
  });
});
