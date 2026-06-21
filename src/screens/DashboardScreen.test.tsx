import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import {
  DEFICIT_FORECAST,
  EMPTY_POCKETS,
  FORECAST,
  MONTH_GRID,
  POCKETS,
  SUMMARY,
  mockCommands,
  mockInvoke,
} from "../test/commands";
import { invalidateCommands } from "../lib/useCommand";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// `listenEvent` (write-back/conflito) importa este módulo dinamicamente; `unlisten` no-op.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

describe("DashboardScreen (forecast view)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("shows the safe-to-spend callout", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.getByText("R$ 350,00")).toBeInTheDocument();
  });

  it("renders the daily month grid with today marked and dashes for zero flows", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    // Espera o CORPO da grade carregar (o título do card aparece antes dos dados).
    await waitFor(() => {
      expect(screen.getAllByText("R$ 12.877,00").length).toBeGreaterThanOrEqual(2);
    });
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();

    // "hoje" aparece no sufixo do herói e como marcador na grade do mês.
    expect(screen.getAllByText("hoje").length).toBeGreaterThanOrEqual(1);
    // Income day shows the inflow (também aparece no total do rodapé).
    expect(screen.getAllByText("R$ 7.000,00").length).toBeGreaterThanOrEqual(1);
    // Zero flows render as em-dashes, not R$ 0,00.
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });

  it("shows the deficit warning only when the projection goes negative", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    const { unmount } = render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(screen.queryByText(/Buraco previsto/)).not.toBeInTheDocument();
    unmount();

    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: DEFICIT_FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/Buraco previsto/)).toBeInTheDocument();
    });
    // O buraco aparece ao menos no alerta e no "pode faltar" do herói (a grade do mês é
    // testada à parte). Money usa o minus real (U+2212).
    expect(screen.getAllByText("−R$ 420,00").length).toBeGreaterThanOrEqual(2);
  });

  it("shows liquidity-grouped pockets and the net worth (spec 007)", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: POCKETS,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Caixa")).toBeInTheDocument();
    });
    expect(screen.getByText("Bolsos", { exact: true })).toBeInTheDocument();
    expect(screen.getByText("R$ 15.000,00")).toBeInTheDocument(); // reserva
    expect(screen.getByText("R$ 420,00")).toBeInTheDocument(); // vale
    expect(screen.getByText("R$ 12.000,00")).toBeInTheDocument(); // ilíquido
    expect(screen.getByText("R$ 35.420,00")).toBeInTheDocument(); // patrimônio
  });

  it("hints at Ajustes when no pocket exists yet", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: EMPTY_POCKETS,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Nenhum bolso cadastrado/)).toBeInTheDocument();
    });
  });

  it("shows an explicit error in the pockets card instead of the empty state", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
      get_pockets: new Error("db locked"),
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() => {
      expect(
        screen.getByText(/Não foi possível carregar os bolsos/),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText(/Nenhum bolso cadastrado/)).not.toBeInTheDocument();
  });

  it("names the projected month in the hero forecast head", async () => {
    // O metric tile redundante ("Saldo projetado · Fim de junho") foi removido; o mês projetado
    // continua nomeado no cabeçalho do herói ("Saldo no fim de junho").
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/Saldo no fim de junho/)).toBeInTheDocument();
    });
  });

  it("does not render the redundant 4-tile metric bar", async () => {
    // Os números desses tiles já vivem no herói (saldo projetado, pode-gastar) e nos cards abaixo.
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_month_grid: MONTH_GRID,
    });
    const { container } = render(<DashboardScreen onAskMia={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    expect(container.querySelector(".dash-grid4")).toBeNull();
    expect(screen.queryByRole("article", { name: "Saldo projetado" })).toBeNull();
    expect(screen.queryByRole("article", { name: "Crédito no mês" })).toBeNull();
  });
});

