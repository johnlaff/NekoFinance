import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { Calendar, CalendarRange, CheckCircle2, SlidersHorizontal } from "lucide-react";
import { Button } from "../design-system/components/Button";
import {
  createTransaction,
  getDashboardSummary,
  getForecast,
  getUpcomingBills,
  isTauri,
  type ForecastDay,
} from "../lib/api";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { ModeChip } from "../design-system/components/ModeChip";
import { Money } from "../design-system/components/Money";
import { NoRecordDash } from "../design-system/components/NoRecordDash";
import { parseBRLToCents } from "../lib/format";
import { kindToFields } from "../lib/movement";
import {
  fmtBRL,
  MES,
  MES_ABBR,
  monthOf,
  saldoBand,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { useNekoApp } from "../shell/appContext";

const WEEKDAYS = [
  "Domingo",
  "Segunda-feira",
  "Terça-feira",
  "Quarta-feira",
  "Quinta-feira",
  "Sexta-feira",
  "Sábado",
];

function eyebrowDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return "";
  const wd = new Date(y, m - 1, d).getDay();
  return `${WEEKDAYS[wd] ?? ""}, ${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

function faturaDayLabel(iso: string): string {
  const [, m, d] = iso.split("-").map(Number);
  if (!m || !d) return iso;
  return `${d} de ${(MES[m - 1] ?? "").toLowerCase()}`;
}

/** Mini-gráfico de área do saldo do mês (porte do protótipo). */
function MiniTrajectory({ daily, today }: { daily: ForecastDay[]; today: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [w, setW] = useState(340);
  useEffect(() => {
    if (!ref.current || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setW(Math.max(120, e.contentRect.width));
    });
    // react-doctor-disable-next-line react-doctor/no-initialize-state -- width is measured from the DOM after mount (responsive SVG); no value exists before layout
    ro.observe(ref.current);
    return () => ro.disconnect();
  }, []);
  if (daily.length === 0)
    return <div ref={ref} style={{ width: "100%", height: 96 }} />;
  const H = 96,
    padTop = 10,
    padBot = 10;
  const vals = daily.map((d) => d.balance_cents);
  const min = Math.min(...vals, 0),
    max = Math.max(...vals, 0);
  const range = max - min || 1;
  const innerH = H - padTop - padBot;
  const x = (i: number) => (daily.length <= 1 ? w / 2 : (i / (daily.length - 1)) * w);
  const y = (c: number) => padTop + innerH - ((c - min) / range) * innerH;
  const pts = daily.map((d, i) => [x(i), y(d.balance_cents)] as const);
  const linePts = pts.map((p) => `${p[0].toFixed(1)},${p[1].toFixed(1)}`).join(" ");
  const first = pts[0]!;
  const last = pts[pts.length - 1]!;
  const areaD =
    `M ${first[0].toFixed(1)},${H - padBot} L ` +
    linePts.split(" ").join(" L ") +
    ` L ${last[0].toFixed(1)},${H - padBot} Z`;
  const todayIdx = daily.findIndex((d) => d.date === today);
  const minIdx = vals.indexOf(Math.min(...vals));
  const hasDeficit = min < 0;
  const zeroY = y(0);
  return (
    <div ref={ref} style={{ width: "100%", lineHeight: 0 }}>
      <svg
        width={w}
        height={H}
        viewBox={`0 0 ${w} ${H}`}
        preserveAspectRatio="xMidYMid meet"
        role="img"
        aria-label="Saldo projetado do mês"
        style={{ display: "block" }}
      >
        <defs>
          <linearGradient id="mini-grad" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--primary)" stopOpacity="0.26" />
            <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.02" />
          </linearGradient>
        </defs>
        {hasDeficit ? (
          <line
            x1={0}
            x2={w}
            y1={zeroY}
            y2={zeroY}
            stroke="var(--danger-400)"
            strokeWidth="1"
            strokeDasharray="3 4"
            opacity="0.7"
          />
        ) : null}
        <path d={areaD} fill="url(#mini-grad)" />
        <polyline
          points={linePts}
          fill="none"
          stroke="var(--primary)"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        {minIdx >= 0 && minIdx !== todayIdx ? (
          <circle
            cx={x(minIdx)}
            cy={y(vals[minIdx]!)}
            r="3"
            fill={hasDeficit ? "var(--danger-400)" : "var(--text-faint)"}
          />
        ) : null}
        {todayIdx >= 0 ? (
          <circle
            cx={x(todayIdx)}
            cy={y(daily[todayIdx]!.balance_cents)}
            r="3.5"
            fill="var(--primary)"
            stroke="var(--surface)"
            strokeWidth="2"
          />
        ) : null}
      </svg>
    </div>
  );
}

const CHECKIN_TYPES: MovementType[] = ["diario", "cartao", "saida"];

// Didáticas dos estados epistêmicos das réguas do herói (copy conceitual fixa vive só aqui,
// no padrão InfoPopover; o resto da UI mostra dado derivado).
const TETO_ESTIMATE_TERM = {
  title: "Teto estimado",
  body: "Você ainda não estipulou um teto: este é o Diário médio do mês anterior, exibido como estimativa. Estipule o seu na cerimônia do teto para virar veredito.",
};
const TETO_NONE_TERM = {
  title: "Sem teto estipulado",
  body: "Não há teto escolhido nem histórico de Diário para estimar um. A cerimônia do teto lista o mês variável por categoria e divide pelos dias.",
};
const RESERVE_ESTIMATE_TERM = {
  title: "Retrato vivo",
  body: "Ainda há poucos meses completos de custo de vida: a régua usa o retrato do que existe, como estimativa. Com 6 meses completos ela vira veredito.",
};
const RESERVE_NONE_TERM = {
  title: "Reserva sem registro",
  body: "Nenhuma conta de reserva mapeada (ou nenhum mês de custo de vida ainda). Marque nos bolsos qual conta é a sua reserva para a régua existir.",
};
const RESERVE_ZERO_TERM = {
  title: "Sem reserva",
  body: "Suas contas de reserva estão zeradas. A reserva é a fundação do método: o próximo passo é o ritual de guardar antes de gastar.",
};

const STAT_LINK_STYLE: CSSProperties = {
  background: "none",
  border: "none",
  color: "var(--primary-quiet-text)",
  cursor: "pointer",
  font: "inherit",
  fontSize: 12,
  padding: 0,
  textDecoration: "underline dotted",
};

const HERO_CHIP_ROW_STYLE: CSSProperties = {
  display: "flex",
  gap: 8,
  margin: "0 0 6px",
};

export function DashboardScreen() {
  const { openCompose, navigate } = useNekoApp();
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const billsQ = useCommand("get_upcoming_bills", () => getUpcomingBills(45));

  const summary = summaryQ.data;
  const forecast = forecastQ.data;
  const fetchError = summaryQ.error ?? forecastQ.error;

  // Nunca renderizar R$ 0,00 fingindo dado real: se QUALQUER uma das duas fontes do herói
  // falhou sem dado (nem cache), os números não podem ser calculados — estado de erro com
  // retry. O banner "dados antigos" fica só para quando todas as fontes têm cache.
  const summaryMissing = Boolean(summaryQ.error) && !summary;
  const forecastMissing = Boolean(forecastQ.error) && !forecast;
  if (summaryMissing || forecastMissing) {
    return (
      <div className="hoje neko-app">
        <EmptyState
          variant="error"
          title="Não foi possível carregar o painel"
          description="Os números de hoje não puderam ser calculados. Verifique o app e tente de novo."
          action={
            <Button variant="primary" onClick={() => invalidateCommands()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  // Primeira carga sem cache: esqueleto em vez de estados fabricados (R$ 0,00, "Sem registro"
  // e modo débito nasceriam do `undefined` antes do DTO chegar).
  if (!summary || !forecast) {
    return (
      <div className="hoje neko-app">
        <EmptyState variant="skeleton" skeletonRows={6} />
      </div>
    );
  }

  const today = forecast?.today ?? "";
  const month = today ? monthOf(today) : new Date().getMonth();

  const ceiling = summary?.daily_budget ?? 0;
  const spent = summary?.daily_spend_today ?? 0;
  const safeToSpend = Math.max(0, forecast?.safe_to_spend_today_cents ?? 0);
  const reserve = summary?.reserve_months ?? 0;
  const endBalance = summary?.balance ?? 0;
  const monthDaily = (forecast?.daily ?? []).filter((d) => monthOf(d.date) === month);
  const saldoHoje =
    monthDaily.find((d) => d.date === today)?.balance_cents ?? endBalance;
  const minSaldo = monthDaily.length
    ? Math.min(...monthDaily.map((d) => d.balance_cents))
    : (forecast?.deepest_deficit?.balance_cents ?? endBalance);
  const endBand = saldoBand(endBalance);

  // Estados epistêmicos + modo de gasto (o julgamento vem pronto do domínio; aqui só apresenta).
  const ceilingSource = summary?.daily_ceiling_source ?? "none";
  const reserveState = summary?.reserve_state ?? "no_record";
  const cardMode = summary?.spending_mode === "card";
  const hasCeiling = ceiling > 0;

  return (
    <div className="hoje neko-app">
      {fetchError ? (
        <p
          role="status"
          style={{
            margin: 0,
            padding: "8px 12px",
            borderRadius: "var(--radius-md)",
            background: "var(--warning-tint)",
            color: "var(--warning-400)",
            fontSize: 12.5,
          }}
        >
          Não foi possível atualizar agora — mostrando os últimos dados carregados.
        </p>
      ) : null}
      <section className="hoje-hero">
        <div>
          <p className="hoje-hero__eyebrow">{eyebrowDate(today)}</p>
          {summary && (
            <div style={HERO_CHIP_ROW_STYLE}>
              <ModeChip mode={summary.spending_mode} gate={summary.card_gate} />
            </div>
          )}
          <p className="hoje-hero__label">Pode gastar hoje</p>
          <p className="hoje-hero__kpi">
            {fmtBRL(safeToSpend)}{" "}
            <small>{cardMode ? "sem faltar caixa" : "sem furar o teto"}</small>
          </p>
          <p className="hoje-hero__reason">
            {cardMode ? (
              // No modo cartão o teto não governa o dia (é referência): a frase cita só os
              // limites que de fato mordem — caixa e poupança — para não contradizer o check-in.
              <>
                É o que o caixa aguenta sem nenhum dia no vermelho até o fim do mês,
                mantendo a economia do ano viva — no modo cartão, o dia acompanha as
                faturas.
              </>
            ) : hasCeiling ? (
              <>
                É o menor de dois limites: o teto diário
                {ceilingSource === "estimate" ? " estimado" : ""} de{" "}
                <Money cents={ceiling} size="inherit" /> e o que o caixa aguenta sem
                nenhum dia no vermelho até o fim do mês.
              </>
            ) : (
              <>
                É o que o caixa aguenta sem nenhum dia no vermelho até o fim do mês —
                ainda sem teto diário estipulado.
              </>
            )}
          </p>
          <dl className="hoje-hero__stats">
            <div>
              <dt>Saldo hoje</dt>
              <dd>
                <Money cents={saldoHoje} size="inherit" />
              </dd>
            </div>
            <div>
              <dt>Reserva</dt>
              <dd>
                {reserveState === "no_record" ? (
                  <NoRecordDash
                    term={RESERVE_NONE_TERM}
                    cta={
                      <button
                        type="button"
                        style={STAT_LINK_STYLE}
                        onClick={() => navigate("config")}
                      >
                        Mapear
                      </button>
                    }
                  />
                ) : reserveState === "zero" ? (
                  <NoRecordDash term={RESERVE_ZERO_TERM} label="Sem reserva" />
                ) : (
                  <>
                    {reserve.toLocaleString("pt-BR", {
                      minimumFractionDigits: 1,
                      maximumFractionDigits: 1,
                    })}{" "}
                    meses{" "}
                    {reserveState === "estimate" && (
                      <EstimateMark term={RESERVE_ESTIMATE_TERM} />
                    )}
                  </>
                )}
              </dd>
            </div>
            <div>
              <dt>Teto diário</dt>
              <dd>
                {ceilingSource === "none" ? (
                  <NoRecordDash
                    term={TETO_NONE_TERM}
                    // Com proposta pendente o convite único é revisá-la (ela já resolve o
                    // "estipular"); dois CTAs para o mesmo destino seriam ruído.
                    cta={
                      summary?.ceiling_proposal_pending ? undefined : (
                        <button
                          type="button"
                          style={STAT_LINK_STYLE}
                          onClick={() => navigate("teto")}
                        >
                          Estipular
                        </button>
                      )
                    }
                  />
                ) : (
                  <>
                    <Money cents={ceiling} size="inherit" />{" "}
                    {ceilingSource === "estimate" && (
                      <EstimateMark term={TETO_ESTIMATE_TERM} />
                    )}
                  </>
                )}
                {summary?.ceiling_proposal_pending && (
                  <div>
                    <button
                      type="button"
                      style={STAT_LINK_STYLE}
                      onClick={() => navigate("teto")}
                    >
                      Proposta da planilha aguardando — revisar
                    </button>
                  </div>
                )}
              </dd>
            </div>
          </dl>
        </div>
        <aside className="hoje-fc">
          <div className="hoje-fc__top">
            <span className="hoje-fc__lab">
              Saldo no fim de {(MES[month] ?? "").toLowerCase()}
            </span>
            <span
              className="hoje-chip"
              style={{
                background: `color-mix(in srgb, ${endBand.text} 14%, transparent)`,
                color: endBand.text,
              }}
            >
              <span
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: endBand.text,
                }}
              />
              {endBand.label}
            </span>
          </div>
          <div className="hoje-fc__val" style={{ color: endBand.text }}>
            <Money cents={endBalance} size="inherit" />
          </div>
          <MiniTrajectory daily={monthDaily} today={today} />
          <p className="hoje-fc__foot">
            {minSaldo < 0 ? (
              <>
                Atenção: chega a <Money cents={minSaldo} size="inherit" /> no pior dia.
              </>
            ) : (
              <>
                Menor saldo previsto no mês: <Money cents={minSaldo} size="inherit" />.
              </>
            )}
          </p>
        </aside>
      </section>

      <div className="hoje-grid">
        <CheckinCard
          ceiling={ceiling}
          ceilingSource={ceilingSource}
          spent={spent}
          remaining={ceiling - spent}
          today={today}
          lastReal={summary?.last_real_tx_date ?? null}
          cardMode={cardMode}
          cartaoMonthCents={summary?.cartao_month_cents ?? 0}
          nextFaturaDate={summary?.next_fatura_date ?? null}
          nextFaturaAmountCents={summary?.next_fatura_amount_cents ?? 0}
          onCompose={openCompose}
          onEditCeiling={() => navigate("teto")}
        />
        <UpcomingCard
          onSeeAll={() => navigate("lancamentos")}
          bills={billsQ.data ?? []}
        />
      </div>

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}

function CheckinCard({
  ceiling,
  ceilingSource,
  spent,
  remaining,
  today,
  lastReal,
  cardMode,
  cartaoMonthCents,
  nextFaturaDate,
  nextFaturaAmountCents,
  onCompose,
  onEditCeiling,
}: {
  ceiling: number;
  ceilingSource: "chosen" | "estimate" | "none";
  spent: number;
  remaining: number;
  today: string;
  lastReal: string | null;
  cardMode: boolean;
  cartaoMonthCents: number;
  nextFaturaDate: string | null;
  nextFaturaAmountCents: number;
  onCompose: (opts?: { mode?: "new"; type?: MovementType; date?: string }) => void;
  onEditCeiling: () => void;
}) {
  // No modo cartão o gesto-base é somar na fatura: o registro rápido nasce em "cartao".
  const [kind, setKind] = useState<MovementType>(cardMode ? "cartao" : "diario");
  const [amount, setAmount] = useState("");
  const [saving, setSaving] = useState(false);

  const pct = ceiling > 0 ? Math.min(100, Math.round((spent / ceiling) * 100)) : 0;
  const over = remaining < 0;

  function register() {
    const cents = parseBRLToCents(amount);
    if (!cents || cents <= 0 || !isTauri) return;
    setSaving(true);
    const fields = kindToFields(kind);
    createTransaction({
      txnType: fields.txnType,
      amountCents: cents,
      description: null,
      date: today,
      paymentMethod: fields.paymentMethod,
      isFixed: fields.isFixed,
      tagIds: [],
      recurrence: null,
    })
      .then(() => {
        setAmount("");
        invalidateCommands();
      })
      // eslint-disable-next-line @typescript-eslint/no-empty-function
      .catch(() => {})
      .finally(() => setSaving(false));
  }

  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <Calendar size={16} strokeWidth={1.75} className="ic" />
          Check-in de hoje
        </span>
        {!cardMode && ceiling > 0 && (
          <span
            style={{
              fontSize: 12.5,
              fontWeight: 600,
              color: over ? "var(--danger-400)" : "var(--text-muted)",
            }}
          >
            {over ? (
              <>
                <Money cents={-remaining} size="inherit" /> acima
              </>
            ) : (
              <>
                <Money cents={remaining} size="inherit" /> livre
              </>
            )}
          </span>
        )}
      </div>
      <div className="card__body">
        {cardMode ? (
          // Re-roteamento do modo cartão: o Diário zerado é legítimo por design — o dia lê as
          // faturas. O teto estipulado permanece visível como referência, nunca como régua.
          <>
            <div className="ci-top">
              <span style={{ color: "var(--text-muted)" }}>Cartão do mês</span>
              <span className="ci-spent">
                <Money cents={cartaoMonthCents} size="inherit" />
              </span>
            </div>
            <p
              style={{ margin: "0 0 10px", color: "var(--text-faint)", fontSize: 12.5 }}
            >
              {nextFaturaDate ? (
                <>
                  Próxima fatura: <Money cents={nextFaturaAmountCents} size="inherit" />{" "}
                  em {faturaDayLabel(nextFaturaDate)}.
                </>
              ) : (
                <>Nenhuma fatura à vista no horizonte.</>
              )}{" "}
              {ceilingSource === "chosen" && (
                <>
                  Teto estipulado de <Money cents={ceiling} size="inherit" /> como
                  referência.
                </>
              )}
              {ceilingSource === "estimate" && (
                <>
                  Teto de referência: <Money cents={ceiling} size="inherit" />{" "}
                  <EstimateMark term={TETO_ESTIMATE_TERM} />
                </>
              )}
            </p>
          </>
        ) : (
          <>
            <div className="ci-top">
              <span style={{ color: "var(--text-muted)" }}>Diário de hoje</span>
              <span className="ci-spent">
                <Money cents={spent} size="inherit" />
                {ceilingSource === "none" ? (
                  <span style={{ color: "var(--text-faint)", fontWeight: 400 }}>
                    {" "}
                    /{" "}
                    <button
                      type="button"
                      style={STAT_LINK_STYLE}
                      onClick={onEditCeiling}
                    >
                      Sem teto — estipular
                    </button>
                  </span>
                ) : (
                  <span style={{ color: "var(--text-faint)", fontWeight: 400 }}>
                    {" "}
                    / {fmtBRL(ceiling)}{" "}
                    {ceilingSource === "estimate" && (
                      <EstimateMark term={TETO_ESTIMATE_TERM} />
                    )}
                  </span>
                )}
              </span>
            </div>
            <div className="ci-track">
              <div
                className="ci-fill"
                style={{
                  width: `${pct}%`,
                  background: over ? "var(--danger-500)" : "var(--type-diario)",
                }}
              />
            </div>
          </>
        )}

        <div className="ci-types" role="radiogroup" aria-label="Tipo de movimento">
          {CHECKIN_TYPES.map((k) => {
            const tm = TYPE_META[k];
            const sel = kind === k;
            return (
              <button
                type="button"
                key={k}
                role="radio"
                aria-checked={sel}
                className="ci-type"
                onClick={() => setKind(k)}
                style={
                  sel
                    ? {
                        color: "var(--text-strong)",
                        background: `color-mix(in srgb, ${tm.color} 16%, transparent)`,
                      }
                    : undefined
                }
              >
                <span className="ci-type__dot" style={{ background: tm.color }} />
                {tm.name}
              </button>
            );
          })}
        </div>

        <div className="ci-row">
          <input
            className="ci-input"
            inputMode="decimal"
            placeholder="Valor de hoje (R$)"
            aria-label="Valor"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void register();
            }}
          />
          <Button variant="primary" onClick={() => void register()} disabled={saving}>
            Registrar
          </Button>
        </div>
        <div style={{ marginTop: 10 }}>
          <button
            type="button"
            className="ci-compose"
            onClick={() => onCompose({ mode: "new", type: kind, date: today })}
          >
            <SlidersHorizontal size={13} strokeWidth={1.75} />
            Compor por itens (descrever cada valor)
          </button>
        </div>
        <div className="ci-done">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {lastReal === today
            ? "Em dia. Você já lançou hoje."
            : cardMode
              ? "Comprou no cartão? Some na fatura em aberto para ela seguir fiel."
              : "Lance o gasto de hoje para manter o saldo fiel."}
        </div>
      </div>
    </section>
  );
}

function UpcomingCard({
  bills,
  onSeeAll,
}: {
  bills: { id: string; description: string; amount: number; due_date: string }[];
  onSeeAll: () => void;
}) {
  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <CalendarRange size={16} strokeWidth={1.75} className="ic" />A pagar em breve
        </span>
        <Button size="sm" variant="ghost" onClick={onSeeAll}>
          Ver tudo
        </Button>
      </div>
      <div className="card__body" style={{ paddingTop: 4 }}>
        {bills.length === 0 ? (
          <div style={{ color: "var(--text-faint)", fontSize: 13, padding: "8px 0" }}>
            Nada vencendo nos próximos dias.
          </div>
        ) : (
          bills.map((e) => {
            const d = parseInt(e.due_date.split("-")[2] ?? "0", 10);
            const mm = MES_ABBR[monthOf(e.due_date)];
            return (
              <div className="up-row" key={e.id}>
                <div className="up-when">
                  <div className="up-when__d">{d}</div>
                  <div className="up-when__m">{mm}</div>
                </div>
                <div className="up-desc">
                  <div className="up-desc__t">{e.description}</div>
                </div>
                <div className="up-amt">
                  <Money cents={-e.amount} size="inherit" />
                </div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
