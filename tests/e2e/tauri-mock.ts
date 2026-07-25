import type { Page } from "@playwright/test";

/**
 * Installs a fake `window.__TAURI_INTERNALS__` before the app loads, so the
 * real frontend renders with deterministic data in a plain browser. Commands
 * mirror the fixtures used by the vitest suite (src/test/commands.ts).
 */
export async function mockTauri(page: Page, overrides: Record<string, unknown> = {}) {
  await page.addInitScript((ov: Record<string, unknown>) => {
    const SUMMARY = {
      balance: 842000,
      daily_budget: 4300,
      daily_ceiling_source: "chosen",
      ceiling_proposal_pending: false,
      daily_spend_today: 3800,
      card_spend_today_cents: 0,
      reserve_months: 4.5,
      reserve_state: "verdict",
      reserve_basis_months: 6,
      reserve_trend: "down",
      spending_mode: "debit",
      card_gate: "unknown",
      card_gate_economy: "unknown",
      card_gate_economy_bps: null,
      card_gate_reserve: "unknown",
      cartao_month_cents: 0,
      next_fatura_date: null,
      next_fatura_amount_cents: 0,
      upcoming_invoices: [],
      transaction_count: 42,
      last_real_tx_date: "2026-06-09",
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
          fixed_out_cents: 230000,
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
      // Saldos de fim de mês da projeção: junho (corrente) + jul–dez (futuro no horizonte),
      // para a tela O ano projetar onde dezembro termina.
      month_end: [
        { year: 2026, month: 6, balance_cents: 1234000 },
        { year: 2026, month: 7, balance_cents: 1299520 },
        { year: 2026, month: 8, balance_cents: 1468037 },
        { year: 2026, month: 9, balance_cents: 1825323 },
        { year: 2026, month: 10, balance_cents: 2212289 },
        { year: 2026, month: 11, balance_cents: 2605000 },
        { year: 2026, month: 12, balance_cents: 2997711 },
      ],
      months: [
        {
          year: 2026,
          month: 6,
          income_cents: 700000,
          income_performance_cents: 700000,
          performance_cents: 450000,
          cost_of_living_cents: 250000,
          fixed_out_cents: 250000,
          daily_out_cents: 0,
          cartao_cents: 0,
          savings_rate_bps: 2500,
          real_daily_avg_cents: 3000,
          economia_cents: 175000,
          patrimonio_cents: 0,
        },
        {
          year: 2026,
          month: 7,
          income_cents: 900000,
          income_performance_cents: 900000,
          performance_cents: 90000,
          cost_of_living_cents: 810000,
          fixed_out_cents: 810000,
          daily_out_cents: 0,
          cartao_cents: 0,
          savings_rate_bps: 1000,
          real_daily_avg_cents: 0,
          economia_cents: 0,
          patrimonio_cents: 0,
        },
      ],
      cash_headroom_cents: 587700,
      savings_headroom_cents: 35000,
      binding_guardrail: "cash",
      savings_target_bps: 2500,
      annual_savings: {
        realized_income_cents: 5000000,
        realized_savings_cents: 300000,
        registered_economia_cents: 250000,
        patrimonio_cents: 0,
        economia_ruler_cents: 250000,
        economia_ruler_rate_bps: 500,
        includes_previdencia: false,
        economia_state: "verdict",
        realized_rate_bps: 600,
        projected_income_cents: 6000000,
        projected_savings_cents: 1500000,
        projected_rate_bps: 2500,
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
        description: "Despesa demo variável",
        date: "2026-06-10",
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
        has_refund_link: false,
      },
      {
        id: "t2",
        type: "expense",
        amount: 18900,
        description: "Compromisso demo no crédito",
        date: "2026-06-08",
        payment_method: "credit",
        is_projection: false,
        is_fixed: false,
        owners: [],
        tags: [],
        provenance: "manual",
        line_items: [],
        due_date: null,
        installment_index: 3,
        installment_total: 12,
        has_refund_link: true,
      },
      {
        id: "t3",
        type: "expense",
        amount: 12500,
        description: "Compromisso fixo demo",
        date: "2026-06-05",
        payment_method: "pix",
        is_projection: false,
        is_fixed: true,
        owners: [],
        tags: [],
        provenance: "importado",
        line_items: [
          {
            id: "li-t3-card",
            transaction_id: "t3",
            amount_cents: 4500,
            description: "Compra no crédito demo",
            position: 0,
            kind: "cartao",
            section: "CARTÕES |",
          },
          {
            id: "li-t3-saida",
            transaction_id: "t3",
            amount_cents: 5500,
            description: "Conta fixa demo",
            position: 1,
            kind: "saida",
            section: "CONTAS:",
          },
        ],
        due_date: "2026-06-28",
        installment_index: null,
        installment_total: null,
        has_refund_link: false,
      },
      {
        id: "t4",
        type: "income",
        amount: 700000,
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
        has_refund_link: false,
      },
      {
        id: "t5",
        type: "expense",
        amount: 230000,
        description: "Despesa fixa demo projetada",
        date: "2026-06-15",
        payment_method: "debit",
        is_projection: true,
        is_fixed: true,
        owners: [],
        tags: [],
        provenance: "projetado",
        line_items: [],
        due_date: null,
        installment_index: null,
        installment_total: null,
        has_refund_link: false,
      },
    ];

    const UPCOMING_BILLS = [
      {
        id: "t3",
        description: "Compromisso fixo demo",
        amount: 12500,
        due_date: "2026-06-28",
        is_projection: false,
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
          name: "Conta demo principal",
          type: "bank",
          liquidity: "liquid",
          balance: 842000,
          institution: null,
        },
        {
          id: "a2",
          name: "Reserva demo",
          type: "savings",
          liquidity: "reserve",
          balance: 1500000,
          institution: null,
        },
        {
          id: "a3",
          name: "Benefício demo",
          type: "meal_voucher",
          liquidity: "restricted",
          balance: 42000,
          institution: null,
        },
        {
          id: "a4",
          name: "Ativo demo de longo prazo",
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

    // Planilha real de 2026 (centavos): renda e performance por mês, economia zerada — o ano
    // que abre o veredito "não guardou nada". Com o relógio em junho, jul–dez são futuros e
    // set–dez reprovam o lastro (saída bem abaixo do gasto típico).
    const REAL_2026 = [
      { m: 1, income: 965132, perf: -99751 },
      { m: 2, income: 1623670, perf: 492689 },
      { m: 3, income: 1042963, perf: -50308 },
      { m: 4, income: 1342641, perf: -135002 },
      { m: 5, income: 1274701, perf: 189619 },
      { m: 6, income: 1018860, perf: -124321 },
      { m: 7, income: 1211421, perf: -30506 },
      { m: 8, income: 1015607, perf: 168517 },
      { m: 9, income: 740808, perf: 357286 },
      { m: 10, income: 739857, perf: 386966 },
      { m: 11, income: 739857, perf: 392711 },
      { m: 12, income: 736867, perf: 392711 },
    ];
    const ANNUAL = {
      year: 2026,
      months: REAL_2026.map((r) => ({
        year: 2026,
        month: r.m,
        income_cents: r.income,
        income_performance_cents: r.income,
        performance_cents: r.perf,
        cost_of_living_cents: r.income - r.perf,
        fixed_out_cents: r.income - r.perf,
        daily_out_cents: 0,
        daily_avg_out_cents: 0,
        daily_projected_cents: 0,
        cartao_cents: 0,
        real_daily_avg_cents: 0,
        economia_cents: 0,
        patrimonio_cents: 0,
        savings_rate_bps: 0,
      })),
    };

    // Anos anteriores para "Sua renda ao longo dos anos": renda crescendo, economia zero —
    // a narrativa medida na planilha real (ganhar mais não vira economia sozinho).
    const yearMonths = (year: number, base: number) => ({
      year,
      months: Array.from({ length: 12 }, (_, i) => {
        const income = base + (i % 3) * 20000;
        return {
          year,
          month: i + 1,
          income_cents: income,
          income_performance_cents: income,
          performance_cents: Math.round(income * 0.05),
          cost_of_living_cents: Math.round(income * 0.95),
          fixed_out_cents: Math.round(income * 0.95),
          daily_out_cents: 0,
          daily_avg_out_cents: 0,
          daily_projected_cents: 0,
          cartao_cents: 0,
          real_daily_avg_cents: 0,
          economia_cents: 0,
          patrimonio_cents: 0,
          savings_rate_bps: 0,
        };
      }),
    });
    const ANNUAL_BY_YEAR: Record<number, unknown> = {
      2026: ANNUAL,
      2025: yearMonths(2025, 1010000),
      2024: yearMonths(2024, 850000),
    };
    const annualForYear = (args?: Record<string, unknown>) => {
      const year = Number(args?.["year"]);
      return ANNUAL_BY_YEAR[year] ?? { year, months: [] };
    };

    // A régua anual do método sobre 2026, como o motor a devolve com o relógio em 10/06: seis
    // meses vividos, gasto típico de R$ 11.121,26 (mediana das saídas de jan–jun) e set–dez sem
    // lastro. Anos anteriores chegam fechados; qualquer outro, sem registro.
    const TIPICO = 1112126;
    const rulerForYear = (args?: Record<string, unknown>) => {
      const year = Number(args?.["year"]);
      const known = year === 2026 || year === 2025 || year === 2024;
      const lived = (m: number) => year < 2026 || m <= 6;
      const suspect = (m: number) => known && year === 2026 && m >= 9;
      const rows =
        year === 2026
          ? REAL_2026
          : REAL_2026.map((r) => ({ ...r, income: 1010000, perf: 50500 }));
      const outflow = (r: { income: number; perf: number }) => r.income - r.perf;
      const livedRows = rows.filter((r) => lived(r.m));
      const incomeLived = livedRows.reduce((s, r) => s + r.income, 0);
      const incomeYear = rows.reduce((s, r) => s + r.income, 0);
      const futureMonths = 12 - livedRows.length;
      const shortfallYear = Math.round(incomeYear * 0.2);
      return {
        year,
        lived_months: known ? livedRows.length : 12,
        future_months: known ? futureMonths : 0,
        typical_spend_cents: known ? TIPICO : 0,
        income_lived_cents: known ? incomeLived : 0,
        economia_lived_cents: 0,
        surplus_lived_cents: known ? livedRows.reduce((s, r) => s + r.perf, 0) : 0,
        income_year_cents: known ? incomeYear : 0,
        economia_year_cents: 0,
        recorded_months: known ? livedRows.length : 0,
        avg_income_cents: known ? Math.trunc(incomeLived / livedRows.length) : 0,
        lived_bps: known ? 0 : null,
        projected_bps: known ? 0 : null,
        bps: known ? 0 : null,
        scope_lived: year === 2026,
        has_data: known,
        shortfall_lived_cents: known ? Math.round(incomeLived * 0.2) : 0,
        shortfall_year_cents: known ? shortfallYear : 0,
        per_month_shortfall_cents:
          known && futureMonths > 0 ? Math.round(shortfallYear / futureMonths) : null,
        verdict: known ? "below_band" : "no_record",
        band: { floor_bps: 2000, target_bps: 2500, ceiling_bps: 3000 },
        months: rows.map((r) => ({
          month: r.m,
          outflow_cents: known ? outflow(r) : 0,
          lived: lived(r.m),
          suspect: suspect(r.m),
          missing_cents: suspect(r.m) ? TIPICO - outflow(r) : 0,
        })),
        month_end: known
          ? FORECAST.month_end
              .filter((m) => m.year === 2026)
              .map((m) => ({ ...m, year }))
          : [],
        year_end: known
          ? {
              end_month: 12,
              end_balance_cents: 2997711,
              // 2.997.711 menos o silêncio de set–dez (R$ 30.207,89).
              end_balance_typical_cents: year === 2026 ? -23078 : null,
            }
          : {
              end_month: null,
              end_balance_cents: null,
              end_balance_typical_cents: null,
            },
      };
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
        name: "Categoria demo A",
        color: "var(--cat-sky)",
        emoji: null,
        is_special: false,
        total_cents: 10000,
      },
      {
        id: "d",
        name: "Categoria demo B",
        color: "var(--cat-coral)",
        emoji: null,
        is_special: false,
        total_cents: 35000,
      },
    ];

    // Tela Tags — estado A rico: 3 exceções com efeitos distintos por régua, 5 pessoas
    // cobrindo os 5 estados epistêmicos de terceiros, 4 rótulos de movimentação livre.
    // Números da planilha real (Gio/Trânsito/Reembolso) — mesmo dataset do desenho aprovado.
    const ZERO_EFFECTS = {
      performance_delta_cents: 0,
      cost_delta_cents: 0,
      savings_base_delta_cents: 0,
      savings_amount_delta_cents: 0,
      daily_avg_delta_cents: 0,
    };
    const ALL_ON = {
      performance: true,
      cost_of_living: true,
      savings: true,
      daily_avg: true,
    };
    const TAGS_SCREEN = {
      month: "2026-06",
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 1211288,
        third_party_avg_cents: 282300,
        third_party_people: 5,
        has_exceptions: true,
      },
      third_parties: [
        {
          person_id: "gio",
          name: "Gio",
          out_cents: 407764,
          back_cents: 497764,
          expected_cents: 0,
          state: "favor",
          open_since_days: null,
          series_done: null,
          series_total: null,
          settled_date: null,
        },
        {
          person_id: "edvaldo",
          name: "Edvaldo",
          out_cents: 5000,
          back_cents: 0,
          expected_cents: 5000,
          state: "open",
          open_since_days: 13,
          series_done: null,
          series_total: null,
          settled_date: null,
        },
        {
          person_id: "pai",
          name: "Pai",
          out_cents: 0,
          back_cents: 11700,
          expected_cents: 11700,
          state: "series",
          open_since_days: null,
          series_done: 2,
          series_total: 3,
          settled_date: null,
        },
        {
          person_id: "pablo",
          name: "Pablo",
          out_cents: 2200,
          back_cents: 2200,
          expected_cents: 0,
          state: "settled",
          open_since_days: null,
          series_done: null,
          series_total: null,
          settled_date: "2026-06-04",
        },
        {
          person_id: "bruna",
          name: "Bruna",
          out_cents: 0,
          back_cents: 0,
          expected_cents: 0,
          state: "none",
          open_since_days: null,
          series_done: null,
          series_total: null,
          settled_date: null,
        },
      ],
      tags: [
        {
          id: "gio",
          name: "Gio",
          color: "var(--cat-orchid)",
          emoji: null,
          is_special: false,
          counts_in: {
            performance: false,
            cost_of_living: false,
            savings: false,
            daily_avg: false,
          },
          month_total_cents: 407764,
          txn_count: 6,
          // Contribuição marginal (contando − excluído): a Gio entra mais do que sai,
          // então a contribuição à Performance é POSITIVA — fora, o resultado mostra a menos.
          effects: {
            ...ZERO_EFFECTS,
            performance_delta_cents: 90000,
            cost_delta_cents: 407764,
          },
        },
        {
          id: "transito",
          name: "Trânsito",
          color: "var(--cat-sky)",
          emoji: null,
          is_special: false,
          counts_in: {
            performance: false,
            cost_of_living: false,
            savings: false,
            daily_avg: false,
          },
          month_total_cents: 100651,
          txn_count: 2,
          effects: { ...ZERO_EFFECTS, cost_delta_cents: 100651 },
        },
        {
          id: "reembolso",
          name: "Reembolso",
          color: "var(--cat-teal)",
          emoji: null,
          is_special: false,
          counts_in: {
            performance: true,
            cost_of_living: true,
            savings: false,
            daily_avg: true,
          },
          month_total_cents: 16700,
          txn_count: 3,
          effects: { ...ZERO_EFFECTS, savings_base_delta_cents: 16700 },
        },
        {
          id: "moradia",
          name: "Moradia",
          color: "var(--cat-coral)",
          emoji: null,
          is_special: false,
          counts_in: ALL_ON,
          month_total_cents: 176656,
          txn_count: 2,
          effects: ZERO_EFFECTS,
        },
        {
          id: "educacao",
          name: "Educação",
          color: "var(--cat-violet)",
          emoji: null,
          is_special: false,
          counts_in: ALL_ON,
          month_total_cents: 54412,
          txn_count: 2,
          effects: ZERO_EFFECTS,
        },
        {
          id: "assinaturas",
          name: "Assinaturas",
          color: "var(--cat-amber)",
          emoji: null,
          is_special: false,
          counts_in: ALL_ON,
          month_total_cents: 9489,
          txn_count: 3,
          effects: ZERO_EFFECTS,
        },
        {
          id: "cancelar",
          name: "Cancelar",
          color: "var(--cat-jade)",
          emoji: null,
          is_special: false,
          counts_in: ALL_ON,
          month_total_cents: 8490,
          txn_count: 2,
          effects: ZERO_EFFECTS,
        },
      ],
      last_sync_at: null,
    };

    const responses: Record<string, unknown> = {
      check_auth_status: "disconnected",
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_daily_budget_cmd: {
        per_day_cents: 4300,
        divisor_days: 30,
        ceremony_month: "2025-09",
        // A nota reproduz exatamente os itens abaixo: a prova da tela do teto não pode
        // divergir do que a citação afirma.
        source_note:
          "Mensal  R$ 900,00  Alimentação\nMensal  R$ 390,00  Transporte\nTotal = R$ 1290,00\nR$ 1290,00 / 30 Dias = R$ 43,00",
        categories: [
          { id: "cat-1", name: "Alimentação", amount_cents: 90000, position: 0 },
          { id: "cat-2", name: "Transporte", amount_cents: 39000, position: 1 },
        ],
      },
      get_ceiling_proposal_cmd: null,
      // Dias 01–09 são corrente realizada (o Calendário costura realizado ×
      // projeção; sem eles os dias passados virariam travessão), 10+ espelham
      // a projeção — mesma fonte dos demais consumidores do grid.
      get_month_grid: [
        ...[
          [1, 700000, 0, 0, 910000],
          [2, 0, 120000, 4300, 785700],
          [3, 0, 0, 0, 785700],
          [4, 0, 0, 9400, 776300],
          [5, 0, 0, 0, 776300],
          [6, 0, 18900, 0, 757400],
          [7, 0, 0, 0, 757400],
          [8, 100000, 0, 0, 857400],
          [9, 0, 0, 11600, 845800],
        ].map(([day, inc, fixed, daily, bal]) => ({
          date: `2026-06-${String(day).padStart(2, "0")}`,
          day,
          income_cents: inc,
          fixed_out_cents: fixed,
          daily_out_cents: daily,
          balance_cents: bal,
        })),
        ...FORECAST.daily.map((d) => ({
          date: d.date,
          day: Number(d.date.slice(8, 10)),
          income_cents: d.income_cents,
          fixed_out_cents: d.fixed_out_cents,
          daily_out_cents: d.daily_out_cents,
          balance_cents: d.balance_cents,
        })),
      ],
      tag_totals_for_month_cmd: TAG_TOTALS,
      list_tags_cmd: TAG_TOTALS,
      get_tags_screen: TAGS_SCREEN,
      update_tag_rulers_cmd: null,
      get_annual_metrics: annualForYear,
      get_annual_ruler: rulerForYear,
      get_recent_transactions: TXNS,
      get_upcoming_bills_cmd: UPCOMING_BILLS,
      get_import_conflicts: [],
      create_transaction: "e2e-txn-id",
      get_app_info: APP_INFO,
      get_pockets: POCKETS,
      create_account: "e2e-account-id",
      list_cards: [],
      // Onboarding já concluído nestes cenários — o overlay não cobre o app.
      get_app_setting: "true",
      set_app_setting: null,
      ...ov,
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        // Alguns comandos dependem dos argumentos (ex.: `get_annual_metrics` por ano). O
        // handler pode ser um valor estático ou uma função dos args.
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          if (cmd in responses) {
            const r = responses[cmd];
            const handler = r as ((a?: Record<string, unknown>) => unknown) | undefined;
            return Promise.resolve(typeof r === "function" ? handler!(args) : r);
          }
          return Promise.reject(new Error(`e2e mock: unmocked command ${cmd}`));
        },
        transformCallback: () => 0,
      },
      configurable: true,
    });
  }, overrides);
}
