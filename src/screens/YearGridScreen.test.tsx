import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import { mockCommands, mockInvoke, MONTH_GRID } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("YearGridScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders all 12 month sections", async () => {
    // get_month_grid retorna o mesmo MONTH_GRID para as 12 chamadas.
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);

    await waitFor(() => expect(screen.getByText("Janeiro")).toBeInTheDocument());
    expect(screen.getByText("Fevereiro")).toBeInTheDocument();
    expect(screen.getByText("Dezembro")).toBeInTheDocument();
    // 12 <section aria-label> = 12 regiões.
    expect(screen.getAllByRole("region").length).toBe(12);
  });

  it("shows termômetro coloring on non-null Saldo cells", async () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);
    await waitFor(() => expect(screen.getByText("Janeiro")).toBeInTheDocument());
    // Ao menos uma célula de Saldo colorida (MONTH_GRID tem balance_cents setado);
    // o title carrega o rótulo da faixa do termômetro.
    const titledCells = document.querySelectorAll('td[title^="Saldo "]');
    expect(titledCells.length).toBeGreaterThan(0);
  });

  it("shows empty state per month when no data", async () => {
    mockCommands({ get_month_grid: [] });
    render(<YearGridScreen />);
    await waitFor(() =>
      expect(screen.getAllByText(/Sem lançamentos/).length).toBeGreaterThan(0),
    );
  });

  it("shows year heading with the current year", () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<YearGridScreen />);
    expect(screen.getByText("Ano inteiro")).toBeInTheDocument();
  });
});
