import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { RangeRuler } from "./RangeRuler";

const ZONE = { from: 20, to: 30 };
const MARKS = [
  { at: 20, label: "20%" },
  { at: 30, label: "30%" },
  { at: 40, label: "40%" },
];

describe("RangeRuler", () => {
  it("com label vira role=img com o texto equivalente completo", () => {
    const { getByRole } = render(
      <RangeRuler
        max={40}
        zone={ZONE}
        marks={MARKS}
        pin={{ value: 25, label: "25%", color: "var(--success-400)" }}
        label="Régua de 0% a 40%, zona-alvo de 20% a 30%; este mês em 25%"
      />,
    );
    expect(
      getByRole("img", { name: /zona-alvo de 20% a 30%; este mês em 25%/ }),
    ).toBeInTheDocument();
  });

  it("sem label é decorativa (aria-hidden)", () => {
    const { container } = render(
      <RangeRuler max={40} zone={ZONE} marks={MARKS} pin={null} />,
    );
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });

  it("pino satura na borda da escala; o rótulo diz o valor verdadeiro", () => {
    const { getByText } = render(
      <RangeRuler
        max={40}
        zone={ZONE}
        marks={MARKS}
        pin={{ value: 55, label: "55%", color: "var(--success-400)" }}
      />,
    );
    const pinLabel = getByText("55%");
    // A geometria clampa em 100%; o texto segue dizendo 55%.
    expect(pinLabel.style.left).toBe("100%");
  });

  it("sem pino (régua que não julga) renderiza só trilho, zona e marcas", () => {
    const { queryByText, getByText } = render(
      <RangeRuler max={40} zone={ZONE} marks={MARKS} pin={null} />,
    );
    expect(getByText("20%")).toBeInTheDocument();
    expect(getByText("30%")).toBeInTheDocument();
    expect(queryByText(/pino/)).not.toBeInTheDocument();
  });

  it("zona-alvo posiciona pela escala (20–30 em 0→40 = 50%–75%)", () => {
    const { container } = render(
      <RangeRuler max={40} zone={ZONE} marks={MARKS} pin={null} />,
    );
    const zone = container.querySelector<HTMLElement>("div > div > span")!;
    expect(zone.style.left).toBe("50%");
    expect(zone.style.width).toBe("25%");
  });
});
