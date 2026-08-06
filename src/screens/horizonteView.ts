import {
  getForecast,
  getMonthTransactions,
  getScenarioForecast,
  lastSyncAt,
  type Forecast,
  type MonthEnd,
  type ScenarioCompareDto,
  type TransactionRow,
} from "../lib/api";
import { MES, MES_ABBR } from "../lib/nkFormat";
import { saldoBand as classifyBand, type SaldoBand } from "../lib/saldoHeatmap";

// View-model puro da tela Horizonte — o radar do caixa. A tela olha só para a frente
// (previsto · meses · até o fim dos dados) e responde "tem buraco na estrada?". Aqui mora a
// COMPOSIÇÃO: qual voz o veredito assume, a geometria da estrada, os estados epistêmicos da
// grade e o agrupamento dos compromissos. Nenhuma regra de método nasce aqui — a régua de
// lastro, o gasto típico, a fronteira de confiança e o custo de fechar cada mês incompleto
// vêm todos do motor (`Forecast`); o frontend só os declara e desenha. É também a porta inteira
// do shim para a tela (ADR-0007): tipos reexportados e fetchers estáveis do `useCommand` — a
// tela nunca importa `lib/api`.

// Tipos do shim reexportados pela view — a tela e seu teste leem daqui.
export type { Forecast, MonthEnd, ScenarioCompareDto, TransactionRow };

// Piso de confiança da régua de lastro, em basis points — mesma régua de O ano, computada no
// motor. Um mês futuro tem lastro quando a saída lançada cobre ≥ 60% do gasto típico; abaixo
// disso o mês é "Conferir" (previsto sem lastro), e a estrada troca sua saída pelo típico.
const LASTRO_FLOOR_BPS = 6000;

// ------------------------------------------------------------------- tipos --

export type VerdictVoice = "loading" | "livre" | "aperto" | "vazio";

/** Um ponto da estrada: uma data e o saldo projetado naquele dia. */
export interface RoadPoint {
  dateISO: string;
  cents: number;
}

export interface RoadModel {
  /** Saldo diário lançado, de hoje ao fim do horizonte. */
  points: RoadPoint[];
  /** Índice do primeiro ponto SEM lastro (mês posterior à fronteira); -1 = tudo com lastro. */
  fogFromIndex: number;
  /** Índice do menor ponto do lançado. */
  minIndex: number;
  /** Traçado "se custar o de sempre": fim de cada mês futuro com o gasto típico nos meses
   *  sem lastro (coincide com o lançado enquanto há lastro). */
  typicalPath: RoadPoint[];
  /** Domínio do eixo Y (centavos), nice-scaled e sempre incluindo o zero. */
  yMin: number;
  yMax: number;
  /** Valores das linhas de grade do eixo Y (centavos), de `yMin` a `yMax`. */
  yTicks: number[];
  /** Rótulos de mês (1º dia de cada mês na série) com o índice do ponto. */
  monthTicks: { index: number; label: string }[];
  /** Saldo no fim do horizonte: lançado × típico (típico null quando não difere). */
  endLaunchedCents: number;
  endTypicalCents: number | null;
}

export type GridState = "vivido" | "prev" | "conf" | "sem";

export interface GridMonth {
  year: number;
  month: number; // 1..12
  label: string; // "Jul"
  state: GridState;
  /** Saldo no fim do mês (null quando sem registro). */
  endBalanceCents: number | null;
  /** Faixa do termômetro do saldo de fim de mês (null quando sem registro). */
  band: SaldoBand | null;
  daysInMonth: number;
  /** Dia da semana do 1º dia (0 = domingo). */
  firstDow: number;
  /** Dia corrente (só no mês vivido; null nos demais). */
  todayDay: number | null;
  /** Chave "YYYY-MM" para abrir no Calendário; null quando não navegável (sem registro). */
  navMonth: string | null;
}

