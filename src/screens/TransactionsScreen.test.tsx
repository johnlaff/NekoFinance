import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TransactionsScreen, filterTransactions } from "./TransactionsScreen";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("filterTransactions", () => {
  it("keeps everything on scope all with empty query", () => {
    expect(filterTransactions(TXNS, "all", "")).toHaveLength(3);
  });

  it("keeps only credit on scope credit", () => {
    const out = filterTransactions(TXNS, "credit", "");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Streaming anual");
  });

  it("keeps only projections on scope future", () => {
    const out = filterTransactions(TXNS, "future", "");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Salário projetado");
  });

  it("matches query case-insensitively with diacritics intact", () => {
    const out = filterTransactions(TXNS, "all", "salário");
    expect(out).toHaveLength(1);
    expect(out[0]?.description).toBe("Salário projetado");
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
      expect(screen.getByText("Café + mercado")).toBeInTheDocument();
    });
    expect(screen.getByText("3 exibidas")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Crédito" }));
    expect(screen.getByText("1 exibida")).toBeInTheDocument();
    expect(screen.queryByText("Café + mercado")).not.toBeInTheDocument();
    expect(screen.getByText("Streaming anual")).toBeInTheDocument();
  });

  it("marks projections with a badge", async () => {
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("previsto")).toBeInTheDocument();
    });
  });

  it("applies the controlled query and reports edits upward", async () => {
    const user = userEvent.setup();
    const onQueryChange = vi.fn();
    render(<TransactionsScreen query="streaming" onQueryChange={onQueryChange} />);

    await waitFor(() => {
      expect(screen.getByText("Streaming anual")).toBeInTheDocument();
    });
    expect(screen.queryByText("Café + mercado")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Filtrar por descrição"), "x");
    expect(onQueryChange).toHaveBeenCalledWith("streamingx");
  });

  it("shows the empty state with a settings hint when there is no data", async () => {
    mockCommands({ get_recent_transactions: [] });
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Nenhuma transação encontrada")).toBeInTheDocument();
    });
    expect(screen.getByText(/Importe sua planilha/)).toBeInTheDocument();
  });

  it("shows the error state when the fetch fails", async () => {
    mockCommands({ get_recent_transactions: new Error("db locked") });
    render(<TransactionsScreen query="" onQueryChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/db locked/)).toBeInTheDocument();
    });
  });
});
