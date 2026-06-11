import type { Mock } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { DashboardSummary, TransactionRow } from "../lib/api";

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * Test helper for Tauri command mocking. Each test file is responsible for
 * `vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))` (vi.mock is
 * hoisted per-file); this module then routes calls by command name.
 */
export const mockInvoke = invoke as unknown as Mock<InvokeFn>;

export function mockCommands(handlers: Record<string, unknown>) {
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

export const APP_INFO = {
  version: "0.1.0",
  db_path: "/tmp/neko-test/neko-finance.db",
};
