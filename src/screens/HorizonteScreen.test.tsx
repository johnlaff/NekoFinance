import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { HorizonteScreen } from "./HorizonteScreen";
import { mockCommands, mockInvoke } from "../test/commands";
import type { Forecast, MonthEnd, TransactionRow } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// A régua do saldo (faixas absolutas) é testada em `lib/saldoHeatmap.test.ts`; a composição do
// veredito/estrada/grade/compromissos, em `horizonteView.test.ts`. Aqui provamos que a tela
// monta os dados reais e mostra as peças-chave.

const ann = {
  realized_income_cents: 0,
  realized_savings_cents: 0,
  realized_rate_bps: 0,
  registered_economia_cents: 0,
  patrimonio_cents: 0,
  economia_ruler_cents: 0,
  economia_ruler_rate_bps: 0,
  includes_previdencia: false,
  economia_state: "no_record" as const,
  projected_income_cents: 0,
  projected_savings_cents: 0,
  projected_rate_bps: 0,
  target_bps: 2500,
};

const MONTH_END: MonthEnd[] = [
  { year: 2026, month: 7, balance_cents: 1299520 },
  { year: 2026, month: 8, balance_cents: 1468037 },
  { year: 2026, month: 9, balance_cents: 1825323 },
  { year: 2026, month: 12, balance_cents: 2997711 },
];

const FORECAST: Forecast = {
  today: "2026-07-22",
  horizon_end: "2026-12-31",
  annual_savings: ann,
  coverage: [
    {
      year: 2026,
      month: 9,
      projected_outflow_cents: 383522,
      baseline_outflow_cents: 1112126,
      coverage_bps: 3300,
      is_complete: false,
      estimated_missing_cents: 728604,
    },
  ],
  baseline_outflow_cents: 1112126,
  trusted_through_month: "2026-08",
  total_missing_cents: 728604,
  safe_to_spend_today_cents: 20000,
  cash_headroom_cents: 752086,
  savings_headroom_cents: 20000,
  binding_guardrail: "savings",
  savings_target_bps: 2500,
  deepest_deficit: null,
  daily: [
    {
      date: "2026-07-22",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 756830,
    },
    {
      date: "2026-07-26",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 752086,
    },
    {
      date: "2026-09-10",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 1825323,
    },
    {
      date: "2026-12-31",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 2997711,
    },
  ],
  month_end: MONTH_END,
  months: [],
};

function tx(
  over: Partial<TransactionRow> & { id: string; date: string },
): TransactionRow {
  return {
    type: "expense",
    amount: 10000,
    description: "",
    payment_method: "debit",
    is_projection: true,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "projetado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    has_refund_link: false,
    ...over,
  };
}

const AGOSTO: TransactionRow[] = [
  tx({
    id: "a1",
    date: "2026-08-07",
    description: "Financiamento do carro",
    amount: 100651,
    installment_index: 11,
    installment_total: 36,
  }),
  tx({
    id: "a2",
    date: "2026-08-27",
    description: "Salário",
    amount: 601273,
    type: "income",
    is_fixed: true,
  }),
];

function mockLivre() {
  mockInvoke.mockReset();
  mockCommands({
    get_forecast: FORECAST,
    last_sync_at: "2026-07-22 22:14:00",
    scenario_forecast: null,
    get_recent_transactions: (args) =>
      (args?.["month"] as string) === "2026-08" ? AGOSTO : [],
  });
}

describe("HorizonteScreen (render)", () => {
  it("veredito livre: manchete com o mês de confiança", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText(/Caminho livre até o fim de agosto/)).toBeInTheDocument(),
    );
  });

  it("desenha a estrada com rótulo acessível", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByRole("img", { name: /Saldo projetado/ })).toBeInTheDocument(),
    );
  });

  it("mostra a grade dos 12 meses e o card de compromissos com parcela", async () => {
    mockLivre();
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText("Os próximos 12 meses")).toBeInTheDocument(),
    );
    // Os compromissos vêm de uma segunda query (meses derivados do forecast) — findByText espera.
    expect(await screen.findByText("O que já está marcado")).toBeInTheDocument();
    expect(screen.getByText("Financiamento do carro")).toBeInTheDocument();
    // Parcela n/N em mono.
    expect(screen.getByText("11/36")).toBeInTheDocument();
  });

  it("aperto: manchete do buraco quando o lançado cruza o zero", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: {
        ...FORECAST,
        deepest_deficit: { date: "2026-09-12", balance_cents: -124000 },
      },
      last_sync_at: null,
      scenario_forecast: null,
      get_recent_transactions: () => [],
    });
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(screen.getByText(/O caminho aperta em setembro/)).toBeInTheDocument(),
    );
  });

  it("estado de erro oferece tentar novamente", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: new Error("boom"),
      last_sync_at: null,
      get_recent_transactions: () => [],
    });
    render(<HorizonteScreen />);
    await waitFor(() =>
      expect(
        screen.getByText("Não foi possível carregar o horizonte"),
      ).toBeInTheDocument(),
    );
  });
});
