import "./calendario.css";
import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
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
import { MES, saldoBand } from "../lib/nkFormat";
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
  shiftIso,
  type AgendaComponent,
  type CalDayCell,
  type DayRow,
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

const HOW_TERM = {
  title: "Como ler o calendário",
  body: "Cada dia mostra o movimento e o saldo que ele deixou; borda tracejada é dia previsto. As cores marcam hoje, entradas e o menor saldo — no celular a cor é o termômetro do saldo e os números moram na agenda do dia tocado.",
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

/** Gridcell interativo do dia — os eventos viram classe (a cor é CSS) e o
 *  rótulo acessível repete tudo que a borda diz. */
function CalendarioCell({
  cell,
  selected,
  focused,
  onSelect,
  cellRef,
}: CalendarioCellProps) {
  const band = saldoBand(cell.balanceCents);
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
  return (
    <button
      type="button"
      role="gridcell"
      className={cls}
      style={{ "--cell-band": band.fill } as CSSProperties}
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

/** A legenda descreve as cores que o viewport usa: eventos na grade cheia do
 *  desktop, termômetro + pontos no celular (CSS alterna os conjuntos). */
function CalendarioLegend() {
  return (
    <div className="calendario__legend">
      <span className="calendario__legend-item calendario__legend-item--desk">
        <i className="calendario__k calendario__k--today" /> Hoje
      </span>
      <span className="calendario__legend-item calendario__legend-item--desk">
        <i className="calendario__k calendario__k--income" /> Entrada
      </span>
      <span className="calendario__legend-item calendario__legend-item--desk">
        <i className="calendario__k calendario__k--lowest" /> Menor saldo do mês
      </span>
      <span className="calendario__legend-item calendario__legend-item--desk">
        <i className="calendario__k calendario__k--future" /> Previsto — ainda não
        aconteceu
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i
          className="calendario__k"
          style={{ background: "var(--saldo-band-comfortable-fill)" }}
        />{" "}
        Folga
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i
          className="calendario__k"
          style={{ background: "var(--saldo-band-ok-fill)" }}
        />{" "}
        OK
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i
          className="calendario__k"
          style={{ background: "var(--saldo-band-tight-fill)" }}
        />{" "}
        Apertado
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i
          className="calendario__k"
          style={{ background: "var(--saldo-band-negative-fill)" }}
        />{" "}
        Negativo
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i
          className="calendario__k"
          style={{ background: "var(--saldo-band-critical-fill)" }}
        />{" "}
        Crítico
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i className="calendario__dot calendario__dot--income" /> Entrada
      </span>
      <span className="calendario__legend-item calendario__legend-item--mob">
        <i className="calendario__dot calendario__dot--lowest" /> Menor saldo
      </span>
    </div>
  );
}

interface CalendarioAgendaProps {
  iso: string;
  isFuture: boolean;
  txs: TransactionRow[];
  comps: AgendaComponent[];
  balanceCents: number | null;
  onOpenLedger: () => void;
}

/** Agenda do dia — painel fixo à direita no desktop, abaixo da grade no
 *  celular; `aria-live` anuncia a troca de dia. */
function CalendarioAgenda({
  iso,
  isFuture,
  txs,
  comps,
  balanceCents,
  onOpenLedger,
}: CalendarioAgendaProps) {
  return (
    <aside className="calendario__agenda" aria-labelledby="cal-agenda-t">
      <div aria-live="polite">
        <h3 id="cal-agenda-t">{eyebrowDate(iso)}</h3>
        {isFuture ? (
          <p className="calendario__agenda-tag">Previsto — ainda não aconteceu</p>
        ) : null}
        {txs.length > 0 ? (
          <ul className="calendario__agenda-list">
            {txs.map((t) => (
              <li key={t.id}>
                <span className="calendario__agenda-desc">
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
          <p className="calendario__agenda-empty">
            {comps.length > 0
              ? "Sem itens detalhados — o dia fecha no resumo."
              : "Sem movimento — o saldo ficou como estava."}
          </p>
        )}
        {comps.length > 0 ? (
          <ul className="calendario__agenda-comps">
            {comps.map((c) => (
              <li key={c.key}>
                <span>{c.label}</span>
                <Money cents={c.cents} size="sm" />
              </li>
            ))}
          </ul>
        ) : null}
        <div className="calendario__agenda-saldo">
          <span>Saldo que o dia deixou</span>
          {balanceCents != null ? (
            <Money cents={balanceCents} size="sm" />
          ) : (
            <NoRecordDash term={NO_CHAIN_TERM} label="Sem corrente" />
          )}
        </div>
      </div>
      <button type="button" className="calendario__agenda-link" onClick={onOpenLedger}>
        Ver no Livro-razão ›
      </button>
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
  const cellRefs = useRef(new Map<string, HTMLButtonElement>());
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
    cellRefs.current.get(iso)?.focus();
  };

  // O ref das células alimenta o roving focus; quando um PageUp/Down pediu um
  // dia do mês novo, o foco acontece no mount da célula-alvo.
  const registerCellRef = (iso: string, el: HTMLButtonElement | null) => {
    if (el) {
      cellRefs.current.set(iso, el);
      if (pendingFocus.current === iso) {
        pendingFocus.current = null;
        el.focus();
      }
    } else {
      cellRefs.current.delete(iso);
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

  return (
    <div className="calendario">
      <div className="calendario__head">
        <div className="calendario__head-text">
          <h2>{MES[month0]} dia a dia</h2>
          <p className="calendario__context">
            Cada dia mostra o movimento e o saldo que ele deixou.{" "}
            <InfoPopover term={HOW_TERM} hideMarker>
              <span className="calendario__how">
                Como funciona?
                <span style={SR_ONLY}> — Calendário do saldo</span>
              </span>
            </InfoPopover>
          </p>
        </div>
        <MonthNav
          label={crumbLabel}
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

      <div className="calendario__main">
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

        <CalendarioLegend />

        <CalendarioAgenda
          iso={selIso}
          isFuture={selFuture}
          txs={selTxs}
          comps={selComps}
          balanceCents={selCell?.balanceCents ?? null}
          onOpenLedger={() => navigate("lancamentos")}
        />
      </div>

      {!isTauri && (
        <p className="calendario__preview-note">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
