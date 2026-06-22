import React from "react";

const CSS = `
.nk-tile{
  display:flex;flex-direction:column;gap:var(--space-2);
  padding:var(--space-6);
  background:var(--surface);
  border:var(--bw-hair) solid var(--border);
  border-radius:var(--radius-md);
  box-shadow:var(--elev-card);
  min-width:0;
}
.nk-tile__header{display:flex;align-items:center;justify-content:space-between;}
.nk-tile__label{
  font-family:var(--font-sans);
  font-size:var(--fs-label);
  font-weight:var(--fw-medium);
  color:var(--text-faint);
  letter-spacing:var(--ls-label);
  text-transform:uppercase;
  margin:0;
}
.nk-tile__icon{color:var(--text-faint);flex:none;display:inline-flex;}
.nk-tile__val{
  font-family:var(--font-money);
  font-size:var(--fs-money-xl);
  font-variant-numeric:tabular-nums;
  font-weight:var(--fw-semibold);
  line-height:var(--lh-tight);
  color:var(--text);
  margin:0;
}
.nk-tile__foot{
  display:flex;align-items:center;gap:var(--space-3);
  margin-top:var(--space-1);
}
.nk-tile__delta{
  display:inline-flex;align-items:center;gap:4px;
  font-size:var(--fs-sm);
  font-weight:var(--fw-semibold);
}
.nk-tile__delta--up{color:var(--money-pos);}
.nk-tile__delta--down{color:var(--money-neg);}
.nk-tile__delta--neutral{color:var(--text-muted);}
.nk-tile__sub{font-size:var(--fs-sm);color:var(--text-muted);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-tile-css")) return;
    const s = document.createElement("style");
    s.id = "nk-tile-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

/* Inline SVG icons (24×24, strokeWidth 1.75, round caps) */
function IconTrendingUp() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polyline points="23 6 13.5 15.5 8.5 10.5 1 18" />
      <polyline points="17 6 23 6 23 12" />
    </svg>
  );
}

function IconTrendingDown() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polyline points="23 18 13.5 8.5 8.5 13.5 1 6" />
      <polyline points="17 18 23 18 23 12" />
    </svg>
  );
}

function IconMinus() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

export function MetricTile({
  label = "Saldo do mês",
  value = "R$ 4.820,00",
  icon = null,
  delta = null,
  deltaDir = "neutral",
  sublabel = "",
  spark = null,
  className = "",
}) {
  useCSS();

  const deltaColor =
    deltaDir === "up"
      ? "var(--money-pos)"
      : deltaDir === "down"
        ? "var(--money-neg)"
        : "var(--text-muted)";

  const sparkPoints =
    spark && spark.length > 0
      ? spark
          .map((v, i) => {
            const max = Math.max(...spark);
            const min = Math.min(...spark);
            const range = max - min || 1;
            const x = i * 6 + 3;
            const y = 26 - ((v - min) / range) * 22;
            return `${x},${y}`;
          })
          .join(" ")
      : null;

  return (
    <article
      className={["nk-tile", className].filter(Boolean).join(" ")}
      aria-label={label}
    >
      <div className="nk-tile__header">
        <p className="nk-tile__label">{label}</p>
        {icon ? <span className="nk-tile__icon">{icon}</span> : null}
      </div>

      <p className="nk-tile__val">{value}</p>

      {delta || sublabel ? (
        <div className="nk-tile__foot">
          {delta ? (
            <span
              className={`nk-tile__delta nk-tile__delta--${deltaDir}`}
              style={{ color: deltaColor }}
            >
              {deltaDir === "up" ? (
                <IconTrendingUp />
              ) : deltaDir === "down" ? (
                <IconTrendingDown />
              ) : (
                <IconMinus />
              )}
              {delta}
            </span>
          ) : null}
          {sublabel ? <span className="nk-tile__sub">{sublabel}</span> : null}
        </div>
      ) : null}

      {sparkPoints ? (
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
            points={sparkPoints}
          />
        </svg>
      ) : null}
    </article>
  );
}
