import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SegBar } from "./SegBar";

describe("SegBar", () => {
  it("com label vira role=img com o texto equivalente completo", () => {
    const { getByRole } = render(
      <SegBar
        segments={[
          { name: "Saídas fixas", fraction: 0.7, color: "var(--type-saida)" },
          { name: "Cartão", fraction: 0.3, color: "var(--type-cartao)" },
        ]}
        label="Composição: Saídas fixas 70%, Cartão 30%"
      />,
    );
    expect(getByRole("img", { name: /Saídas fixas 70%/ })).toBeInTheDocument();
  });

  it("sem label é decorativa (aria-hidden)", () => {
    const { container } = render(
      <SegBar
        segments={[{ name: "Saídas fixas", fraction: 1, color: "var(--type-saida)" }]}
      />,
    );
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });

  it("fatias zeradas não renderizam; as demais dividem o espaço pela fração", () => {
    const { container } = render(
      <SegBar
        segments={[
          { name: "a", fraction: 0.5, color: "a" },
          { name: "b", fraction: 0, color: "b" },
          { name: "c", fraction: 0.5, color: "c" },
        ]}
      />,
    );
    const slices = container.querySelectorAll("span");
    expect(slices).toHaveLength(2);
  });

  it("frações que não somam 1 são normalizadas (composição sempre fecha)", () => {
    const { container } = render(
      <SegBar
        segments={[
          { name: "a", fraction: 0.25, color: "a" },
          { name: "b", fraction: 0.25, color: "b" },
        ]}
      />,
    );
    const slices = Array.from(container.querySelectorAll("span"));
    for (const slice of slices) {
      expect(parseFloat(slice.style.flexGrow)).toBe(50);
    }
  });
});
