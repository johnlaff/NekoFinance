import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Badge } from "./Badge";

describe("Badge", () => {
  it("renders children", () => {
    render(<Badge>Ativo</Badge>);
    expect(screen.getByText("Ativo")).toBeInTheDocument();
  });

  it("renders dot when dot=true", () => {
    const { container } = render(<Badge dot>Com Ponto</Badge>);
    const spans = container.querySelectorAll("span span");
    expect(spans.length).toBeGreaterThan(0);
    expect(screen.getByText("Com Ponto")).toBeInTheDocument();
  });

  it("applies square border radius when square=true", () => {
    const { container } = render(<Badge square>Quadrado</Badge>);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.borderRadius).toBe("4px");
  });

  it("renders with success tone", () => {
    const { container } = render(<Badge tone="success">Sucesso</Badge>);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.color).toBeTruthy();
    expect(screen.getByText("Sucesso")).toBeInTheDocument();
  });

  it("renders with danger tone", () => {
    const { container } = render(<Badge tone="danger">Perigo</Badge>);
    const el = container.firstElementChild as HTMLElement;
    expect(el.style.color).toBeTruthy();
    expect(screen.getByText("Perigo")).toBeInTheDocument();
  });

  it("renders with warning tone", () => {
    render(<Badge tone="warning">Aviso</Badge>);
    expect(screen.getByText("Aviso")).toBeInTheDocument();
  });

  it("renders with info tone", () => {
    render(<Badge tone="info">Info</Badge>);
    expect(screen.getByText("Info")).toBeInTheDocument();
  });

  it("accepts className prop", () => {
    const { container } = render(<Badge className="my-class">Custom</Badge>);
    const el = container.firstElementChild as HTMLElement;
    expect(el.className).toContain("my-class");
  });
});
