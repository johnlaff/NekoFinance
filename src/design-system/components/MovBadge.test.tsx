import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { MovBadge, type MovKind } from "./MovBadge";

describe("MovBadge", () => {
  const cases: Array<[MovKind, string, string]> = [
    ["entrada", "E", "Entrada"],
    ["saida", "S", "Saída"],
    ["diario", "D", "Diário"],
    ["economia", "E", "Economia"],
    ["cartao", "C", "Cartão"],
  ];

  it.each(cases)(
    "%s renderiza o glifo e expõe o nome acessível",
    (kind, glyph, name) => {
      render(<MovBadge kind={kind} />);
      expect(screen.getByText(glyph)).toBeInTheDocument();
      // Nome sempre disponível (visível ou sr-only) — acessibilidade.
      expect(screen.getByText(name)).toBeInTheDocument();
    },
  );

  it("usa o token de cor do tipo no fundo do glifo", () => {
    const { container } = render(<MovBadge kind="cartao" />);
    const glyph = container.querySelector(
      "span[aria-hidden='true']",
    ) as HTMLElement;
    expect(glyph.style.background).toBe("var(--type-cartao)");
  });

  it("entrada e economia compartilham a letra E mas cores diferentes", () => {
    const e = render(<MovBadge kind="entrada" />).container.querySelector(
      "span[aria-hidden='true']",
    ) as HTMLElement;
    const ec = render(<MovBadge kind="economia" />).container.querySelector(
      "span[aria-hidden='true']",
    ) as HTMLElement;
    expect(e.textContent).toBe("E");
    expect(ec.textContent).toBe("E");
    expect(e.style.background).not.toBe(ec.style.background);
  });

  it("mostra o nome visível quando showLabel", () => {
    render(<MovBadge kind="diario" showLabel />);
    expect(screen.getByText("Diário")).toBeVisible();
  });

  it("aceita className", () => {
    const { container } = render(
      <MovBadge kind="entrada" className="x" />,
    );
    expect(
      (container.firstElementChild as HTMLElement).className,
    ).toContain("x");
  });
});
