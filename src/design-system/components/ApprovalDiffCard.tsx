import type { ReactNode } from "react";

/**
 * ApprovalDiffCard — diff de uma mudança PROPOSTA na planilha (fluxo de write-back gated). Mostra
 * a célula/range de origem, cada campo (antes → depois) e o status (pendente/aprovado/recusado).
 * É a UI de "nenhuma escrita sem confirmação humana" — a escrita real fica atrás de flag desligada.
 * Portado do novo DS em inline-style (puro, sem hooks). Usa os tokens de diff/status do states.css.
 */
export type DiffStatus = "pending" | "approved" | "rejected";

export interface DiffChange {
  field: string;
  before?: string;
  after: string;
}

const PILL: Record<DiffStatus, { label: string; bg: string; color: string }> = {
  pending: { label: "Precisa de aprovação", bg: "var(--warning-tint)", color: "var(--warning-400)" },
  approved: { label: "Aprovado", bg: "var(--success-tint)", color: "var(--success-400)" },
  rejected: { label: "Recusado", bg: "var(--danger-tint)", color: "var(--danger-400)" },
};

interface ApprovalDiffCardProps {
  title?: string;
  sheet: string;
  range?: string;
  changes: DiffChange[];
  note?: ReactNode;
  status?: DiffStatus;
  actions?: ReactNode;
  className?: string;
}

export function ApprovalDiffCard({
  title = "Mudança proposta",
  sheet,
  range,
  changes,
  note,
  status = "pending",
  actions,
  className = "",
}: ApprovalDiffCardProps) {
  const pill = PILL[status];
  return (
    <div
      className={className}
      role="group"
      aria-label={`${title} — ${pill.label}`}
      style={{
        background: "var(--surface)",
        border: "var(--bw-hair) solid var(--border)",
        borderRadius: "var(--radius-md)",
        overflow: "hidden",
        fontFamily: "var(--font-sans)",
        boxShadow: "var(--shadow-2)",
        maxWidth: 480,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          gap: "11px",
          padding: "14px 16px",
          borderBottom: "var(--bw-hair) solid var(--border)",
        }}
      >
        <span
          aria-hidden="true"
          style={{
            width: 28,
            height: 28,
            borderRadius: "var(--radius-sm)",
            background: "var(--primary-quiet)",
            color: "var(--primary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flex: "none",
          }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M4 4h16v16H4z" />
            <path d="M4 9h16M9 9v11" />
          </svg>
        </span>
        <span style={{ flex: 1, minWidth: 0 }}>
          <span style={{ display: "block", fontSize: "14px", fontWeight: "var(--fw-bold)", color: "var(--text-strong)" }}>
            {title}
          </span>
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontSize: "11px",
              color: "var(--text-faint)",
              marginTop: 3,
              display: "flex",
              gap: "6px",
              flexWrap: "wrap",
            }}
          >
            <b style={{ color: "var(--text-muted)", fontWeight: "var(--fw-semibold)" }}>{sheet}</b>
            {range ? <span>· {range}</span> : null}
          </span>
        </span>
        <span
          style={{
            fontSize: "10px",
            fontWeight: "var(--fw-bold)",
            letterSpacing: "0.05em",
            textTransform: "uppercase",
            padding: "3px 8px",
            borderRadius: "var(--radius-pill)",
            flex: "none",
            background: pill.bg,
            color: pill.color,
          }}
        >
          {pill.label}
        </span>
      </div>

      <div style={{ padding: "6px 16px 12px" }}>
        {changes.map((c, i) => (
          <div
            key={i}
            style={{
              display: "grid",
              gridTemplateColumns: "104px 1fr",
              gap: "10px",
              padding: "8px 0",
              borderBottom:
                i === changes.length - 1 ? "none" : "1px dashed var(--border)",
              alignItems: "center",
            }}
          >
            <span style={{ fontSize: "12px", color: "var(--text-muted)", fontWeight: "var(--fw-semibold)" }}>
              {c.field}
            </span>
            <span
              style={{
                display: "flex",
                alignItems: "center",
                gap: "8px",
                flexWrap: "wrap",
                fontFamily: "var(--font-money)",
                fontVariantNumeric: "tabular-nums",
                fontSize: "13px",
              }}
            >
              {c.before != null && c.before !== "" ? (
                <span
                  style={{
                    color: "var(--diff-remove)",
                    background: "var(--diff-remove-bg)",
                    padding: "2px 7px",
                    borderRadius: "var(--radius-xs)",
                    textDecoration: "line-through",
                  }}
                >
                  {c.before}
                </span>
              ) : null}
              <span style={{ color: "var(--text-faint)" }}>→</span>
              <span
                style={{
                  color: "var(--diff-add)",
                  background: "var(--diff-add-bg)",
                  padding: "2px 7px",
                  borderRadius: "var(--radius-xs)",
                  fontWeight: "var(--fw-semibold)",
                }}
              >
                {c.after}
              </span>
            </span>
          </div>
        ))}
      </div>

      {note ? (
        <div
          style={{
            display: "flex",
            gap: "8px",
            padding: "11px 16px",
            background: "var(--bg-subtle)",
            borderTop: "var(--bw-hair) solid var(--border)",
            fontSize: "12px",
            lineHeight: 1.45,
            color: "var(--text-muted)",
          }}
        >
          {note}
        </div>
      ) : null}

      {actions ? (
        <div
          style={{
            display: "flex",
            gap: "8px",
            padding: "12px 16px",
            borderTop: "var(--bw-hair) solid var(--border)",
          }}
        >
          {actions}
        </div>
      ) : null}
    </div>
  );
}
