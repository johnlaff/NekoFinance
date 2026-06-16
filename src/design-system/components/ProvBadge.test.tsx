import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import { ProvBadge } from "./ProvBadge";

describe("ProvBadge", () => {
  it("mostra o rótulo da proveniência", () => {
    render(<ProvBadge provenance="importado" />);
    expect(screen.getByText("Da planilha")).toBeInTheDocument();
    render(<ProvBadge provenance="manual" />);
    expect(screen.getByText("Do app")).toBeInTheDocument();
    render(<ProvBadge provenance="projetado" />);
    expect(screen.getByText("Previsto")).toBeInTheDocument();
  });

  it("explica a proveniência no popover (didático)", async () => {
    const user = userEvent.setup();
    render(<ProvBadge provenance="projetado" />);
    await user.click(screen.getByRole("button", { name: /Previsto/ }));
    expect(screen.getByRole("tooltip")).toHaveTextContent(/Ainda não aconteceu/);
  });

  it("proveniência desconhecida não renderiza nada", () => {
    const { container } = render(<ProvBadge provenance="???" />);
    expect(container).toBeEmptyDOMElement();
  });
});
