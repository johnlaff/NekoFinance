import { useState, type KeyboardEvent, type ReactNode } from "react";
import { formatBRL } from "../../lib/format";

/**
 * TransactionRow — linha de lançamento fiel ao método: data, descrição, método, valor, procedência,
 * titular e nota. Quando o lançamento é um lump de fatura (Saída agregada), expande os ITENS da nota
 * da célula — a preservação de notas do método (cada item sobrevive, nunca vira um "Saída" genérico).
 * Portado do novo DS em inline-style; mantém o único hook necessário (expandir/fechar o lump).
 */
export type Provenance = "importado" | "manual" | "projetado" | "conciliado";

const PROV: Record<Provenance, { label: string; color: string }> = {
  importado: { label: "Da planilha", color: "var(--prov-imported)" },
  manual: { label: "Do app", color: "var(--prov-app)" },
  projetado: { label: "Previsto", color: "var(--prov-projected)" },
  conciliado: { label: "Conferido", color: "var(--prov-reconciled)" },
};

export interface LumpItem {
  what: string;
  amount: number;
  owner?: ReactNode;
  passthrough?: boolean;
}

interface TransactionRowProps {
  date: string;
  desc: string;
  amount: number;
  method?: string;
  provenance?: Provenance;
  owner?: ReactNode;
  note?: string;
  passthrough?: boolean;
  future?: boolean;
  lump?: LumpItem[];
  defaultOpen?: boolean;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

function ProvBadge({ provenance }: { provenance: Provenance | undefined }) {
  if (!provenance) return null;
  const g = PROV[provenance];
  if (!g) return null;
  return (
    <span
      title={g.label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        fontSize: "var(--fs-micro)",
        fontWeight: "var(--fw-semibold)",
        color: "var(--text-muted)",
        whiteSpace: "nowrap",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          flex: "none",
          background: g.color,
        }}
      />
      {g.label}
    </span>
  );
}

const moneyStyle = (amount: number): React.CSSProperties => ({
  fontFamily: "var(--font-money)",
  fontVariantNumeric: "tabular-nums",
  fontWeight: "var(--fw-semibold)",
  fontSize: "var(--fs-money-sm)",
  textAlign: "right",
  whiteSpace: "nowrap",
  color: amount > 0 ? "var(--money-pos)" : "var(--text)",
});

function lumpItemKey(item: LumpItem): string {
  return `${item.what}:${item.amount}:${item.passthrough ? "repasse" : "normal"}`;
}

// Base estática do botão de expandir o lump (não recria por render); só o `transform` é dinâmico.
const LUMP_TOGGLE_BASE: React.CSSProperties = {
  width: 18,
  height: 18,
  display: "grid",
  placeItems: "center",
  border: "none",
  background: "transparent",
  color: "var(--text-faint)",
  borderRadius: "4px",
  cursor: "pointer",
  flexShrink: 0,
  transition: "transform var(--dur-fast) var(--ease-standard)",
};

const PASSTHROUGH_BADGE_STYLE: React.CSSProperties = {
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-bold)",
  textTransform: "uppercase",
  letterSpacing: "0.04em",
  color: "var(--info-400)",
  background: "var(--info-tint)",
  padding: "1px 6px",
  borderRadius: "4px",
  whiteSpace: "nowrap",
};

export function TransactionRow({
  date,
  desc,
  amount,
  method,
  provenance,
  owner,
  note,
  passthrough = false,
  future = false,
  lump,
  defaultOpen = false,
  selected = false,
  onClick,
  className = "",
}: TransactionRowProps) {
  const [open, setOpen] = useState(defaultOpen);
  const hasLump = Array.isArray(lump) && lump.length > 0;
  const toggleStyle: React.CSSProperties = {
    ...LUMP_TOGGLE_BASE,
    transform: open ? "rotate(90deg)" : "none",
  };
  const rowInteractionProps = onClick
    ? {
        onClick,
        onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onClick();
          }
        },
        role: "button",
        tabIndex: 0,
      }
    : {};

  return (
    <div
      className={className}
      style={{
        borderBottom: "var(--bw-hair) solid var(--border)",
        fontFamily: "var(--font-sans)",
        background: selected
          ? "var(--surface-selected)"
          : future
            ? "repeating-linear-gradient(135deg, transparent, transparent 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 18px)"
            : "transparent",
        boxShadow: "none",
      }}
    >
      <div
        {...rowInteractionProps}
        style={{
          display: "grid",
          gridTemplateColumns: "58px 1fr auto auto",
          alignItems: "center",
          gap: "14px",
          padding: "12px 18px",
        }}
      >
        <span
          style={{
            fontSize: "var(--fs-sm)",
            color: "var(--text-faint)",
            fontFamily: "var(--font-money)",
            whiteSpace: "nowrap",
          }}
        >
          {date}
        </span>
        <div
          style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: "4px" }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            {hasLump ? (
              <button
                type="button"
                aria-expanded={open}
                aria-label={open ? "Fechar itens" : "Abrir itens"}
                onClick={(e) => {
                  e.stopPropagation();
                  setOpen((o) => !o);
                }}
                style={toggleStyle}
              >
                ›
              </button>
            ) : (
              <span style={{ width: 18, flexShrink: 0 }} />
            )}
            <span
              style={{
                fontSize: "var(--fs-body)",
                color: "var(--text)",
                overflowWrap: "anywhere",
              }}
            >
              {desc}
            </span>
            {passthrough ? <span style={PASSTHROUGH_BADGE_STYLE}>Repasse</span> : null}
          </div>
          {provenance || owner || note ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                flexWrap: "wrap",
                paddingLeft: 26,
              }}
            >
              <ProvBadge provenance={provenance} />
              {owner}
              {note ? (
                <span
                  style={{
                    fontSize: "var(--fs-micro)",
                    color: "var(--text-faint)",
                    fontStyle: "italic",
                  }}
                >
                  {`“${note}”`}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
        {method ? (
          <span
            style={{
              fontSize: "var(--fs-micro)",
              color: "var(--text-muted)",
              padding: "3px 9px",
              border: "var(--bw-hair) solid var(--border)",
              borderRadius: "var(--radius-pill)",
              whiteSpace: "nowrap",
            }}
          >
            {method}
          </span>
        ) : null}
        <span style={{ ...moneyStyle(amount), opacity: passthrough ? 0.55 : 1 }}>
          {formatBRL(amount)}
        </span>
      </div>
      {hasLump && open ? (
        <div
          style={{
            padding: "4px 18px 14px 76px",
            background: "var(--bg-subtle)",
            borderTop: "1px dashed var(--border)",
          }}
        >
          {lump.map((it) => (
            <div
              key={lumpItemKey(it)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                padding: "7px 0",
                borderBottom: "var(--bw-hair) solid var(--border)",
                fontSize: "var(--fs-sm)",
              }}
            >
              <span
                style={{ color: "var(--text-faint)", fontFamily: "var(--font-money)" }}
              >
                ↳
              </span>
              <span
                style={{
                  flex: 1,
                  color: "var(--text-muted)",
                  minWidth: 0,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {it.what}
              </span>
              {it.owner}
              <span style={moneyStyle(it.amount)}>{formatBRL(it.amount)}</span>
            </div>
          ))}
          <p
            style={{
              margin: "10px 0 0",
              fontSize: "var(--fs-micro)",
              color: "var(--text-faint)",
            }}
          >
            Esse detalhe vem das notas da célula da planilha. Cada item é preservado;
            nunca vira um “Saída” genérico.
          </p>
        </div>
      ) : null}
    </div>
  );
}
