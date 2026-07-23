import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Meter } from "./Meter";

describe("Meter", () => {
  it("é decorativa sem label e vira imagem nomeada com label", () => {
    const { container, rerender } = render(<Meter fraction={0.5} />);
    const bare = container.firstElementChild!;
    expect(bare.getAttribute("aria-hidden")).toBe("true");
    expect(bare.getAttribute("role")).toBeNull();

    rerender(<Meter fraction={0.5} label="Metade do teto" />);
    const named = container.firstElementChild!;
    expect(named.getAttribute("role")).toBe("img");
    expect(named.getAttribute("aria-label")).toBe("Metade do teto");
  });

  it("clampa o preenchimento a 0–100% — o texto vizinho é quem diz o excesso", () => {
    const { container, rerender } = render(<Meter fraction={1.5} />);
    let fill = container.querySelector("span")!;
    expect(fill.style.width).toBe("100%");

    rerender(<Meter fraction={-0.2} />);
    fill = container.querySelector("span")!;
    expect(fill.style.width).toBe("0%");
  });
});
