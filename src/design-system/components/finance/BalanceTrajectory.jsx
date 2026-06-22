import React from "react";

const CSS = `
.nk-btraj{position:relative;width:100%;}
.nk-btraj svg{display:block;}
.nk-btraj__line{
  stroke-dasharray:1;
  stroke-dashoffset:1;
  animation:nk-btraj-draw var(--dur-deliberate,480ms) var(--ease-entrance,cubic-bezier(0.16,1,0.3,1)) forwards;
}
@keyframes nk-btraj-draw{to{stroke-dashoffset:0;}}
@media (prefers-reduced-motion:reduce){
  .nk-btraj__line{animation:none;stroke-dasharray:none;stroke-dashoffset:0;}
}
.nk-btraj__tip{
  position:absolute;
  top:4px;
  pointer-events:none;
  background:var(--surface-elevated);
  border:1px solid var(--border-strong);
  border-radius:var(--radius-sm);
  padding:5px 9px;
  display:flex;
  flex-direction:column;
  gap:1px;
  box-shadow:var(--shadow-2);
  white-space:nowrap;
  z-index:10;
}
.nk-btraj__tip-day{
  font-family:var(--font-sans);
  font-size:11px;
  font-weight:600;
  color:var(--text-muted);
  letter-spacing:var(--ls-label);
}
.nk-btraj__tip-val{
  font-family:var(--font-money);
  font-variant-numeric:tabular-nums;
  font-size:13px;
  font-weight:700;
  color:var(--text-strong);
}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-btraj-css")) return;
    const s = document.createElement("style");
    s.id = "nk-btraj-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

/* ── helpers ──────────────────────────────────────────────────────────────── */

function fmtBRL(cents) {
  const abs = Math.abs(cents);
  const sign = cents < 0 ? "-" : "";
  const reais = Math.floor(abs / 100);
  const centavos = String(abs % 100).padStart(2, "0");
  const formatted = reais.toLocaleString("pt-BR");
  return `${sign}R$ ${formatted},${centavos}`;
}

function fmtCompact(cents) {
  const abs = Math.abs(cents);
  const sign = cents < 0 ? "-" : "";
  if (abs >= 100_000_00) return `${sign}R$ ${(abs / 100_000_00).toFixed(1)}M`;
  if (abs >= 1_000_00) return `${sign}R$ ${(abs / 1_000_00).toFixed(1)}mil`;
  return fmtBRL(cents);
}

function fmtDayMonth(dateStr) {
  if (!dateStr) return "";
  const [, m, d] = dateStr.split("-");
  const months = [
    "jan",
    "fev",
    "mar",
    "abr",
    "mai",
    "jun",
    "jul",
    "ago",
    "set",
    "out",
    "nov",
    "dez",
  ];
  return `${parseInt(d, 10)} ${months[parseInt(m, 10) - 1] || ""}`;
}

/* ── demo data ────────────────────────────────────────────────────────────── */

function buildDemo() {
  const today = new Date();
  const pad = (n) => String(n).padStart(2, "0");
  const fmt = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const days = [];
  let bal = 820000; // R$ 8 200,00 in cents
  for (let i = 0; i < 30; i++) {
    const d = new Date(today);
    d.setDate(today.getDate() + i);
    // gentle decline with a dip
    bal -= Math.round(Math.random() * 3000 + 1000);
    if (i === 18) bal -= 50000; // big expense spike
    days.push({ date: fmt(d), balance_cents: bal });
  }
  return { daily: days, today: fmt(today) };
}

const DEMO = buildDemo();

/* ── component ────────────────────────────────────────────────────────────── */

export function BalanceTrajectory({
  daily = DEMO.daily,
  today = DEMO.today,
  variant = "full",
}) {
  useCSS();

  const compact = variant === "compact";
  const W = 1000;
  const H = compact ? 120 : 260;
  const padX = 8;
  const padTop = compact ? 22 : 16;
  const padBottom = compact ? 22 : 28;
  const fs = compact ? 13 : 12;
  const gid = `bt-area-${variant}`;

  const wrapRef = React.useRef(null);
  const [hover, setHover] = React.useState(null);

  const vals = daily.map((d) => d.balance_cents);
  const maxVal = Math.max(...vals, 0);
  const minVal = Math.min(...vals, 0);
  const range = maxVal - minVal || 1;
  const innerW = W - padX * 2;
  const innerH = H - padTop - padBottom;

  const xOf = (i) =>
    padX + (daily.length === 1 ? innerW / 2 : (i / (daily.length - 1)) * innerW);
  const yOf = (cents) => padTop + innerH - ((cents - minVal) / range) * innerH;
  const labelX = (i) => Math.max(padX + 18, Math.min(W - padX - 18, xOf(i)));

  const linePts = daily.map((d, i) => `${xOf(i)},${yOf(d.balance_cents)}`).join(" ");
  const areaPath = `M ${xOf(0)},${yOf(minVal)} L ${linePts.replace(/ /g, " L ")} L ${xOf(daily.length - 1)},${yOf(minVal)} Z`;
  const zeroY = yOf(0);
  const todayIdx = daily.findIndex((d) => d.date === today);
  const minIdx = vals.indexOf(Math.min(...vals));
  const hasDeficit = minVal < 0;

  // Accessible summary for screen readers
  const todayBal = todayIdx >= 0 ? daily[todayIdx] : null;
  const minDay = daily[minIdx];
  const lastDay = daily[daily.length - 1];
  const ariaSummary = daily.length
    ? [
        "Trajetória do saldo projetado.",
        todayBal ? `Hoje: ${fmtBRL(todayBal.balance_cents)}.` : "",
        minDay
          ? `Menor saldo: ${fmtBRL(minDay.balance_cents)} em ${fmtDayMonth(minDay.date)}${hasDeficit ? " (fica negativo)" : ""}.`
          : "",
        lastDay ? `Fim do horizonte: ${fmtBRL(lastDay.balance_cents)}.` : "",
      ]
        .filter(Boolean)
        .join(" ")
    : "Sem dados de saldo projetado.";

  const onMove = (e) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return;
    const frac = (e.clientX - rect.left) / rect.width;
    const i = Math.max(
      0,
      Math.min(daily.length - 1, Math.round(frac * (daily.length - 1))),
    );
    setHover(i);
  };

  const hovered = hover != null ? daily[hover] : null;
  const hoverFrac = hover != null ? xOf(hover) / W : 0;

  return (
    <div
      ref={wrapRef}
      className="nk-btraj"
      onMouseMove={onMove}
      onMouseLeave={() => setHover(null)}
    >
      <svg
        viewBox={`0 0 ${W} ${H}`}
        width="100%"
        preserveAspectRatio={compact ? "none" : "xMidYMid meet"}
        role="img"
        aria-label={ariaSummary}
        style={{ display: "block", height: compact ? H : undefined }}
      >
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--primary)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.02" />
          </linearGradient>
        </defs>

        {/* Deficit band: dashed zero line + label */}
        {hasDeficit && (
          <>
            <line
              x1={padX}
              x2={W - padX}
              y1={zeroY}
              y2={zeroY}
              stroke="var(--danger-400)"
              strokeWidth="1"
              strokeDasharray="3 4"
              opacity="0.7"
            />
            {!compact && (
              <text
                x={W - padX}
                y={zeroY - 5}
                textAnchor="end"
                fontSize={fs}
                fill="var(--danger-400)"
              >
                R$ 0
              </text>
            )}
          </>
        )}

        {/* Area fill */}
        <path d={areaPath} fill={`url(#${gid})`} />

        {/* Trajectory line with stroke-draw animation */}
        <polyline
          className="nk-btraj__line"
          pathLength={1}
          points={linePts}
          fill="none"
          stroke="var(--primary)"
          strokeWidth={compact ? 2 : 2.5}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {/* Crosshair on hover */}
        {hovered && (
          <g aria-hidden="true">
            <line
              x1={xOf(hover)}
              x2={xOf(hover)}
              y1={padTop}
              y2={H - padBottom}
              stroke="var(--border-strong)"
              strokeWidth="1"
            />
            <circle
              cx={xOf(hover)}
              cy={yOf(hovered.balance_cents)}
              r={compact ? 3.5 : 4}
              fill="var(--primary)"
              stroke="var(--surface)"
              strokeWidth="2"
            />
          </g>
        )}

        {/* Today marker */}
        {todayIdx >= 0 && (
          <g>
            <circle
              cx={xOf(todayIdx)}
              cy={yOf(daily[todayIdx].balance_cents)}
              r={compact ? 3.5 : 4}
              fill="var(--primary)"
              stroke="var(--surface)"
              strokeWidth="2"
            />
            {!compact && (
              <text
                x={labelX(todayIdx)}
                y={H - 9}
                textAnchor="middle"
                fontSize={fs}
                fontWeight="600"
                fill="var(--text-muted)"
              >
                hoje
              </text>
            )}
          </g>
        )}

        {/* Min balance marker (not shown if same day as today) */}
        {minIdx >= 0 && minIdx !== todayIdx && (
          <g>
            <circle
              cx={xOf(minIdx)}
              cy={yOf(vals[minIdx])}
              r={compact ? 3 : 3.5}
              fill={hasDeficit ? "var(--danger-400)" : "var(--text-faint)"}
            />
            <text
              x={labelX(minIdx)}
              y={yOf(vals[minIdx]) + (compact ? 16 : 18)}
              textAnchor="middle"
              fontSize={fs}
              fontWeight="600"
              fill={hasDeficit ? "var(--danger-400)" : "var(--text-muted)"}
            >
              {fmtCompact(vals[minIdx])}
            </text>
          </g>
        )}

        {/* Max balance label (full variant only) */}
        {!compact && (
          <text
            x={xOf(0)}
            y={yOf(maxVal) - 8}
            textAnchor="start"
            fontSize={fs}
            fontWeight="600"
            fill="var(--text-muted)"
          >
            {fmtCompact(maxVal)}
          </text>
        )}
      </svg>

      {/* Hover tooltip (HTML overlay, aria-hidden) */}
      {hovered && (
        <div
          className="nk-btraj__tip"
          aria-hidden="true"
          style={{
            left: `${hoverFrac * 100}%`,
            transform: `translateX(${
              hoverFrac > 0.85 ? "-100%" : hoverFrac < 0.15 ? "0" : "-50%"
            })`,
          }}
        >
          <span className="nk-btraj__tip-day">{fmtDayMonth(hovered.date)}</span>
          <span className="nk-btraj__tip-val">{fmtBRL(hovered.balance_cents)}</span>
        </div>
      )}
    </div>
  );
}
