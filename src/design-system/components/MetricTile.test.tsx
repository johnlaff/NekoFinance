import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { MetricTile } from "./MetricTile";

describe("MetricTile", () => {
  it("renders label and value", () => {
    render(<MetricTile label="Saldo" value="R$ 8.420" />);
    expect(screen.getByText("Saldo")).toBeInTheDocument();
    expect(screen.getByText("R$ 8.420")).toBeInTheDocument();
  });

  it("renders delta with up direction (green)", () => {
    render(<MetricTile label="Saldo" value="R$ 1.000" delta="+12%" deltaDir="up" />);
    const deltaEl = screen.getByText(/\+12%/);
    expect(deltaEl.style.color).toBe("var(--money-pos)");
  });

  it("renders delta with down direction (red)", () => {
    render(<MetricTile label="Saldo" value="R$ 1.000" delta="-5%" deltaDir="down" />);
    const deltaEl = screen.getByText(/-5%/);
    expect(deltaEl.style.color).toBe("var(--money-neg)");
  });

  it("renders delta with neutral direction", () => {
    render(<MetricTile label="Saldo" value="R$ 1.000" delta="0%" deltaDir="neutral" />);
    const deltaEl = screen.getByText(/0%/);
    expect(deltaEl.style.color).toBe("var(--text-muted)");
  });

  it("renders sublabel", () => {
    render(<MetricTile label="Saldo" value="R$ 1.000" sublabel="Fim do mês" />);
    expect(screen.getByText("Fim do mês")).toBeInTheDocument();
  });

  it("renders sparkline SVG when spark data provided", () => {
    const { container } = render(
      <MetricTile label="Trend" value="R$ 500" spark={[10, 20, 15, 30, 25]} />,
    );
    const svg = container.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("does not render sparkline when spark array empty", () => {
    const { container } = render(<MetricTile label="Flat" value="R$ 0" spark={[]} />);
    const svg = container.querySelector("svg");
    expect(svg).toBeNull();
  });

  it("renders icon", () => {
    render(
      <MetricTile
        label="Saldo"
        value="R$ 100"
        icon={<span data-testid="icon">+</span>}
      />,
    );
    expect(screen.getByTestId("icon")).toBeInTheDocument();
  });
});
