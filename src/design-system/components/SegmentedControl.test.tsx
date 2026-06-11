import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SegmentedControl } from "./SegmentedControl";

const OPTIONS = [
  { value: "day", label: "Dia" },
  { value: "week", label: "Semana" },
  { value: "month", label: "Mês" },
];

describe("SegmentedControl", () => {
  it("renders all options", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="day" onChange={onChange} />);
    expect(screen.getByText("Dia")).toBeInTheDocument();
    expect(screen.getByText("Semana")).toBeInTheDocument();
    expect(screen.getByText("Mês")).toBeInTheDocument();
  });

  it("calls onChange when option clicked", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="day" onChange={onChange} />);
    fireEvent.click(screen.getByText("Semana"));
    expect(onChange).toHaveBeenCalledWith("week");
  });

  it("highlights active option", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="month" onChange={onChange} />);
    const monthBtn = screen.getByText("Mês") as HTMLButtonElement;
    expect(monthBtn.style.color).toBe("var(--primary)");
  });

  it("does not call onChange when disabled", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl options={OPTIONS} value="day" onChange={onChange} disabled />,
    );
    fireEvent.click(screen.getByText("Semana"));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("renders with sm size", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl options={OPTIONS} value="day" onChange={onChange} size="sm" />,
    );
    const btn = screen.getByText("Dia") as HTMLButtonElement;
    expect(btn.style.fontSize).toBe("var(--fs-sm)");
  });
});
