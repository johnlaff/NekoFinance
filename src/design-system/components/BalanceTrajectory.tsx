import { useRef, useState } from "react";
import type { ForecastDay } from "../../lib/api";
import { fmtDayMonth } from "../../lib/format";
import { formatBRL } from "./Money";

/** R$ compacto para rótulos de gráfico: "R$ 5.8k", "−R$ 320". Minus tipográfico (U+2212). */
export function fmtCompactBRL(cents: number): string {
  const v = cents / 100;
  const abs = Math.abs(v);
  const sign = v < 0 ? "−" : "";
  if (abs >= 1000) return `${sign}R$ ${(abs / 1000).toFixed(abs >= 10000 ? 0 : 1)}k`;
  return `${sign}R$ ${abs.toFixed(0)}`;
}

/**
 * BalanceTrajectory — gráfico de área da trajetória do saldo projetado (o Sparkline do design
 * system). "Previsão primeiro": o forecast é o herói. `full` enche a tela no Horizonte; `compact`
 * embute no herói. Interativo: hover mostra crosshair + tooltip ("dia · saldo"); a linha desenha-se
 * uma vez no mount (stroke-draw, respeitando prefers-reduced-motion via CSS). Déficit nunca é só
 * cor: banda do zero + linha tracejada.
 */
export function BalanceTrajectory({
  daily,
  today,
  variant = "full",
}: {
  daily: ForecastDay[];
  today: string;
  variant?: "full" | "compact";
}) {
  const compact = variant === "compact";
  const W = 1000;
  const H = compact ? 120 : 260;
  const padX = 8;
  const padTop = compact ? 22 : 16;
  const padBottom = compact ? 22 : 28;
  const fs = compact ? 13 : 12;
  const gid = `bt-area-${variant}`;
  const wrapRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);

  const vals = daily.map((d) => d.balance_cents);
  const max = Math.max(...vals, 0);
  const min = Math.min(...vals, 0);
  const range = max - min || 1;
  const innerW = W - padX * 2;
  const innerH = H - padTop - padBottom;
  const x = (i: number) =>
    padX + (daily.length === 1 ? innerW / 2 : (i / (daily.length - 1)) * innerW);
  const y = (cents: number) => padTop + innerH - ((cents - min) / range) * innerH;
  const labelX = (i: number) => Math.max(padX + 18, Math.min(W - padX - 18, x(i)));

  const linePts = daily.map((d, i) => `${x(i)},${y(d.balance_cents)}`).join(" ");
  const areaPath = `M ${x(0)},${y(min)} L ${linePts.replace(/ /g, " L ")} L ${x(daily.length - 1)},${y(min)} Z`;
  const zeroY = y(0);
  const todayIdx = daily.findIndex((d) => d.date === today);
  const minIdx = vals.indexOf(Math.min(...vals));
  const hasDeficit = min < 0;

  const onMove = (e: React.MouseEvent) => {
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
  const hoverFrac = hover != null ? x(hover) / W : 0;

  return (
    <div
      ref={wrapRef}
      className="nk-spark"
      style={{ position: "relative" }}
      onMouseMove={onMove}
      onMouseLeave={() => setHover(null)}
    >
      <svg
        viewBox={`0 0 ${W} ${H}`}
        width="100%"
        preserveAspectRatio={compact ? "none" : "xMidYMid meet"}
        role="img"
        aria-label="Trajetória do saldo projetado ao longo do horizonte"
        style={{ display: "block", height: compact ? H : undefined }}
      >
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--primary)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.02" />
          </linearGradient>
        </defs>

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

        <path d={areaPath} fill={`url(#${gid})`} />
        <polyline
          className="nk-spark__line"
          pathLength={1}
          points={linePts}
          fill="none"
          stroke="var(--primary)"
          strokeWidth={compact ? 2 : 2.5}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {/* crosshair de hover */}
        {hovered && (
          <g aria-hidden="true">
            <line
              x1={x(hover!)}
              x2={x(hover!)}
              y1={padTop}
              y2={H - padBottom}
              stroke="var(--border-strong)"
              strokeWidth="1"
            />
            <circle
              cx={x(hover!)}
              cy={y(hovered.balance_cents)}
              r={compact ? 3.5 : 4}
              fill="var(--primary)"
              stroke="var(--surface)"
              strokeWidth="2"
            />
          </g>
        )}

        {todayIdx >= 0 && (
          <g>
            <circle
              cx={x(todayIdx)}
              cy={y(daily[todayIdx]!.balance_cents)}
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

        {minIdx >= 0 && minIdx !== todayIdx && (
          <g>
            <circle
              cx={x(minIdx)}
              cy={y(vals[minIdx]!)}
              r={compact ? 3 : 3.5}
              fill={hasDeficit ? "var(--danger-400)" : "var(--text-faint)"}
            />
            <text
              x={labelX(minIdx)}
              y={y(vals[minIdx]!) + (compact ? 16 : 18)}
              textAnchor="middle"
              fontSize={fs}
              fontWeight="600"
              fill={hasDeficit ? "var(--danger-400)" : "var(--text-muted)"}
            >
              {fmtCompactBRL(vals[minIdx]!)}
            </text>
          </g>
        )}

        {!compact && (
          <text
            x={x(0)}
            y={y(max) - 8}
            textAnchor="start"
            fontSize={fs}
            fontWeight="600"
            fill="var(--text-muted)"
          >
            {fmtCompactBRL(max)}
          </text>
        )}
      </svg>

      {/* tooltip de hover (HTML, posicionado sobre o gráfico) */}
      {hovered && (
        <div
          className="nk-spark__tip"
          role="status"
          style={{
            left: `${hoverFrac * 100}%`,
            transform: `translateX(${hoverFrac > 0.85 ? "-100%" : hoverFrac < 0.15 ? "0" : "-50%"})`,
          }}
        >
          <span className="nk-spark__tip-day">{fmtDayMonth(hovered.date)}</span>
          <span className="nk-spark__tip-val">{formatBRL(hovered.balance_cents)}</span>
        </div>
      )}
    </div>
  );
}