// Indicador de write-back pendente (plano 031): conta as células local → planilha por enviar e
// abre o MESMO painel de aprovação (plano 028); os conflitos de importação são resolvíveis no
// próprio dashboard (via ConflictGate). Roteia `invoke` por comando E (no get_app_setting) por chave.
const MAPPING_JSON = JSON.stringify({ spreadsheetId: "ss-1", label: "minha planilha" });

const PREVIEW_2_PENDING = {
  cells: [
    {
      a1: "B5",
      row: 5,
      col: 2,
      date: "2026-06-01",
      kind: "saida",
      current: "R$ 100,00",
      proposed: "R$ 120,00",
      value_cents: 12000,
      changed: true,
    },
    {
      a1: "B6",
      row: 6,
      col: 2,
      date: "2026-06-02",
      kind: "saida",
      current: "R$ 50,00",
      proposed: "R$ 80,00",
      value_cents: 8000,
      changed: true,
    },
  ],
  preview_revision: "rev-abc",
  conflicts_pending: false,
  multi_card_warning: false,
};

const CONFLICT_AMOUNT = {
  id: "c1",
  transaction_id: "t1",
  field: "amount",
  base_value: "10000",
  local_value: "15000",
  sheet_value: "20000",
};

function routeDashboard(opts: {
  appSetting?: Record<string, string | null>;
  preview?: unknown;
  conflicts?: unknown;
  writeBackEnabled?: unknown;
  applyWriteBack?: unknown;
}) {
  invalidateCommands();
  mockInvoke.mockImplementation(
    (cmd: string, args?: Record<string, unknown>): Promise<unknown> => {
      switch (cmd) {
        case "get_dashboard_summary":
          return Promise.resolve(SUMMARY);
        case "get_forecast":
          return Promise.resolve(FORECAST);
        case "get_month_grid":
          return Promise.resolve(MONTH_GRID);
        case "get_pockets":
          return Promise.resolve(POCKETS);
        case "get_app_setting":
          return Promise.resolve(opts.appSetting?.[String(args?.["key"])] ?? null);
        case "preview_write_back_status":
          return opts.preview instanceof Error
            ? Promise.reject(opts.preview)
            : Promise.resolve(opts.preview ?? { cells: [], preview_revision: "x" });
        case "apply_write_back":
          return opts.applyWriteBack instanceof Error
            ? Promise.reject(opts.applyWriteBack)
            : Promise.resolve(
                opts.applyWriteBack ?? { written: 2, note_warning: null },
              );
        case "get_import_conflicts":
          return Promise.resolve(opts.conflicts ?? []);
        case "write_back_enabled":
          return Promise.resolve(opts.writeBackEnabled ?? true);
        default:
          return Promise.reject(new Error(`unmocked command: ${cmd}`));
      }
    },
  );
}

describe("DashboardScreen — write-back pendente (plano 031)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra a contagem de células pendentes de um status mockado", async () => {
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_2_PENDING,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() =>
      expect(
        screen.getByText(/2 célula\(s\) local → planilha pendente\(s\)/),
      ).toBeInTheDocument(),
    );
  });

  it("'Revisar e enviar' abre o painel de aprovação completo (plano 028)", async () => {
    const user = userEvent.setup();
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_2_PENDING,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    // O selo mostra a contagem; o fluxo completo fica atrás de "Revisar e enviar".
    await waitFor(() =>
      expect(
        screen.getByText(/2 célula\(s\) local → planilha pendente\(s\)/),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByText("Write-back para a planilha")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Revisar e enviar" }));

    // O MESMO componente de aprovação (plano 028) aparece — cabeçalho + botão de prévia.
    await waitFor(() =>
      expect(screen.getByText("Write-back para a planilha")).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: "Gerar prévia do diff" }),
    ).toBeInTheDocument();
  });

  it("surfaca os conflitos com um ponto de resolução no dashboard", async () => {
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: { cells: [], preview_revision: "x" },
      conflicts: [CONFLICT_AMOUNT],
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    // ConflictGate reaproveitado: cabeçalho + ações de resolução (sem ir a Lançamentos).
    await waitFor(() =>
      expect(screen.getByText(/1 conflito de importação/)).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Manter o meu" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Usar da planilha" }),
    ).toBeInTheDocument();
  });

  it("não mostra o indicador sem planilha mapeada (degradação silenciosa)", async () => {
    routeDashboard({ appSetting: { sheets_last_import: null } });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() =>
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/local → planilha pendente/)).not.toBeInTheDocument();
  });

  it("com a flag desligada, mostra o indicador como aviso não-clicável", async () => {
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_2_PENDING,
      writeBackEnabled: false,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await waitFor(() =>
      expect(
        screen.getByText(/2 célula\(s\) local → planilha pendente\(s\)/),
      ).toBeInTheDocument(),
    );
    // Sem botão clicável (vira <output> de status); o envio mora nas Configurações.
    expect(
      screen.queryByRole("button", { name: /local → planilha pendente/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/Envio desativado nas Configurações/)).toBeInTheDocument();
  });
});

