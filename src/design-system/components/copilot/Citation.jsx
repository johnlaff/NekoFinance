import React from "react";

const CSS = `
.nk-cite{display:inline-flex;align-items:center;gap:5px;height:18px;padding:0 6px 0 5px;border-radius:var(--radius-xs);
  background:var(--surface-elevated);border:1px solid var(--border);font-family:var(--font-money);
  font-size:10.5px;color:var(--text-muted);vertical-align:middle;cursor:default;transition:var(--t-hover);}
.nk-cite:hover{border-color:var(--border-strong);color:var(--text);}
.nk-cite__n{display:inline-flex;align-items:center;justify-content:center;min-width:13px;height:13px;padding:0 3px;
  border-radius:3px;background:var(--primary-quiet);color:var(--primary);font-weight:700;font-size:9px;}
.nk-tool{border:1px solid var(--border);border-radius:var(--radius-sm);overflow:hidden;background:var(--bg-subtle);
  font-family:var(--font-sans);max-width:420px;}
.nk-tool__bar{display:flex;align-items:center;gap:7px;padding:7px 11px;background:var(--surface);
  border-bottom:1px solid var(--border);}
.nk-tool__badge{font-size:9px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;color:var(--primary);
  background:var(--primary-quiet);padding:2px 6px;border-radius:3px;}
.nk-tool__fn{font-family:var(--font-money);font-size:11.5px;color:var(--text);font-weight:500;}
.nk-tool__body{padding:9px 11px;display:flex;flex-direction:column;gap:5px;}
.nk-tool__line{display:flex;justify-content:space-between;gap:12px;font-size:12px;}
.nk-tool__line span:first-child{color:var(--text-muted);}
.nk-tool__line span:last-child{font-family:var(--font-money);font-variant-numeric:tabular-nums;color:var(--text);}
.nk-tool__total{border-top:1px solid var(--border);margin-top:3px;padding-top:7px;font-weight:700;}
.nk-tool__total span:last-child{color:var(--primary);font-weight:700;}
.nk-tool__src{display:flex;align-items:center;gap:6px;padding:7px 11px;border-top:1px solid var(--border);
  font-family:var(--font-money);font-size:10px;color:var(--text-faint);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-cite-css")) return;
    const s = document.createElement("style");
    s.id = "nk-cite-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function Citation({
  variant = "inline",
  index,
  source,
  fn,
  lines = [],
  total = null,
  className = "",
  ...rest
}) {
  useCSS();
  if (variant === "tool") {
    return (
      <div className={["nk-tool", className].filter(Boolean).join(" ")} {...rest}>
        <div className="nk-tool__bar">
          <span className="nk-tool__badge">calc</span>
          <span className="nk-tool__fn">{fn}</span>
        </div>
        <div className="nk-tool__body">
          {lines.map((l, i) => (
            <div className="nk-tool__line" key={i}>
              <span>{l.label}</span>
              <span>{l.value}</span>
            </div>
          ))}
          {total ? (
            <div className="nk-tool__line nk-tool__total">
              <span>{total.label}</span>
              <span>{total.value}</span>
            </div>
          ) : null}
        </div>
        {source ? (
          <div className="nk-tool__src">
            <svg
              width="11"
              height="11"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.2"
            >
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M3 9h18M9 3v18" />
            </svg>
            {source}
          </div>
        ) : null}
      </div>
    );
  }
  return (
    <span className={["nk-cite", className].filter(Boolean).join(" ")} {...rest}>
      {index != null ? <span className="nk-cite__n">{index}</span> : null}
      {source}
    </span>
  );
}
