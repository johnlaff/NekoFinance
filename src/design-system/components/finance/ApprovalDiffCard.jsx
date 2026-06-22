import React from "react";

const CSS = `
.nk-diff{background:var(--surface);border:var(--bw-hair) solid var(--border);border-radius:var(--radius-md);
  overflow:hidden;font-family:var(--font-sans);box-shadow:var(--shadow-2);max-width:480px;}
.nk-diff__head{display:flex;align-items:flex-start;gap:11px;padding:14px 16px;border-bottom:var(--bw-hair) solid var(--border);}
.nk-diff__mark{width:28px;height:28px;border-radius:var(--radius-sm);background:var(--primary-quiet);color:var(--primary);
  display:flex;align-items:center;justify-content:center;flex:none;}
.nk-diff__htxt{flex:1;min-width:0;}
.nk-diff__title{display:block;font-size:14px;font-weight:var(--fw-bold);color:var(--text-strong);letter-spacing:-0.005em;}
.nk-diff__src{font-family:var(--font-money);font-size:var(--fs-label);color:var(--text-faint);margin-top:3px;
  display:flex;align-items:center;gap:6px;flex-wrap:wrap;}
.nk-diff__src b{color:var(--text-muted);font-weight:var(--fw-semibold);}
.nk-diff__pill{font-size:var(--fs-label);font-weight:var(--fw-bold);letter-spacing:.05em;text-transform:uppercase;padding:3px 8px;
  border-radius:var(--radius-pill);flex:none;}
.nk-diff__pill--pending{background:var(--warning-tint);color:var(--warning-400);}
.nk-diff__pill--approved{background:var(--success-tint);color:var(--success-400);}
.nk-diff__pill--rejected{background:var(--danger-tint);color:var(--danger-400);}
.nk-diff__rows{padding:6px 16px 12px;}
.nk-diff__row{display:grid;grid-template-columns:104px 1fr;gap:10px;padding:8px 0;border-bottom:1px dashed var(--border);align-items:center;}
.nk-diff__row:last-child{border-bottom:none;}
.nk-diff__field{font-size:12px;color:var(--text-muted);font-weight:var(--fw-semibold);}
.nk-diff__vals{display:flex;align-items:center;gap:8px;flex-wrap:wrap;font-family:var(--font-money);
  font-variant-numeric:tabular-nums;font-size:13px;}
.nk-diff__before{color:var(--diff-remove);background:var(--diff-remove-bg);padding:2px 7px;border-radius:var(--radius-xs);
  text-decoration:line-through;}
.nk-diff__arrow{color:var(--text-faint);}
.nk-diff__after{color:var(--diff-add);background:var(--diff-add-bg);padding:2px 7px;border-radius:var(--radius-xs);font-weight:var(--fw-semibold);}
.nk-diff__note{display:flex;gap:8px;padding:11px 16px;background:var(--bg-subtle);border-top:var(--bw-hair) solid var(--border);
  font-size:12px;line-height:1.45;color:var(--text-muted);}
.nk-diff__actions{display:flex;gap:8px;padding:12px 16px;border-top:var(--bw-hair) solid var(--border);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-diff-css")) return;
    const s = document.createElement("style");
    s.id = "nk-diff-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const PILL = {
  pending: { label: "Precisa de aprovação", cls: "nk-diff__pill--pending" },
  approved: { label: "Aprovado", cls: "nk-diff__pill--approved" },
  rejected: { label: "Recusado", cls: "nk-diff__pill--rejected" },
};

export function ApprovalDiffCard({
  title = "Mudança proposta",
  sheet = "Gastos 2025",
  range,
  changes = [
    { field: "Categoria", before: "Sem categoria", after: "Alimentação" },
    { field: "Dono", after: "Compartilhado" },
  ],
  note = null,
  status = "pending",
  actions = null,
  className = "",
}) {
  useCSS();
  const pill = PILL[status] ?? PILL.pending;
  return (
    <article
      className={["nk-diff", className].filter(Boolean).join(" ")}
      aria-label={`${title} — ${pill.label}`}
    >
      <div className="nk-diff__head">
        <span className="nk-diff__mark" aria-hidden="true">
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M4 4h16v16H4z" />
            <path d="M4 9h16M9 9v11" />
          </svg>
        </span>
        <span className="nk-diff__htxt">
          <span className="nk-diff__title">{title}</span>
          <span className="nk-diff__src">
            <b>{sheet}</b>
            {range ? <span>· {range}</span> : null}
          </span>
        </span>
        <span className={`nk-diff__pill ${pill.cls}`}>{pill.label}</span>
      </div>
      <div className="nk-diff__rows">
        {changes.map((c, i) => (
          <div
            className="nk-diff__row"
            key={`${c.field}:${c.before ?? ""}:${c.after}:${i}`}
          >
            <span className="nk-diff__field">{c.field}</span>
            <span className="nk-diff__vals">
              {c.before != null && c.before !== "" ? (
                <span className="nk-diff__before">{c.before}</span>
              ) : null}
              <span className="nk-diff__arrow">→</span>
              <span className="nk-diff__after">{c.after}</span>
            </span>
          </div>
        ))}
      </div>
      {note ? <div className="nk-diff__note">{note}</div> : null}
      {actions ? <div className="nk-diff__actions">{actions}</div> : null}
    </article>
  );
}
