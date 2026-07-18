import type { CSSProperties } from "react";
import { Banknote, CreditCard, TriangleAlert } from "lucide-react";
import { InfoPopover } from "./InfoPopover";

// Contexto do modo de gasto (débito × cartão), detectado dos próprios dados — didática, não
// configuração. No modo cartão o Diário zerado é zero-legítimo por design e o dia lê as
// faturas; o popover explica a detecção. O gate do método (economia 20–30% viva) entra como
// atenção com palavra+ícone (cores de STATUS do método, nunca o acento).
const CHIP_STYLE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 6,
  padding: "2px 9px",
  borderRadius: 999,
  border: "1px solid var(--border)",
  color: "var(--text-muted)",
  fontSize: "var(--fs-micro)",
  fontWeight: 500,
  lineHeight: 1.7,
  whiteSpace: "nowrap",
};

const MODE_WORD_STYLE: CSSProperties = {
  // A didática da detecção mora no popover; o pontilhado é o convite para abri-la.
  textDecoration: "underline dotted",
  textUnderlineOffset: 3,
};

const GATE_STYLE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  color: "var(--warning-400)",
};

export type SpendingModeKind = "debit" | "card";
export type CardGate = "alive" | "below" | "unknown";

const MODE_LABEL: Record<SpendingModeKind, string> = {
  debit: "Modo débito",
  card: "Modo cartão",
};

const MODE_BODY: Record<SpendingModeKind, string> = {
  debit:
    "Detectado dos seus dados: o gasto variável vive no débito/Pix e mexe o saldo na hora. O dia compara o Diário com o teto.",
  card: "Detectado dos seus dados: o Diário está sem constância e as faturas seguem vivas — o gasto do dia a dia vive no cartão. O dia lê as faturas; o Diário zerado aqui é legítimo, não lacuna.",
};

const GATE_BODY =
  " O método só considera o cartão legítimo com a economia de 20–30% viva; a sua está abaixo do piso de 20%. O caminho de volta: registrar economia todo mês — acompanhe o Economizado% na tela O ano.";

export interface ModeChipProps {
  mode: SpendingModeKind;
  /** Gate de legitimidade do modo cartão (economia 20–30% viva). Ignorado no modo débito. */
  gate?: CardGate;
  className?: string;
}

export function ModeChip({ mode, gate = "unknown", className }: ModeChipProps) {
  const gateBelow = mode === "card" && gate === "below";
  const body = MODE_BODY[mode] + (gateBelow ? GATE_BODY : "");
  const Icon = mode === "card" ? CreditCard : Banknote;
  return (
    <InfoPopover
      term={{ title: MODE_LABEL[mode], body }}
      hideMarker
      {...(className ? { className } : {})}
    >
      <span style={CHIP_STYLE}>
        <Icon size={12} strokeWidth={1.75} aria-hidden="true" />
        <span style={MODE_WORD_STYLE}>{MODE_LABEL[mode]}</span>
        {gateBelow && (
          <span style={GATE_STYLE}>
            <TriangleAlert size={12} strokeWidth={1.75} aria-hidden="true" />
            Economia abaixo do piso
          </span>
        )}
      </span>
    </InfoPopover>
  );
}