export interface CommitmentItem {
  key: string;
  dayLabel: string;
  title: string;
  subtitle: string;
  /** Rótulo de parcela `n/N` (mono); null quando não é série. */
  installment: string | null;
  signedCents: number; // + para entrada, − para saída/transfer
  isIn: boolean;
}

export interface CommitmentMonth {
  monthKey: string; // "YYYY-MM"
  label: string; // "Agosto"
  inCents: number;
  outCents: number;
  days: number;
  items: CommitmentItem[];
}

export interface HorizonteInput {
  forecast: Forecast | undefined;
  /** Lançamentos projetados por mês futuro ("YYYY-MM" → linhas). Vazio até carregar. */
  rowsByMonth: Record<string, TransactionRow[]>;
  /** Rótulo de frescura da planilha para a proveniência (ex.: "22h14"); null quando ausente. */
  syncLabel: string | null;
}

export interface HorizonteView {
  voice: VerdictVoice;
  today: string;
  horizonEnd: string;
  trustedThroughMonth: string | null;
  /** Mês da fronteira de confiança, por extenso e minúsculo ("agosto"); null sem baseline. */
  trustedMonthLabel: string | null;
  minPoint: RoadPoint | null;
  endLaunchedCents: number | null;
  endTypicalCents: number | null;
  /** O traçado típico raspa o zero (fim < 0) — o gêmeo honesto que a manchete conta. */
  typicalHitsZero: boolean;
  deficit: RoadPoint | null;
  /** Mês do buraco, por extenso e minúsculo — a voz do aperto. */
  deficitMonthLabel: string | null;
  baselineOutflowCents: number;
  road: RoadModel | null;
  grid: GridMonth[];
  commitments: CommitmentMonth[];
  commitmentsTotal: { inCents: number; outCents: number; days: number } | null;
  syncLabel: string | null;
}

// --------------------------------------------------------------- helpers ----

function yearOf(iso: string): number {
  return parseInt(iso.slice(0, 4), 10);
}
function monthOf(iso: string): number {
  return parseInt(iso.slice(5, 7), 10);
}
function monthKey(year: number, month: number): string {
  return `${year}-${String(month).padStart(2, "0")}`;
}
/** Mês por extenso e minúsculo a partir de "YYYY-MM" (nome no meio de frase é minúsculo). */
export function monthLongLower(ym: string | null): string | null {
  if (!ym) return null;
  const m = monthOf(ym + "-01");
  const name = MES[m - 1];
  return name ? name.toLowerCase() : null;
}
function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}
function firstDow(year: number, month: number): number {
  return new Date(Date.UTC(year, month - 1, 1)).getUTCDay();
}

/** Passo "bonito" (1·2·2.5·5·10 × potência de 10) para um alvo de passo do eixo. */
function niceStep(target: number): number {
  if (target <= 0) return 1;
  const pow = Math.pow(10, Math.floor(Math.log10(target)));
  const f = target / pow;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
  return nice * pow;
}

/** Domínio nice do eixo Y (centavos) incluindo sempre o zero, com ~4 divisões. */
function niceAxis(
  minV: number,
  maxV: number,
): { yMin: number; yMax: number; step: number } {
  const lo = Math.min(0, minV);
  const hi = Math.max(0, maxV);
  const span = hi - lo || 1;
  const step = niceStep(span / 4);
  return {
    yMin: Math.floor(lo / step) * step,
    yMax: Math.ceil(hi / step) * step,
    step,
  };
}

// --------------------------------------------------------------- estrada ----

/**
 * Custo de fechar cada mês futuro incompleto (o `estimated_missing_cents` do motor), por mês,
 * SÓ para os meses sem lastro (`coverage_bps < 60%`). É o quanto o traçado "se custar o de
 * sempre" desce em relação ao lançado, mês a mês.
 */
function missingByMonth(forecast: Forecast): Map<string, number> {
  const map = new Map<string, number>();
  for (const c of forecast.coverage) {
    if (c.coverage_bps < LASTRO_FLOOR_BPS && c.estimated_missing_cents > 0) {
      map.set(monthKey(c.year, c.month), c.estimated_missing_cents);
    }
  }
  return map;
}

