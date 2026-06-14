import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { HealthBadge, type HealthLevel } from "./HealthBadge";

describe("HealthBadge", () => {
  const defaults: Array<[HealthLevel, string]> = [
    ["strong", "Forte"],
    ["steady", "Estável"],
    ["watch", "Atenção"],
    ["risk", "Em risco"],
  ];

  it.each(defaults)("%s mostra o rótulo padrão %s", (level, label) => {
    render(<HealthBadge level={level} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("sobrescreve o rótulo com o texto do método", () => {
    render(<HealthBadge level="strong" label="Sobrou dinheiro" />);
    expect(screen.getByText("Sobrou dinheiro")).toBeInTheDocument();
    expect(screen.queryByText("Forte")).not.toBeInTheDocument();
  });

  it("aria-label combina rótulo e sublabel para leitores de tela", () => {
    render(
      <HealthBadge level="risk" label="Faltou dinheiro" sublabel="−R$ 120,00" />,
    );
    expect(
      screen.getByRole("img", { name: "Faltou dinheiro — −R$ 120,00" }),
    ).toBeInTheDocument();
  });

  it("renderiza o anel de progresso (svg)", () => {
    const { container } = render(<HealthBadge level="steady" score={50} />);
    expect(container.querySelector("svg")).toBeInTheDocument();
    expect(container.querySelectorAll("circle").length).toBe(2);
  });

  it("aplica a cor do tom", () => {
    const { container } = render(<HealthBadge level="risk" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.color).toBe("var(--danger-400)");
  });
});
