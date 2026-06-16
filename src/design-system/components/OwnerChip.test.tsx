import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { OwnerChip, type OwnerWho } from "./OwnerChip";

describe("OwnerChip", () => {
  it.each([
    ["personal", "Eu"],
    ["partner", "Parceiro(a)"],
    ["shared", "Compartilhado"],
  ] as [OwnerWho, string][])("%s → rótulo padrão %s", (who, label) => {
    render(<OwnerChip who={who} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("sobrescreve com o nome real", () => {
    render(<OwnerChip who="partner" name="Ana" />);
    expect(screen.getByText("Ana")).toBeInTheDocument();
    expect(screen.queryByText("Parceiro(a)")).not.toBeInTheDocument();
  });

  it("usa a cor do titular no ponto", () => {
    const { container } = render(<OwnerChip who="partner" />);
    const dot = container.querySelector<HTMLElement>("span[aria-hidden='true']")!;
    expect(dot.style.background).toBe("var(--owner-partner)");
  });

  it("mostra o papel e o title", () => {
    render(<OwnerChip who="shared" name="Casa" note="paga" />);
    expect(screen.getByText("paga")).toBeInTheDocument();
    expect(screen.getByTitle("Casa · paga")).toBeInTheDocument();
  });

  it("avatar mostra o monograma (iniciais) do nome", () => {
    render(<OwnerChip name="Joana Alves" avatar />);
    expect(screen.getByText("JA")).toBeInTheDocument();
  });
});
