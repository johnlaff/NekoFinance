/**
 * OwnerChip — quem é o titular de um lançamento/conta (multi-titular: eu / parceiro / compartilhado).
 * Ponto colorido + nome. Portado do novo DS em inline-style (puro, sem hooks). Usa os tokens
 * --owner-* já presentes no colors.css.
 */
export type OwnerWho = "personal" | "partner" | "shared";

const OWNERS: Record<OwnerWho, { label: string; color: string }> = {
  personal: { label: "Eu", color: "var(--owner-personal)" },
  partner: { label: "Parceiro(a)", color: "var(--owner-partner)" },
  shared: { label: "Compartilhado", color: "var(--owner-shared)" },
};

interface OwnerChipProps {
  who?: OwnerWho;
  /** Nome real do titular (sobrescreve o rótulo padrão). */
  name?: string;
  /** Papel secundário, ex.: "paga". */
  role?: string;
  /** Sem fundo/borda (só ponto + nome inline). */
  bare?: boolean;
  className?: string;
}

export function OwnerChip({
  who = "personal",
  name,
  role,
  bare = false,
  className = "",
}: OwnerChipProps) {
  const o = OWNERS[who];
  const label = name ?? o.label;
  return (
    <span
      className={className}
      title={role ? `${label} · ${role}` : label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        height: 22,
        padding: bare ? 0 : "0 9px 0 7px",
        borderRadius: "var(--radius-pill)",
        border: bare ? "none" : "var(--bw-hair) solid var(--border)",
        background: bare ? "none" : "var(--surface-2)",
        fontSize: "var(--fs-micro)",
        color: "var(--text-muted)",
        whiteSpace: "nowrap",
        fontFamily: "var(--font-sans)",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          flex: "none",
          background: o.color,
        }}
      />
      {label}
      {role ? <span style={{ color: "var(--text-faint)" }}>{role}</span> : null}
    </span>
  );
}
