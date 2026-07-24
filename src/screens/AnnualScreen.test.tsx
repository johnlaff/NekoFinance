import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AnnualScreen } from "./AnnualScreen";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";
import type { AnnualMetrics, Forecast, MonthMetric, MonthEnd } from "../lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Planilha real de 2026 (em centavos) — a mesma que ancora o desenho aprovado. Com o relógio
// em junho, os vividos são jan–jun; jul–dez são futuros e set–dez reprovam o lastro.
interface Row {
  m: number;
  income: number;
  perf: number;
  eco: number;
  end: number;
}
const REAL: Row[] = [
  { m: 1, income: 965132, perf: -99751, eco: 0, end: 957349 },
  { m: 2, income: 1623670, perf: 492689, eco: 0, end: 1450038 },
  { m: 3, income: 1042963, perf: -50308, eco: 0, end: 1399730 },
  { m: 4, income: 1342641, perf: -135002, eco: 0, end: 1264728 },
  { m: 5, income: 1274701, perf: 189619, eco: 0, end: 1454347 },
  { m: 6, income: 1018860, perf: -124321, eco: 0, end: 1330026 },
  { m: 7, income: 1211421, perf: -30506, eco: 0, end: 1299520 },
  { m: 8, income: 1015607, perf: 168517, eco: 0, end: 1468037 },
  { m: 9, income: 740808, perf: 357286, eco: 0, end: 1825323 },
  { m: 10, income: 739857, perf: 386966, eco: 0, end: 2212289 },
  { m: 11, income: 739857, perf: 392711, eco: 0, end: 2605000 },
  { m: 12, income: 736867, perf: 392711, eco: 0, end: 2997711 },
];

function mkMonth(r: Row, year = 2026): MonthMetric {
  return {
    year,
    month: r.m,
    income_cents: r.income,
    income_performance_cents: r.income,
    performance_cents: r.perf,
    cost_of_living_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    daily_avg_out_cents: 0,
    daily_projected_cents: 0,
    cartao_cents: 0,
    real_daily_avg_cents: 0,
    economia_cents: r.eco,
    patrimonio_cents: 0,
    savings_rate_bps: r.income > 0 ? Math.round((r.eco / r.income) * 10000) : 0,
  };
}

const ANNUAL_2026: AnnualMetrics = { year: 2026, months: REAL.map((r) => mkMonth(r)) };

// Handler ciente do ano: 2026 e 2025 têm dados; qualquer outro ano vem vazio (no_record).
const annualByYear = (args?: Record<string, unknown>): AnnualMetrics => {
  const year = Number(args?.["year"]);
  if (year === 2026) return ANNUAL_2026;
  if (year === 2025) return { year: 2025, months: REAL.map((r) => mkMonth(r, 2025)) };
  return {
    year,
    months: Array.from({ length: 12 }, (_, i) =>
      mkMonth({ m: i + 1, income: 0, perf: 0, eco: 0, end: 0 }, year),
    ),
  };
};

// Forecast com month_end de todos os 12 meses — dezembro projetado para o cenário do ano.
const MONTH_END_2026: MonthEnd[] = REAL.map((r) => ({
  year: 2026,
  month: r.m,
  balance_cents: r.end,
}));
const testForecast: Forecast = {
  ...FORECAST,
  today: "2026-06-10",
  month_end: MONTH_END_2026,
};

const SUMMARY = (reserveMonths: number) => ({
  balance: 1330026,
  daily_budget: 20000,
  daily_ceiling_source: "chosen" as const,
  ceiling_proposal_pending: false,
  daily_spend_today: 0,
  card_spend_today_cents: 0,
  reserve_months: reserveMonths,
  reserve_state: "verdict" as const,
  reserve_basis_months: 6,
  reserve_trend: "flat",
  spending_mode: "debit" as const,
  card_gate: "unknown" as const,
  card_gate_economy: "unknown" as const,
  card_gate_economy_bps: null,
  card_gate_reserve: "unknown" as const,
  cartao_month_cents: 0,
  next_fatura_date: null,
  next_fatura_amount_cents: 0,
  upcoming_invoices: [],
  transaction_count: 40,
  last_real_tx_date: "2026-06-09",
});

