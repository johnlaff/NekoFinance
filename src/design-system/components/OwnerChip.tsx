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
  /** Papel secundário, ex.: "paga". (Evita o nome `role`, que colide com o atributo ARIA.) */
  note?: string;
  /** Sem fundo/borda (só ponto + nome inline). */
  bare?: boolean;
  /** Mostra um avatar circular com as iniciais no lugar do ponto (padrão DS). */
  avatar?: boolean;
  className?: string;
}

/** Iniciais (até 2) para o monograma do avatar. */
function initials(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .map((w) => w[0] ?? "")
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

export function OwnerChip({
  who = "personal",
  name,
  note,
  bare = false,
  avatar = false,
  className = "",
}: OwnerChipProps) {
  const o = OWNERS[who];
  const label = name ?? o.label;
  return (
    <span
      className={className}
      title={note ? `${label} · ${note}` : label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: avatar ? "7px" : "6px",
        height: avatar ? 26 : 22,
        padding: bare ? 0 : avatar ? "0 10px 0 3px" : "0 9px 0 7px",
        borderRadius: "var(--radius-pill)",
        border: bare ? "none" : "var(--bw-hair) solid var(--border)",
        background: bare ? "none" : "var(--surface-2)",
        fontSize: "var(--fs-micro)",
        color: "var(--text-muted)",
        whiteSpace: "nowrap",
        fontFamily: "var(--font-sans)",
      }}
    >
      {avatar ? (
        <span
          aria-hidden="true"
          style={{
            width: 20,
            height: 20,
            borderRadius: "50%",
            flex: "none",
            display: "inline-grid",
            placeItems: "center",
            background: "var(--surface-elevated)",
            border: `var(--bw-strong) solid ${o.color}`,
            color: "var(--text)",
            fontSize: "10px",
            fontWeight: "var(--fw-bold)",
          }}
        >
          {initials(label)}
        </span>
      ) : (
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
      )}
      {label}
      {note ? <span style={{ color: "var(--text-faint)" }}>{note}</span> : null}
    </span>
  );
}
