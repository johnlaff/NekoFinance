import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { TotaisScreen } from "./TotaisScreen";
import {
  currentMonthMetric,
  performanceStatus,
  economizadoStatus,
  custoVidaStatus,
  serieLeitura,
  SAVINGS_MIN_BPS,
} from "./totaisStatus";
import { crumbOverridesSnapshot } from "../shell/crumbStore";
import type { MonthMetric } from "../lib/api";
import {
  ANNUAL_METRICS,
  FORECAST,
  OWNER_TOTALS,
  SUMMARY,
  mockCommands,
  mockInvoke,
} from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function monthFixture(overrides: Partial<MonthMetric>): MonthMetric {
  return { ...FORECAST.months[0]!, ...overrides };
}

describe("TotaisScreen — regras do método (puro)", () => {
  it("Performance: >=0 Sobrou, <0 Faltou", () => {
    expect(performanceStatus(100).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(0).label).toBe("Sobrou dinheiro");
    expect(performanceStatus(-1).label).toBe("Faltou dinheiro");
    expect(performanceStatus(-1).level).toBe("risk");
  });

  it("Economizado: >=20% (2000bps) dentro do ideal", () => {
    expect(economizadoStatus(2500).label).toBe("Dentro do ideal");
    expect(economizadoStatus(2000).label).toBe("Dentro do ideal");
    expect(economizadoStatus(1900).label).toBe("Abaixo do ideal");
  });

  it("Economizado: zero tem nome próprio — Nada guardado", () => {
    expect(economizadoStatus(0).label).toBe("Nada guardado");
    expect(economizadoStatus(0).level).toBe("watch");
    // Guardou algo (mesmo 1 bps) → já não é "Nada guardado".
    expect(economizadoStatus(1).label).toBe("Abaixo do ideal");
  });

  it("Economizado: acima de 30% é Acima do ideal", () => {
    expect(economizadoStatus(3001).label).toBe("Acima do ideal");
    expect(economizadoStatus(3000).label).toBe("Dentro do ideal");
  });

  it("SAVINGS_MIN_BPS é a constante canônica de 20% (compartilhada entre as telas), travada contra o piso publicado pelo DTO", () => {
    expect(SAVINGS_MIN_BPS).toBe(FORECAST.savings_band.floor_bps);
    expect(economizadoStatus(SAVINGS_MIN_BPS).label).toBe("Dentro do ideal");
    expect(economizadoStatus(SAVINGS_MIN_BPS - 1).label).toBe("Abaixo do ideal");
  });

  it("Custo de vida: custo<=renda dentro da renda", () => {
    expect(custoVidaStatus(500, 1000).label).toBe("Dentro da renda");
    expect(custoVidaStatus(1200, 1000).label).toBe("Acima da renda");
  });

  it("currentMonthMetric acha o mês do `today`", () => {
    const months = FORECAST.months.filter((month) => month.month === 6);
    expect(currentMonthMetric(months, "2026-06-13")?.month).toBe(6);
    expect(currentMonthMetric(months, "2026-07-01")).toBeNull();
  });
});

describe("serieLeitura — a leitura diz o fato, nunca julga mês isolado", () => {
  it("sem meses anteriores não fabrica comparação", () => {
    expect(serieLeitura([monthFixture({ month: 6 })])).toBe(
      "Sem meses anteriores para comparar ainda.",
    );
  });

  it("todos zero: é o mesmo zero em todos, não uma queda", () => {
    const trend = [4, 5, 6].map((month) =>
      monthFixture({ month, savings_rate_bps: 0 }),
    );
    expect(serieLeitura(trend)).toBe(
      "O economizado está em zero nos últimos 3 meses — é o mesmo zero em todos, não uma queda.",
    );
  });

  it("caso geral: faixa da janela + melhor mês (percentuais truncados)", () => {
    const trend = [
      monthFixture({ month: 5, savings_rate_bps: 1590 }),
      monthFixture({ month: 6, savings_rate_bps: 2540 }),
    ];
    expect(serieLeitura(trend)).toBe(
      "Entre Mai e Jun, o economizado foi de 15% a 25% — o melhor mês foi Junho.",
    );
  });
});

