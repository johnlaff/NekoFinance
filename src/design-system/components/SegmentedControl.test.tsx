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

  it("exposes radiogroup/radio semantics with aria-checked", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl
        options={OPTIONS}
        value="week"
        onChange={onChange}
        ariaLabel="Período"
      />,
    );
    expect(screen.getByRole("radiogroup", { name: "Período" })).toBeInTheDocument();
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(3);
    expect(screen.getByRole("radio", { name: "Semana" })).toBeChecked();
  });

  it("uses roving tabindex (only the selected radio is tabbable)", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="day" onChange={onChange} />);
    expect(screen.getByRole("radio", { name: "Dia" })).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("radio", { name: "Semana" })).toHaveAttribute(
      "tabindex",
      "-1",
    );
  });

  it("moves selection with arrow keys", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="day" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("radio", { name: "Dia" }), {
      key: "ArrowRight",
    });
    expect(onChange).toHaveBeenCalledWith("week");
  });

  it("wraps and supports Home/End keys", () => {
    const onChange = vi.fn();
    render(<SegmentedControl options={OPTIONS} value="day" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("radio", { name: "Dia" }), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith("month"); // wrap-around
    fireEvent.keyDown(screen.getByRole("radio", { name: "Dia" }), { key: "End" });
    expect(onChange).toHaveBeenLastCalledWith("month");
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
    const monthBtn = screen.getByText("Mês");
    expect(monthBtn.style.color).toBe("var(--primary)");
  });

  it("updates the active option when value prop changes", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <SegmentedControl options={OPTIONS} value="day" onChange={onChange} />,
    );

    rerender(<SegmentedControl options={OPTIONS} value="week" onChange={onChange} />);

    expect(screen.getByText("Semana").style.color).toBe("var(--primary)");
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
    const btn = screen.getByText("Dia");
    expect(btn.style.fontSize).toBe("var(--fs-sm)");
  });
});
