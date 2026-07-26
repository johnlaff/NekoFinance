import "./horizonte.css";
import { useState } from "react";
import { TrendingUp, LayoutGrid, CalendarCheck, Lightbulb } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Money } from "../design-system/components/Money";
import {
  getForecast,
  getMonthTransactions,
  getScenarioForecast,
  lastSyncAt,
  isTauri,
  type TransactionRow,
} from "../lib/api";
import { fmtDate, fmtDayMonth, formatBRL, fmtAxisBRL } from "../lib/format";
import { saldoBand } from "../lib/nkFormat";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { syncRecencyLabel } from "../lib/syncRecency";
import type { Screen } from "../shell/screens";
import {
  buildHorizonteView,
  type CommitmentMonth,
  type GridMonth,
  type HorizonteView,
  type RoadModel,
} from "./horizonteView";
import { SimulateScenarioButton, ScenarioSheet, ScenarioCompare } from "./scenarios";

// O Horizonte é o radar do caixa: a única tela que olha só para a frente (previsto · meses ·
// até o fim dos dados) e responde "tem buraco na estrada?". A composição — voz do veredito,
// geometria da estrada, estados epistêmicos da grade, agrupamento dos compromissos — vive no
// view-model puro `horizonteView`; aqui é só a superfície e o wiring dos dados reais.

const TYPICAL_TERM = {
  title: "Gasto típico",
  body: "É a mediana das saídas dos seus meses vividos completos. Um mês futuro só sustenta o veredito quando a saída lançada cobre ao menos 60% dele; abaixo disso o mês fica em compasso de conferir, e a estrada mostra também onde ele iria se custasse o de sempre.",
};

const SEMAPHORE_TERM = {
  title: "O semáforo do saldo",
  body: "A cor de cada mês é a faixa do seu saldo no fim dele — as faixas fixas da sua planilha (folga, ok, apertado, negativo, crítico). Um mês sem lastro não ganha cor de aprovação: fica tracejado, em compasso de conferir. A verdade dia a dia mora no Calendário, que cada mês abre.",
};

// ---- fetch multi-mês dos compromissos (fetcher estável por chave, no padrão do YearGrid) ----

const _commitFetchers = new Map<string, () => Promise<TransactionRow[][]>>();
function commitmentsFetcher(monthsKey: string) {
  let fn = _commitFetchers.get(monthsKey);
  if (!fn) {
    const months = monthsKey ? monthsKey.split(",") : [];
    fn = () => Promise.all(months.map((m) => getMonthTransactions(m)));
    _commitFetchers.set(monthsKey, fn);
  }
  return fn;
}

