import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { filterTransactions } from "./transactionsFilter";
import { TXNS, RECURRING_TXN, mockCommands, mockInvoke } from "../test/commands";
import type { Tag, TransactionRow } from "../lib/api";

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
    exclude_from_totals: false,
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
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);

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
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      // Projeção → "Previsto"; importado → "Da planilha"; manual → "Do app".
      expect(screen.getByText("Previsto")).toBeInTheDocument();
    });
    expect(screen.getAllByText("Da planilha").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Do app")).toBeInTheDocument();
  });

  it("applies the controlled query from the global header search", async () => {
    // A tela não tem mais busca própria: o filtro chega só pela prop `query` (⌘K do header).
    render(<TransactionsScreen query="crédito" onGoToSettings={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Compromisso demo no crédito")).toBeInTheDocument();
    });
    expect(screen.queryByText("Despesa demo variável")).not.toBeInTheDocument();
    // Indicador "Busca: …" mostra a consulta ativa sem reintroduzir um segundo input.
    expect(screen.getByText("Busca:")).toBeInTheDocument();
    // Não existe mais um campo de busca local concorrendo com o ⌘K do header.
    expect(screen.queryByLabelText("Filtrar por descrição")).not.toBeInTheDocument();
  });

  it("shows the empty state with a settings hint when there is no data", async () => {
    mockCommands({ get_recent_transactions: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Nenhum lançamento encontrado")).toBeInTheDocument();
    });
    expect(screen.getByText(/Importe sua planilha/)).toBeInTheDocument();
  });

  it("shows a Settings CTA when there are no transactions", async () => {
    const user = userEvent.setup();
    const onGoToSettings = vi.fn();
    mockCommands({ get_recent_transactions: [] });
    render(<TransactionsScreen query="" onGoToSettings={onGoToSettings} />);
    await waitFor(() => {
      expect(screen.getByText("Ir para Configurações")).toBeInTheDocument();
    });
    await user.click(screen.getByText("Ir para Configurações"));
    expect(onGoToSettings).toHaveBeenCalledOnce();
  });

  it("does not show the Settings CTA when a filter merely returns nothing", async () => {
    // Há lançamentos, mas a busca ativa não casa nenhum → empty state SEM CTA (não é falta de dados).
    mockCommands({ get_recent_transactions: TXNS });
    render(<TransactionsScreen query="zzzz-sem-resultado" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(
        screen.getByText(/Nenhum resultado para o filtro atual/),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText("Ir para Configurações")).not.toBeInTheDocument();
  });

  it("shows the error state when the fetch fails", async () => {
    mockCommands({ get_recent_transactions: new Error("db locked") });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
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
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
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

describe("TransactionsScreen — apagar/editar (ações da linha)", () => {
  let confirmSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mockInvoke.mockReset();
    confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    confirmSpy.mockRestore();
  });

  it("apaga um lançamento único pelo painel de ações (caminho feliz)", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_recent_transactions: TXNS,
      list_tags_cmd: [],
      delete_transaction_cmd: null,
    });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Despesa demo variável/ }),
    );
    await user.click(screen.getByRole("button", { name: "Apagar" }));

    expect(confirmSpy).toHaveBeenCalled();
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("delete_transaction_cmd", {
        id: "t1",
      });
    });
  });

  it("mostra um alerta e mantém a linha quando apagar falha", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_recent_transactions: TXNS,
      list_tags_cmd: [],
      delete_transaction_cmd: new Error("db locked"),
    });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Despesa demo variável/ }),
    );
    await user.click(screen.getByRole("button", { name: "Apagar" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/O banco local está ocupado/);
    });
    // A linha continua presente (não foi removida da lista).
    expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
  });

  it("apaga toda a série quando o lançamento é recorrente e o usuário confirma 'toda a série'", async () => {
    const user = userEvent.setup();
    // window.confirm retorna true → "Apagar TODA a série" → delete_series_all_cmd.
    mockCommands({
      get_recent_transactions: [RECURRING_TXN],
      list_tags_cmd: [],
      delete_series_all_cmd: 3,
    });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Compromisso recorrente demo")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Compromisso recorrente demo/ }),
    );
    await user.click(screen.getByRole("button", { name: "Apagar da série" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("delete_series_all_cmd", {
        recurrenceId: "rec-uuid-abc",
      });
    });
  });

  it("apaga apenas deste ponto em diante quando o usuário recusa 'toda a série'", async () => {
    const user = userEvent.setup();
    // 1º confirm (toda a série) → false; 2º confirm (este e futuros) → true → delete_series_from_cmd.
    confirmSpy.mockReturnValueOnce(false).mockReturnValueOnce(true);
    mockCommands({
      get_recent_transactions: [RECURRING_TXN],
      list_tags_cmd: [],
      delete_series_from_cmd: 1,
    });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Compromisso recorrente demo")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Compromisso recorrente demo/ }),
    );
    await user.click(screen.getByRole("button", { name: "Apagar da série" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("delete_series_from_cmd", {
        transactionId: "rec-uuid-abc:2",
      });
    });
  });

  it("fecha o painel de ações ao clicar no botão uma segunda vez", async () => {
    const user = userEvent.setup();
    mockCommands({ get_recent_transactions: TXNS, list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    const actionBtn = screen.getByRole("button", {
      name: /Ações para Despesa demo variável/,
    });
    await user.click(actionBtn);
    expect(screen.getByRole("button", { name: "Apagar" })).toBeInTheDocument();

    await user.click(actionBtn);
    expect(screen.queryByRole("button", { name: "Apagar" })).not.toBeInTheDocument();
  });
});

