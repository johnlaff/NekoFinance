import type { Mock } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type {
  DashboardSummary,
  Forecast,
  OwnerTotal,
  Pockets,
  TransactionRow,
} from "../lib/api";
import { invalidateCommands } from "../lib/useCommand";

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * Test helper for Tauri command mocking. Each test file is responsible for
 * `vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))` (vi.mock is
 * hoisted per-file); this module then routes calls by command name.
 */
export const mockInvoke = invoke as unknown as Mock<InvokeFn>;

export function mockCommands(handlers: Record<string, unknown>) {
  invalidateCommands(); // each scenario starts with a cold command cache
  mockInvoke.mockImplementation((cmd) => {
    if (cmd in handlers) {
      const value = handlers[cmd];
      return value instanceof Error ? Promise.reject(value) : Promise.resolve(value);
    }
    return Promise.reject(new Error(`unmocked command: ${cmd}`));
  });
}

export const SUMMARY: DashboardSummary = {
  balance: 842000,
  daily_budget: 4300,
  daily_spend_today: 3800,
  reserve_months: 4.5,
  reserve_trend: "down",
  transaction_count: 42,
  last_real_tx_date: "2026-06-19",
};

export const TXNS: TransactionRow[] = [
  {
    id: "t1",
    type: "expense",
    amount: 4300,
    description: "Despesa demo variável",
    date: "2026-03-15",
    payment_method: "debit",
    is_projection: false,
    is_fixed: false,
    owners: ["Pessoa A", "Pessoa B"],
    tags: [],
    provenance: "importado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
  },
  {
    id: "t2",
    type: "expense",
    amount: 120000,
    description: "Compromisso demo no crédito",
    date: "2026-03-10",
    payment_method: "credit",
    is_projection: false,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "manual",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
  },
  {
    id: "t3",
    type: "income",
    amount: 350000,
    description: "Receita demo projetada",
    date: "2026-06-25",
    payment_method: "",
    is_projection: true,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "projetado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
  },
];

/** Totais por titular de um mês (multi-titular) para a seção "Por titular" dos Totais. */
export const OWNER_TOTALS: OwnerTotal[] = [
  { owner_person_id: "p1", owner_name: "Titular A", total_cents: 320000 },
  { owner_person_id: "p2", owner_name: "Titular B", total_cents: 180000 },
];

export const POCKETS: Pockets = {
  liquid_cents: 842000,
  reserve_cents: 1500000,
  restricted_cents: 42000,
  illiquid_cents: 1200000,
  net_worth_cents: 3542000,
  accounts: [
    {
      id: "a1",
      name: "Conta corrente",
      type: "bank",
      liquidity: "liquid",
      balance: 842000,
      institution: null,
    },
    {
      id: "a2",
      name: "Poupança",
      type: "savings",
      liquidity: "reserve",
      balance: 1500000,
      institution: null,
    },
    {
      id: "a3",
      name: "Vale refeição",
      type: "meal_voucher",
      liquidity: "restricted",
      balance: 42000,
      institution: null,
    },
    {
      id: "a4",
      name: "Previdência",
      type: "pension",
      liquidity: "illiquid",
      balance: 1200000,
      institution: null,
    },
  ],
};

export const EMPTY_POCKETS: Pockets = {
  liquid_cents: 0,
  reserve_cents: 0,
  restricted_cents: 0,
  illiquid_cents: 0,
  net_worth_cents: 0,
  accounts: [],
};

export const APP_INFO = {
  version: "0.1.0",
  db_path: "/tmp/neko-test/neko-finance.db",
};

const ANNUAL_SAVINGS = {
  realized_income_cents: 5000000,
  realized_savings_cents: 300000,
  realized_rate_bps: 600,
  registered_economia_cents: 250000,
  projected_income_cents: 6000000,
  projected_savings_cents: 1500000,
  projected_rate_bps: 2500,
  target_bps: 2500,
};

export const FORECAST: Forecast = {
  today: "2026-06-10",
  horizon_end: "2026-12-31",
  annual_savings: ANNUAL_SAVINGS,
  coverage: [
    {
      year: 2026,
      month: 8,
      projected_outflow_cents: 416500,
      baseline_outflow_cents: 1064900,
      coverage_bps: 3911,
      is_complete: false,
      estimated_missing_cents: 648400,
    },
  ],
  baseline_outflow_cents: 1064900,
  trusted_through_month: "2026-07",
  total_missing_cents: 648400,
  safe_to_spend_today_cents: 35000,
  cash_headroom_cents: 587700,
  savings_headroom_cents: 35000,
  binding_guardrail: "savings",
  savings_target_bps: 2500,
  months: [
    {
      year: 2026,
      month: 6,
      income_cents: 700000,
      performance_cents: 450000,
      cost_of_living_cents: 250000,
      fixed_out_cents: 250000,
      daily_out_cents: 0,
      real_daily_avg_cents: 0,
      economia_cents: 0,
      savings_rate_bps: 2500,
    },
    {
      year: 2026,
      month: 7,
      income_cents: 900000,
      performance_cents: 90000,
      cost_of_living_cents: 810000,
      fixed_out_cents: 810000,
      daily_out_cents: 0,
      real_daily_avg_cents: 0,
      economia_cents: 0,
      savings_rate_bps: 1000,
    },
  ],
  deepest_deficit: { date: "2026-06-15", balance_cents: 587700 },
  daily: [
    {
      date: "2026-06-10",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 842000,
    },
    {
      date: "2026-06-15",
      income_cents: 0,
      fixed_out_cents: 250000,
      daily_out_cents: 4300,
      economia_cents: 0,
      balance_cents: 587700,
    },
    {
      date: "2026-06-25",
      income_cents: 700000,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 1287700,
    },
    {
      date: "2026-06-30",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      economia_cents: 0,
      balance_cents: 1287700,
    },
  ],
  month_end: [{ year: 2026, month: 6, balance_cents: 1287700 }],
};
