import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it("renders children", () => {
    render(<Button>Clique aqui</Button>);
    expect(screen.getByText("Clique aqui")).toBeInTheDocument();
  });

  it("calls onClick when clicked", () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    fireEvent.click(screen.getByText("Click"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("does not call onClick when disabled", () => {
    const onClick = vi.fn();
    render(
      <Button onClick={onClick} disabled>
        Click
      </Button>,
    );
    fireEvent.click(screen.getByText("Click"));
    expect(onClick).not.toHaveBeenCalled();
  });

  it("renders with type submit", () => {
    render(<Button type="submit">Enviar</Button>);
    const el = screen.getByText("Enviar");
    expect(el.getAttribute("type")).toBe("submit");
  });

  it("renders iconLeft", () => {
    render(<Button iconLeft={<span data-testid="icon-left">L</span>}>Texto</Button>);
    expect(screen.getByTestId("icon-left")).toBeInTheDocument();
  });

  it("renders iconRight", () => {
    render(<Button iconRight={<span data-testid="icon-right">R</span>}>Texto</Button>);
    expect(screen.getByTestId("icon-right")).toBeInTheDocument();
  });

  it("renders with ghost variant", () => {
    render(<Button variant="ghost">Ghost</Button>);
    expect(screen.getByText("Ghost")).toBeInTheDocument();
  });

  it("renders with danger variant", () => {
    render(<Button variant="danger">Danger</Button>);
    expect(screen.getByText("Danger")).toBeInTheDocument();
  });

  it("renders with size sm", () => {
    render(<Button size="sm">Small</Button>);
    expect(screen.getByText("Small")).toBeInTheDocument();
  });

  it("renders with size lg", () => {
    render(<Button size="lg">Large</Button>);
    expect(screen.getByText("Large")).toBeInTheDocument();
  });

  it("applies disabled attribute when disabled", () => {
    render(<Button disabled>Off</Button>);
    const el = screen.getByText("Off");
    expect(el).toBeDisabled();
  });
});