function buildRoad(forecast: Forecast): RoadModel | null {
  const daily = forecast.daily;
  if (daily.length < 2) return null;

  const points: RoadPoint[] = daily.map((d) => ({
    dateISO: d.date,
    cents: d.balance_cents,
  }));
  const vals = points.map((p) => p.cents);
  const minVal = Math.min(...vals);
  const minIndex = vals.indexOf(minVal);
  const maxVal = Math.max(...vals);

  // Fronteira do lastro: o primeiro ponto cujo mês é posterior a `trusted_through_month`.
  const trusted = forecast.trusted_through_month;
  let fogFromIndex = -1;
  if (trusted) {
    fogFromIndex = points.findIndex((p) => p.dateISO.slice(0, 7) > trusted);
  }

  // Traçado típico: fim de cada mês futuro = saldo lançado − Σ custo dos meses sem lastro até
  // ele. Ancora no lançado enquanto há lastro e diverge a partir do primeiro mês sem lastro.
  const missing = missingByMonth(forecast);
  const futureEnds = forecast.month_end
    .filter((e) => monthKey(e.year, e.month) >= forecast.today.slice(0, 7))
    .sort((a, b) => monthKey(a.year, a.month).localeCompare(monthKey(b.year, b.month)));
  let cumMissing = 0;
  const typicalPath: RoadPoint[] = futureEnds.map((e) => {
    cumMissing += missing.get(monthKey(e.year, e.month)) ?? 0;
    return {
      dateISO: `${monthKey(e.year, e.month)}-${String(daysInMonth(e.year, e.month)).padStart(2, "0")}`,
      cents: e.balance_cents - cumMissing,
    };
  });

  // O fim do horizonte lançado é o saldo de dezembro (`month_end`), não o último ponto diário:
  // é o que a régua "Se custar o que está lançado" declara, e coincide com o fim da série real.
  const endLaunchedCents =
    futureEnds.length > 0
      ? futureEnds[futureEnds.length - 1]!.balance_cents
      : vals[vals.length - 1]!;
  const endTypicalCents =
    typicalPath.length > 0 ? typicalPath[typicalPath.length - 1]!.cents : null;
  const typicalDiffers =
    endTypicalCents !== null && endTypicalCents !== endLaunchedCents;

  // O eixo precisa conter as DUAS linhas por inteiro — o típico pode furar mais fundo que o
  // lançado (piso) e o lançado subir mais alto que o típico (teto).
  const typicalMin = typicalPath.reduce((m, p) => Math.min(m, p.cents), Infinity);
  const typicalMax = typicalPath.reduce((m, p) => Math.max(m, p.cents), -Infinity);
  const axis = niceAxis(
    Math.min(minVal, Number.isFinite(typicalMin) ? typicalMin : minVal),
    Math.max(maxVal, Number.isFinite(typicalMax) ? typicalMax : maxVal),
  );
  const yTicks: number[] = [];
  for (let v = axis.yMin; v <= axis.yMax + 1; v += axis.step)
    yTicks.push(Math.round(v));

  // Rótulos de mês: o primeiro ponto de cada mês na série.
  const monthTicks: { index: number; label: string }[] = [];
  let seen = "";
  points.forEach((p, i) => {
    const ym = p.dateISO.slice(0, 7);
    if (ym !== seen) {
      seen = ym;
      monthTicks.push({ index: i, label: MES_ABBR[monthOf(p.dateISO) - 1]! });
    }
  });

  return {
    points,
    fogFromIndex,
    minIndex,
    typicalPath,
    yMin: axis.yMin,
    yMax: axis.yMax,
    yTicks,
    monthTicks,
    endLaunchedCents,
    endTypicalCents: typicalDiffers ? endTypicalCents : null,
  };
}

// ------------------------------------------------------------------ grade ----