describe("TotaisScreen (render)", () => {
  it("bento mostra os cards canônicos do mês e os status do método", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Performance")).toBeInTheDocument();
    });
    expect(screen.getByText("Economia guardada")).toBeInTheDocument();
    expect(screen.getAllByText("Custo de vida").length).toBeGreaterThan(0);
    expect(screen.getByText("Diário médio")).toBeInTheDocument();
    // Status do método (performance positiva e economizado 25% no mock).
    expect(screen.getByText("Sobrou dinheiro")).toBeInTheDocument();
    expect(screen.getByText("Dentro do ideal")).toBeInTheDocument();
  });

  it("régua de economia: pino do mês na escala 0→40 e leitura anual", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    expect(
      await screen.findByRole("img", {
        name: "Régua de economia de 0% a 40% com zona-alvo de 20% a 30%; Junho em 25%",
      }),
    ).toBeInTheDocument();
    // A nota fecha com a régua anual (economia_ruler_rate_bps: 500 → 5%).
    expect(
      screen.getByText("No ano: 5% — a régua julga a média anual, não o mês."),
    ).toBeInTheDocument();
  });

  it("crumb da appbar mostra o mês visto", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    const { unmount } = render(<TotaisScreen />);

    await waitFor(() => {
      expect(crumbOverridesSnapshot().mes).toBe("Junho de 2026");
    });
    unmount();
    expect(crumbOverridesSnapshot().mes).toBeUndefined();
  });

  it("seletor de mês: avança para o próximo mês e volta ao hoje", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);
    await waitFor(() => expect(screen.getByText("Performance")).toBeInTheDocument());
    // Começa no mês corrente (junho) → sem botão "Hoje".
    expect(screen.queryByRole("button", { name: "Hoje" })).not.toBeInTheDocument();
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Próximo mês" }));
    expect(screen.getByText(/Julho de 2026/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hoje" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Hoje" }));
    expect(screen.getByText(/Junho de 2026/)).toBeInTheDocument();
  });

  it("mostra totais por titular quando há 2+ titulares", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: OWNER_TOTALS,
    });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByRole("region", { name: "Por titular" })).toBeInTheDocument();
    });
    expect(screen.getByText("Titular A")).toBeInTheDocument();
    expect(screen.getByText("Titular B")).toBeInTheDocument();
    expect(screen.getByText(/3\.200,00/)).toBeInTheDocument();
    expect(screen.getByText(/1\.800,00/)).toBeInTheDocument();
  });

  it("não mostra a seção por titular quando não há split (lista vazia)", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Performance")).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("region", { name: "Por titular" }),
    ).not.toBeInTheDocument();
  });

  it("série do economizado inclui meses realizados vindos do annual metrics", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    // Cada barra é botão-atalho para o mês dela (Jan 15% e Mai 20% do annual;
    // Jun 25% do forecast).
    expect(
      await screen.findByRole("button", { name: "Maio: 20% — ver o mês" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Junho: 25% — ver o mês" }),
    ).toBeInTheDocument();
    // A leitura diz o fato da janela.
    expect(
      screen.getByText(
        "Entre Jan e Jun, o economizado foi de 15% a 25% — o melhor mês foi Junho.",
      ),
    ).toBeInTheDocument();

    // Clicar numa barra navega para o mês dela.
    await userEvent.click(
      screen.getByRole("button", { name: "Maio: 20% — ver o mês" }),
    );
    expect(screen.getByText(/Maio de 2026/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hoje" })).toBeInTheDocument();
  });

  it("custo de vida decompõe por componente com o bucket Cartão", async () => {
    mockInvoke.mockReset();
    const forecast = {
      ...FORECAST,
      months: FORECAST.months.map((month) =>
        month.month === 6
          ? {
              ...month,
              fixed_out_cents: 100_000,
              daily_out_cents: 50_000,
              cartao_cents: 150_000,
              cost_of_living_cents: 300_000,
            }
          : month,
      ),
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    const custo = await screen.findByRole("region", { name: "Custo de vida" });
    expect(within(custo).getByText("Cartão")).toBeInTheDocument();
    expect(within(custo).getByText(/1\.500,00/)).toBeInTheDocument();
    expect(within(custo).getByText("Saídas fixas")).toBeInTheDocument();
    expect(within(custo).getByText(/1\.000,00/)).toBeInTheDocument();
    // O segbar carrega o texto equivalente completo da composição.
    expect(
      within(custo).getByRole("img", { name: /Composição do custo de vida/ }),
    ).toBeInTheDocument();
  });

  it("modo cartão: Diário zerado no mês corrente explica onde o variável vive", async () => {
    mockInvoke.mockReset();
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: { ...SUMMARY, spending_mode: "card" },
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    expect(
      await screen.findByText("Não lançado — o variável vive no cartão"),
    ).toBeInTheDocument();
  });

  it("Nada guardado: mês com economia zero recebe o estado próprio", async () => {
    mockInvoke.mockReset();
    const forecast = {
      ...FORECAST,
      months: FORECAST.months.map((month) =>
        month.month === 6
          ? { ...month, economia_cents: 0, savings_rate_bps: 0 }
          : month,
      ),
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    expect(await screen.findByText("Nada guardado")).toBeInTheDocument();
  });

  it("sem registro de economia: a régua não julga e a série não compara", async () => {
    mockInvoke.mockReset();
    const forecast = {
      ...FORECAST,
      annual_savings: {
        ...FORECAST.annual_savings,
        economia_state: "no_record" as const,
      },
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => {
      expect(screen.getByText("Sem registro")).toBeInTheDocument();
    });
    expect(
      screen.getByText(
        "Sem registro de economia na planilha — a régua espera o primeiro aporte.",
      ),
    ).toBeInTheDocument();
    // Sem registro não há o que comparar: a faixa histórica não renderiza.
    expect(
      screen.queryByText("Comparado aos meses anteriores"),
    ).not.toBeInTheDocument();
    // E o estado "Nada guardado" (que julga) não aparece.
    expect(screen.queryByText("Nada guardado")).not.toBeInTheDocument();
  });

  it("Performance: subtexto inclui a Economia como termo e a conta fecha (economia > 0)", async () => {
    mockInvoke.mockReset();
    // Junho: 7.000 − 2.500 − 1.000 = 3.500 (Performance do motor, que desconta a economia).
    const forecast = {
      ...FORECAST,
      months: FORECAST.months.map((month) =>
        month.month === 6
          ? {
              ...month,
              income_cents: 700_000,
              cost_of_living_cents: 250_000,
              economia_cents: 100_000,
              performance_cents: 350_000,
            }
          : month,
      ),
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => expect(screen.getByText("Performance")).toBeInTheDocument());

    // Os termos exibidos fecham com o valor da Performance mostrado no card.
    // Os valores renderizam dentro de <Money> (a11y), então casamos pelo textContent
    // completo do parágrafo em vez do texto direto de um único nó.
    expect(
      screen.getByText(
        (_content, el) =>
          el?.classList.contains("mes__equation") === true &&
          (el.textContent ?? "").replace(/\s+/g, " ") ===
            "Entradas R$ 7.000,00 − Custo de vida R$ 2.500,00 − Economia R$ 1.000,00",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("+R$ 3.500,00").length).toBeGreaterThan(0);
  });

  it("Performance: Patrimônio também é termo explícito e a conta fecha (patrimonio > 0)", async () => {
    mockInvoke.mockReset();
    // Junho 7.000 − 2.500 − 1.000 − 500 = 3.000; sem o termo
    // Patrimônio o subtexto implicava 3.500 ao lado de uma Performance de 3.000.
    const forecast = {
      ...FORECAST,
      months: FORECAST.months.map((month) =>
        month.month === 6
          ? {
              ...month,
              income_cents: 700_000,
              cost_of_living_cents: 250_000,
              economia_cents: 100_000,
              patrimonio_cents: 50_000,
              performance_cents: 300_000,
            }
          : month,
      ),
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => expect(screen.getByText("Performance")).toBeInTheDocument());

    expect(
      screen.getByText(
        (_content, el) =>
          el?.classList.contains("mes__equation") === true &&
          (el.textContent ?? "").replace(/\s+/g, " ") ===
            "Entradas R$ 7.000,00 − Custo de vida R$ 2.500,00 − Economia R$ 1.000,00 − Patrimônio R$ 500,00",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("+R$ 3.000,00").length).toBeGreaterThan(0);
  });

  it("Performance: previsão de diário restante é termo explícito e a conta fecha", async () => {
    mockInvoke.mockReset();
    // Junho em andamento: 7.000 − 2.500 − 1.200 de previsão restante = 3.300 (motor).
    const forecast = {
      ...FORECAST,
      months: FORECAST.months.map((month) =>
        month.month === 6
          ? {
              ...month,
              income_cents: 700_000,
              cost_of_living_cents: 250_000,
              economia_cents: 0,
              patrimonio_cents: 0,
              daily_projected_cents: 120_000,
              performance_cents: 330_000,
            }
          : month,
      ),
    };
    mockCommands({
      get_forecast: forecast,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    await waitFor(() => expect(screen.getByText("Performance")).toBeInTheDocument());

    expect(
      screen.getByText(
        (_content, el) =>
          el?.classList.contains("mes__equation") === true &&
          (el.textContent ?? "").replace(/\s+/g, " ") ===
            "Entradas R$ 7.000,00 − Custo de vida R$ 2.500,00 − Previsão de diário R$ 1.200,00",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("+R$ 3.300,00").length).toBeGreaterThan(0);
  });

  it("Diário médio zerado recebe o estado próprio", async () => {
    mockInvoke.mockReset();
    // FORECAST tem real_daily_avg_cents: 0 em junho.
    mockCommands({
      get_forecast: FORECAST,
      get_dashboard_summary: SUMMARY,
      get_annual_metrics: ANNUAL_METRICS,
      owner_totals_for_month_cmd: [],
    });
    render(<TotaisScreen />);

    const card = await screen.findByRole("region", { name: "Diário médio" });
    expect(within(card).getByText("Zerado")).toBeInTheDocument();
  });
});