const monthGrid = (args?: Record<string, unknown>) => {
  const month = Number(args?.["month"]);
  const found = REAL.find((r) => r.m === month);
  return [
    {
      date: `2026-${String(month).padStart(2, "0")}-28`,
      day: 28,
      income_cents: 0,
      fixed_out_cents: 0,
      daily_out_cents: 0,
      balance_cents: found ? found.end : null,
    },
  ];
};

function setup({ reserve = 4.5 }: { reserve?: number } = {}) {
  mockInvoke.mockReset();
  mockCommands({
    get_annual_metrics: annualByYear,
    get_forecast: testForecast,
    get_dashboard_summary: SUMMARY(reserve),
    get_month_grid: monthGrid,
  });
  render(<AnnualScreen />);
}

describe("AnnualScreen — direção Conversa com a Mia", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("abre pelo veredito na voz da marca (economia zero + reserva baixa = não guardou nada)", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Você não guardou nada em 2026.")).toBeInTheDocument(),
    );
    // Rótulo do veredito e selo de estimativa (há meses suspeitos à frente).
    expect(screen.getByText(/Economizado · 2026/)).toBeInTheDocument();
    expect(screen.getByText("Estimativa")).toBeInTheDocument();
  });

  it("a régua da faixa é o instrumento, com escala 0→40 e recorte declarado", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("A faixa do método")).toBeInTheDocument(),
    );
    // O recorte declara os meses vividos — a régua não afirma o ano medindo 6 meses.
    expect(screen.getByText(/em 6 de 12 meses já vividos/i)).toBeInTheDocument();
    // A régua expõe o texto equivalente (role img) com a escala e a zona-alvo.
    const ruler = screen.getByRole("img", {
      name: /faixa vai de 20% a 30%, numa escala de 0% a 40%/i,
    });
    expect(ruler).toBeInTheDocument();
  });

  it("onde dezembro termina: dois cenários com o alternativo quando há suspeitos", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Onde dezembro termina")).toBeInTheDocument(),
    );
    // Cenário lançado (dezembro projetado) e o cenário do gasto típico.
    expect(
      screen.getByText("Se o resto do ano custar o que está lançado"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Se os meses a conferir custarem o de sempre"),
    ).toBeInTheDocument();
  });

  it("os doze meses aparecem como linhas; meses futuros sem percentual", async () => {
    setup();
    await waitFor(() => expect(screen.getByText("Os doze meses")).toBeInTheDocument());
    // Abreviações capitalizadas dos meses.
    expect(screen.getAllByText("Jan").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Dez").length).toBeGreaterThan(0);
    // Selo "Conferir" nos meses suspeitos (set–dez).
    expect(screen.getAllByText("Conferir").length).toBeGreaterThan(0);
  });

  it("o ano em números é disclosure (lista), abre e mostra o detalhe do mês", async () => {
    setup();
    const fold = await screen.findByRole("button", { name: /O ano em números/i });
    expect(fold).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(fold);
    expect(fold).toHaveAttribute("aria-expanded", "true");
    // A fronteira previsto é dita uma vez.
    expect(screen.getByText("Daqui para frente é previsão")).toBeInTheDocument();
    // Total Vivido no rodapé.
    expect(screen.getByText("Vivido")).toBeInTheDocument();
  });

  it("sua renda ao longo dos anos aparece com percentual guardado", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Sua renda ao longo dos anos")).toBeInTheDocument(),
    );
    expect(screen.getAllByText(/guardado/).length).toBeGreaterThan(0);
  });

  it("zero por escolha: economia zero mas reserva ≥ 6 meses", async () => {
    setup({ reserve: 8 });
    await waitFor(() =>
      expect(
        screen.getByText("Você zerou a economia para não tocar na reserva."),
      ).toBeInTheDocument(),
    );
  });

  it("sem registro: ano vazio mostra o veredito e não monta os cards", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("A faixa do método")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Ano anterior" })); // 2025
    fireEvent.click(screen.getByRole("button", { name: "Ano anterior" })); // 2024 (vazio)
    await waitFor(() =>
      expect(screen.getByText("2024 não tem registro.")).toBeInTheDocument(),
    );
    expect(screen.queryByText("A faixa do método")).not.toBeInTheDocument();
  });

  it("navegação de ano: próximo desabilitado no ano corrente", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("A faixa do método")).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Próximo ano" })).toBeDisabled();
  });
});
