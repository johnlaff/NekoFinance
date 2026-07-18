import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NoRecordDash } from "./NoRecordDash";

describe("NoRecordDash", () => {
  it("mostra travessão decorativo + rótulo e abre a didática com o que falta", async () => {
    const user = userEvent.setup();
    render(
      <NoRecordDash
        term={{ title: "Sem teto", body: "Estipule o teto na cerimônia." }}
        cta={<button type="button">Estipular</button>}
      />,
    );
    expect(screen.getByText("—")).toHaveAttribute("aria-hidden", "true");
    const trigger = screen.getByRole("button", { name: "Sem registro" });
    expect(screen.getByRole("button", { name: "Estipular" })).toBeInTheDocument();
    await user.click(trigger);
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "Estipule o teto na cerimônia.",
    );
  });

  it("aceita rótulo dedicado de zero-diagnóstico", () => {
    render(
      <NoRecordDash
        term={{ body: "Contas de reserva zeradas." }}
        label="Sem reserva"
      />,
    );
    expect(screen.getByRole("button", { name: "Sem reserva" })).toBeInTheDocument();
  });
});
