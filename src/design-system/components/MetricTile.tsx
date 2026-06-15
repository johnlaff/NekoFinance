import type { ReactNode } from "react";
import { Minus, TrendingDown, TrendingUp } from "lucide-react";

interface MetricTileProps {
  label: string;
  value: string;
  icon?: ReactNode;
  delta?: string;
  deltaDir?: "up" | "down" | "neutral";
  sublabel?: string;
  spark?: number[];
  className?: string;
}

export function MetricTile({
  label,
  value,
  icon,
  delta,
  deltaDir = "neutral",
  sublabel,
  spark,
  className = "",
}: MetricTileProps) {
  const deltaColor =
    deltaDir === "up"
      ? "var(--money-pos)"
      : deltaDir === "down"
        ? "var(--money-neg)"
        : "var(--text-muted)";

  return (
    <article
      className={className}
      style={{
        background: "var(--surface)",
        border: "var(--bw-hair) solid var(--border)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--elev-card)",
        padding: "var(--space-6)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-2)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <p
          style={{
            color: "var(--text-faint)",
            fontSize: "var(--fs-label)",
            fontWeight: "var(--fw-medium)",
            letterSpacing: "var(--ls-label)",
            textTransform: "uppercase",
            margin: 0,
          }}
        >
          {label}
        </p>
        {icon ? (
          <span style={{ color: "var(--text-faint)", flex: "none" }}>{icon}</span>
        ) : null}
      </div>
      <p
        style={{
          fontFamily: "var(--font-money)",
          fontSize: "var(--fs-money-xl)",
          fontVariantNumeric: "tabular-nums",
          fontWeight: "var(--fw-semibold)",
          lineHeight: "var(--lh-tight)",
          color: "var(--text)",
          margin: 0,
        }}
      >
        {value}
      </p>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-3)",
          marginTop: "var(--space-1)",
        }}
      >
        {delta ? (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-semibold)",
              color: deltaColor,
            }}
          >
            {deltaDir === "up" ? (
              <TrendingUp size={13} strokeWidth={2} aria-hidden="true" />
            ) : deltaDir === "down" ? (
              <TrendingDown size={13} strokeWidth={2} aria-hidden="true" />
            ) : (
              <Minus size={13} strokeWidth={2} aria-hidden="true" />
            )}
            {delta}
          </span>
        ) : null}
        {sublabel ? (
          <span
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--text-muted)",
            }}
          >
            {sublabel}
          </span>
        ) : null}
      </div>
      {spark && spark.length > 0 ? (
        <svg
          height="28"
          width="100%"
          viewBox={`0 0 ${spark.length * 6} 28`}
          preserveAspectRatio="none"
          style={{ marginTop: "var(--space-2)" }}
        >
          <polyline
            fill="none"
            stroke="var(--primary)"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            points={spark
              .map((v, i) => {
                const max = Math.max(...spark);
                const min = Math.min(...spark);
                const range = max - min || 1;
                const x = i * 6 + 3;
                const y = 26 - ((v - min) / range) * 22;
                return `${x},${y}`;
              })
              .join(" ")}
          />
        </svg>
      ) : null}
    </article>
  );
}
