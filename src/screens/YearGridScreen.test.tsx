import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import { NekoAppProvider } from "../shell/appContext";
import type { TransactionRow } from "../lib/api";
import { FORECAST, MONTH_GRID, TXNS, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderCal() {
  return render(
    <NekoAppProvider value={app}>
      <YearGridScreen />
    </NekoAppProvider>,
  );
}

/** Grid realizado só para junho — maio (véspera do dia 1) e demais meses vazios. */
const gridByMonth = (args?: Record<string, unknown>) =>
  Number(args?.["month"]) === 6 ? MONTH_GRID : [];

/** Lançamentos de junho: um no dia 15 (projetado) e um no dia 10. */
const JUNE_TXNS: TransactionRow[] = [
  { ...TXNS[0]!, id: "j1", date: "2026-06-15", description: "Conta de luz" },
  { ...TXNS[0]!, id: "j2", date: "2026-06-10", description: "Café do dia" },
];

function mockAll() {
  mockCommands({
    get_forecast: FORECAST,
    get_month_grid: gridByMonth,
    get_recent_transactions: JUNE_TXNS,
  });
}

describe("YearGridScreen (Calendário)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-06-10T12:00:00-03:00"));
    mockInvoke.mockReset();
    app.navigate.mockReset();
    app.openCompose.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("abre a grade do mês com legenda de eventos e sem a aba anual", async () => {
    mockAll();
    renderCal();
    expect(
      await screen.findByRole("grid", { name: /junho de 2026/i }),
    ).toBeInTheDocument();
    expect(screen.getByText("Menor saldo do mês")).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "Ano inteiro" })).toBeNull();
  });

  it("costura as correntes: passado do grid, hoje em diante da projeção", async () => {
    mockAll();
    renderCal();
    // 01/06 realizado (R$ 9.100,00) e 25/06 previsto com entrada.
    const day1 = await screen.findByRole("gridcell", { name: /^1 de junho/ });
    expect(day1).toHaveAccessibleName(/Saldo R\$\u00a09\.100,00/);
    const day25 = screen.getByRole("gridcell", { name: /25 de junho/ });
    expect(day25).toHaveAccessibleName(/Entrada/);
    expect(day25).toHaveAccessibleName(/Previsto/);
  });

  it("elege o menor saldo do mês na corrente costurada", async () => {
    mockAll();
    renderCal();
    const day15 = await screen.findByRole("gridcell", { name: /15 de junho/ });
    expect(day15).toHaveAccessibleName(/Menor saldo do mês/);
    expect(day15).toHaveAccessibleName(/Saldo R\$\u00a05\.877,00/);
  });

  it("o movimento da célula é o delta contra a véspera", async () => {
    mockAll();
    renderCal();
    const day2 = await screen.findByRole("gridcell", { name: /^2 de junho/ });
    // 785700 − 910000 = −124300 → "Movimento −R$ 1.243,00" no rótulo.
    expect(day2).toHaveAccessibleName(/Movimento −R\$\u00a01\.243,00/);
  });

  it("hoje nasce selecionado e a agenda mostra o dia", async () => {
    mockAll();
    renderCal();
    const today = await screen.findByRole("gridcell", { name: /10 de junho/ });
    expect(today).toHaveAttribute("aria-selected", "true");
    const agenda = screen.getByRole("complementary");
    expect(
      within(agenda).getByRole("heading", { name: "Quarta-feira, 10 de junho" }),
    ).toBeInTheDocument();
    expect(within(agenda).getByText("Café do dia")).toBeInTheDocument();
    expect(within(agenda).getByText("Saldo que o dia deixou")).toBeInTheDocument();
  });

  it("tocar um dia move a agenda para ele", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    const day15 = await screen.findByRole("gridcell", { name: /15 de junho/ });
    await user.click(day15);
    expect(day15).toHaveAttribute("aria-selected", "true");
    const agenda = screen.getByRole("complementary");
    expect(
      within(agenda).getByRole("heading", { name: "Segunda-feira, 15 de junho" }),
    ).toBeInTheDocument();
    expect(within(agenda).getByText("Conta de luz")).toBeInTheDocument();
    // Dia futuro declara-se previsto uma vez na agenda — nunca pílula por linha.
    expect(
      within(agenda).getByText("Previsto — ainda não aconteceu"),
    ).toBeInTheDocument();
    expect(within(agenda).getByText("Saídas fixas")).toBeInTheDocument();
  });

  it("dia sem lançamentos e sem componentes tem o vazio honesto", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    // 13/06 (projeção): nenhum componente e nenhum lançamento.
    const day13 = await screen.findByRole("gridcell", { name: /13 de junho/ });
    await user.click(day13);
    const agenda = screen.getByRole("complementary");
    expect(
      within(agenda).getByText("Sem movimento — o saldo ficou como estava."),
    ).toBeInTheDocument();
  });

  it("dia com componentes mas sem itens nunca alega 'sem movimento'", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    // 02/06 tem saídas realizadas (componentes), mas nenhum lançamento listado.
    const day2 = await screen.findByRole("gridcell", { name: /^2 de junho/ });
    await user.click(day2);
    const agenda = screen.getByRole("complementary");
    expect(
      within(agenda).queryByText("Sem movimento — o saldo ficou como estava."),
    ).toBeNull();
    expect(
      within(agenda).getByText("Sem itens detalhados — o dia fecha no resumo."),
    ).toBeInTheDocument();
    expect(within(agenda).getByText("Saídas fixas")).toBeInTheDocument();
  });

  it("dia fora das correntes mostra o travessão epistêmico na agenda", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    // 20/06 não existe em nenhuma corrente (grid nem forecast).
    const day20 = await screen.findByRole("gridcell", { name: /20 de junho/ });
    await user.click(day20);
    const agenda = screen.getByRole("complementary");
    expect(within(agenda).getByText("Sem corrente")).toBeInTheDocument();
  });

  it("Ver no Livro-razão navega para Lançamentos", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    await screen.findByRole("grid", { name: /junho/i });
    await user.click(screen.getByRole("button", { name: /Ver no Livro-razão/ }));
    expect(app.navigate).toHaveBeenCalledWith("lancamentos");
  });

  it("roving tabindex: uma célula tabável; setas movem o foco dentro do mês", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    const grid = await screen.findByRole("grid", { name: /junho/i });
    const tabbable = within(grid)
      .getAllByRole("gridcell")
      .filter((c) => c.getAttribute("tabindex") === "0");
    expect(tabbable).toHaveLength(1);
    expect(tabbable[0]).toHaveAccessibleName(/10 de junho/);

    tabbable[0]!.focus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("gridcell", { name: /11 de junho/ })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("gridcell", { name: /18 de junho/ })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(screen.getByRole("gridcell", { name: /15 de junho/ })).toHaveFocus();
  });

  it("PageDown troca o mês e o roving segue o dia focado, não o dia 1", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    const grid = await screen.findByRole("grid", { name: /junho/i });
    const today = within(grid).getByRole("gridcell", { name: /10 de junho/ });
    today.focus();
    await user.keyboard("{PageDown}");
    const july10 = await screen.findByRole("gridcell", { name: /10 de julho/ });
    expect(july10).toHaveFocus();
    expect(july10).toHaveAttribute("tabindex", "0");
    // A próxima seta parte do dia focado — nunca salta de volta ao dia 1.
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("gridcell", { name: /11 de julho/ })).toHaveFocus();
  });

  it("navegar o mês atualiza a grade e zera a seleção para o dia 1", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockAll();
    renderCal();
    await screen.findByRole("grid", { name: /junho/i });
    await user.click(screen.getByRole("button", { name: "Próximo mês" }));
    expect(
      await screen.findByRole("grid", { name: /julho de 2026/i }),
    ).toBeInTheDocument();
    const day1 = screen.getByRole("gridcell", { name: /^1 de julho/ });
    expect(day1).toHaveAttribute("aria-selected", "true");
  });
});
