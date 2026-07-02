import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TagsScreen } from "./TagsScreen";
import type { TagTotal } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const TOTALS: TagTotal[] = [
  {
    id: "t1",
    name: "Moradia",
    color: "var(--cat-jade)",
    emoji: null,
    is_special: false,
    exclude_from_totals: false,
    total_cents: 226070,
  },
  {
    id: "t2",
    name: "Cartão",
    color: "var(--cat-violet)",
    emoji: null,
    is_special: false,
    exclude_from_totals: true,
    total_cents: 218500,
  },
];

describe("TagsScreen (Tags)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the per-tag spend list for the month", async () => {
    mockCommands({ tag_totals_for_month_cmd: TOTALS });
    render(<TagsScreen />);
    expect(await screen.findByText("Gasto por tag")).toBeInTheDocument();
    expect(screen.getByText("Moradia")).toBeInTheDocument();
  });

  // Feature 1: criar tag (nome + cor + emoji opcional).
  it("creates a tag from the Nova tag form", async () => {
    mockCommands({
      tag_totals_for_month_cmd: TOTALS,
      create_tag_cmd: "new-id",
    });
    render(<TagsScreen />);
    await screen.findByText("Gasto por tag");

    await userEvent.click(screen.getByRole("button", { name: "Nova tag" }));
    await userEvent.type(screen.getByLabelText("Nome da tag"), "Viagem");
    await userEvent.click(screen.getByRole("radio", { name: "Azul" }));
    await userEvent.click(screen.getByRole("button", { name: "Criar tag" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("create_tag_cmd", {
        name: "Viagem",
        color: "var(--cat-sky)",
        emoji: null,
        isSpecial: false,
      }),
    );
  });

  // Feature 1: nomes com "!" viram tag especial (fixada no topo).
  it("marks tags starting with ! as special", async () => {
    mockCommands({
      tag_totals_for_month_cmd: TOTALS,
      create_tag_cmd: "new-id",
    });
    render(<TagsScreen />);
    await screen.findByText("Gasto por tag");

    await userEvent.click(screen.getByRole("button", { name: "Nova tag" }));
    await userEvent.type(screen.getByLabelText("Nome da tag"), "! Pagar");
    await userEvent.click(screen.getByRole("button", { name: "Criar tag" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "create_tag_cmd",
        expect.objectContaining({ name: "! Pagar", isSpecial: true }),
      ),
    );
  });

  // Feature 2: toggle "Ignorar nos cálculos" por tag.
  it("toggles 'ignorar nos cálculos' for a tag", async () => {
    mockCommands({
      tag_totals_for_month_cmd: TOTALS,
      update_tag_exclude_cmd: null,
    });
    render(<TagsScreen />);
    await screen.findByText("Moradia");

    const sw = screen.getByRole("switch", {
      name: 'Ignorar "Moradia" nos cálculos',
    });
    expect(sw).toHaveAttribute("aria-checked", "false");
    // A tag já excluída expõe o rótulo inverso.
    expect(
      screen.getByRole("switch", { name: 'Incluir "Cartão" nos cálculos' }),
    ).toHaveAttribute("aria-checked", "true");

    await userEvent.click(sw);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("update_tag_exclude_cmd", {
        tagId: "t1",
        exclude: true,
      }),
    );
  });

  // Feature 4: navegação de mês (o comando de totais aceita ano/mês).
  it("navigates to the previous month and refetches tag totals", async () => {
    const now = new Date();
    const prev = new Date(Date.UTC(now.getFullYear(), now.getMonth() - 1, 1));
    const prevY = prev.getUTCFullYear();
    const prevM = prev.getUTCMonth() + 1;

    mockCommands({
      tag_totals_for_month_cmd: (args) => {
        const a = args as { year: number; month: number };
        return a.year === prevY && a.month === prevM
          ? [{ ...TOTALS[0]!, id: "past", name: "Gasto do mês passado" }]
          : TOTALS;
      },
    });
    render(<TagsScreen />);
    await screen.findByText("Moradia");

    await userEvent.click(screen.getByRole("button", { name: "Mês anterior" }));
    expect(await screen.findByText("Gasto do mês passado")).toBeInTheDocument();
  });

  // Renomear/recolorir tag existente (update_tag_cmd).
  it("edits an existing tag (rename + recolor) via the pencil button", async () => {
    mockCommands({
      tag_totals_for_month_cmd: TOTALS,
      update_tag_cmd: null,
    });
    render(<TagsScreen />);
    await screen.findByText("Moradia");

    await userEvent.click(screen.getByRole("button", { name: 'Editar tag "Moradia"' }));
    const nameInput = screen.getByLabelText("Nome da tag");
    expect(nameInput).toHaveValue("Moradia");

    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Casa");
    await userEvent.click(screen.getByRole("radio", { name: "Coral" }));
    await userEvent.click(screen.getByRole("button", { name: "Salvar tag" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("update_tag_cmd", {
        tagId: "t1",
        name: "Casa",
        color: "var(--cat-coral)",
        emoji: null,
      }),
    );
  });
});
