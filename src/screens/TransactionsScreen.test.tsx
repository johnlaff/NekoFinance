import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { filterTransactions } from "./transactionsFilter";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";
import type { Tag } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const TAGS: Tag[] = [
  {
    id: "tag-viagem",
    name: "Categoria demo A",
    color: "var(--cat-sky)",
    emoji: null,
    is_special: false,
  },
];

describe("filterTransactions", () => {
  it("keeps everything on scope all with empty query", () => {
    expect(filterTransactions(TXNS, "all", "")).toHaveLength(3);
  });

  it("keeps only credit on scope credit", () => {
    const out = filterTransactions(TXNS, "credit", "");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Compromisso demo no crédito");
  });

  it("keeps only projections on scope future", () => {
    const out = filterTransactions(TXNS, "future", "");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Receita demo projetada");
  });

  it("matches query case-insensitively with diacritics intact", () => {
    const out = filterTransactions(TXNS, "all", "receita");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Receita demo projetada");
  });

  it("combines scope and query", () => {
    expect(filterTransactions(TXNS, "credit", "mercado")).toHaveLength(0);
  });
});

describe("TransactionsScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockCommands({ get_recent_transactions: TXNS });
  });

  it("lists transactions and updates the shown count when filtering", async () => {
    const user = userEvent.setup();
    const onQueryChange = vi.fn();
    render(<TransactionsScreen query="" onQueryChange={onQueryChange} />);

    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });
    expect(screen.getByText("3 exibidas")).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "Crédito" }));
    expect(screen.getByText("1 exibida")).toBeInTheDocument();
    expect(screen.queryByText("Despesa demo variável")).not.toBeInTheDocument();
    expect(screen.getByText("Compromisso demo no crédito")).toBeInTheDocument();
  });

  it("marca a proveniência de cada lançamento (ProvBadge)", async () => {
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      // Projeção → "Previsto"; importado → "Da planilha"; manual → "Do app".
      expect(screen.getByText("Previsto")).toBeInTheDocument();
    });
    expect(screen.getAllByText("Da planilha").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Do app")).toBeInTheDocument();
  });

  it("applies the controlled query and reports edits upward", async () => {
    const user = userEvent.setup();
    const onQueryChange = vi.fn();
    render(<TransactionsScreen query="crédito" onQueryChange={onQueryChange} />);

    await waitFor(() => {
      expect(screen.getByText("Compromisso demo no crédito")).toBeInTheDocument();
    });
    expect(screen.queryByText("Despesa demo variável")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Filtrar por descrição"), "x");
    expect(onQueryChange).toHaveBeenCalledWith("créditox");
  });

  it("shows the empty state with a settings hint when there is no data", async () => {
    mockCommands({ get_recent_transactions: [] });
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Nenhum lançamento encontrado")).toBeInTheDocument();
    });
    expect(screen.getByText(/Importe sua planilha/)).toBeInTheDocument();
  });

  it("shows the error state when the fetch fails", async () => {
    mockCommands({ get_recent_transactions: new Error("db locked") });
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/O banco local está ocupado/)).toBeInTheDocument();
    });
  });

  it("keeps the tag editor open and shows an alert when tagging fails", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_recent_transactions: TXNS,
      list_tags_cmd: TAGS,
      set_transaction_tags_cmd: new Error("db locked"),
    });
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Editar tags de Despesa demo/ }),
    );
    await user.click(screen.getByRole("button", { name: "Categoria demo A" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/O banco local está ocupado/);
    });
    expect(
      screen.getByRole("button", { name: "Categoria demo A" }),
    ).toBeInTheDocument();
  });
});
