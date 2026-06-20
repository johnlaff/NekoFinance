import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { TagsScreen } from "./TagsScreen";
import type { TagTotal } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const TOTALS: TagTotal[] = [
  {
    id: "p",
    name: "! Pagar",
    color: "var(--brass-400)",
    emoji: null,
    is_special: true,
    exclude_from_totals: false,
    total_cents: 2500,
  },
  {
    id: "v",
    name: "Categoria demo A",
    color: "var(--cat-sky)",
    emoji: null,
    is_special: false,
    exclude_from_totals: false,
    total_cents: 10000,
  },
];

describe("TagsScreen", () => {
  it("lista as tags com total do mês (especial no topo)", async () => {
    mockInvoke.mockReset();
    mockCommands({ tag_totals_for_month_cmd: TOTALS });
    render(<TagsScreen />);
    await waitFor(() => expect(screen.getByText("! Pagar")).toBeInTheDocument());
    expect(screen.getByText("Categoria demo A")).toBeInTheDocument();
    // Ordem: a especial vem antes (lista <li> na ordem do backend).
    const items = screen.getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("! Pagar");
  });

  it('toggle "Ignorar nos cálculos" persiste via update_tag_exclude_cmd', async () => {
    const user = userEvent.setup();
    mockInvoke.mockReset();
    mockCommands({
      tag_totals_for_month_cmd: TOTALS,
      update_tag_exclude_cmd: null,
    });
    render(<TagsScreen />);
    await waitFor(() =>
      expect(screen.getByText("Categoria demo A")).toBeInTheDocument(),
    );

    const switches = screen.getAllByRole("switch");
    expect(switches).toHaveLength(2);
    // Default: incluído (não excluído).
    expect(switches[0]).toHaveAttribute("aria-checked", "false");

    await user.click(
      screen.getByRole("switch", { name: 'Ignorar "Categoria demo A" nos cálculos' }),
    );

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("update_tag_exclude_cmd", {
        tagId: "v",
        exclude: true,
      }),
    );
  });

  it("mostra EmptyState quando não há tags", async () => {
    mockInvoke.mockReset();
    mockCommands({ tag_totals_for_month_cmd: [] });
    render(<TagsScreen />);
    await waitFor(() =>
      expect(screen.getByText("Nenhuma tag ainda")).toBeInTheDocument(),
    );
  });

  it("abre o formulário de nova tag", async () => {
    mockInvoke.mockReset();
    mockCommands({ tag_totals_for_month_cmd: [] });
    render(<TagsScreen />);
    await waitFor(() =>
      expect(screen.getByText("Nenhuma tag ainda")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Nova tag" }));
    expect(screen.getByLabelText("Nome da tag")).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Cor da tag" })).toBeInTheDocument();
  });

  it("mantém o formulário aberto e mostra erro quando a criação falha", async () => {
    const user = userEvent.setup();
    mockInvoke.mockReset();
    mockCommands({
      tag_totals_for_month_cmd: [],
      create_tag_cmd: new Error("db locked"),
    });
    render(<TagsScreen />);
    await waitFor(() =>
      expect(screen.getByText("Nenhuma tag ainda")).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "Nova tag" }));
    await user.type(screen.getByLabelText("Nome da tag"), "Categoria demo A");
    await user.click(screen.getByRole("button", { name: "Criar tag" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/O banco local está ocupado/);
    });
    expect(screen.getByLabelText("Nome da tag")).toHaveValue("Categoria demo A");
  });

  it("seletor de cor: padrão radiogroup (roving tabindex + setas)", async () => {
    mockInvoke.mockReset();
    mockCommands({ tag_totals_for_month_cmd: [] });
    render(<TagsScreen />);
    await waitFor(() =>
      expect(screen.getByText("Nenhuma tag ainda")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Nova tag" }));
    const swatches = screen.getAllByRole("radio");
    // Roving tabindex: só o selecionado (1º, default) é tabbable.
    expect(swatches[0]).toHaveAttribute("tabindex", "0");
    expect(swatches[1]).toHaveAttribute("tabindex", "-1");
    expect(swatches[0]).toHaveAttribute("aria-checked", "true");
    // Seta direita move a seleção e o foco para o próximo swatch.
    swatches[0]!.focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(swatches[1]).toHaveAttribute("aria-checked", "true");
    expect(swatches[1]).toHaveFocus();
    expect(swatches[1]).toHaveAttribute("tabindex", "0");
    // Wrap: da primeira para a esquerda vai para a última.
    swatches[0]!.focus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(swatches[swatches.length - 1]).toHaveAttribute("aria-checked", "true");
  });
});
