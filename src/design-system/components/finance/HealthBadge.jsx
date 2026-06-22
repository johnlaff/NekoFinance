import React from "react";

const TONE = {
  strong: {
    bg: "var(--success-tint)",
    border: "color-mix(in srgb, var(--success-400) 25%, transparent)",
    color: "var(--success-400)",
  },
  steady: {
    bg: "var(--primary-quiet)",
    border: "color-mix(in srgb, var(--primary) 22%, transparent)",
    color: "var(--primary)",
  },
  watch: {
    bg: "var(--warning-tint)",
    border: "color-mix(in srgb, var(--warning-400) 25%, transparent)",
    color: "var(--warning-400)",
  },
  risk: {
    bg: "var(--danger-tint)",
    border: "color-mix(in srgb, var(--danger-400) 25%, transparent)",
    color: "var(--danger-400)",
  },
};

const DEFAULT_LABEL = {
  strong: "Forte",
  steady: "Estável",
  watch: "Atenção",
  risk: "Em risco",
};

const DEFAULT_SCORE = { strong: 92, steady: 74, watch: 48, risk: 24 };

export function HealthBadge({
  level = "steady",
  label,
  score,
  sublabel = "",
  size = "md",
  className = "",
}) {
  const tone = TONE[level];
  const text = label ?? DEFAULT_LABEL[level];
  const pct = score ?? DEFAULT_SCORE[level];
  const dim = size === "lg" ? 34 : 24;
  const r = size === "lg" ? 15 : 10;
  const c = 2 * Math.PI * r;
  const cx = dim / 2;

  const badgeStyle = {
    display: "inline-flex",
    alignItems: "center",
    gap: "10px",
    padding: size === "lg" ? "10px 18px 10px 12px" : "7px 13px 7px 9px",
    borderRadius: "var(--radius-pill)",
    fontFamily: "var(--font-sans)",
    lineHeight: 1,
    border: `1px solid ${tone.border}`,
    background: tone.bg,
    color: tone.color,
  };

  const ringStyle = {
    flex: "none",
    transform: "rotate(-90deg)",
  };

  const progressStyle = {
    transition: "stroke-dashoffset var(--dur-slow) var(--ease-entrance)",
  };

  const labelStyle = {
    fontSize: size === "lg" ? "var(--fs-title)" : "var(--fs-sm)",
    fontWeight: "var(--fw-bold)",
    letterSpacing: "-0.005em",
  };

  const sublabelStyle = {
    fontSize: "var(--fs-micro)",
    fontWeight: "var(--fw-medium)",
    opacity: 0.8,
  };

  return (
    <span className={className} style={badgeStyle}>
      <svg
        aria-hidden="true"
        width={dim}
        height={dim}
        viewBox={`0 0 ${dim} ${dim}`}
        style={ringStyle}
      >
        <circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          opacity="0.2"
        />
        <circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - pct / 100)}
          style={progressStyle}
        />
      </svg>
      <span style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
        <span style={labelStyle}>{text}</span>
        {sublabel ? <span style={sublabelStyle}>{sublabel}</span> : null}
      </span>
    </span>
  );
}