describe("TransactionsScreen — painel de ações de linha importada (plano 043)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra o aviso de re-import no painel de ações de uma linha importada", async () => {
    const user = userEvent.setup();
    // t1 (Despesa demo variável) tem provenance "importado" nas fixtures.
    mockCommands({ get_recent_transactions: TXNS, list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Despesa demo variável/ }),
    );
    expect(screen.getByText(/Linha importada da planilha/)).toBeInTheDocument();
  });

  it("o botão Editar dispara mesmo numa linha importada (não fica desabilitado)", async () => {
    const user = userEvent.setup();
    mockCommands({ get_recent_transactions: TXNS, list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Despesa demo variável/ }),
    );
    const editar = screen.getByRole("button", { name: "Editar" });
    expect(editar).not.toBeDisabled();
    // Editar abre o form inline de edição (não há rejeição silenciosa para importados).
    await user.click(editar);
    expect(screen.getByRole("button", { name: "Salvar" })).toBeInTheDocument();
  });

  it("NÃO mostra o aviso de importado numa linha manual", async () => {
    const user = userEvent.setup();
    // t2 (Compromisso demo no crédito) tem provenance "manual" nas fixtures.
    mockCommands({ get_recent_transactions: TXNS, list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Compromisso demo no crédito")).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /Ações para Compromisso demo no crédito/ }),
    );
    expect(screen.queryByText(/Linha importada da planilha/)).not.toBeInTheDocument();
  });
});

describe("TransactionsScreen — breakdown itemizado", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("não mostra botão de expand quando não há itens", async () => {
    mockCommands({ get_recent_transactions: TXNS, list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("button", {
        name: /Ver itens de Despesa demo variável/,
      }),
    ).not.toBeInTheDocument();
  });

  it("mostra e oculta os itens ao clicar no botão de expand", async () => {
    const user = userEvent.setup();
    const itemizedTxn: TransactionRow = {
      ...TXNS[0]!,
      id: "t-itemized",
      description: "Despesa com itens",
      amount: 15000,
      line_items: [
        {
          id: "li:t-itemized:0",
          transaction_id: "t-itemized",
          amount_cents: 10000,
          description: "Parte A",
          position: 0,
        },
        {
          id: "li:t-itemized:1",
          transaction_id: "t-itemized",
          amount_cents: 5000,
          description: "Parte B",
          position: 1,
        },
      ],
    };
    mockCommands({ get_recent_transactions: [itemizedTxn], list_tags_cmd: [] });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Despesa com itens")).toBeInTheDocument();
    });

    // Itens escondidos inicialmente.
    expect(screen.queryByText("Parte A")).not.toBeInTheDocument();
    expect(screen.queryByText("Parte B")).not.toBeInTheDocument();

    // Clica para expandir.
    await user.click(
      screen.getByRole("button", { name: /Ver itens de Despesa com itens/ }),
    );
    expect(screen.getByText("Parte A")).toBeInTheDocument();
    expect(screen.getByText("Parte B")).toBeInTheDocument();

    // Clica de novo para recolher.
    await user.click(
      screen.getByRole("button", { name: /Fechar itens de Despesa com itens/ }),
    );
    expect(screen.queryByText("Parte A")).not.toBeInTheDocument();
  });

  it("mostra itens de lançamentos projetados (futuro)", async () => {
    const projectedItemized: TransactionRow = {
      ...TXNS[2]!,
      id: "t-projected-itemized",
      description: "Receita projetada com itens",
      line_items: [
        {
          id: "li:t-projected-itemized:0",
          transaction_id: "t-projected-itemized",
          amount_cents: 200000,
          description: "Parte projetada",
          position: 0,
        },
        {
          id: "li:t-projected-itemized:1",
          transaction_id: "t-projected-itemized",
          amount_cents: 150000,
          description: "Outra parte projetada",
          position: 1,
        },
      ],
    };
    mockCommands({
      get_recent_transactions: [projectedItemized],
      list_tags_cmd: [],
    });
    render(<TransactionsScreen query="" onGoToSettings={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Receita projetada com itens")).toBeInTheDocument();
    });
    await userEvent.setup().click(
      screen.getByRole("button", {
        name: /Ver itens de Receita projetada com itens/,
      }),
    );
    expect(screen.getByText("Parte projetada")).toBeInTheDocument();
    expect(screen.getByText("Outra parte projetada")).toBeInTheDocument();
  });
});
