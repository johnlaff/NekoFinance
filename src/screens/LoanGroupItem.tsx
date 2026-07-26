import { useEffect, useRef, useState, type ReactNode } from "react";
import { Pencil, Trash2 } from "lucide-react";
import type { ScenarioLoanRow, ScenarioTransactionRow } from "../lib/api";
import { motionEnabled } from "../lib/motion";
import { stripScenarioMarker } from "../lib/scenarioHelpers";
import { Button } from "../design-system/components/Button";
import { Disclosure } from "../design-system/components/Disclosure";
import { Money } from "../design-system/components/Money";

/** Linhas de um mesmo empréstimo simulado, reconhecidas por `loan_id`; `loan` traz os
 * parâmetros persistidos (cabeçalho do grupo + formulário de edição). */
interface LoanGroup {
  loanId: string;
  loan: ScenarioLoanRow | null;
  principal: ScenarioTransactionRow | null;
  installments: ScenarioTransactionRow[];
}

interface LoanEditRequest {
  loan: ScenarioLoanRow;
  missingRows: number;
}

/** "O principal + 12 parcelas saem do cenário." — a confirmação de remover o grupo nomeia o
 * que morre, com o plural certo para cada combinação de linhas presentes. */
function loanDeathNote(hasPrincipal: boolean, installmentCount: number): string {
  if (hasPrincipal && installmentCount > 0) {
    const s = installmentCount === 1 ? "parcela" : "parcelas";
    return `O principal + ${installmentCount} ${s} saem do cenário.`;
  }
  if (hasPrincipal) return "O principal sai do cenário.";
  return installmentCount === 1
    ? "A parcela restante sai do cenário."
    : `As ${installmentCount} parcelas saem do cenário.`;
}

/** Solta a trava de "ocupado" aconteça o que acontecer com `action` — falha na remoção não pode
 *  deixar o grupo preso, já que a própria trava barra uma segunda tentativa. Mora fora do
 *  componente porque o React Compiler não compila `try` sem `catch` e desistiria de memoizar. */
async function runThenRelease(
  action: () => Promise<void>,
  release: () => void,
): Promise<void> {
  try {
    await action();
  } finally {
    release();
  }
}

interface LoanGroupItemProps {
  group: LoanGroup;
  isNew: boolean;
  /** Muda a cada salvamento — refaz o recibo (scroll/foco) mesmo re-salvando o mesmo alvo. */
  focusTick: number;
  isEditing: boolean;
  onEdit: (request: LoanEditRequest) => void;
  /** Resolve `true` quando o empréstimo saiu de fato do cenário. */
  onRemove: (loanId: string) => Promise<boolean>;
  renderRow: (r: ScenarioTransactionRow, label?: string) => ReactNode;
}

