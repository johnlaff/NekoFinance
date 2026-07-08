import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Money, SignedMoney } from "./Money";
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
  it("size=inherit não impõe font-size/weight/tracking (a classe do wrapper vence)", () => {
    const { container } = render(<Money cents={100} size="inherit" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.fontSize).toBe("");
    expect(el.style.fontWeight).toBe("");
    expect(el.style.letterSpacing).toBe("");
    // Mantém o tratamento tipográfico não-métrico.
    expect(el.style.fontVariantNumeric).toBe("tabular-nums");
    expect(el.style.whiteSpace).toBe("nowrap");
  });
});

describe("SignedMoney", () => {
  it("força o + visível em positivos", () => {
    render(<SignedMoney cents={2500} />);
    expect(screen.getByText(/^\+R\$/)).toBeInTheDocument();
  });
  it("mantém o sinal de menos real em negativos (sem + duplicado)", () => {
    render(<SignedMoney cents={-2500} />);
    const el = screen.getByText(/R\$/);
    expect(el.textContent?.startsWith("+")).toBe(false);
    expect(el.textContent?.charCodeAt(0)).toBe(0x2212);
  });
  it("aria-label anuncia positivo/negativo por extenso", () => {
    render(<SignedMoney cents={2500} />);
    expect(screen.getByLabelText(/^positivo /)).toBeInTheDocument();
  });
  it("aria-label anuncia negativo por extenso", () => {
    render(<SignedMoney cents={-2500} />);
    expect(screen.getByLabelText(/^negativo /)).toBeInTheDocument();
  });
  it("size=inherit não impõe font-size/weight/tracking (a classe do wrapper vence)", () => {
    const { container } = render(<SignedMoney cents={100} size="inherit" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.fontSize).toBe("");
    expect(el.style.fontWeight).toBe("");
    expect(el.style.letterSpacing).toBe("");
    expect(el.style.fontVariantNumeric).toBe("tabular-nums");
  });
});
