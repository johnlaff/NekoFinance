import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EstimateMark } from "./EstimateMark";

describe("EstimateMark", () => {
  it("mostra a palavra do selo e abre a didática do ritual ao clicar", async () => {
    const user = userEvent.setup();
    render(<EstimateMark term={{ title: "Teto estimado", body: "Derivado do mês anterior." }} />);
    const trigger = screen.getByRole("button", { name: /estimativa/i });
    expect(trigger).toBeInTheDocument();
    await user.click(trigger);
    expect(screen.getByRole("tooltip")).toHaveTextContent("Derivado do mês anterior.");
  });
});
