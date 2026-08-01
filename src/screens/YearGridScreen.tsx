import "./calendario.css";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  getForecast,
  getMonthGrid,
  getMonthTransactions,
  isTauri,
  type MonthGridDay,
  type TransactionRow,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { useNekoApp } from "../shell/appContext";
import { setCrumb } from "../shell/crumbStore";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Money, SignedMoney } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import { NoRecordDash } from "../design-system/components/NoRecordDash";
import { SR_ONLY } from "../design-system/srOnly";
import { MES, fmtBRL, saldoBand } from "../lib/nkFormat";
import { todayISO } from "../lib/format";
import { eyebrowDate } from "./hojeView";
import { monthTitle } from "./lancamentosView";
import {
  addMonths,
  agendaSignedCents,
  agendaTransactions,
  buildCalendarMonth,
  CAL_DOW,
  cellLabel,
  cellMoney,
  cellSigned,
  dayComponents,
  gridBand,
  monthHeadline,
  monthMarks,
  railSeries,
  shiftIso,
  shortDate,
  type AgendaComponent,
  type CalDayCell,
  type CalendarMonth,
  type DayRow,
  type MonthMark,
} from "./calendarioView";

// Fetchers com identidade estável por argumento (contrato do useCommand: o
// fetcher é capturado no primeiro render da key — um arrow inline fecharia
// sobre valores velhos).
const _gridFetchers = new Map<string, () => Promise<MonthGridDay[]>>();
function gridFetcher(ym: string): () => Promise<MonthGridDay[]> {
  const cached = _gridFetchers.get(ym);
  if (cached) return cached;
  const fn = () => getMonthGrid(Number(ym.slice(0, 4)), Number(ym.slice(5, 7)));
  _gridFetchers.set(ym, fn);
  return fn;
}

const _txFetchers = new Map<string, () => Promise<TransactionRow[]>>();
function txFetcher(ym: string): () => Promise<TransactionRow[]> {
  const cached = _txFetchers.get(ym);
  if (cached) return cached;
  const fn = () => getMonthTransactions(ym);
  _txFetchers.set(ym, fn);
  return fn;
}

// A didática inteira mora aqui — inclusive as faixas do termômetro, que saíram
// da tela como legenda fixa (regra 1: didática atrás de uma pergunta).
const HOW_TERM = {
  title: "Como ler o calendário",
  body: "Cada dia mostra o saldo que deixou; borda tracejada é dia previsto. O anel do acento marca hoje, o preenchimento marca o dia aberto, o triângulo verde marca entrada e o contorno âmbar, o menor saldo do mês. No celular a cor de fundo só aparece quando o dia aperta — o termômetro do saldo vai de Folga (acima de R$ 2.000) a Crítico (abaixo de −R$ 500), e a faixa cheia acompanha o dia aberto em palavra.",
};

const NO_CHAIN_TERM = {
  title: "Sem corrente para o dia",
  body: "A planilha não tem o dia e a projeção não chega até ele — o saldo aparece assim que houver registro ou previsão.",
};

interface CalendarioCellProps {
  cell: CalDayCell;
  selected: boolean;
  focused: boolean;
  onSelect: (iso: string) => void;
  cellRef: (iso: string, el: HTMLButtonElement | null) => void;
}

/** Gridcell interativo do dia: número, movimento e o saldo que o dia deixou. Os
 *  eventos viram classe (a cor é CSS) e o rótulo acessível repete tudo que a
 *  borda diz — cor nunca é o único canal. */
