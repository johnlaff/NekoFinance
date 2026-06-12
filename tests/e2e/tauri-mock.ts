import type { Page } from "@playwright/test";

/**
 * Installs a fake `window.__TAURI_INTERNALS__` before the app loads, so the
 * real frontend renders with deterministic data in a plain browser. Commands
 * mirror the fixtures used by the vitest suite (src/test/commands.ts).
 */
export async function mockTauri(page: Page) {
  await page.addInitScript(() => {
    const SUMMARY = {
      balance: 842000,
      daily_budget: 4300,
      daily_spend_today: 3800,
      credit_spend_month: 120000,
      reserve_months: 4.5,
      reserve_trend: "down",
      transaction_count: 42,
    };

    const FORECAST = {
      today: "2026-06-10",
      horizon_end: "2026-06-30",
      safe_to_spend_today_cents: 35000,
      deepest_deficit: { date: "2026-06-15", balance_cents: 587700 },
      daily: [
        {
          date: "2026-06-10",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 3800,
          balance_cents: 842000,
        },
        {
          date: "2026-06-11",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 842000,
        },
        {
          date: "2026-06-12",
          income_cents: 0,
          fixed_out_cents: 18900,
          daily_out_cents: 0,
          balance_cents: 823100,
        },
        {
          date: "2026-06-13",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 823100,
        },
        {
          date: "2026-06-14",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 823100,
        },
        {
          date: "2026-06-15",
          income_cents: 0,
          fixed_out_cents: 231100,
          daily_out_cents: 4300,
          balance_cents: 587700,
        },
        {
          date: "2026-06-16",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 587700,
        },
        {
          date: "2026-06-17",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 587700,
        },
        {
          date: "2026-06-18",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 587700,
        },
        {
          date: "2026-06-19",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 587700,
        },
        {
          date: "2026-06-20",
          income_cents: 0,
          fixed_out_cents: 12500,
          daily_out_cents: 0,
          balance_cents: 575200,
        },
        {
          date: "2026-06-21",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 575200,
        },
        {
          date: "2026-06-22",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 575200,
        },
        {
          date: "2026-06-23",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 575200,
        },
        {
          date: "2026-06-24",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 575200,
        },
        {
          date: "2026-06-25",
          income_cents: 700000,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 1275200,
        },
        {
          date: "2026-06-26",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 1275200,
        },
        {
          date: "2026-06-27",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 1275200,
        },
        {
          date: "2026-06-28",
          income_cents: 0,
          fixed_out_cents: 41200,
          daily_out_cents: 0,
          balance_cents: 1234000,
        },
        {
          date: "2026-06-29",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 1234000,
        },
        {
          date: "2026-06-30",
          income_cents: 0,
          fixed_out_cents: 0,
          daily_out_cents: 0,
          balance_cents: 1234000,
        },
      ],
      month_end: [{ year: 2026, month: 6, balance_cents: 1234000 }],
    };

    const TXNS = [
      {
        id: "t1",
        type: "expense",
        amount: 4300,
        description: "Café + mercado",
        date: "2026-06-10",
        payment_method: "debit",
        is_projection: false,
      },
      {
        id: "t2",
        type: "expense",
        amount: 18900,
        description: "Assinatura streaming",
        date: "2026-06-08",
        payment_method: "credit",
        is_projection: false,
      },
      {
        id: "t3",
        type: "expense",
        amount: 12500,
        description: "Conta de luz",
        date: "2026-06-05",
        payment_method: "pix",
        is_projection: false,
      },
      {
        id: "t4",
        type: "income",
        amount: 700000,
        description: "Salário",
        date: "2026-06-25",
        payment_method: "",
        is_projection: true,
      },
      {
        id: "t5",
        type: "expense",
        amount: 231100,
        description: "Aluguel",
        date: "2026-06-15",
        payment_method: "debit",
        is_projection: true,
      },
    ];

    const POCKETS = {
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

    const APP_INFO = {
      version: "0.1.0",
      db_path: "C:\\Users\\you\\AppData\\Roaming\\app.neko.finance\\neko-finance.db",
    };

    const responses: Record<string, unknown> = {
      check_auth_status: "disconnected",
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_recent_transactions: TXNS,
      get_app_info: APP_INFO,
      get_pockets: POCKETS,
      create_account: "e2e-account-id",
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: (cmd: string) => {
          if (cmd in responses) return Promise.resolve(responses[cmd]);
          return Promise.reject(new Error(`e2e mock: unmocked command ${cmd}`));
        },
        transformCallback: () => 0,
      },
      configurable: true,
    });
  });
}