export function HorizonteScreen({
  onNavigate = () => undefined,
}: {
  onNavigate?: (s: Screen) => void;
}) {
  const forecastQ = useCommand("get_forecast", getForecast);
  const syncQ = useCommand("last_sync_at", lastSyncAt);
  const forecast = forecastQ.data;

  // Meses futuros com saldo de fim de mês → os compromissos que cada um já traz lançados.
  const cur = forecast ? forecast.today.slice(0, 7) : "";
  const futureMonths = forecast
    ? forecast.month_end
        .reduce<string[]>((acc, e) => {
          const k = `${e.year}-${String(e.month).padStart(2, "0")}`;
          if (k > cur) acc.push(k);
          return acc;
        }, [])
        .sort()
    : [];
  const monthsKey = futureMonths.join(",");
  const commitmentsQ = useCommand(
    `horizon_commitments:${monthsKey}`,
    commitmentsFetcher(monthsKey),
  );

  const [sheetOpen, setSheetOpen] = useState(false);
  const [activeScenarioId, setActiveScenarioId] = useState<string | null>(null);
  const compareQ = useCommand(
    activeScenarioId
      ? `scenario_forecast:${activeScenarioId}`
      : "scenario_forecast:none",
    () =>
      activeScenarioId
        ? getScenarioForecast(activeScenarioId)
        : Promise.reject(new Error("nenhum cenário selecionado")),
  );
  const compare = activeScenarioId ? (compareQ.data ?? null) : null;

  const rowsByMonth: Record<string, TransactionRow[]> = {};
  const commitmentRows = commitmentsQ.data ?? [];
  futureMonths.forEach((m, i) => {
    rowsByMonth[m] = commitmentRows[i] ?? [];
  });

  const view = buildHorizonteView({
    forecast,
    rowsByMonth,
    syncLabel: syncRecencyLabel(syncQ.data),
  });

  if (forecastQ.error) {
    return (
      <div className="hz neko-app">
        <EmptyState
          variant="error"
          title="Não foi possível carregar o horizonte"
          description="Confira a conexão e tente de novo."
          action={
            <Button size="sm" variant="ghost" onClick={() => invalidateCommands()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  if (view.voice === "loading") {
    return (
      <div className="hz neko-app">
        <EmptyState variant="skeleton" skeletonRows={6} />
      </div>
    );
  }

  return (
    // O side-sheet de cenários é NÃO-MODAL: `hz--sheet-open` refluí o conteúdo para ele seguir
    // operável ao lado (regra de reflow em scenarios.css).
    <div className={"hz neko-app" + (sheetOpen ? " hz--sheet-open" : "")}>
      <ScenarioSheet
        open={sheetOpen}
        onClose={() => setSheetOpen(false)}
        activeScenarioId={activeScenarioId}
        onSelectScenario={setActiveScenarioId}
      />

      <div className="hz__verdict">
        <Verdict
          view={view}
          onSimulate={() => setSheetOpen(true)}
          onNavigate={onNavigate}
        />
      </div>

      {compare ? (
        <div className="hz__scnrow">
          <ScenarioCompare
            compare={compare}
            onClose={() => setActiveScenarioId(null)}
          />
        </div>
      ) : null}

      {view.road ? (
        <section className="hz__card hz__roadcard" aria-labelledby="hz-estrada">
          <div className="hz__cardhead">
            <TrendingUp
              size={16}
              strokeWidth={1.75}
              className="ic"
              aria-hidden="true"
            />
            <h2 id="hz-estrada">A estrada até dezembro</h2>
            <span className="hz__note">Só o lançado</span>
          </div>
          <Road road={view.road} />
          <div className="hz__roadlegend">
            <span className="hz__lg hz__lg--lanc">
              <i aria-hidden="true" />
              Lançado
            </span>
            <span className="hz__lg hz__lg--fog">
              <i aria-hidden="true" />
              Lançado, sem lastro
            </span>
            <span className="hz__lg hz__lg--tip">
              <i aria-hidden="true" />
              Se custar o de sempre
            </span>
          </div>
          <RoadEnds view={view} />
          <p className="hz__secnote">
            Um mês à frente só sustenta o veredito quando a saída lançada cobre ao menos{" "}
            <b>60%</b> do gasto típico —{" "}
            <b>
              <Money cents={view.baselineOutflowCents} size="inherit" />
            </b>
            , a mediana dos seus meses vividos.{" "}
            <span className="hz__cf">
              A régua do ano (Economizado%) mora em O ano — aqui o juiz é o caixa.
            </span>
          </p>
          <NumbersFold view={view} />
        </section>
      ) : null}

      <div className="hz__col">
        {view.road ? (
          <section className="hz__card" aria-labelledby="hz-meses">
            <div className="hz__cardhead">
              <LayoutGrid
                size={16}
                strokeWidth={1.75}
                className="ic"
                aria-hidden="true"
              />
              <h2 id="hz-meses">Os próximos 12 meses</h2>
              <span className="hz__note">Semáforo do saldo</span>
            </div>
            <div className="hz__mgrid">
              {view.grid.map((m) => (
                <MonthCard
                  key={`${m.year}-${m.month}`}
                  month={m}
                  onOpen={() => onNavigate("calendario")}
                />
              ))}
            </div>
            <div className="hz__gridlegend">
              <span className="hz__gl hz__gl--viv">
                <i aria-hidden="true" />
                Vivido
              </span>
              <span className="hz__gl hz__gl--prev">
                <i aria-hidden="true" />
                Previsto com lastro
              </span>
              <span className="hz__gl hz__gl--conf">
                <i aria-hidden="true" />
                Sem lastro · Conferir
              </span>
              <span className="hz__gl hz__gl--sem">
                <i aria-hidden="true" />
                Sem registro
              </span>
            </div>
            <p className="hz__secnote">
              A cor de cada mês é o{" "}
              <InfoPopover term={SEMAPHORE_TERM}>semáforo</InfoPopover> do saldo — as
              faixas fixas da sua planilha. Mês sem lastro não ganha cor de aprovação:
              fica em compasso de conferir.{" "}
              <span className="hz__cf">Cada mês abre no Calendário.</span>
            </p>
          </section>
        ) : null}
      </div>

      <div className="hz__col">
        {view.commitments.length > 0 ? (
          <section className="hz__card" aria-labelledby="hz-marcado">
            <div className="hz__cardhead">
              <CalendarCheck
                size={16}
                strokeWidth={1.75}
                className="ic"
                aria-hidden="true"
              />
              <h2 id="hz-marcado">O que já está marcado</h2>
              {view.commitments.length > 0 ? (
                <span className="hz__note">
                  {view.commitments[0]!.label} → {view.commitments.at(-1)!.label}
                </span>
              ) : null}
            </div>
            {view.commitmentsTotal ? (
              <div className="hz__comsum">
                <span>
                  Entra{" "}
                  <b>
                    <Money cents={view.commitmentsTotal.inCents} size="inherit" />
                  </b>
                </span>
                <span>
                  Sai{" "}
                  <b>
                    <Money cents={-view.commitmentsTotal.outCents} size="inherit" />
                  </b>
                </span>
                <span>
                  <b>{view.commitmentsTotal.days}</b> dias com lançamento
                </span>
              </div>
            ) : null}
            {view.commitments.map((m, i) => (
              <CommitmentFold key={m.monthKey} month={m} defaultOpen={i === 0} />
            ))}
          </section>
        ) : null}

        <section className="hz__card hz__ese" aria-labelledby="hz-ese">
          <div className="hz__cardhead">
            <Lightbulb size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
            <h2 id="hz-ese">E se?</h2>
            <span className="hz__note">Cenários</span>
          </div>
          <p className="hz__esebody">
            Teste uma compra, um financiamento ou uma troca de plano <b>antes</b> de
            assumir: o cenário entra como camada na estrada e o método responde com as
            duas réguas — a reserva continua com <b>6 meses ou mais</b>? A economia de{" "}
            <b>20–30%</b> segue viva?
          </p>
          <div className="hz__eseacts">
            <SimulateScenarioButton onClick={() => setSheetOpen(true)} />
            <Button variant="ghost" size="sm" onClick={() => setSheetOpen(true)}>
              Cenários salvos
            </Button>
          </div>
        </section>
      </div>

      {!isTauri ? (
        <p className="hz__webhint">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      ) : null}
    </div>
  );
}

// ----------------------------------------------------------------- veredito --

function Verdict({
  view,
  onSimulate,
  onNavigate,
}: {
  view: HorizonteView;
  onSimulate: () => void;
  onNavigate: (s: Screen) => void;
}) {
  const label = `Horizonte · Hoje → ${fmtDayMonth(view.horizonEnd)}`;

  if (view.voice === "vazio") {
    return (
      <>
        <p className="hz__vlabel">Horizonte</p>
        <h1 data-large-title>O radar só enxerga o que está lançado.</h1>
        <p className="hz__vbody">
          Pré-lance o que você já sabe que vem — contas fixas, parcelas, faturas,
          salário — e a estrada aparece sozinha.{" "}
          <span className="hz__cf">
            Começa pelo próximo mês; o resto o app propõe das recorrências.
          </span>
        </p>
        <div className="hz__vactions">
          <Button variant="primary" onClick={() => onNavigate("lancamentos")}>
            Pré-lançar o futuro
          </Button>
        </div>
      </>
    );
  }

  if (view.voice === "aperto" && view.deficit) {
    return (
      <>
        <p className="hz__vlabel">{label}</p>
        <h1 data-large-title>O caminho aperta em {view.deficitMonthLabel}.</h1>
        <p className="hz__vbody">
          Do jeito que está lançado, o saldo passa por{" "}
          <b className="hz__neg">
            <Money cents={view.deficit.cents} size="inherit" />
          </b>{" "}
          em {fmtDayMonth(view.deficit.dateISO)}. É um{" "}
          <InfoPopover term="buraco_do_futuro">buraco</InfoPopover> na estrada — dá para
          atravessar: <b>antecipar</b> uma entrada, <b>adiar</b> uma saída que caiba, ou
          cruzar com a <b>reserva</b>, por partes, repondo depois.
        </p>
        <div className="hz__vactions">
          <Button variant="primary" size="sm" onClick={onSimulate}>
            Simular uma saída
          </Button>
          <Button variant="ghost" size="sm" onClick={() => onNavigate("calendario")}>
            Abrir {view.deficitMonthLabel}
          </Button>
        </div>
        <Provenance view={view} />
      </>
    );
  }

  // Livre
  return (
    <>
      <p className="hz__vlabel">{label}</p>
      <h1 data-large-title>
        Caminho livre até o fim de {view.trustedMonthLabel ?? "dezembro"}.
      </h1>
      <p className="hz__vbody">
        {view.minPoint ? (
          <>
            O menor saldo à vista é{" "}
            <b>
              <Money cents={view.minPoint.cents} size="inherit" />
            </b>
            , em {fmtDayMonth(view.minPoint.dateISO)} — folga.{" "}
          </>
        ) : null}
        {view.endTypicalCents !== null ? (
          <>
            Se os meses sem lastro custarem o{" "}
            <InfoPopover term={TYPICAL_TERM}>gasto típico</InfoPopover>,{" "}
            {view.typicalHitsZero ? (
              <>
                dezembro raspa o zero —{" "}
                <b className="hz__neg">
                  <Money cents={view.endTypicalCents} size="inherit" />
                </b>
                <EstimateMark
                  term={{
                    title: "Estimativa",
                    body: "O traçado 'se custar o de sempre' troca a saída dos meses sem lastro pelo gasto típico. É uma projeção, não o lançado — por isso vem marcada.",
                  }}
                />
              </>
            ) : (
              <>
                dezembro fecha em{" "}
                <b>
                  <Money cents={view.endTypicalCents} size="inherit" />
                </b>
              </>
            )}
            . <span className="hz__cf">Falta lançar — ou decidir onde vai sobrar.</span>
          </>
        ) : (
          <span className="hz__cf">
            Todos os meses à frente têm lastro — a estrada é o que está lançado.
          </span>
        )}
      </p>
      <Provenance view={view} />
    </>
  );
}

function Provenance({ view }: { view: HorizonteView }) {
  return (
    <span className="hz__prov">
      <span className="hz__provdot" aria-hidden="true" />
      Lançado até {fmtDate(view.horizonEnd)}
      {view.syncLabel ? ` · Planilha lida ${view.syncLabel}` : ""}
    </span>
  );
}

// ------------------------------------------------------------------ estrada --

const ROAD_W = 560;
const ROAD_H = 232;
const ROAD_L = 46;
const ROAD_R = 10;
const ROAD_T = 14;
const ROAD_B = 26;

function Road({ road }: { road: RoadModel }) {
  const t0 = Date.parse(road.points[0]!.dateISO);
  const t1 = Date.parse(road.points.at(-1)!.dateISO);
  const span = t1 - t0 || 1;
  const xOf = (iso: string) =>
    ROAD_L + ((ROAD_W - ROAD_L - ROAD_R) * (Date.parse(iso) - t0)) / span;
  const yOf = (cents: number) =>
    ROAD_T +
    (ROAD_H - ROAD_T - ROAD_B) *
      (1 - (cents - road.yMin) / (road.yMax - road.yMin || 1));

  const path = (pts: { dateISO: string; cents: number }[]) =>
    pts
      .map(
        (p, i) =>
          `${i ? "L" : "M"}${xOf(p.dateISO).toFixed(1)},${yOf(p.cents).toFixed(1)}`,
      )
      .join("");

  const fog = road.fogFromIndex >= 0 ? road.points.slice(road.fogFromIndex) : [];
  const lanc =
    road.fogFromIndex >= 0 ? road.points.slice(0, road.fogFromIndex + 1) : road.points;
  const minPt = road.points[road.minIndex]!;
  const endTyp = road.endTypicalCents;
  const zeroY = yOf(0);

  return (
    <div className="hz__road">
      <svg viewBox={`0 0 ${ROAD_W} ${ROAD_H}`} role="img" aria-label={roadAria(road)}>
        <g aria-hidden="true">
          {road.fogFromIndex >= 0 ? (
            <rect
              className="hz-road-fogzone"
              x={xOf(road.points[road.fogFromIndex]!.dateISO)}
              y={ROAD_T}
              width={ROAD_W - ROAD_R - xOf(road.points[road.fogFromIndex]!.dateISO)}
              height={ROAD_H - ROAD_T - ROAD_B}
            />
          ) : null}
          {road.yTicks.map((v) => (
            <g key={v}>
              <line
                className="hz-road-ax"
                x1={ROAD_L}
                y1={yOf(v)}
                x2={ROAD_W - ROAD_R}
                y2={yOf(v)}
              />
              <text
                className="hz-road-ylab"
                x={ROAD_L - 6}
                y={yOf(v) + 3}
                textAnchor="end"
              >
                {v === 0 ? "0" : fmtAxisBRL(v)}
              </text>
            </g>
          ))}
          <line
            className="hz-road-zero"
            x1={ROAD_L}
            y1={zeroY}
            x2={ROAD_W - ROAD_R}
            y2={zeroY}
          />
          {road.monthTicks.map((t) => (
            <text
              key={t.index}
              className="hz-road-mlab"
              x={xOf(road.points[t.index]!.dateISO)}
              y={ROAD_H - 8}
            >
              {t.label}
            </text>
          ))}
          <path className="hz-road-tip" d={path(road.typicalPath)} />
          {fog.length > 1 ? <path className="hz-road-fog" d={path(fog)} /> : null}
          <path className="hz-road-lanc" pathLength={1} d={path(lanc)} />
          <circle
            className="hz-road-mark"
            cx={xOf(minPt.dateISO)}
            cy={yOf(minPt.cents)}
            r={3.4}
          />
          {endTyp !== null && endTyp < 0 ? (
            <circle
              className="hz-road-markz"
              cx={xOf(road.typicalPath.at(-1)!.dateISO)}
              cy={yOf(endTyp)}
              r={2.6}
            />
          ) : null}
          {road.fogFromIndex >= 0 ? (
            <text
              className="hz-road-mlab"
              x={xOf(road.points[road.fogFromIndex]!.dateISO) + 6}
              y={ROAD_T + 10}
            >
              Sem lastro →
            </text>
          ) : null}
        </g>
      </svg>
    </div>
  );
}

function roadAria(road: RoadModel): string {
  const first = road.points[0]!;
  const last = road.points.at(-1)!;
  const min = road.points[road.minIndex]!;
  const parts = [
    `Saldo projetado de ${fmtDayMonth(first.dateISO)} a ${fmtDayMonth(last.dateISO)}.`,
    `Lançado vai de ${formatBRL(first.cents)} a ${formatBRL(last.cents)}.`,
    `Menor ponto ${formatBRL(min.cents)} em ${fmtDayMonth(min.dateISO)}.`,
  ];
  if (road.endTypicalCents !== null) {
    parts.push(
      `Se custar o de sempre, o fim do horizonte é ${formatBRL(road.endTypicalCents)}.`,
    );
  }
  return parts.join(" ");
}

function RoadEnds({ view }: { view: HorizonteView }) {
  return (
    <div className="hz__roadends">
      <span>
        Se custar o que está lançado:{" "}
        <b>
          <Money cents={view.endLaunchedCents ?? 0} size="inherit" />
        </b>
      </span>
      {view.endTypicalCents !== null ? (
        <span className={view.typicalHitsZero ? "hz__neg" : undefined}>
          Se custar o de sempre:{" "}
          <b>
            <Money cents={view.endTypicalCents} size="inherit" />
          </b>
          {view.typicalHitsZero ? (
            <EstimateMark
              term={{
                title: "Estimativa",
                body: "Projeção: os meses sem lastro custam o gasto típico. Não é o lançado.",
              }}
            />
          ) : null}
        </span>
      ) : null}
    </div>
  );
}

function NumbersFold({ view }: { view: HorizonteView }) {
  const road = view.road;
  if (!road) return null;
  // Fim de mês futuro: lançado (do month_end) × típico (do traçado). Alinhados por mês.
  const curYm = view.today.slice(0, 7);
  const rows = road.typicalPath.reduce<
    { ym: string; launched: number | null; typical: number }[]
  >((acc, p) => {
    const ym = p.dateISO.slice(0, 7);
    if (ym > curYm)
      acc.push({ ym, launched: launchedEndFor(view, ym), typical: p.cents });
    return acc;
  }, []);
  if (rows.length === 0) return null;
  return (
    <details className="hz__fold">
      <summary>Ver a estrada em números</summary>
      <table className="hz__numtbl">
        <thead>
          <tr>
            <th scope="col">Fim de</th>
            <th scope="col" className="hz__num">
              Lançado
            </th>
            <th scope="col" className="hz__num">
              Se custar o de sempre
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.ym}>
              <td>{monthLabel(r.ym)}</td>
              <td className="hz__num">
                {r.launched !== null ? formatBRL(r.launched) : "—"}
              </td>
              <td className={r.typical < 0 ? "hz__num hz__neg" : "hz__num"}>
                {formatBRL(r.typical)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}

function launchedEndFor(view: HorizonteView, ym: string): number | null {
  const m = view.grid.find(
    (g) => `${g.year}-${String(g.month).padStart(2, "0")}` === ym,
  );
  return m?.endBalanceCents ?? null;
}

function monthLabel(ym: string): string {
  const idx = parseInt(ym.slice(5, 7), 10) - 1;
  return (
    [
      "Janeiro",
      "Fevereiro",
      "Março",
      "Abril",
      "Maio",
      "Junho",
      "Julho",
      "Agosto",
      "Setembro",
      "Outubro",
      "Novembro",
      "Dezembro",
    ][idx] ?? ym
  );
}

// -------------------------------------------------------------------- grade --

function MonthCard({ month, onOpen }: { month: GridMonth; onOpen: () => void }) {
  if (month.state === "sem") {
    return (
      <span
        className="hz__mcard hz__mcard--sem"
        role="img"
        aria-label={`${month.label} de ${month.year}: sem registro`}
      >
        <span className="hz__mh" aria-hidden="true">
          <b>{month.label}</b>
          <small>’{String(month.year).slice(2)}</small>
        </span>
        <span className="hz__dash" aria-hidden="true">
          —
        </span>
        <small aria-hidden="true">Sem registro</small>
      </span>
    );
  }

  const stateLabel =
    month.state === "conf"
      ? "sem lastro, conferir"
      : month.state === "vivido"
        ? "em curso, previsto com lastro"
        : "previsto com lastro";
  const band = saldoBand(month.endBalanceCents);

  return (
    <button
      type="button"
      className={`hz__mcard${month.state === "conf" ? " hz__mcard--conf" : ""}`}
      onClick={onOpen}
      aria-label={`Abrir ${month.label} no Calendário — ${stateLabel}, saldo no fim do mês ${month.endBalanceCents !== null ? formatBRL(month.endBalanceCents) : "sem registro"}`}
    >
      <span className="hz__mh" aria-hidden="true">
        <b>{month.label}</b>
        {month.state === "conf" ? (
          <span className="hz__seal">Conferir</span>
        ) : (
          <small>’{String(month.year).slice(2)}</small>
        )}
      </span>
      <Dots month={month} color={band.text} />
      <span className="hz__mfim" aria-hidden="true">
        <span>Fim</span>
        <b>{month.endBalanceCents !== null ? formatBRL(month.endBalanceCents) : "—"}</b>
      </span>
    </button>
  );
}

function Dots({ month, color }: { month: GridMonth; color: string }) {
  const cells: { key: string; cls: string; solid: boolean }[] = [];
  for (let i = 0; i < month.firstDow; i++) {
    cells.push({ key: `off-${i}`, cls: "hz__d hz__d--off", solid: false });
  }
  for (let d = 1; d <= month.daysInMonth; d++) {
    let cls = "hz__d";
    let solid = false;
    if (month.state === "conf") {
      cls += " hz__d--conf";
    } else if (month.state === "vivido" && month.todayDay && d < month.todayDay) {
      cls += " hz__d--viv";
      solid = true;
    } else {
      cls += " hz__d--prev";
    }
    if (month.todayDay && d === month.todayDay) cls += " hz__d--today";
    cells.push({ key: `d-${d}`, cls, solid });
  }
  return (
    <span className="hz__dots" aria-hidden="true">
      {cells.map((c) =>
        c.solid ? (
          <i key={c.key} className={c.cls} style={{ background: color }} />
        ) : c.cls.includes("hz__d--prev") ? (
          <i key={c.key} className={c.cls} style={{ color }} />
        ) : (
          <i key={c.key} className={c.cls} />
        ),
      )}
    </span>
  );
}

// ----------------------------------------------------------- compromissos --

function CommitmentFold({
  month,
  defaultOpen,
}: {
  month: CommitmentMonth;
  defaultOpen: boolean;
}) {
  return (
    <details className="hz__mfold" open={defaultOpen}>
      <summary>
        <b>{month.label}</b>
        <span className="hz__msum">
          +{formatBRL(month.inCents)} · −{formatBRL(month.outCents)}
        </span>
      </summary>
      <div className="hz__mbody">
        {month.items.map((it) => (
          <div key={it.key} className="hz__line">
            <span className="hz__dia">{it.dayLabel}</span>
            <span className="hz__what">
              <b>{it.title}</b>
              <small>
                {it.subtitle}
                {it.installment ? (
                  <>
                    {" "}
                    <span className="hz__nn">{it.installment}</span>
                  </>
                ) : null}
              </small>
            </span>
            <span className={`hz__amt${it.isIn ? " hz__amt--in" : ""}`}>
              <Money cents={it.signedCents} size="inherit" />
            </span>
          </div>
        ))}
      </div>
    </details>
  );
}
