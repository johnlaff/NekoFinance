import type { Mock } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { DashboardSummary, Forecast, Pockets, TransactionRow } from "../lib/api";
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
  credit_spend_month: 120000,
  reserve_months: 4.5,
  reserve_trend: "down",
  transaction_count: 42,
};

export const EMPTY_SUMMARY: DashboardSummary = {
  balance: 0,
  daily_budget: 0,
  daily_spend_today: 0,
  credit_spend_month: 0,
  reserve_months: 0,
  reserve_trend: "flat",
  transaction_count: 0,
};

export const TXNS: TransactionRow[] = [
  {
    id: "t1",
    type: "expense",
    amount: 4300,
    description: "Café + mercado",
    date: "2026-03-15",
    payment_method: "debit",
    is_projection: false,
  },
  {
    id: "t2",
    type: "expense",
    amount: 120000,
    description: "Streaming anual",
    date: "2026-03-10",
    payment_method: "credit",
    is_projection: false,
  },
  {
    id: "t3",
    type: "income",
    amount: 350000,
    description: "Salário projetado",
    date: "2026-06-25",
    payment_method: "",
    is_projection: true,
  },
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

export const FORECAST: Forecast = {
  today: "2026-06-10",
  horizon_end: "2026-06-30",
  safe_to_spend_today_cents: 35000,
  deepest_deficit: { date: "2026-06-15", balance_cents: 587700 },
  daily: [
    {
      date: "2026-06-10",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      balance_cents: 842000,
    },
    {
      date: "2026-06-15",
      income_cents: 0,
      fixed_out_cents: 250000,
      daily_out_cents: 4300,
      balance_cents: 587700,
    },
    {
      date: "2026-06-25",
      income_cents: 700000,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      balance_cents: 1287700,
    },
    {
      date: "2026-06-30",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      balance_cents: 1287700,
    },
  ],
  month_end: [{ year: 2026, month: 6, balance_cents: 1287700 }],
};

export const DEFICIT_FORECAST: Forecast = {
  ...FORECAST,
  safe_to_spend_today_cents: 0,
  deepest_deficit: { date: "2026-06-28", balance_cents: -42000 },
  daily: [
    ...FORECAST.daily.slice(0, 2),
    {
      date: "2026-06-28",
      income_cents: 0,
      fixed_out_cents: 629700,
      daily_out_cents: 0,
      balance_cents: -42000,
    },
  ],
};

export const EMPTY_FORECAST: Forecast = {
  today: "2026-06-10",
  horizon_end: "2026-06-30",
  safe_to_spend_today_cents: 0,
  deepest_deficit: { date: "2026-06-10", balance_cents: 0 },
  daily: [
    {
      date: "2026-06-10",
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      balance_cents: 0,
    },
  ],
  month_end: [{ year: 2026, month: 6, balance_cents: 0 }],
};
