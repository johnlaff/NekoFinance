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
      has_credit: true,
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
      months: [
        {
          year: 2026,
          month: 6,
          income_cents: 700000,
          performance_cents: 445700,
          cost_of_living_cents: 254300,
          savings_rate_bps: 6367,
          real_daily_avg_cents: 2971,
          economia_cents: 150000,
        },
        {
          year: 2026,
          month: 7,
          income_cents: 899331,
          performance_cents: 87645,
          cost_of_living_cents: 811686,
          savings_rate_bps: 974,
          real_daily_avg_cents: 0,
          economia_cents: 0,
        },
      ],
      cash_headroom_cents: 587700,
      savings_headroom_cents: 35000,
      binding_guardrail: "cash",
      savings_target_bps: 2500,
      annual_savings: {
        realized_income_cents: 6500000,
        realized_savings_cents: 400000,
        realized_rate_bps: 615,
        projected_income_cents: 11800000,
        projected_savings_cents: 2200000,
        projected_rate_bps: 1864,
        target_bps: 2500,
      },
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

    const ANNUAL = {
      year: 2026,
      months: Array.from({ length: 12 }, (_, i) => ({
        year: 2026,
        month: i + 1,
        income_cents: i + 1 === 6 ? 700000 : 0,
        performance_cents: i + 1 === 6 ? 445700 : 0,
        cost_of_living_cents: i + 1 === 6 ? 254300 : 0,
        real_daily_avg_cents: 0,
        economia_cents: 0,
        savings_rate_bps: i + 1 === 6 ? 2200 : 0,
      })),
    };

    const TAG_TOTALS = [
      {
        id: "p",
        name: "! Pagar",
        color: "var(--brass-400)",
        emoji: null,
        is_special: true,
        total_cents: 2500,
      },
      {
        id: "v",
        name: "Viagem",
        color: "var(--cat-sky)",
        emoji: "\u2708\uFE0F",
        is_special: false,
        total_cents: 10000,
      },
      {
        id: "d",
        name: "Delivery",
        color: "var(--cat-coral)",
        emoji: "\uD83C\uDF54",
        is_special: false,
        total_cents: 35000,
      },
    ];

    const responses: Record<string, unknown> = {
      check_auth_status: "disconnected",
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      tag_totals_for_month_cmd: TAG_TOTALS,
      get_annual_metrics: ANNUAL,
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
