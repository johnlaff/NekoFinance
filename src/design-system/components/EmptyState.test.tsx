import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("renders empty variant with title and description", () => {
    render(<EmptyState title="Nada aqui" description="Nenhum item encontrado" />);
    expect(screen.getByText("Nada aqui")).toBeInTheDocument();
    expect(screen.getByText("Nenhum item encontrado")).toBeInTheDocument();
  });

  it("renders loading variant with spinner", () => {
    const { container } = render(<EmptyState variant="loading" />);
    const spinner = container.querySelector(".nk-state__spin");
    expect(spinner).toBeInTheDocument();
  });

  it("renders error variant", () => {
    render(<EmptyState variant="error" title="Erro" description="Algo deu errado" />);
    expect(screen.getByText("Erro")).toBeInTheDocument();
    expect(screen.getByText("Algo deu errado")).toBeInTheDocument();
  });

  it("renders skeleton variant with correct number of rows", () => {
    const { container } = render(<EmptyState variant="skeleton" skeletonRows={3} />);
    const rows = container.querySelectorAll(".nk-skel__row");
    expect(rows.length).toBe(3);
  });

  it("renders default skeleton rows when not specified", () => {
    const { container } = render(<EmptyState variant="skeleton" />);
    const rows = container.querySelectorAll(".nk-skel__row");
    expect(rows.length).toBe(4);
  });

  it("renders action slot", () => {
    render(
      <EmptyState title="VAZIO" action={<button type="button">Recarregar</button>} />,
    );
    expect(screen.getByText("Recarregar")).toBeInTheDocument();
  });

  it("renders custom icon", () => {
    render(<EmptyState icon={<span data-testid="custom-icon">*</span>} />);
    expect(screen.getByTestId("custom-icon")).toBeInTheDocument();
  });
});
