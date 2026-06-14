import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import { Disclosure } from "./Disclosure";

describe("Disclosure", () => {
  it("começa fechado e alterna o aria-expanded ao clicar", async () => {
    const user = userEvent.setup();
    render(
      <Disclosure title="Como pré-lançar o ano">
        <p>conteúdo denso</p>
      </Disclosure>,
    );
    const head = screen.getByRole("button", { name: /Como pré-lançar/ });
    expect(head).toHaveAttribute("aria-expanded", "false");
    // O resumo (título) está sempre visível; o corpo existe no DOM (animado por CSS).
    expect(screen.getByText("conteúdo denso")).toBeInTheDocument();

    await user.click(head);
    expect(head).toHaveAttribute("aria-expanded", "true");
    await user.click(head);
    expect(head).toHaveAttribute("aria-expanded", "false");
  });

  it("respeita defaultOpen e liga head→region por aria-controls", () => {
    render(
      <Disclosure title="Detalhe" defaultOpen>
        <p>corpo</p>
      </Disclosure>,
    );
    const head = screen.getByRole("button", { name: "Detalhe" });
    expect(head).toHaveAttribute("aria-expanded", "true");
    const region = screen.getByRole("region");
    expect(head.getAttribute("aria-controls")).toBe(region.getAttribute("id"));
  });
});