function buildGrid(forecast: Forecast): GridMonth[] {
  const today = forecast.today;
  const curYear = yearOf(today);
  const curMonth = monthOf(today);
  const curDay = parseInt(today.slice(8, 10), 10);
  const trusted = forecast.trusted_through_month;

  const endByKey = new Map<string, number>();
  for (const e of forecast.month_end)
    endByKey.set(monthKey(e.year, e.month), e.balance_cents);

  const out: GridMonth[] = [];
  for (let i = 0; i < 12; i++) {
    const abs = curMonth - 1 + i; // 0-based mês absoluto a partir de janeiro do ano corrente
    const year = curYear + Math.floor(abs / 12);
    const month = (abs % 12) + 1;
    const key = monthKey(year, month);
    const endBalanceCents = endByKey.has(key) ? endByKey.get(key)! : null;
    const isCurrent = year === curYear && month === curMonth;

    let state: GridState;
    if (endBalanceCents === null) {
      state = "sem";
    } else if (isCurrent) {
      state = "vivido";
    } else if (trusted && key <= trusted) {
      state = "prev";
    } else {
      state = "conf";
    }

    out.push({
      year,
      month,
      label: MES_ABBR[month - 1]!,
      state,
      endBalanceCents,
      band: endBalanceCents === null ? null : classifyBand(endBalanceCents),
      daysInMonth: daysInMonth(year, month),
      firstDow: firstDow(year, month),
      todayDay: isCurrent ? curDay : null,
      navMonth: state === "sem" ? null : key,
    });
  }
  return out;
}

// ----------------------------------------------------------- compromissos ----

/** O subtítulo derivado dos campos reais da linha — nunca um agrupamento fabricado. */
function subtitleFor(row: TransactionRow): string {
  if (row.has_refund_link && row.type === "income") return "Entrada vinculada";
  if (row.installment_index != null && row.installment_total != null) return "Parcela";
  if (row.is_fixed) return row.type === "income" ? "Entrada fixa" : "Conta fixa";
  if (row.type === "income") return "Entrada";
  if (row.type === "transfer") return "Economia";
  return "Diário";
}

function commitmentItem(row: TransactionRow): CommitmentItem {
  const day = parseInt(row.date.slice(8, 10), 10);
  const isIn = row.type === "income";
  const mag = Math.abs(row.amount);
  const installment =
    row.installment_index != null && row.installment_total != null
      ? `${row.installment_index}/${row.installment_total}`
      : null;
  return {
    key: row.id,
    dayLabel: day === 1 ? "1º" : String(day),
    title: row.description || "—",
    subtitle: subtitleFor(row),
    installment,
    signedCents: isIn ? mag : -mag,
    isIn,
  };
}

function buildCommitments(rowsByMonth: Record<string, TransactionRow[]>): {
  months: CommitmentMonth[];
  total: { inCents: number; outCents: number; days: number } | null;
} {
  const keys = Object.keys(rowsByMonth).sort();
  const months: CommitmentMonth[] = [];
  for (const key of keys) {
    const rows = rowsByMonth[key]!.toSorted((a, b) => a.date.localeCompare(b.date));
    if (rows.length === 0) continue;
    const items = rows.map(commitmentItem);
    const { inCents, outCents } = items.reduce(
      (acc, i) =>
        i.isIn
          ? { inCents: acc.inCents + i.signedCents, outCents: acc.outCents }
          : { inCents: acc.inCents, outCents: acc.outCents + Math.abs(i.signedCents) },
      { inCents: 0, outCents: 0 },
    );
    const days = new Set(rows.map((r) => r.date)).size;
    months.push({
      monthKey: key,
      label: MES[monthOf(key + "-01") - 1]!,
      inCents,
      outCents,
      days,
      items,
    });
  }
  if (months.length === 0) return { months, total: null };
  const total = months.reduce(
    (acc, m) => ({
      inCents: acc.inCents + m.inCents,
      outCents: acc.outCents + m.outCents,
      days: acc.days + m.days,
    }),
    { inCents: 0, outCents: 0, days: 0 },
  );
  return { months, total };
}

