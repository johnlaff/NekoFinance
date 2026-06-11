import React from "react";

const CSS = `
.nk-tile{display:flex;flex-direction:column;gap:10px;padding:16px 18px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);min-width:0;}
.nk-tile__top{display:flex;align-items:center;justify-content:space-between;gap:10px;}
.nk-tile__label{font-family:var(--font-sans);font-size:12px;font-weight:600;color:var(--text-muted);
  letter-spacing:.01em;display:flex;align-items:center;gap:7px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0;}
.nk-tile__ic{width:15px;height:15px;color:var(--text-faint);display:inline-flex;}
.nk-tile__val{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;
  font-size:var(--fs-money-lg);letter-spacing:-0.01em;color:var(--text-strong);line-height:1.05;}
.nk-tile__val .cents{color:var(--text-faint);}
.nk-tile__foot{display:flex;align-items:center;gap:8px;}
.nk-tile__delta{display:inline-flex;align-items:center;gap:4px;font-family:var(--font-money);
  font-variant-numeric:tabular-nums;font-size:12.5px;font-weight:600;}
.nk-tile__delta--up{color:var(--money-pos);}
.nk-tile__delta--down{color:var(--money-neg);}
.nk-tile__delta--flat{color:var(--text-muted);}
.nk-tile__sub{font-family:var(--font-sans);font-size:11.5px;color:var(--text-faint);}
.nk-tile__spark{display:flex;align-items:flex-end;gap:2px;height:24px;}
.nk-tile__spark span{width:4px;border-radius:1px;background:var(--primary);opacity:.55;}
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

function splitMoney(v) {
  const str = String(v);
  const dot = str.lastIndexOf(".");
  if (dot === -1) return [str, ""];
  return [str.slice(0, dot), str.slice(dot)];
}

export function MetricTile({
  label,
  value,
  icon = null,
  delta = null,
  deltaDir = "up",
  sublabel = "",
  spark = null,
  className = "",
}) {
  useCSS();
  const [whole, cents] = splitMoney(value);
  return (
    <div className={["nk-tile", className].filter(Boolean).join(" ")}>
      <div className="nk-tile__top">
        <span className="nk-tile__label">
          {icon ? <span className="nk-tile__ic">{icon}</span> : null}
          {label}
        </span>
        {spark ? (
          <span className="nk-tile__spark">
            {spark.map((h, i) => (
              <span key={i} style={{ height: `${Math.max(8, h)}%` }} />
            ))}
          </span>
        ) : null}
      </div>
      <div className="nk-tile__val">
        {whole}
        {cents ? <span className="cents">{cents}</span> : null}
      </div>
      {delta || sublabel ? (
        <div className="nk-tile__foot">
          {delta ? (
            <span className={`nk-tile__delta nk-tile__delta--${deltaDir}`}>
              {deltaDir === "up" ? "▲" : deltaDir === "down" ? "▼" : "▬"} {delta}
            </span>
          ) : null}
          {sublabel ? <span className="nk-tile__sub">{sublabel}</span> : null}
        </div>
      ) : null}
    </div>
  );
}
