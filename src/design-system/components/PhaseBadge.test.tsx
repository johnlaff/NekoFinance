import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PhaseBadge } from "./PhaseBadge";

describe("PhaseBadge", () => {
  it("mostra o rótulo da fase e a posição na jornada", () => {
    render(<PhaseBadge phase="calibrate" />);
    // Rótulo visível + texto sr-only com a posição na jornada (substitui o antigo role="img").
    expect(screen.getByText("Calibrar")).toBeInTheDocument();
    expect(
      screen.getByText(/Fase de adaptação: Calibrar \(2 de 3\)/),
    ).toBeInTheDocument();
  });

  it("Mapear é 1 de 3, Operar é 3 de 3", () => {
    const { rerender } = render(<PhaseBadge phase="map" />);
    expect(screen.getByText(/Mapear \(1 de 3\)/)).toBeInTheDocument();
    rerender(<PhaseBadge phase="operate" />);
    expect(screen.getByText(/Operar \(3 de 3\)/)).toBeInTheDocument();
  });
});
