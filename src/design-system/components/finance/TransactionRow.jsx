import React from "react";

const CSS = `
.nk-txn{display:grid;grid-template-columns:84px minmax(0,1fr) auto auto 132px;align-items:center;gap:14px;
  padding:0 14px;height:var(--row-h-default);border-bottom:1px solid var(--border);font-family:var(--font-sans);
  cursor:default;transition:background var(--dur-fast) var(--ease-standard);}
.nk-txn:hover{background:var(--surface-hover);}
.nk-txn--selected{background:var(--surface-selected);box-shadow:inset 2px 0 0 var(--primary);}
.nk-txn--flag{box-shadow:inset 2px 0 0 var(--warning-500);}
.nk-txn__date{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:12px;color:var(--text-faint);}
.nk-txn__main{min-width:0;display:flex;flex-direction:column;gap:2px;}
.nk-txn__merchant{font-size:13.5px;font-weight:600;color:var(--text);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;}
.nk-txn__cat{display:inline-flex;align-items:center;gap:6px;font-size:11.5px;color:var(--text-muted);}
.nk-txn__catdot{width:7px;height:7px;border-radius:2px;flex:none;}
.nk-txn__owner{display:flex;justify-content:flex-end;}
.nk-txn__status{display:flex;align-items:center;gap:6px;font-size:11px;font-weight:600;justify-content:flex-end;min-width:96px;}
.nk-txn__dot{width:7px;height:7px;border-radius:50%;flex:none;}
.nk-txn__conf{display:inline-flex;gap:2px;align-items:center;}
.nk-txn__conf i{width:3px;border-radius:1px;background:currentColor;display:inline-block;}
.nk-txn__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:14px;font-weight:600;text-align:right;}
.nk-txn__amt--pos{color:var(--money-pos);}
.nk-txn__amt--neg{color:var(--text);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-txn-css")) return;
    const s = document.createElement("style");
    s.id = "nk-txn-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const STATUS = {
  reconciled: { c: "var(--success-500)", t: "var(--success-400)", label: "Reconciled" },
  imported: { c: "var(--info-500)", t: "var(--info-400)", label: "Imported" },
  "needs-owner": {
    c: "var(--warning-500)",
    t: "var(--warning-400)",
    label: "Needs owner",
  },
};

export function TransactionRow({
  date,
  merchant,
  category,
  categoryColor = "var(--chart-3)",
  owner = null,
  amount,
  positive = false,
  status = "reconciled",
  confidence = null,
  selected = false,
  onClick,
  className = "",
}) {
  useCSS();
  const st = STATUS[status] || STATUS.reconciled;
  const flag = status === "needs-owner";
  const bars = { high: 3, medium: 2, low: 1 }[confidence] || 0;
  return (
    <div
      className={[
        "nk-txn",
        selected ? "nk-txn--selected" : "",
        flag && !selected ? "nk-txn--flag" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      onClick={onClick}
    >
      <span className="nk-txn__date">{date}</span>
      <span className="nk-txn__main">
        <span className="nk-txn__merchant">{merchant}</span>
        {category ? (
          <span className="nk-txn__cat">
            <span className="nk-txn__catdot" style={{ background: categoryColor }} />
            {category}
          </span>
        ) : null}
      </span>
      <span className="nk-txn__owner">{owner}</span>
      <span className="nk-txn__status" style={{ color: st.t }}>
        {confidence ? (
          <span className="nk-txn__conf" title={`${confidence} confidence`}>
            {[0, 1, 2].map((i) => (
              <i
                key={i}
                style={{ height: `${6 + i * 3}px`, opacity: i < bars ? 1 : 0.25 }}
              />
            ))}
          </span>
        ) : (
          <span className="nk-txn__dot" style={{ background: st.c }} />
        )}
        {st.label}
      </span>
      <span className={`nk-txn__amt nk-txn__amt--${positive ? "pos" : "neg"}`}>
        {positive ? "+ " : ""}
        {amount}
      </span>
    </div>
  );
}
