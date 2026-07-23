import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { Switch } from "./Switch";

describe("Switch", () => {
  it("é um switch acessível com estado refletido em aria-checked", () => {
    const { rerender } = render(
      <Switch on={false} onChange={vi.fn()} label="Tema escuro" />,
    );
    const sw = screen.getByRole("switch", { name: "Tema escuro" });
    expect(sw).toHaveAttribute("aria-checked", "false");

    rerender(<Switch on onChange={vi.fn()} label="Tema escuro" />);
    expect(sw).toHaveAttribute("aria-checked", "true");
  });

  it("clique aciona onChange com o próximo estado", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Switch on={false} onChange={onChange} label="Animações" />);
    await user.click(screen.getByRole("switch", { name: "Animações" }));
    expect(onChange).toHaveBeenCalledWith(true, expect.anything());
  });

  it("teclado: Space/Enter acionam (button nativo)", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Switch on onChange={onChange} label="Lembrete diário" />);
    const sw = screen.getByRole("switch", { name: "Lembrete diário" });
    sw.focus();
    await user.keyboard(" ");
    expect(onChange).toHaveBeenCalledWith(false, expect.anything());
  });

  it("disabled não aciona", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Switch on={false} onChange={onChange} label="Lembrete" disabled />);
    await user.click(screen.getByRole("switch", { name: "Lembrete" }));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("switch", { name: "Lembrete" })).toBeDisabled();
  });
});
