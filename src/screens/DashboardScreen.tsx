import { useEffect, useRef, useState } from "react";
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
import { parseBRLToCents } from "../lib/format";
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

/** Mini-gráfico de área do saldo do mês (porte do protótipo). */
function MiniTrajectory({ daily, today }: { daily: ForecastDay[]; today: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [w, setW] = useState(340);
  useEffect(() => {
    if (!ref.current || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setW(Math.max(120, e.contentRect.width));
    });
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

export function DashboardScreen() {
  const { openCompose, navigate } = useNekoApp();
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const billsQ = useCommand("get_upcoming_bills", () => getUpcomingBills(45));

  const summary = summaryQ.data;
  const forecast = forecastQ.data;
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

  return (
    <div className="hoje neko-app">
      <section className="hoje-hero">
        <div>
          <p className="hoje-hero__eyebrow">{eyebrowDate(today)}</p>
          <p className="hoje-hero__label">Pode gastar hoje</p>
          <p className="hoje-hero__kpi">
            {fmtBRL(safeToSpend)} <small>sem furar o teto</small>
          </p>
          <p className="hoje-hero__reason">
            É o menor de dois limites: o teto diário de {fmtBRL(ceiling)} e o que o
            caixa aguenta sem nenhum dia no vermelho até o fim do mês.
          </p>
          <dl className="hoje-hero__stats">
            <div>
              <dt>Saldo hoje</dt>
              <dd>{fmtBRL(saldoHoje)}</dd>
            </div>
            <div>
              <dt>Reserva</dt>
              <dd>{reserve.toFixed(1)} meses</dd>
            </div>
            <div>
              <dt>Teto diário</dt>
              <dd>{fmtBRL(ceiling)}</dd>
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
            {fmtBRL(endBalance)}
          </div>
          <MiniTrajectory daily={monthDaily} today={today} />
          <p className="hoje-fc__foot">
            {minSaldo < 0
              ? `Atenção: chega a ${fmtBRL(minSaldo)} no pior dia.`
              : `Menor saldo previsto no mês: ${fmtBRL(minSaldo)}.`}
          </p>
        </aside>
      </section>

      <div className="hoje-grid">
        <CheckinCard
          ceiling={ceiling}
          spent={spent}
          remaining={ceiling - spent}
          today={today}
          lastReal={summary?.last_real_tx_date ?? null}
          onCompose={openCompose}
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
  spent,
  remaining,
  today,
  lastReal,
  onCompose,
}: {
  ceiling: number;
  spent: number;
  remaining: number;
  today: string;
  lastReal: string | null;
  onCompose: (opts?: { mode?: "new"; type?: MovementType; date?: string }) => void;
}) {
  const [kind, setKind] = useState<MovementType>("diario");
  const [amount, setAmount] = useState("");
  const [saving, setSaving] = useState(false);

  const pct = ceiling > 0 ? Math.min(100, Math.round((spent / ceiling) * 100)) : 0;
  const over = remaining < 0;

  function register() {
    const cents = parseBRLToCents(amount);
    if (!cents || cents <= 0 || !isTauri) return;
    setSaving(true);
    createTransaction({
      txnType: "expense",
      amountCents: cents,
      description: null,
      date: today,
      paymentMethod: kind === "cartao" ? "credito" : null,
      isFixed: kind === "saida",
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
        <span
          style={{
            fontSize: 12.5,
            fontWeight: 600,
            color: over ? "var(--danger-400)" : "var(--text-muted)",
          }}
        >
          {over ? `${fmtBRL(-remaining)} acima` : `${fmtBRL(remaining)} livre`}
        </span>
      </div>
      <div className="card__body">
        <div className="ci-top">
          <span style={{ color: "var(--text-muted)" }}>Diário de hoje</span>
          <span className="ci-spent">
            {fmtBRL(spent)}
            <span style={{ color: "var(--text-faint)", fontWeight: 400 }}>
              {" "}
              / {fmtBRL(ceiling)}
            </span>
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
                <div className="up-amt">−{fmtBRL(e.amount)}</div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