function CalendarioCell({
  cell,
  selected,
  focused,
  onSelect,
  cellRef,
}: CalendarioCellProps) {
  const cls = [
    "calendario__cell",
    cell.isToday ? "calendario__cell--today" : "",
    cell.isFuture ? "calendario__cell--future" : "",
    cell.hasIncome ? "calendario__cell--income" : "",
    cell.isLowest ? "calendario__cell--lowest" : "",
    selected ? "calendario__cell--selected" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const band = gridBand(cell.balanceCents);
  return (
    <button
      type="button"
      role="gridcell"
      className={cls}
      data-band={band ?? undefined}
      tabIndex={focused ? 0 : -1}
      aria-selected={selected}
      aria-label={cellLabel(cell)}
      onClick={() => onSelect(cell.iso)}
      ref={(el) => cellRef(cell.iso, el)}
    >
      <span className="calendario__cell-d">{cell.day}</span>
      <span className="calendario__cell-mv" aria-hidden="true">
        {cell.movementCents != null ? (
          <span className={cell.movementCents > 0 ? "calendario__mv-up" : ""}>
            {cellSigned(cell.movementCents)}
          </span>
        ) : null}
      </span>
      <span className="calendario__cell-s" aria-hidden="true">
        {cell.balanceCents != null ? cellMoney(cell.balanceCents) : "—"}
      </span>
    </button>
  );
}

/** O trilho: o saldo do mês numa linha do tamanho de uma frase. Sólido no
 *  realizado, tracejado na projeção — a mesma gramática da borda da célula.
 *  Decorativo por construção: cada valor que ele desenha está impresso na grade. */
function CalendarioRail({ month }: { month: CalendarMonth }) {
  const series = railSeries(month);
  if (!series) return null;
  const W = 340;
  const H = 74;
  const PAD = 8;
  const at = (i: number) => {
    const p = series.points[i];
    if (!p) return "";
    return `${(p.x * W).toFixed(1)},${(H - PAD - p.v * (H - PAD * 2)).toFixed(1)}`;
  };
  const cut = series.points.findIndex((p) => p.isFuture);
  const lastRealized = cut === -1 ? series.points.length - 1 : cut - 1;
  // Uma passada por traço: o trecho previsto começa no último ponto realizado,
  // para que a linha sólida e a tracejada se encontrem sem intervalo.
  const trace = (from: number, to: number) => {
    const parts: string[] = [];
    for (let i = from; i <= to; i++) parts.push(at(i));
    return parts.join(" ");
  };
  const solid = trace(0, lastRealized);
  const dashed = trace(Math.max(lastRealized, 0), series.points.length - 1);
  const dot = (i: number, cls: string) => {
    const p = series.points[i];
    if (!p) return null;
    return (
      <circle
        key={cls}
        className={cls}
        cx={p.x * W}
        cy={H - PAD - p.v * (H - PAD * 2)}
        r={3}
      />
    );
  };
  return (
    <svg
      className="calendario__rail"
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      aria-hidden="true"
      focusable="false"
    >
      {solid ? (
        <polyline
          className="calendario__rail-real"
          points={solid}
          vectorEffect="non-scaling-stroke"
        />
      ) : null}
      {cut !== -1 ? (
        <polyline
          className="calendario__rail-proj"
          points={dashed}
          vectorEffect="non-scaling-stroke"
        />
      ) : null}
      {series.points.map((p, i) =>
        p.hasIncome ? dot(i, `calendario__rail-in i${i}`) : null,
      )}
      {series.lowestIndex >= 0 ? dot(series.lowestIndex, "calendario__rail-low") : null}
      {series.todayIndex >= 0 ? dot(series.todayIndex, "calendario__rail-today") : null}
    </svg>
  );
}

interface CalendarioDayProps {
  iso: string;
  isFuture: boolean;
  txs: TransactionRow[];
  comps: AgendaComponent[];
  balanceCents: number | null;
  movementCents: number | null;
  onOpenLedger: () => void;
}

/** O dia aberto — painel fixo à direita no desktop, bloco abaixo da grade no
 *  celular. O saldo é o herói e o termômetro vem em palavra ao lado dele;
 *  `aria-live` anuncia a troca de dia. */
function CalendarioDay({
  iso,
  isFuture,
  txs,
  comps,
  balanceCents,
  movementCents,
  onOpenLedger,
}: CalendarioDayProps) {
  const band = balanceCents != null ? saldoBand(balanceCents) : null;
  return (
    <aside className="calendario__day" aria-labelledby="cal-day-t">
      <div aria-live="polite">
        <h3 id="cal-day-t">
          {eyebrowDate(iso)}
          {isFuture ? <span className="calendario__day-prev"> · previsto</span> : null}
        </h3>
        <div className="calendario__day-head">
          {balanceCents != null ? (
            <p className="calendario__day-money">{fmtBRL(balanceCents)}</p>
          ) : (
            <NoRecordDash term={NO_CHAIN_TERM} label="Sem corrente" />
          )}
          {band ? (
            <span className="calendario__day-band">
              <i style={{ background: band.text }} aria-hidden="true" />
              {band.label}
            </span>
          ) : null}
        </div>
        <p className="calendario__day-mv">
          {movementCents == null
            ? "Sem movimento conhecido"
            : movementCents === 0
              ? "Sem movimento no dia"
              : `${cellSignedFull(movementCents)} no dia`}
        </p>
        {txs.length > 0 ? (
          <ul className="calendario__day-list">
            {txs.map((t) => (
              <li key={t.id}>
                <span className="calendario__day-desc">
                  {t.description}
                  {t.installment_index != null && t.installment_total != null ? (
                    <i className="calendario__pill calendario__pill--mono">
                      {t.installment_index}/{t.installment_total}
                    </i>
                  ) : null}
                  {t.has_refund_link ? (
                    <i className="calendario__pill calendario__pill--ok">Reembolso</i>
                  ) : null}
                  {t.is_projection && !isFuture ? (
                    <i className="calendario__pill calendario__pill--prev">Previsto</i>
                  ) : null}
                </span>
                <SignedMoney cents={agendaSignedCents(t)} size="sm" />
              </li>
            ))}
          </ul>
        ) : (
          <p className="calendario__day-empty">
            {comps.length > 0
              ? "Sem itens detalhados — o dia fecha no resumo."
              : "Sem movimento — o saldo ficou como estava."}
          </p>
        )}
        {comps.length > 0 ? (
          <ul className="calendario__day-comps">
            {comps.map((c) => (
              <li key={c.key}>
                <span>{c.label}</span>
                <Money cents={c.cents} size="sm" />
              </li>
            ))}
          </ul>
        ) : null}
      </div>
      <button type="button" className="calendario__day-link" onClick={onOpenLedger}>
        Ver no Livro-razão ›
      </button>
    </aside>
  );
}

/** Assinatura de dinheiro com sinal e precisão cheia (o `cellSigned` é a versão
 *  compacta da célula). */
function cellSignedFull(cents: number): string {
  return `${cents > 0 ? "+" : ""}${fmtBRL(cents)}`;
}

/** "O que marca o mês": os dias que decidem o mês, cada linha navegando para o
 *  dia. É a legenda virada conteúdo — as mesmas marcas que a grade desenha,
 *  agora com data e valor. */
function CalendarioMarks({
  marks,
  onSelect,
}: {
  marks: MonthMark[];
  onSelect: (iso: string) => void;
}) {
  if (marks.length === 0) return null;
  return (
    <aside className="calendario__marks" aria-labelledby="cal-marks-t">
      <h3 id="cal-marks-t">O que marca o mês</h3>
      <ul>
        {marks.map((m, i) => {
          // As entradas se agrupam sob um rótulo só: repetir "Entradas" em cada
          // linha transforma o papel em ruído (regra 41).
          const repeated = m.kind === "income" && marks[i - 1]?.kind === "income";
          const tone =
            m.kind === "income"
              ? "calendario__mark--in"
              : m.kind === "out"
                ? ""
                : "calendario__mark--low";
          return (
            <li key={m.iso + m.kind}>
              <button type="button" onClick={() => onSelect(m.iso)}>
                <span className="calendario__mark-l">
                  {repeated ? null : <b>{m.label}</b>}
                  <span>{eyebrowDate(m.iso)}</span>
                </span>
                <span className={`calendario__mark-v ${tone}`}>
                  {m.kind === "lowest" || m.kind === "lowest-out"
                    ? fmtBRL(m.cents)
                    : cellSignedFull(m.cents)}
                  {m.extraCents != null ? (
                    <em>{cellSignedFull(m.extraCents)}</em>
                  ) : null}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

/** Linha do dia na fonte certa da costura: passado no realizado, dali em diante na projeção. */
function dayRowAt(
  realized: DayRow[],
  forecast: DayRow[],
  today: string,
  iso: string,
): DayRow | undefined {
  const source = iso < today ? realized : forecast;
  return source.find((r) => r.date === iso);
}

export function YearGridScreen() {
  const TODAY = todayISO();
  const todayYm = TODAY.slice(0, 7);
  const [ym, setYm] = useState<string | null>(null);
  const activeYm = ym ?? todayYm;
  const isCurrentMonth = activeYm === todayYm;
  const year = Number(activeYm.slice(0, 4));
  const month0 = Number(activeYm.slice(5, 7)) - 1;
  const prevYm = addMonths(activeYm, -1);

  const [selectedIso, setSelectedIso] = useState<string | null>(null);
  const [focusedIso, setFocusedIso] = useState<string | null>(null);
  // Lazy init: `useRef(new Map())` aloca um Map a cada render e joga fora.
  const cellRefsBox = useRef<Map<string, HTMLButtonElement> | null>(null);
  cellRefsBox.current ??= new Map();
  const cellRefs = cellRefsBox.current;
  const pendingFocus = useRef<string | null>(null);
  const { navigate } = useNekoApp();

  const forecastQ = useCommand("get_forecast", getForecast);
  const gridQ = useCommand(`get_month_grid:${activeYm}`, gridFetcher(activeYm));
  const prevGridQ = useCommand(`get_month_grid:${prevYm}`, gridFetcher(prevYm));
  const txQ = useCommand(`month_transactions:${activeYm}`, txFetcher(activeYm));

  // O crumb da appbar mostra o mês visto; `setCrumb` é função de módulo
  // (identidade fixa), então o efeito só re-dispara quando o rótulo muda.
  const crumbLabel = monthTitle(activeYm);
  useEffect(() => {
    setCrumb("calendario", crumbLabel);
    return () => setCrumb("calendario", null);
  }, [crumbLabel]);

  if (forecastQ.loading || gridQ.loading || prevGridQ.loading) {
    return <EmptyState variant="skeleton" skeletonRows={6} />;
  }
  // A condição é falha de consulta, não ausência de dado: a variante de erro anuncia por
  // `role="alert"` e a copy não atribui ao usuário o que quebrou na leitura.
  if (forecastQ.error || gridQ.error) {
    return (
      <EmptyState
        variant="error"
        title="Não foi possível carregar o calendário"
        description="A leitura do saldo dia a dia falhou. Tente de novo em instantes."
      />
    );
  }

  const realized: DayRow[] = [...(gridQ.data ?? []), ...(prevGridQ.data ?? [])];
  const forecast: DayRow[] = forecastQ.data?.daily ?? [];
  const calMonth = buildCalendarMonth({
    year,
    month0,
    today: TODAY,
    realized,
    forecast,
  });

  // Seleção e foco caem no default quando o mês visto muda (nenhum efeito:
  // um iso de outro mês simplesmente não vale aqui).
  const defaultIso = isCurrentMonth ? TODAY : `${activeYm}-01`;
  const selIso = selectedIso?.startsWith(activeYm) ? selectedIso : defaultIso;
  const focusIso = focusedIso?.startsWith(activeYm) ? focusedIso : selIso;

  const goMonth = (delta: number) => {
    const next = addMonths(activeYm, delta);
    setYm(next === todayYm ? null : next);
    setSelectedIso(null);
    setFocusedIso(null);
  };

  const selectDay = (iso: string) => {
    setSelectedIso(iso);
    setFocusedIso(iso);
  };

  const focusCell = (iso: string) => {
    setFocusedIso(iso);
    cellRefs.get(iso)?.focus();
  };

  // O ref das células alimenta o roving focus; quando um PageUp/Down pediu um
  // dia do mês novo, o foco acontece no mount da célula-alvo.
  const registerCellRef = (iso: string, el: HTMLButtonElement | null) => {
    if (el) {
      cellRefs.set(iso, el);
      if (pendingFocus.current === iso) {
        pendingFocus.current = null;
        el.focus();
      }
    } else {
      cellRefs.delete(iso);
    }
  };

  // Roving tabindex (padrão APG de grade de datas): setas movem ±1/±7 dentro
  // do mês, Home/End vão às pontas da semana, PageUp/Down trocam o mês
  // focando o mesmo dia (ou o último existente).
  const onGridKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const moves: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: 7,
      ArrowUp: -7,
    };
    if (e.key in moves) {
      e.preventDefault();
      const target = shiftIso(focusIso, moves[e.key] ?? 0);
      if (target.startsWith(activeYm)) focusCell(target);
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      for (const week of calMonth.weeks) {
        if (!week.some((c) => c?.iso === focusIso)) continue;
        const days = week.filter((c): c is CalDayCell => c != null);
        const edge = e.key === "Home" ? days[0] : days[days.length - 1];
        if (edge) focusCell(edge.iso);
        return;
      }
      return;
    }
    if (e.key === "PageUp" || e.key === "PageDown") {
      e.preventDefault();
      const delta = e.key === "PageUp" ? -1 : 1;
      const nextYm = addMonths(activeYm, delta);
      const day = Number(focusIso.slice(8, 10));
      const dim = new Date(
        Number(nextYm.slice(0, 4)),
        Number(nextYm.slice(5, 7)),
        0,
      ).getDate();
      const target = `${nextYm}-${String(Math.min(day, dim)).padStart(2, "0")}`;
      pendingFocus.current = target;
      goMonth(delta);
      // Depois do reset do goMonth: o roving tabIndex segue o dia-alvo do mês
      // novo (sem isto a próxima seta partiria do dia 1, não do dia focado).
      setFocusedIso(target);
    }
  };

  const selCell = calMonth.weeks.flat().find((c): c is CalDayCell => c?.iso === selIso);
  const selRow = dayRowAt(realized, forecast, TODAY, selIso);
  const selTxs = agendaTransactions(txQ.data ?? [], selIso);
  const selComps = dayComponents(selRow);
  const selFuture = selIso > TODAY;
  const headline = monthHeadline(calMonth, MES[month0] ?? "");

  return (
    <div className="calendario">
      <header className="calendario__verdict">
        <div className="calendario__eyebrow">
          {calMonth.lowestIso ? `Realizado até ${shortDate(TODAY)}` : "Sem corrente"}
          {" · "}
          <InfoPopover term={HOW_TERM} hideMarker>
            <span className="calendario__how">
              Como funciona?
              <span style={SR_ONLY}> — Calendário do saldo</span>
            </span>
          </InfoPopover>
          <MonthNav
            className="calendario__nav"
            label={crumbLabel}
            hideLabel
            atToday={isCurrentMonth}
            onPrev={() => goMonth(-1)}
            onNext={() => goMonth(1)}
            onToday={() => {
              setYm(null);
              setSelectedIso(null);
              setFocusedIso(null);
            }}
            prevLabel="Mês anterior"
            nextLabel="Próximo mês"
          />
        </div>
        <h2 className="calendario__headline" data-large-title>
          {headline ?? `${MES[month0]} ainda não tem corrente.`}
        </h2>
      </header>

      <div className="calendario__main">
        <div className="calendario__rail-wrap" aria-hidden="true">
          <CalendarioRail month={calMonth} />
        </div>

        <div
          className="calendario__grid"
          role="grid"
          aria-label={`Saldo dia a dia — ${crumbLabel}`}
          onKeyDown={onGridKeyDown}
        >
          <div className="calendario__dow" role="row">
            {CAL_DOW.map((d) => (
              <span key={d} role="columnheader">
                {d}
              </span>
            ))}
          </div>
          <div className="calendario__weeks">
            {calMonth.weeks.map((week, wi) => (
              <div className="calendario__week" role="row" key={wi}>
                {week.map((cell, ci) =>
                  cell ? (
                    <CalendarioCell
                      key={cell.iso}
                      cell={cell}
                      selected={cell.iso === selIso}
                      focused={cell.iso === focusIso}
                      onSelect={selectDay}
                      cellRef={registerCellRef}
                    />
                  ) : (
                    <span
                      key={`out-${ci}`}
                      role="gridcell"
                      aria-hidden="true"
                      className="calendario__cell calendario__cell--out"
                    />
                  ),
                )}
              </div>
            ))}
          </div>
        </div>

        <CalendarioDay
          iso={selIso}
          isFuture={selFuture}
          txs={selTxs}
          comps={selComps}
          balanceCents={selCell?.balanceCents ?? null}
          movementCents={selCell?.movementCents ?? null}
          onOpenLedger={() => navigate("lancamentos")}
        />

        <CalendarioMarks marks={monthMarks(calMonth)} onSelect={selectDay} />
      </div>

      {!isTauri && (
        <p className="calendario__preview-note">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
