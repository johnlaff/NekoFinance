import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Money } from "./Money";
import { formatBRL } from "../../lib/format";

describe("formatBRL", () => {
  it("formata positivo com agrupamento e 2 casas (pt-BR)", () => {
    expect(formatBRL(123456)).toContain("1.234,56");
  });
  it("usa sinal de menos real (U+2212) no negativo", () => {
    const s = formatBRL(-50000);
    expect(s.charCodeAt(0)).toBe(0x2212);
    expect(s).toContain("500,00");
  });
  it("usa NBSP após R$ (mantém o símbolo colado ao número)", () => {
    expect(formatBRL(100)).toContain("R$ ");
  });
  it("hideCents arredonda sem casas decimais", () => {
    const s = formatBRL(123456, true);
    expect(s).toContain("1.235");
    expect(s).not.toContain(",");
  });
  it("zero", () => {
    expect(formatBRL(0)).toContain("0,00");
  });
});

describe("Money", () => {
  it("sign=auto colore negativo de money-neg", () => {
    const { container } = render(<Money cents={-100} sign="auto" />);
    expect((container.firstElementChild as HTMLElement).style.color).toBe(
      "var(--money-neg)",
    );
  });
  it("sign=auto colore positivo de money-pos", () => {
    const { container } = render(<Money cents={100} sign="auto" />);
    expect((container.firstElementChild as HTMLElement).style.color).toBe(
      "var(--money-pos)",
    );
  });
  it("sign=none não força cor", () => {
    const { container } = render(<Money cents={-100} />);
    expect((container.firstElementChild as HTMLElement).style.color).toBe("");
  });
  it("aria-label descreve negativo para leitores de tela", () => {
    render(<Money cents={-2500} sign="auto" />);
    expect(screen.getByLabelText(/negativo/)).toBeInTheDocument();
  });
  it("renderiza em mono tabular", () => {
    const { container } = render(<Money cents={100} />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.fontFamily).toBe("var(--font-money)");
    expect(el.style.fontVariantNumeric).toBe("tabular-nums");
  });
});