// Caminho rápido "Sincronizar" (plano 039): colapsa os cliques para mudanças de só-valor seguras
// (prévia silenciosa → resumo inline → 1 confirmação) — SEM colapsar nenhuma salvaguarda. Cai no
// fluxo completo quando o diff não é seguro (conflito, multi-cartão, risco de coluna de fórmula).
describe("DashboardScreen — caminho rápido Sincronizar (plano 039)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("diff seguro: Sincronizar abre a confirmação inline e UMA confirmação escreve com o previewRevision", async () => {
    const user = userEvent.setup();
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_2_PENDING, // só células `saida` (kind seguro), sem conflito/multi-cartão
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await user.click(await screen.findByRole("button", { name: "Sincronizar" }));

    // Confirmação aparece SEM expandir o painel completo; ainda não escreveu.
    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    expect(screen.queryByText("Write-back para a planilha")).not.toBeInTheDocument();
    // Resumo inline lista as células (≤ 5 → uma linha).
    expect(screen.getByText(/2 célula\(s\):/)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());

    await user.click(screen.getByRole("button", { name: "Confirmar envio" }));

    // UMA escrita, com o token de frescura DAQUELA prévia.
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "apply_write_back",
        expect.objectContaining({ previewRevision: "rev-abc" }),
      ),
    );
    const applyCalls = mockInvoke.mock.calls.filter((c) => c[0] === "apply_write_back");
    expect(applyCalls).toHaveLength(1);
  });

  it("cancelar a confirmação não escreve nada", async () => {
    const user = userEvent.setup();
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_2_PENDING,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await user.click(await screen.findByRole("button", { name: "Sincronizar" }));
    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Cancelar" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());
  });

  it("diff NÃO seguro (multi-cartão): Sincronizar cai no fluxo completo, sem confirmação rápida", async () => {
    const user = userEvent.setup();
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: { ...PREVIEW_2_PENDING, multi_card_warning: true },
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await user.click(await screen.findByRole("button", { name: "Sincronizar" }));

    // Sem confirmação rápida; em vez disso, o painel multi-etapas completo abre.
    await waitFor(() =>
      expect(screen.getByText("Write-back para a planilha")).toBeInTheDocument(),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());
  });

  it("diff NÃO seguro (coluna de fórmula): Sincronizar cai no fluxo completo", async () => {
    const user = userEvent.setup();
    const withFormula = {
      ...PREVIEW_2_PENDING,
      cells: [
        ...PREVIEW_2_PENDING.cells,
        {
          a1: "F7",
          row: 7,
          col: 6,
          date: "2026-06-03",
          kind: "balance", // coluna de fórmula → nunca pelo caminho rápido
          current: "R$ 0,00",
          proposed: "R$ 10,00",
          value_cents: 1000,
          changed: true,
        },
      ],
    };
    routeDashboard({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: withFormula,
    });
    render(<DashboardScreen onAskMia={vi.fn()} />);

    await user.click(await screen.findByRole("button", { name: "Sincronizar" }));

    await waitFor(() =>
      expect(screen.getByText("Write-back para a planilha")).toBeInTheDocument(),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());
  });
});