export function LoanGroupItem({
  group: g,
  isNew,
  focusTick,
  isEditing,
  onEdit,
  onRemove,
  renderRow,
}: LoanGroupItemProps) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [busy, setBusy] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const confirmRef = useRef<HTMLDivElement>(null);
  const wasConfirming = useRef(false);

  // Recibo da criação/edição: rola até o grupo e move o foco para o cabeçalho do Disclosure —
  // o realce visual fica no CSS (`.scn-loan-flash`, que colapsa sob reduced-motion).
  useEffect(() => {
    if (!isNew) return;
    const el = rootRef.current;
    if (!el) return;
    el.scrollIntoView?.({
      behavior: motionEnabled() ? "smooth" : "auto",
      block: "nearest",
    });
    el.querySelector<HTMLElement>(".nk-disc__head")?.focus({ preventScroll: true });
  }, [isNew, focusTick]);

  // A troca botões→confirmação desmonta o botão "Remover" que tinha o foco; sem gestão o foco
  // cai no <body> bem no momento da ação destrutiva. Foca o bloco do aviso (não o botão
  // destrutivo — Enter repetido não pode confirmar sem querer); cancelar devolve ao "Remover".
  useEffect(() => {
    if (confirmRemove) {
      confirmRef.current?.focus({ preventScroll: true });
    } else if (wasConfirming.current) {
      rootRef.current
        ?.querySelector<HTMLElement>(".scn-loan-group__actions button:last-of-type")
        ?.focus({ preventScroll: true });
    }
    wasConfirming.current = confirmRemove;
  }, [confirmRemove]);

  const anyRow = g.principal ?? g.installments[0];
  if (!anyRow) return null;
  const label =
    g.loan?.description ??
    stripScenarioMarker(anyRow.description).replace(/ parcela \d+\/\d+$/, "");
  const installmentCents = Math.abs(g.installments[0]?.amount ?? 0);
  // Linhas que a lixeira removeu deste grupo: a edição regenera a série e as restaura — o
  // formulário avisa antes de salvar.
  const presentRows = g.installments.length + (g.principal ? 1 : 0);
  const missingRows = g.loan ? Math.max(0, g.loan.term_months + 1 - presentRows) : 0;

  async function confirmRemoval() {
    if (busy) return;
    setBusy(true);
    await runThenRelease(
      async () => {
        // A confirmação só fecha quando o empréstimo sai mesmo: falhou, o usuário continua
        // diante do botão que tentou apertar, com o erro à vista.
        if (await onRemove(g.loanId)) setConfirmRemove(false);
      },
      () => setBusy(false),
    );
  }

  return (
    <div ref={rootRef} className={isNew ? "scn-loan-flash" : undefined}>
      <Disclosure
        className={"scn-loan-group" + (isEditing ? " scn-loan-group--editing" : "")}
        title={label}
        {...(isEditing
          ? {
              accent: "brass" as const,
              icon: <Pencil size={14} strokeWidth={1.75} />,
            }
          : {})}
        defaultOpen={isNew || isEditing}
        summary={
          <>
            {g.principal && (
              <>
                Recebe <Money cents={Math.abs(g.principal.amount)} size="inherit" />
                {" · "}
              </>
            )}
            {/* Grupo só com o principal (parcelas excluídas à mão): não inventa
                "Paga 0× de R$ 0,00". */}
            {g.installments.length > 0 && (
              <>
                Paga {g.installments.length}× de{" "}
                <Money cents={installmentCents} size="inherit" />
              </>
            )}
          </>
        }
      >
        {g.loan &&
          (confirmRemove ? (
            <div
              role="alert"
              className="scn-loan-group__confirm"
              ref={confirmRef}
              tabIndex={-1}
            >
              <p className="scn-hint">
                {loanDeathNote(g.principal != null, g.installments.length)}
              </p>
              <div className="scn-loan-group__actions">
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => void confirmRemoval()}
                  disabled={busy}
                >
                  {busy ? "Removendo…" : "Remover empréstimo"}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setConfirmRemove(false)}
                  disabled={busy}
                >
                  Cancelar
                </Button>
              </div>
            </div>
          ) : (
            <div className="scn-loan-group__actions">
              <Button
                size="sm"
                variant="secondary"
                iconLeft={<Pencil size={13} strokeWidth={1.75} />}
                onClick={() => onEdit({ loan: g.loan!, missingRows })}
                disabled={isEditing}
              >
                {isEditing ? "Em edição…" : "Editar"}
              </Button>
              {/* Remover também trava durante a edição: apagar o alvo do formulário aberto
                  deixaria a edição órfã (salvar iria falhar contra um id que não existe). */}
              <Button
                size="sm"
                variant="ghost"
                iconLeft={<Trash2 size={13} strokeWidth={1.75} />}
                onClick={() => setConfirmRemove(true)}
                disabled={isEditing}
              >
                Remover
              </Button>
            </div>
          ))}
        <div className="scn-txn-list scn-loan-group__rows">
          {g.principal && renderRow(g.principal, "Principal")}
          {g.installments.map((r) => {
            const m = /parcela (\d+\/\d+)/.exec(stripScenarioMarker(r.description));
            return renderRow(r, m ? `Parcela ${m[1]}` : undefined);
          })}
        </div>
      </Disclosure>
    </div>
  );
}
