import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { MonthLedgerCard } from "./MonthLedgerCard";
import { MONTH_GRID, mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Characterization tests (plan 010): the card loads get_month_grid via useCommand.
// PIN the rendered month name, footer totals (footerOf), empty state, and month nav.
// Money usa NBSP entre "R$" e o número, então as asserts usam \s no regex (como o
// DailyCheckinCard.test.tsx).

describe("MonthLedgerCard", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders_month_name_and_grid_rows", async () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<MonthLedgerCard today="2026-06-10" />);
    await waitFor(() => expect(screen.getByText("Junho de 2026")).toBeInTheDocument());
    // Pelo menos uma linha de dados (o dia 25 tem entrada de R$ 7.000,00).
    expect(screen.getAllByText(/R\$\s?7\.000,00/).length).toBeGreaterThan(0);
  });

  it("footer_shows_correct_totals", async () => {
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<MonthLedgerCard today="2026-06-10" />);
    await waitFor(() => expect(screen.getByText("Junho de 2026")).toBeInTheDocument());
    // income total 700000 aparece no dia 25 E no rodapé → pelo menos 2 ocorrências.
    expect(screen.getAllByText(/R\$\s?7\.000,00/).length).toBeGreaterThanOrEqual(2);
    // saidaTotal = 250000 + 4300 = 254300 → R$ 2.543,00 (só no rodapé).
    expect(screen.getByText(/R\$\s?2\.543,00/)).toBeInTheDocument();
    // performance = 700000 − 254300 = 445700 → R$ 4.457,00 (só no rodapé).
    expect(screen.getByText(/R\$\s?4\.457,00/)).toBeInTheDocument();
  });

  it("shows_empty_state_when_no_data", async () => {
    mockCommands({ get_month_grid: [] });
    render(<MonthLedgerCard today="2026-06-10" />);
    await waitFor(() =>
      expect(screen.getByText(/Mês sem lançamentos/)).toBeInTheDocument(),
    );
  });

  it("month_nav_changes_the_loaded_month", async () => {
    const user = userEvent.setup();
    // Mesmo mock para junho e julho — o rótulo do mês vem do estado, não dos dados.
    mockCommands({ get_month_grid: MONTH_GRID });
    render(<MonthLedgerCard today="2026-06-10" />);
    await waitFor(() => expect(screen.getByText("Junho de 2026")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Próximo mês" }));
    await waitFor(() => expect(screen.getByText("Julho de 2026")).toBeInTheDocument());
  });
});
