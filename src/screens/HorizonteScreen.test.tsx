import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { HorizonteScreen } from "./HorizonteScreen";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// O termômetro do saldo (faixas absolutas da planilha) é testado em `lib/saldoHeatmap.test.ts`.

describe("HorizonteScreen (render)", () => {
  it("renderiza o heatmap com a coluna do mês", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, get_upcoming_bills_cmd: [] });
    render(<HorizonteScreen />);
    await waitFor(() => {
      expect(screen.getByText("Horizonte de saldos")).toBeInTheDocument();
    });
    // FORECAST.daily está em junho/2026 → coluna "Junho".
    expect(screen.getByText("Junho")).toBeInTheDocument();
  });

  it("lista os vencimentos próximos com data e valor formatados", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [
        {
          id: "b1",
          description: "Conta demo A",
          amount: 12500,
          due_date: "2026-06-28",
          is_projection: false,
        },
        {
          id: "b2",
          description: "Conta demo B",
          amount: 30000,
          due_date: "2026-07-05",
          is_projection: true,
        },
      ],
    });
    render(<HorizonteScreen />);

    // Espera o conteúdo dos vencimentos (a revalidação em segundo plano do useCommand pode
    // entregar primeiro um cache vazio compartilhado entre arquivos; aguardamos a lista real).
    await waitFor(() => {
      expect(screen.getByText("Conta demo A")).toBeInTheDocument();
    });
    expect(screen.getByText("Vencimentos próximos")).toBeInTheDocument();
    expect(screen.getByText("Conta demo B")).toBeInTheDocument();
    // Data formatada (DD/MM/YYYY) e valor (saída → negativo) presentes.
    expect(screen.getByText("28/06/2026")).toBeInTheDocument();
    expect(screen.getByText("05/07/2026")).toBeInTheDocument();
    expect(screen.getByText(/125,00/)).toBeInTheDocument();
    // A conta projetada mostra o badge "Previsto".
    expect(screen.getByText("Previsto")).toBeInTheDocument();
  });

  it("mostra o estado vazio quando não há vencimentos", async () => {
    mockInvoke.mockReset();
    mockCommands({ get_forecast: FORECAST, get_upcoming_bills_cmd: [] });
    render(<HorizonteScreen />);

    await waitFor(() => {
      expect(
        screen.getByText("Nenhum vencimento nos próximos 60 dias"),
      ).toBeInTheDocument();
    });
  });
});
