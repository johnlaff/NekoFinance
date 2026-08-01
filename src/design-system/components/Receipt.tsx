import { useId, useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { EstimateMark } from "./EstimateMark";
import { Money } from "./Money";
import { SR_ONLY } from "../srOnly";

/**
 * Receipt — a conta impressa: cada operando numa linha, o sinal da operação na margem, o
 * resultado destacado. É a resposta do app para "de onde veio esse número".
 *
 * Onde entra, o recibo SUBSTITUI a prosa que descrevia a fórmula. Uma frase pode divergir do
 * motor; a conta impressa não pode — ela é os operandos. Regra de uso: número derivado que é
 * herói de uma superfície carrega recibo, um por tela; número que o usuário digitou não tem
 * conta a mostrar. A porta é "Ver a conta" (aritmética de hoje), distinta de "Como funciona?"
 * (o que o conceito significa no método).
 */

export interface EpistemicMark {
  kind: "estimate";
  term: { title?: string; body: string };
}

/** Operação impressa entre os operandos. */
export type ReceiptOp = "min" | "minus" | "div" | "eq";

/** Tom do método: paz, atenção, alerta. Nunca segue o acento da marca. */
export type Tone = "ok" | "warn" | "bad";

export interface ReceiptLine {
  label: string;
  /** Valor monetário (renderiza tabular); exclusivo com `text`. */
  cents?: number;
  text?: string;
  op?: ReceiptOp;
  result?: boolean;
  tone?: Tone;
  mark?: EpistemicMark;
}

const TONE_CLASS: Record<Tone, string> = {
  ok: "nk-receipt__val--ok",
  warn: "nk-receipt__val--warn",
  bad: "nk-receipt__val--bad",
};

// O glifo é visual; o falado é o que o leitor de tela ouve no lugar dele.
const OP_META: Record<ReceiptOp, { glyph: string; spoken: string }> = {
  min: { glyph: "mín", spoken: "O menor dos dois — " },
  minus: { glyph: "−", spoken: "Menos " },
  div: { glyph: "÷", spoken: "Dividido por " },
  eq: { glyph: "=", spoken: "Resultado — " },
};

function ReceiptRow({ line }: { line: ReceiptLine }) {
  const op = line.op ? OP_META[line.op] : null;
  return (
    <div
      className={"nk-receipt__row" + (line.result ? " nk-receipt__row--result" : "")}
    >
      <dt className="nk-receipt__label">
        {op ? (
          <>
            <span className="nk-receipt__op" aria-hidden="true">
              {op.glyph}
            </span>
            <span style={SR_ONLY}>{op.spoken}</span>
          </>
        ) : null}
        {line.label}
      </dt>
      <dd className={"nk-receipt__val " + (line.tone ? TONE_CLASS[line.tone] : "")}>
        {line.cents === undefined ? (
          <span className="nk-receipt__text">{line.text}</span>
        ) : (
          <Money cents={line.cents} size="inherit" />
        )}
        {line.mark ? <EstimateMark term={line.mark.term} /> : null}
      </dd>
    </div>
  );
}

/**
 * Rótulo repetido acontece — duas faturas do mesmo cartão, duas séries de mesmo nome — e um
 * `key` só de rótulo faria o React descartar uma linha: a conta impressa deixaria de fechar.
 * O contador desempata pelo conteúdo, sem depender da posição no array.
 */
function keyed(lines: ReceiptLine[]): { line: ReceiptLine; key: string }[] {
  const seen = new Map<string, number>();
  return lines.map((line) => {
    const base = `${line.label}:${line.cents ?? line.text ?? ""}`;
    const nth = (seen.get(base) ?? 0) + 1;
    seen.set(base, nth);
    return { line, key: `${base}#${nth}` };
  });
}

function ReceiptLines({
  lines,
  className,
}: {
  lines: ReceiptLine[];
  className: string;
}) {
  return (
    <dl className={className}>
      {keyed(lines).map(({ line, key }) => (
        <ReceiptRow key={key} line={line} />
      ))}
    </dl>
  );
}

/** A conta inteira à mostra. */
export function Receipt({ lines }: { lines: ReceiptLine[] }) {
  return <ReceiptLines lines={lines} className="nk-receipt" />;
}

/**
 * Recibo com a aritmética recolhida: a preferência de exibição esconde os operandos, nunca
 * o estado do dado — a linha `result` fica sempre à mostra, e o botão abre o resto da conta
 * ali mesmo, sem navegar.
 */
export function CollapsedReceipt({ lines }: { lines: ReceiptLine[] }): ReactNode {
  const [open, setOpen] = useState(false);
  const foldId = useId();
  const resultLines = lines.filter((line) => line.result);
  const restLines = lines.filter((line) => !line.result);

  if (restLines.length === 0) return <Receipt lines={lines} />;

  return (
    // Os operandos vêm antes do resultado mesmo recolhidos: aberta, a conta se lê na ordem
    // em que foi feita, sem o resultado saltar para o topo. A moldura tracejada é o sinal de
    // "aqui tem conta" — fechada, ela não promete o que não mostra.
    <div className="nk-receipt" data-open={open}>
      <div id={foldId} data-open={open} inert={!open} className="nk-receipt__fold">
        <ReceiptLines lines={restLines} className="nk-receipt__lines" />
      </div>
      <ReceiptLines lines={resultLines} className="nk-receipt__lines" />
      <button
        type="button"
        className="nk-receipt__toggle"
        aria-expanded={open}
        aria-controls={foldId}
        onClick={() => setOpen((current) => !current)}
      >
        <ChevronDown size={14} aria-hidden="true" />
        {open ? "Ocultar a conta" : "Ver a conta"}
      </button>
    </div>
  );
}