// ------------------------------------------------------------- view builder --

export function buildHorizonteView(input: HorizonteInput): HorizonteView {
  const { forecast, rowsByMonth, syncLabel } = input;

  if (!forecast) {
    return {
      voice: "loading",
      today: "",
      horizonEnd: "",
      trustedThroughMonth: null,
      trustedMonthLabel: null,
      minPoint: null,
      endLaunchedCents: null,
      endTypicalCents: null,
      typicalHitsZero: false,
      deficit: null,
      deficitMonthLabel: null,
      baselineOutflowCents: 0,
      road: null,
      grid: [],
      commitments: [],
      commitmentsTotal: null,
      syncLabel,
    };
  }

  const road = buildRoad(forecast);
  const grid = buildGrid(forecast);
  const { months: commitments, total: commitmentsTotal } =
    buildCommitments(rowsByMonth);

  const minPoint = road && road.minIndex >= 0 ? road.points[road.minIndex]! : null;
  const endLaunchedCents = road ? road.endLaunchedCents : null;
  const endTypicalCents = road ? road.endTypicalCents : null;
  const typicalHitsZero = endTypicalCents !== null && endTypicalCents < 0;

  const deficitRaw = forecast.deepest_deficit;
  const deficit =
    deficitRaw && deficitRaw.balance_cents < 0
      ? { dateISO: deficitRaw.date, cents: deficitRaw.balance_cents }
      : null;

  // Sem estrada (nada projetado à frente) ou sem gasto típico → o radar não tem veredito.
  const hasFuture = road !== null && forecast.baseline_outflow_cents > 0;
  let voice: VerdictVoice;
  if (!hasFuture) voice = "vazio";
  else if (deficit) voice = "aperto";
  else voice = "livre";

  return {
    voice,
    today: forecast.today,
    horizonEnd: forecast.horizon_end,
    trustedThroughMonth: forecast.trusted_through_month,
    trustedMonthLabel: monthLongLower(forecast.trusted_through_month),
    minPoint,
    endLaunchedCents,
    endTypicalCents,
    typicalHitsZero,
    deficit,
    deficitMonthLabel: deficit ? monthLongLower(deficit.dateISO.slice(0, 7)) : null,
    baselineOutflowCents: forecast.baseline_outflow_cents,
    road,
    grid,
    commitments,
    commitmentsTotal,
    syncLabel,
  };
}

// ---------------------------------------------------------------------------
// Leitura — fetchers com identidade estável por chave (o contrato do useCommand rejeita
// closures novas a cada render) e a convenção da chave de cache do `useCommand`.
// ---------------------------------------------------------------------------

export function fetchForecast(): Promise<Forecast> {
  return getForecast();
}

export function fetchLastSyncAt(): Promise<string | null> {
  return lastSyncAt();
}

/** Chave de cache do `useCommand` para os compromissos dos meses futuros. */
export function commitmentsCacheKey(monthsKey: string): string {
  return `horizon_commitments:${monthsKey}`;
}

const _commitFetchers = new Map<string, () => Promise<TransactionRow[][]>>();
export function commitmentsFetcher(
  monthsKey: string,
): () => Promise<TransactionRow[][]> {
  let fn = _commitFetchers.get(monthsKey);
  if (!fn) {
    const months = monthsKey ? monthsKey.split(",") : [];
    fn = () => Promise.all(months.map((m) => getMonthTransactions(m)));
    _commitFetchers.set(monthsKey, fn);
  }
  return fn;
}

/** Chave de cache do `useCommand` para o comparativo do cenário simulado ativo. */
export function scenarioCompareCacheKey(scenarioId: string | null): string {
  return scenarioId ? `scenario_forecast:${scenarioId}` : "scenario_forecast:none";
}

export function fetchScenarioCompare(
  scenarioId: string | null,
): Promise<ScenarioCompareDto> {
  return scenarioId
    ? getScenarioForecast(scenarioId)
    : Promise.reject(new Error("nenhum cenário selecionado"));
}
