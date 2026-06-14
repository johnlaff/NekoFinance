import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PhaseBadge } from "./PhaseBadge";

describe("PhaseBadge", () => {
  it("mostra o rótulo da fase e a posição na jornada", () => {
    render(<PhaseBadge phase="calibrate" />);
    expect(screen.getByText("Calibrar")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: /Calibrar \(2 de 3\)/ }),
    ).toBeInTheDocument();
  });

  it("Mapear é 1 de 3, Operar é 3 de 3", () => {
    const { rerender } = render(<PhaseBadge phase="map" />);
    expect(screen.getByRole("img", { name: /1 de 3/ })).toBeInTheDocument();
    rerender(<PhaseBadge phase="operate" />);
    expect(screen.getByRole("img", { name: /3 de 3/ })).toBeInTheDocument();
  });
});
