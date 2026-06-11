import React from "react";

const CSS = `
.nk-health{display:inline-flex;align-items:center;gap:10px;padding:7px 13px 7px 9px;border-radius:var(--radius-pill);
  font-family:var(--font-sans);border:1px solid transparent;line-height:1;}
.nk-health__ring{width:24px;height:24px;flex:none;transform:rotate(-90deg);}
.nk-health__txt{display:flex;flex-direction:column;gap:2px;}
.nk-health__label{font-size:13px;font-weight:700;letter-spacing:-0.005em;}
.nk-health__sub{font-size:10.5px;font-weight:500;opacity:.8;}
.nk-health--strong{background:var(--success-tint);border-color:rgba(52,185,129,.25);color:var(--success-400);}
.nk-health--steady{background:var(--primary-quiet);border-color:rgba(63,191,143,.22);color:var(--primary);}
.nk-health--watch{background:var(--warning-tint);border-color:rgba(224,163,62,.25);color:var(--warning-400);}
.nk-health--risk{background:var(--danger-tint);border-color:rgba(224,98,91,.25);color:var(--danger-400);}
.nk-health--lg{padding:10px 18px 10px 12px;}
.nk-health--lg .nk-health__ring{width:34px;height:34px;}
.nk-health--lg .nk-health__label{font-size:16px;}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-health-css")) return;
    const s = document.createElement("style");
    s.id = "nk-health-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const LABELS = { strong: "Strong", steady: "Steady", watch: "Watch", risk: "At risk" };

export function HealthBadge({
  level = "steady",
  score = null,
  sublabel = "",
  size = "md",
  className = "",
}) {
  useCSS();
  const pct =
    score == null ? { strong: 92, steady: 74, watch: 48, risk: 24 }[level] : score;
  const r = size === "lg" ? 15 : 10;
  const c = 2 * Math.PI * r;
  const dim = size === "lg" ? 34 : 24;
  const cx = dim / 2;
  return (
    <span
      className={[
        "nk-health",
        `nk-health--${level}`,
        size === "lg" ? "nk-health--lg" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <svg className="nk-health__ring" viewBox={`0 0 ${dim} ${dim}`}>
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
        />
      </svg>
      <span className="nk-health__txt">
        <span className="nk-health__label">{LABELS[level]}</span>
        {sublabel ? <span className="nk-health__sub">{sublabel}</span> : null}
      </span>
    </span>
  );
}
