import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { MonthNav } from "./MonthNav";

describe("MonthNav", () => {
  it("mostra o rótulo do mês", () => {
    render(
      <MonthNav label="Fevereiro de 2026" onPrev={vi.fn()} onNext={vi.fn()} onToday={vi.fn()} />,
    );
    expect(screen.getByText("Fevereiro de 2026")).toBeInTheDocument();
  });

  it("chama onPrev/onNext nos botões de seta", async () => {
    const onPrev = vi.fn();
    const onNext = vi.fn();
    render(
      <MonthNav label="Fev/26" onPrev={onPrev} onNext={onNext} onToday={vi.fn()} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Mês anterior" }));
    await userEvent.click(screen.getByRole("button", { name: "Próximo mês" }));
    expect(onPrev).toHaveBeenCalledOnce();
    expect(onNext).toHaveBeenCalledOnce();
  });

  it("desabilita as setas nos limites", () => {
    render(
      <MonthNav
        label="Fev/26"
        onPrev={vi.fn()}
        onNext={vi.fn()}
        onToday={vi.fn()}
        canPrev={false}
        canNext={false}
      />,
    );
    expect(screen.getByRole("button", { name: "Mês anterior" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Próximo mês" })).toBeDisabled();
  });

  it('esconde "Hoje" quando atToday e mostra/clica quando não', async () => {
    const onToday = vi.fn();
    const { rerender } = render(
      <MonthNav label="Fev/26" onPrev={vi.fn()} onNext={vi.fn()} onToday={onToday} atToday />,
    );
    expect(screen.queryByRole("button", { name: "Hoje" })).not.toBeInTheDocument();

    rerender(
      <MonthNav
        label="Mar/26"
        onPrev={vi.fn()}
        onNext={vi.fn()}
        onToday={onToday}
        atToday={false}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Hoje" }));
    expect(onToday).toHaveBeenCalledOnce();
  });
});
