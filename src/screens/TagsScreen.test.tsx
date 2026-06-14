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
    total_cents: 2500,
  },
  {
    id: "v",
    name: "Viagem",
    color: "var(--cat-sky)",
    emoji: "✈️",
    is_special: false,
    total_cents: 10000,
  },
];

describe("TagsScreen", () => {
  it("lista as tags com total do mês (especial no topo)", async () => {
    mockInvoke.mockReset();
    mockCommands({ tag_totals_for_month_cmd: TOTALS });
    render(<TagsScreen />);
    await waitFor(() => expect(screen.getByText("! Pagar")).toBeInTheDocument());
    expect(screen.getByText("Viagem")).toBeInTheDocument();
    expect(screen.getByText("✈️")).toBeInTheDocument();
    // Ordem: a especial vem antes (lista <li> na ordem do backend).
    const items = screen.getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("! Pagar");
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
});
