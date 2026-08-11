import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AnnualScreen } from "./AnnualScreen";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";
import type {
  AnnualMetrics,
  AnnualRuler,
  BandVerdict,
  Forecast,
  MonthMetric,
  MonthEnd,
} from "./anoView";

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

// Handler ciente do ano: 2026 e 2025 têm dados; qualquer outro ano vem vazio (no_record).
// `junBps` repõe a economia de junho — o último mês vivido, que é o mês que a Mia observa.
const annualByYear =
  (junBps = 0) =>
  (args?: Record<string, unknown>): AnnualMetrics => {
    const year = Number(args?.["year"]);
    const rows = REAL.map((r) =>
      r.m === 6 ? { ...r, eco: Math.ceil((r.income * junBps) / 10000) } : r,
    );
    if (year === 2026) return { year: 2026, months: rows.map((r) => mkMonth(r)) };
    if (year === 2025) return { year: 2025, months: rows.map((r) => mkMonth(r, 2025)) };
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

// A régua que o motor devolve sobre essas linhas com o relógio em 10/06/2026: seis meses
// vividos, gasto típico de R$ 11.121,26 (mediana das saídas de jan–jun) e set–dez sem lastro
// (jul e ago passam o piso de 60%). Os números são fato do motor — provados em
// `forecast::annual_ruler`; aqui são ENTRADA da tela.
const TIPICO_2026 = 1112126;
const SUSPEITOS_2026 = [9, 10, 11, 12];

interface RulerOpts {
  verdict?: BandVerdict;
  /** Economia dos meses vividos, em centavos (0 = o ano que não guardou nada). */
  economia?: number;
  /** Economizado% que o motor publica, em pontos-base. */
  bps?: number;
}

function mkRuler(year: number, opts: RulerOpts = {}): AnnualRuler {
  const { verdict = "below_band", economia = 0, bps = 0 } = opts;
  const lived = (m: number) => (year < 2026 ? true : m <= 6);
  const suspect = (m: number) => year === 2026 && SUSPEITOS_2026.includes(m);
  const outflow = (r: Row) => r.income - r.perf;
  const livedRows = REAL.filter((r) => lived(r.m));
  const sum = (rows: Row[], pick: (r: Row) => number) =>
    rows.reduce((acc, r) => acc + pick(r), 0);
  const incomeLived = sum(livedRows, (r) => r.income);
  const incomeYear = sum(REAL, (r) => r.income);
  const futureMonths = 12 - livedRows.length;
  const shortfallYear = Math.round(incomeYear * 0.2);
  return {
    year,
    lived_months: livedRows.length,
    future_months: futureMonths,
    typical_spend_cents: year === 2026 ? TIPICO_2026 : 1130981,
    income_lived_cents: incomeLived,
    economia_lived_cents: economia,
    surplus_lived_cents: sum(livedRows, (r) => r.perf),
    income_year_cents: incomeYear,
    economia_year_cents: economia,
    recorded_months: livedRows.length,
    avg_income_cents: Math.trunc(incomeLived / livedRows.length),
    lived_bps: bps,
    projected_bps: bps,
    bps,
    scope_lived: year === 2026,
    has_data: true,
    shortfall_lived_cents: Math.round(incomeLived * 0.2),
    shortfall_year_cents: shortfallYear,
    per_month_shortfall_cents:
      futureMonths > 0 ? Math.round(shortfallYear / futureMonths) : null,
    verdict,
    band: { floor_bps: 2000, ceiling_bps: 3000 },
    months: REAL.map((r) => ({
      month: r.m,
      outflow_cents: outflow(r),
      lived: lived(r.m),
      suspect: suspect(r.m),
      missing_cents: suspect(r.m) ? TIPICO_2026 - outflow(r) : 0,
    })),
    month_end: REAL.map((r) => ({ year, month: r.m, balance_cents: r.end })),
    year_end: {
      end_month: 12,
      end_balance_cents: 2997711,
      end_balance_typical_cents: suspect(12) ? -23078 : null,
    },
  };
}

/** Ano sem um único lançamento: a régua não tem o que julgar. */
function emptyRuler(year: number): AnnualRuler {
  const base = mkRuler(year);
  return {
    ...base,
    typical_spend_cents: 0,
    income_lived_cents: 0,
    surplus_lived_cents: 0,
    income_year_cents: 0,
    recorded_months: 0,
    avg_income_cents: 0,
    lived_bps: null,
    projected_bps: null,
    bps: null,
    scope_lived: false,
    has_data: false,
    shortfall_lived_cents: 0,
    shortfall_year_cents: 0,
    per_month_shortfall_cents: null,
    verdict: "no_record",
    months: base.months.map((m) => ({
      ...m,
      outflow_cents: 0,
      suspect: false,
      missing_cents: 0,
    })),
    month_end: [],
    year_end: {
      end_month: null,
      end_balance_cents: null,
      end_balance_typical_cents: null,
    },
  };
}

// O veredito da faixa é decidido no motor, que lê a reserva no backend: a tela recebe o
// resultado e o narra. `reserve` aqui escolhe qual leitura a régua devolve.
function setup({
  reserve = 4.5,
  economia = 0,
  bps = 0,
  junBps = 0,
  verdict,
}: {
  reserve?: number;
  economia?: number;
  bps?: number;
  junBps?: number;
  verdict?: BandVerdict;
} = {}) {
  mockInvoke.mockReset();
  const rulerByYear = (args?: Record<string, unknown>): AnnualRuler => {
    const year = Number(args?.["year"]);
    if (year === 2026 || year === 2025) {
      return mkRuler(year, {
        verdict: verdict ?? (reserve >= 6 ? "zero_by_choice" : "below_band"),
        economia,
        bps,
      });
    }
    return emptyRuler(year);
  };
  mockCommands({
    get_annual_metrics: annualByYear(junBps),
    get_annual_ruler: rulerByYear,
    get_forecast: testForecast,
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

  it("abaixo da faixa: a manchete conta o realizado e o selo devolve as duas alavancas", async () => {
    setup({ economia: 900000, bps: 1200 });
    await waitFor(() =>
      expect(screen.getByText("Você guardou 12% até aqui.")).toBeInTheDocument(),
    );
    expect(
      screen.getByText("O que aproxima o ano dos 20 — soltar menos ou entrar mais?", {
        selector: "p",
      }),
    ).toBeInTheDocument();
    // A didática do método recolheu para o popover da régua.
    expect(screen.queryByText(/o convite é cortar custo ou aumentar renda/)).toBeNull();
    // Os operandos moram no cabeçalho da régua — o herói não os reimprime (regra 41).
    expect(screen.queryByText(/que entraram/)).toBeNull();
  });

  it("dentro da faixa: manchete e selo ficam, sem os operandos que a régua já imprime", async () => {
    setup({ verdict: "in_band", economia: 2500000, bps: 2400 });
    await waitFor(() =>
      expect(screen.getByText("Você guardou 24% do que ganhou.")).toBeInTheDocument(),
    );
    expect(
      screen.getByText("Dentro da faixa do método — dá para seguir a vida.", {
        selector: "p",
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/que entraram/)).toBeNull();
  });

  it("sem economia: o selo guarda o operando e a régua do método sai do herói", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Você não guardou nada em 2026.")).toBeInTheDocument(),
    );
    const hero = screen.getByText("Você não guardou nada em 2026.").closest("div")!;
    expect(hero).toHaveTextContent("Sobraram R$ 2.729,26 nos 6 meses que você viveu.");
    expect(screen.queryByText(/O método pede de 20% a 30%/)).toBeNull();
  });

  it("zero por escolha: a troca certa deixa o herói e passa a viver no popover da faixa", async () => {
    setup({ reserve: 8 });
    await waitFor(() =>
      expect(
        screen.getByText("Você zerou a economia para não tocar na reserva."),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByText(
        "Foram 6 meses sem guardar nada, e a reserva seguiu protegida.",
        {
          selector: "p",
        },
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Na ordem do método, é a troca certa/)).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Como funciona? — A faixa do método" }),
    );
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      /na ordem do método, a troca certa/i,
    );
  });

  it("card da Mia: observação que muda com o mês, sem a metade didática", async () => {
    setup({ junBps: 1200, economia: 900000, bps: 1200 });
    const mia = await screen.findByLabelText("A linha da Mia");
    expect(mia).toHaveTextContent("Junho guardou pouco — a média do ano segue em 12%.");
    // A metade didática duplicava o popover da faixa (regra 41).
    expect(mia).not.toHaveTextContent(/A régua julga a média do ano/);
    expect(mia).not.toHaveTextContent(/O método não conta dinheiro parado/);
    // O convite à conversa é a ação do card e nunca se esconde (regra 3).
    expect(
      screen.getByRole("button", { name: /Perguntar à Mia sobre 2026/ }),
    ).toBeInTheDocument();
  });

  it("fim do ano: os operandos ficam e a leitura dos dois cenários vai para o popover", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Onde dezembro termina")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/A diferença entre os dois/)).toBeNull();
    expect(screen.queryByText(/Pode ser mês barato de verdade/)).toBeNull();
    expect(
      screen.getByText(/que costumam sair por mês/, { selector: "p" }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Como funciona? — Onde dezembro termina" }),
    );
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      /Enquanto não confirmar, o ano não tem veredito/,
    );
  });

  it("o ano em números: a instrução morre e a chave de leitura vai para o popover", async () => {
    setup();
    const fold = await screen.findByRole("button", { name: /O ano em números/i });
    fireEvent.click(fold);
    expect(screen.queryByText(/Toque num mês/)).toBeNull();
    expect(screen.queryByText(/inclusive dinheiro de terceiros/)).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Como funciona? — O ano em números" }),
    );
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      /inclusive dinheiro de terceiros/,
    );
  });

  it("renda ao longo dos anos: a cauda didática recolhe para o popover do card", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("Sua renda ao longo dos anos")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Ganhar mais não vira economia sozinho/)).toBeNull();
    expect(
      screen.getByText(/Suas entradas médias/, { selector: "p" }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Como funciona? — Sua renda ao longo dos anos",
      }),
    );
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      /Ganhar mais não vira economia sozinho/,
    );
  });

  it("navegação de ano: próximo desabilitado no ano corrente", async () => {
    setup();
    await waitFor(() =>
      expect(screen.getByText("A faixa do método")).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Próximo ano" })).toBeDisabled();
  });
});
