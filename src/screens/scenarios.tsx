/**
 * Cenários "e se" — o FRONTEND do que-se-fosse: um side-sheet para
 * montar/gerenciar o cenário (lançamentos hipotéticos, alterações sobre obrigações reais,
 * dimensionamento de empréstimo) + a superfície de comparação (real × cenário) que aparece no
 * Horizonte quando um cenário está selecionado. Nada aqui grava no livro-razão real nem no
 * Sheets — todo write passa pelos comandos `scenario_*`/`add_scenario_transaction`,
 * que já isolam a linha hipotética via `scenario_id`.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  X,
  Plus,
  Trash2,
  Repeat,
  Landmark,
  Pencil,
  CircleDollarSign,
  ArrowRight,
  TrendingUp,
  TrendingDown,
  AlertTriangle,
  CheckCircle2,
  Minus,
} from "lucide-react";
import {
  createScenario,
  deleteScenario,
  listScenarios,
  addScenarioTransaction,
  createScenarioLoan,
  updateScenarioLoan,
  deleteScenarioLoan,
  listScenarioLoans,
  deleteScenarioTransaction,
  listScenarioTransactions,
  setScenarioOverride,
  listObligations,
  obligationItems,
  listRecurrenceTargets,
  recurrenceOccurrences,
  priceInstallment,
  type Scenario,
  type ScenarioCompareDto,
  type ScenarioLoanRow,
  type ScenarioTransactionRow,
  type Obligation,
  type RecurrenceTarget,
} from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import {
  todayISO,
  parseBRLToCents,
  formatBRL,
  fmtAxisBRL,
  fmtDate,
  fmtDayMonth,
} from "../lib/format";
import {
  fmtBRL,
  fmtCompactBRL,
  fmtSigned,
  saldoBand,
  monthOf,
  MES,
  MES_ABBR,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { performanceStatus, custoVidaStatus } from "./totaisStatus";
import { kindToFields } from "../lib/movement";
import { motionEnabled } from "../lib/motion";
import {
  stripScenarioMarker,
  addMonthsISO,
  placeChartEndLabels,
  niceChartScale,
  centsToAmountInput,
  bpsToRateInput,
} from "../lib/scenarioHelpers";
import { SR_ONLY } from "../design-system/srOnly";
import { Money, SignedMoney } from "../design-system/components/Money";
import { Button } from "../design-system/components/Button";
import { Disclosure } from "../design-system/components/Disclosure";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { safeErrorMessage, errorText } from "../lib/errors";
import "./scenarios.css";

/** Mensagem de erro para ações do cenário: as rejeições do backend (data anterior ao mês
 * corrente, override duplicado) já são frases PT-BR pensadas para o usuário — mostradas
 * verbatim; qualquer outra coisa cai no fallback genérico de `safeErrorMessage`. */
function scenarioErrorMessage(err: unknown): string {
  const raw = errorText(err).trim();
  if (
    /data anterior ao mês corrente/i.test(raw) ||
    /já existe uma alteração/i.test(raw) ||
    /zeraria a célula/i.test(raw) ||
    /informe exatamente um alvo/i.test(raw) ||
    /informe um alvo/i.test(raw)
  ) {
    return raw;
  }
  return safeErrorMessage(err);
}

const MOVEMENT_KINDS: MovementType[] = [
  "saida",
  "cartao",
  "diario",
  "economia",
  "entrada",
];

// ---------------------------------------------------------------------------
// Botão que abre o side-sheet
// ---------------------------------------------------------------------------

export function SimulateScenarioButton({ onClick }: { onClick: () => void }) {
  return (
    <Button variant="secondary" size="sm" className="scn-open-btn" onClick={onClick}>
      <Repeat size={14} strokeWidth={1.75} aria-hidden="true" />
      Simular cenário
    </Button>
  );
}

// ---------------------------------------------------------------------------
// Side-sheet
// ---------------------------------------------------------------------------

interface ScenarioSheetProps {
  open: boolean;
  onClose: () => void;
  activeScenarioId: string | null;
  onSelectScenario: (id: string | null) => void;
}

/** `<dialog>` nativo NÃO-modal (`show()`, não `showModal()`): um side-sheet existe para a
 * comparação ficar visível e OPERÁVEL ao lado — um modal poria um scrim sobre os cards e
 * bloquearia rolar/interagir com o compare enquanto se edita o cenário. Sem modal não há
 * `::backdrop` nem foco preso (correto para side-sheet); Escape-para-fechar e o foco inicial
 * são repostos à mão abaixo. */
export function ScenarioSheet({
  open,
  onClose,
  activeScenarioId,
  onSelectScenario,
}: ScenarioSheetProps) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (open && !el.open) {
      el.show();
      // show() não move o foco (showModal movia): leva ao primeiro campo do sheet.
      el.querySelector<HTMLElement>(
        "input, select, button:not(.scn-sheet__close)",
      )?.focus();
    } else if (!open && el.open) {
      el.close();
    }
  }, [open]);

  // Escape-para-fechar: num dialog não-modal o UA não fecha sozinho — repõe o gesto padrão.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    el.addEventListener("keydown", onKeyDown);
    return () => el.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  if (!open) return null;

  return (
    <dialog
      ref={ref}
      className="scn-sheet"
      aria-labelledby="scn-sheet-title"
      onClose={onClose}
    >
      <div className="scn-sheet__head">
        <span id="scn-sheet-title" className="scn-sheet__title">
          Simular cenário
        </span>
        <button
          type="button"
          className="scn-sheet__close"
          aria-label="Fechar simulação"
          onClick={onClose}
        >
          <X size={18} strokeWidth={1.75} />
        </button>
      </div>
      <div className="scn-sheet__body">
        <ScenarioPicker
          activeScenarioId={activeScenarioId}
          onSelectScenario={onSelectScenario}
        />
        {activeScenarioId && (
          <ScenarioWorkbench key={activeScenarioId} scenarioId={activeScenarioId} />
        )}
      </div>
    </dialog>
  );
}

function ScenarioPicker({
  activeScenarioId,
  onSelectScenario,
}: {
  activeScenarioId: string | null;
  onSelectScenario: (id: string | null) => void;
}) {
  const listQ = useCommand("scenarios", listScenarios);
  const scenarios = listQ.data ?? [];
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  async function create() {
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    setError(null);
    // Sem try/finally (React Compiler não compila TryStatement com finalizer). No sucesso o
    // cenário criado vira o ativo e o form limpa; `setBusy(false)` some do caminho feliz porque
    // o próximo render já reflete o cenário selecionado — só o caminho de erro precisa reabilitar
    // o botão (mesmo padrão de `ObligationsPanel.confirm`).
    try {
      const created = await createScenario(trimmed);
      invalidateCommands();
      setName("");
      setBusy(false);
      onSelectScenario(created.id);
    } catch (err) {
      setBusy(false);
      setError(scenarioErrorMessage(err));
    }
  }

  async function remove(id: string) {
    try {
      await deleteScenario(id);
      invalidateCommands();
      setConfirmDeleteId(null);
      if (activeScenarioId === id) onSelectScenario(null);
    } catch (err) {
      // Refetch mesmo na falha (a lista volta a refletir o estado real) + erro visível —
      // nunca um catch mudo que deixa o usuário sem saber que nada aconteceu.
      invalidateCommands();
      setConfirmDeleteId(null);
      setError(scenarioErrorMessage(err));
    }
  }

  return (
    <section>
      <p className="scn-section-title">Cenários salvos</p>
      {listQ.loading ? (
        <p className="scn-empty">Carregando…</p>
      ) : scenarios.length === 0 ? (
        <p className="scn-empty">Nenhum cenário ainda. Crie um abaixo.</p>
      ) : (
        <div className="scn-list">
          {scenarios.map((sc: Scenario) => (
            <div
              key={sc.id}
              className={
                "scn-scenario-row" +
                (activeScenarioId === sc.id ? " scn-scenario-row--active" : "")
              }
            >
              <button
                type="button"
                className="scn-scenario-row__btn"
                aria-pressed={activeScenarioId === sc.id}
                onClick={() =>
                  onSelectScenario(activeScenarioId === sc.id ? null : sc.id)
                }
              >
                {sc.name}
              </button>
              {confirmDeleteId === sc.id ? (
                <>
                  <button
                    type="button"
                    className="scn-scenario-row__delete"
                    aria-label={`Confirmar apagar cenário "${sc.name}"`}
                    onClick={() => void remove(sc.id)}
                    style={{ color: "var(--danger-400)" }}
                  >
                    Confirmar
                  </button>
                  <button
                    type="button"
                    className="scn-scenario-row__delete"
                    aria-label="Cancelar"
                    onClick={() => setConfirmDeleteId(null)}
                  >
                    Cancelar
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="scn-scenario-row__delete"
                  aria-label={`Apagar cenário "${sc.name}"`}
                  onClick={() => setConfirmDeleteId(sc.id)}
                >
                  <Trash2 size={13} strokeWidth={1.75} />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      <div className="scn-field" style={{ marginTop: 10 }}>
        <label htmlFor="scn-new-name">Novo cenário</label>
        <input
          id="scn-new-name"
          value={name}
          placeholder="Ex.: E se eu financiar um carro"
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void create();
          }}
        />
      </div>
      {error && (
        <p role="alert" className="scn-error">
          {error}
        </p>
      )}
      <Button
        size="sm"
        variant="primary"
        onClick={() => void create()}
        disabled={busy || !name.trim()}
      >
        {busy ? "Criando…" : "Criar cenário"}
      </Button>
    </section>
  );
}

/** Pedido de edição de um empréstimo: além da entidade, carrega quantas linhas o grupo perdeu
 * por ajuste fino (lixeira por linha) — salvar a edição regenera a série e as restaura, e o
 * formulário precisa avisar antes. */
interface LoanEditRequest {
  loan: ScenarioLoanRow;
  missingRows: number;
}

type LoanSaved =
  { kind: "created"; loanId: string } | { kind: "updated"; loanId: string };

/** Estado partilhado do ciclo de vida do empréstimo entre a lista (grupos com Editar/Remover)
 * e o formulário (criação/edição): montado com `key={scenarioId}` no sheet, então trocar de
 * cenário zera edição, realce e anúncio. A região live é o canal de sucesso para leitores de
 * tela; o foco/realce visual acontece no grupo da lista. */
/** Último empréstimo salvo (criado ou editado). `tick` é monotônico e cumpre dois papéis:
 * refazer o foco/flash quando o MESMO empréstimo é salvo de novo, e forçar uma mutação de DOM
 * na região aria-live (o <span> remonta via key) quando duas ações seguidas produzem o mesmo
 * texto — sem isso a região não reanuncia a segunda. */
interface SavedLoanReceipt {
  loanId: string;
  text: string;
  tick: number;
}

function ScenarioWorkbench({ scenarioId }: { scenarioId: string }) {
  const [editing, setEditing] = useState<LoanEditRequest | null>(null);
  const [receipt, setReceipt] = useState<SavedLoanReceipt | null>(null);

  return (
    <>
      <AddHypotheticalSection scenarioId={scenarioId} />
      <HypotheticalList
        scenarioId={scenarioId}
        createdLoanId={receipt?.loanId ?? null}
        createdTick={receipt?.tick ?? 0}
        editingLoanId={editing?.loan.id ?? null}
        onEditLoan={setEditing}
      />
      <OverrideSection scenarioId={scenarioId} />
      <LoanSection
        key={editing?.loan.id ?? "new"}
        scenarioId={scenarioId}
        editing={editing}
        onCancelEdit={() => setEditing(null)}
        onSaved={(saved: LoanSaved) => {
          if (saved.kind === "updated") setEditing(null);
          setReceipt((r) => ({
            loanId: saved.loanId,
            text:
              saved.kind === "created"
                ? "Empréstimo adicionado ao cenário."
                : "Empréstimo atualizado — parcelas regeradas.",
            tick: (r?.tick ?? 0) + 1,
          }));
        }}
      />
      <p aria-live="polite" style={SR_ONLY}>
        {receipt && <span key={receipt.tick}>{receipt.text}</span>}
      </p>
    </>
  );
}

function AddHypotheticalSection({ scenarioId }: { scenarioId: string }) {
  const [kind, setKind] = useState<MovementType>("saida");
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");
  const [date, setDate] = useState(() => todayISO());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const trimmedDesc = description.trim();
    const cents = parseBRLToCents(amount);
    if (!trimmedDesc || cents === null || cents <= 0 || busy) return;
    setBusy(true);
    setError(null);
    // Sem try/finally (React Compiler não compila TryStatement com finalizer) — mesmo padrão de
    // `ObligationsPanel.confirm`: `setBusy(false)` roda em CADA ramo, não num finally.
    try {
      const { txnType, isFixed, paymentMethod } = kindToFields(kind);
      await addScenarioTransaction({
        scenarioId,
        txnType,
        amountCents: cents,
        description: trimmedDesc,
        date,
        paymentMethod,
        isFixed,
      });
      invalidateCommands();
      setDescription("");
      setAmount("");
      setBusy(false);
    } catch (err) {
      setBusy(false);
      setError(scenarioErrorMessage(err));
    }
  }

  return (
    <section aria-labelledby="scn-add-title">
      <p className="scn-section-title" id="scn-add-title">
        Adicionar lançamento
      </p>
      <div className="scn-field">
        <label htmlFor="scn-add-desc">Descrição</label>
        <input
          id="scn-add-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </div>
      <div className="scn-row2" style={{ marginTop: 8 }}>
        <div className="scn-field">
          <label htmlFor="scn-add-amount">Valor/mês</label>
          <input
            id="scn-add-amount"
            inputMode="decimal"
            placeholder="0,00"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
          />
        </div>
        <div className="scn-field">
          <label htmlFor="scn-add-date">Data</label>
          <input
            id="scn-add-date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </div>
      </div>
      <div className="scn-field" style={{ marginTop: 8 }}>
        <label htmlFor="scn-add-kind">Tipo</label>
        <select
          id="scn-add-kind"
          value={kind}
          onChange={(e) => setKind(e.target.value as MovementType)}
        >
          {MOVEMENT_KINDS.map((k) => (
            <option key={k} value={k}>
              {TYPE_META[k].name}
              {k === "saida" ? " fixa" : ""}
            </option>
          ))}
        </select>
      </div>
      {error && (
        <p role="alert" className="scn-error">
          {error}
        </p>
      )}
      <Button
        size="sm"
        variant="primary"
        iconLeft={<Plus size={14} strokeWidth={1.75} />}
        onClick={() => void submit()}
        disabled={busy || !description.trim() || !amount.trim()}
      >
        {busy ? "Adicionando…" : "Adicionar"}
      </Button>
    </section>
  );
}

/** Linhas de um mesmo empréstimo simulado, reconhecidas por `loan_id`; `loan` traz os
 * parâmetros persistidos (cabeçalho do grupo + formulário de edição). */
interface LoanGroup {
  loanId: string;
  loan: ScenarioLoanRow | null;
  principal: ScenarioTransactionRow | null;
  installments: ScenarioTransactionRow[];
}

interface HypotheticalListProps {
  scenarioId: string;
  /** Empréstimo recém-criado/atualizado: o grupo recebe foco + realce breve. */
  createdLoanId: string | null;
  /** Muda a cada salvamento — refaz o foco mesmo quando o alvo é o mesmo empréstimo. */
  createdTick: number;
  /** Empréstimo aberto no formulário de edição: o grupo fica marcado. */
  editingLoanId: string | null;
  onEditLoan: (request: LoanEditRequest) => void;
}

function HypotheticalList({
  scenarioId,
  createdLoanId,
  createdTick,
  editingLoanId,
  onEditLoan,
}: HypotheticalListProps) {
  const listQ = useCommand(`scenario_transactions:${scenarioId}`, () =>
    listScenarioTransactions(scenarioId),
  );
  const loansQ = useCommand(`scenario_loans:${scenarioId}`, () =>
    listScenarioLoans(scenarioId),
  );
  const rows = listQ.data ?? [];
  const loans = loansQ.data ?? [];
  const [error, setError] = useState<string | null>(null);

  async function removeRow(txnId: string) {
    setError(null);
    try {
      await deleteScenarioTransaction(scenarioId, txnId);
      invalidateCommands();
    } catch (err) {
      // Refetch mesmo na falha (a lista volta a refletir o estado real) + erro visível —
      // nunca um catch mudo que deixa o usuário sem saber que nada aconteceu.
      invalidateCommands();
      setError(scenarioErrorMessage(err));
    }
  }

  async function removeLoan(loanId: string) {
    setError(null);
    try {
      await deleteScenarioLoan(scenarioId, loanId);
      invalidateCommands();
    } catch (err) {
      invalidateCommands();
      setError(scenarioErrorMessage(err));
    }
  }

  // Um principal + N parcelas (empréstimo) e a série de substituição (N linhas iguais, uma por
  // ocorrência) geram pills repetidos e tornam a lista monótona. Cada um colapsa num Disclosure
  // (lump-expand do DS): empréstimo por `loan_id`, substituição por `override_id`. As demais
  // linhas seguem soltas na ordem original.
  const singles: ScenarioTransactionRow[] = [];
  const loanGroupsById = new Map<string, ScenarioTransactionRow[]>();
  const replacementGroupsById = new Map<string, ScenarioTransactionRow[]>();
  for (const r of rows) {
    if (r.loan_id) {
      const list = loanGroupsById.get(r.loan_id) ?? [];
      list.push(r);
      loanGroupsById.set(r.loan_id, list);
    } else if (r.override_id) {
      const list = replacementGroupsById.get(r.override_id) ?? [];
      list.push(r);
      replacementGroupsById.set(r.override_id, list);
    } else {
      singles.push(r);
    }
  }
  const groups: LoanGroup[] = [...loanGroupsById.entries()].map(([loanId, list]) => {
    const principal = list.find((r) => r.type === "income") ?? null;
    // Parcelas em ordem CRONOLÓGICA: o backend lista por data decrescente, e um empréstimo
    // lido de trás pra frente ("parcela 12, 11, 10…") parece um bug de datas.
    const installments = list
      .filter((r) => r.type !== "income")
      .toSorted((a, b) => a.date.localeCompare(b.date));
    const loan = loans.find((l) => l.id === loanId) ?? null;
    return { loanId, loan, principal, installments };
  });
  const replacementGroups: ReplacementGroup[] = [
    ...replacementGroupsById.entries(),
  ].map(([overrideId, list]) => ({
    overrideId,
    lines: list.toSorted((a, b) => a.date.localeCompare(b.date)),
  }));

  if (listQ.loading) return null;
  if (rows.length === 0) {
    return (
      <section>
        <p className="scn-section-title">Lançamentos hipotéticos</p>
        <p className="scn-empty">Nenhum lançamento hipotético ainda.</p>
        {error && (
          <p role="alert" className="scn-error">
            {error}
          </p>
        )}
      </section>
    );
  }

  // `label` encurta o texto DENTRO do grupo de empréstimo: o título do Disclosure já diz
  // "Empréstimo" — repetir o prefixo em cada linha só truncava ("Empréstimo parc…"). O
  // aria-label de remover mantém a descrição completa (o contexto do grupo não viaja com
  // o leitor de tela).
  const txnRow = (r: ScenarioTransactionRow, label?: string) => (
    <div className="scn-txn-row" key={r.id}>
      <span className="scn-txn-row__desc">
        {label ?? stripScenarioMarker(r.description)}
      </span>
      <span className="scn-txn-row__date">{fmtDayMonth(r.date)}</span>
      <span className="scn-txn-row__amt">
        <Money
          cents={r.type === "income" ? Math.abs(r.amount) : -Math.abs(r.amount)}
          size="sm"
          sign="auto"
        />
      </span>
      <button
        type="button"
        className="scn-txn-row__del"
        // A data entra no nome acessível: as N linhas de uma série de substituição têm a MESMA
        // descrição, e sem a data o leitor de tela não distingue qual ocorrência cada lixeira apaga.
        aria-label={`Remover "${stripScenarioMarker(r.description)}" de ${fmtDayMonth(r.date)} do cenário`}
        onClick={() => void removeRow(r.id)}
      >
        <Trash2 size={14} strokeWidth={1.75} />
      </button>
    </div>
  );

  return (
    <section>
      <p className="scn-section-title">Lançamentos hipotéticos</p>
      {error && (
        <p role="alert" className="scn-error">
          {error}
        </p>
      )}
      <div className="scn-txn-list">
        {singles.map((r) => txnRow(r))}
        {groups.map((g) => (
          <LoanGroupItem
            key={g.loanId}
            group={g}
            isNew={g.loanId === createdLoanId}
            focusTick={createdTick}
            isEditing={g.loanId === editingLoanId}
            onEdit={onEditLoan}
            onRemove={removeLoan}
            renderRow={txnRow}
          />
        ))}
        {replacementGroups.map((rg) => (
          <ReplacementGroupItem key={rg.overrideId} group={rg} renderRow={txnRow} />
        ))}
      </div>
    </section>
  );
}

/** Série de substituição de um override `replace`: N linhas iguais (uma por ocorrência),
 * colapsadas num Disclosure como o empréstimo. O cabeçalho resume "N× de R$ X". Sem ação de
 * remover o GRUPO: não há remoção individual de override — a série morre em cascata quando o
 * alvo/cenário é apagado; a lixeira por linha segue como ajuste fino. */
interface ReplacementGroup {
  overrideId: string;
  lines: ScenarioTransactionRow[];
}

function ReplacementGroupItem({
  group,
  renderRow,
}: {
  group: ReplacementGroup;
  renderRow: (r: ScenarioTransactionRow, label?: string) => ReactNode;
}) {
  const first = group.lines[0];
  if (!first) return null;
  const label = stripScenarioMarker(first.description) || "Substituição";
  const perOccurrence = Math.abs(first.amount);
  return (
    <Disclosure
      className="scn-loan-group"
      title={label}
      summary={
        <>
          {group.lines.length}× de <Money cents={perOccurrence} size="inherit" />
        </>
      }
    >
      <div className="scn-txn-list scn-loan-group__rows">
        {group.lines.map((r) => renderRow(r))}
      </div>
    </Disclosure>
  );
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

interface LoanGroupItemProps {
  group: LoanGroup;
  isNew: boolean;
  /** Muda a cada salvamento — refaz o recibo (scroll/foco) mesmo re-salvando o mesmo alvo. */
  focusTick: number;
  isEditing: boolean;
  onEdit: (request: LoanEditRequest) => void;
  onRemove: (loanId: string) => Promise<void>;
  renderRow: (r: ScenarioTransactionRow, label?: string) => ReactNode;
}

function LoanGroupItem({
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
    await onRemove(g.loanId);
    setBusy(false);
    setConfirmRemove(false);
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

/** Alvo do override: uma obrigação (escopo line-item) OU uma recorrência nativa (série inteira).
 * O valor do `<select>` codifica o tipo (`obl:<id>` / `rec:<id>`) porque a prévia e o alvo do
 * backend divergem por tipo; os ids são UUID (sem `:`), então o split no primeiro `:` é seguro. */
type OverrideTargetKind = "obligation" | "recurrence";
function parseOverrideTarget(
  value: string,
): { kind: OverrideTargetKind; id: string } | null {
  const sep = value.indexOf(":");
  if (sep < 0) return null;
  const id = value.slice(sep + 1);
  if (!id) return null;
  const prefix = value.slice(0, sep);
  if (prefix === "obl") return { kind: "obligation", id };
  if (prefix === "rec") return { kind: "recurrence", id };
  return null;
}

const RECURRENCE_FREQUENCY_LABEL: Record<string, string> = {
  diaria: "diária",
  semanal: "semanal",
  mensal: "mensal",
};

function recurrenceTargetLabel(r: RecurrenceTarget): string {
  const freq = RECURRENCE_FREQUENCY_LABEL[r.frequency] ?? r.frequency;
  const desc = r.description.trim() || "Recorrência";
  return `${desc} · ${freq}`;
}

function OverrideSection({ scenarioId }: { scenarioId: string }) {
  const obligationsQ = useCommand("obligations", listObligations);
  const obligations = obligationsQ.data ?? [];
  const recurrencesQ = useCommand("recurrence_targets", listRecurrenceTargets);
  const recurrences = recurrencesQ.data ?? [];
  const [selected, setSelected] = useState("");
  const [action, setAction] = useState<"replace" | "suppress">("suppress");
  const [newAmount, setNewAmount] = useState("");
  const [fromDate, setFromDate] = useState(() => todayISO());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const target = parseOverrideTarget(selected);
  // Prévia unificada: obrigação → itens casados; recorrência → ocorrências reais. Ambos expõem
  // `{ date, amount_cents }`, então a contagem "afeta N a partir de {data}" é a mesma.
  const previewQ = useCommand(
    target ? `override_target:${selected}` : "override_target:none",
    () => {
      if (!target)
        return Promise.resolve([] as { date: string; amount_cents: number }[]);
      return target.kind === "obligation"
        ? obligationItems(target.id).then((items) =>
            items.map((it) => ({ date: it.date, amount_cents: it.amount_cents })),
          )
        : recurrenceOccurrences(target.id);
    },
  );
  // Mesmo piso do backend (`replacement_occurrence_dates`): a série/supressão só afeta a projeção
  // a partir do mês corrente, então a contagem exibida ignora ocorrências históricas (inertes)
  // quando o usuário escolhe um from_date de um mês passado.
  const currentMonthStart = todayISO().slice(0, 7) + "-01";
  const previewFloor = fromDate > currentMonthStart ? fromDate : currentMonthStart;
  const affectedCount = (previewQ.data ?? []).filter(
    (o) => o.date >= previewFloor,
  ).length;
  const newCents = parseBRLToCents(newAmount) ?? 0;

  async function confirm() {
    if (!target || busy) return;
    const cents = action === "replace" ? newCents : 0;
    if (action === "replace" && cents <= 0) return;
    setBusy(true);
    setError(null);
    try {
      await setScenarioOverride({
        scenarioId,
        op: action,
        fromDate,
        obligationId: target.kind === "obligation" ? target.id : null,
        recurrenceId: target.kind === "recurrence" ? target.id : null,
        replacement: action === "replace" ? { amount_cents: cents } : null,
      });
      invalidateCommands();
      setSelected("");
      setNewAmount("");
      setBusy(false);
    } catch (err) {
      setBusy(false);
      setError(scenarioErrorMessage(err));
    }
  }

  return (
    <section>
      <p className="scn-section-title">Simular alteração</p>
      <div className="scn-field">
        <label htmlFor="scn-ov-target">Alvo</label>
        <select
          id="scn-ov-target"
          value={selected}
          onChange={(e) => setSelected(e.target.value)}
        >
          <option value="">Selecione…</option>
          {obligations.length > 0 && (
            <optgroup label="Obrigações">
              {obligations.map((ob: Obligation) => (
                <option key={ob.id} value={`obl:${ob.id}`}>
                  {ob.name}
                </option>
              ))}
            </optgroup>
          )}
          {recurrences.length > 0 && (
            <optgroup label="Recorrências">
              {recurrences.map((r: RecurrenceTarget) => (
                <option key={r.id} value={`rec:${r.id}`}>
                  {recurrenceTargetLabel(r)}
                </option>
              ))}
            </optgroup>
          )}
        </select>
      </div>
      {target && (
        <>
          <div className="scn-row2" style={{ marginTop: 8 }}>
            <div className="scn-field">
              <label htmlFor="scn-ov-action">Ação</label>
              <select
                id="scn-ov-action"
                value={action}
                onChange={(e) => setAction(e.target.value as "replace" | "suppress")}
              >
                <option value="suppress">Remover deste cenário</option>
                <option value="replace">Alterar valor</option>
              </select>
            </div>
            <div className="scn-field">
              <label htmlFor="scn-ov-from">A partir de</label>
              <input
                id="scn-ov-from"
                type="date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
              />
            </div>
          </div>
          {action === "replace" && (
            <div className="scn-field" style={{ marginTop: 8 }}>
              <label htmlFor="scn-ov-amount">Novo valor/mês</label>
              <input
                id="scn-ov-amount"
                inputMode="decimal"
                placeholder="0,00"
                value={newAmount}
                onChange={(e) => setNewAmount(e.target.value)}
              />
            </div>
          )}
          {previewQ.error !== null ? (
            // Prévia ilegível ⇒ nunca mostrar "0 ocorrências" (leitura de erro parece "não
            // afeta nada"): o usuário decide às cegas. Erro visível + retry, e o botão bloqueia.
            <div
              role="alert"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 6,
                alignItems: "flex-start",
              }}
            >
              <p className="scn-error">
                Não foi possível carregar as ocorrências afetadas.
              </p>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => invalidateCommands()}
              >
                Tentar novamente
              </Button>
            </div>
          ) : (
            // Uma caixa só: a contagem afetada e — para substituição — a prévia da SÉRIE (uma
            // linha por ocorrência, não "suprime N, repõe 1") na mesma frase.
            <p className="scn-preview" aria-live="polite">
              {previewQ.loading ? (
                "Calculando ocorrências afetadas…"
              ) : (
                <>
                  {`Isto afeta ${affectedCount} ${affectedCount === 1 ? "ocorrência" : "ocorrências"} a partir de ${fmtDate(fromDate)}.`}
                  {action === "replace" && affectedCount > 0 && newCents > 0 && (
                    <>
                      {` Serão criadas ${affectedCount} ${affectedCount === 1 ? "linha" : "linhas"} de `}
                      <Money cents={newCents} size="inherit" />
                      {" (uma por ocorrência)."}
                    </>
                  )}
                </>
              )}
            </p>
          )}
          {error && (
            <p role="alert" className="scn-error">
              {error}
            </p>
          )}
          <Button
            size="sm"
            variant="primary"
            onClick={() => void confirm()}
            disabled={
              busy ||
              previewQ.loading ||
              previewQ.error !== null ||
              affectedCount === 0 ||
              (action === "replace" && newCents <= 0)
            }
          >
            {busy ? "Aplicando…" : "Confirmar alteração"}
          </Button>
        </>
      )}
    </section>
  );
}

interface LoanSectionProps {
  scenarioId: string;
  /** Modo edição: formulário pré-preenchido com os parâmetros persistidos; salvar regenera a
   * série sob a mesma identidade. Montado com `key={loan.id}` — trocar o alvo zera o estado. */
  editing: LoanEditRequest | null;
  onCancelEdit: () => void;
  onSaved: (saved: LoanSaved) => void;
}

function LoanSection({ scenarioId, editing, onCancelEdit, onSaved }: LoanSectionProps) {
  const [principal, setPrincipal] = useState(() =>
    editing ? centsToAmountInput(editing.loan.principal_cents) : "",
  );
  const [termMonths, setTermMonths] = useState(() =>
    editing ? String(editing.loan.term_months) : "12",
  );
  const [ratePct, setRatePct] = useState(() =>
    editing ? bpsToRateInput(editing.loan.rate_bps) : "2",
  );
  const [disbursementDate, setDisbursementDate] = useState(
    () => editing?.loan.disbursement_date ?? todayISO(),
  );
  const [firstDate, setFirstDate] = useState(
    () => editing?.loan.first_installment_date ?? addMonthsISO(todayISO(), 1),
  );
  const [description, setDescription] = useState(
    () => editing?.loan.description ?? "Empréstimo",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Passo extra do modo edição quando há parcelas removidas à mão: o primeiro "Salvar" só
   * arma o aviso de restauração; o clique seguinte confirma. */
  const [confirmRestore, setConfirmRestore] = useState(false);
  const sectionRef = useRef<HTMLElement>(null);

  // Entrar em edição traz o formulário à vista com o foco no primeiro campo — ele fica no fim
  // do sheet e a ação partiu do grupo, lá em cima na lista.
  useEffect(() => {
    if (!editing) return;
    const el = sectionRef.current;
    if (!el) return;
    el.scrollIntoView?.({
      behavior: motionEnabled() ? "smooth" : "auto",
      block: "start",
    });
    el.querySelector<HTMLElement>("input")?.focus({ preventScroll: true });
  }, [editing]);

  // Fronteira estrita: entrada não-numérica NÃO vira 0% silenciosamente e taxa negativa não
  // passa. Cada campo tem validade explícita; o regex ancorado (sem sinal, sem hex/científico)
  // é a barreira — só o que casa alcança o backend/marcador do agrupamento.
  const principalCents = parseBRLToCents(principal) ?? 0;
  const principalValid = principalCents > 0;

  const termRaw = termMonths.trim();
  const termValid =
    /^\d+$/.test(termRaw) && Number(termRaw) >= 1 && Number(termRaw) <= 480;
  const term = termValid ? Number(termRaw) : 0;

  const rateRaw = ratePct.trim().replace(",", ".");
  const rateValid = /^\d+(?:\.\d+)?$/.test(rateRaw);
  const rateBps = rateValid ? Math.round(Number(rateRaw) * 100) : 0;

  // Só sinaliza o erro quando o campo tem conteúdo inválido (campo vazio/pristino não grita).
  const termShowError = termRaw !== "" && !termValid;
  const rateShowError = rateRaw !== "" && !rateValid;
  const disbursementDateValid = /^\d{4}-\d{2}-\d{2}$/.test(disbursementDate);
  const disbursementDateShowError = !disbursementDateValid;
  const firstDateValid = /^\d{4}-\d{2}-\d{2}$/.test(firstDate);
  const firstDateShowError = !firstDateValid;

  const numericInputsValid = principalValid && termValid && rateValid;
  const validInputs = numericInputsValid && disbursementDateValid && firstDateValid;

  const previewKey = numericInputsValid
    ? `price_installment:${principalCents}:${rateBps}:${term}`
    : "price_installment:none";
  const previewQ = useCommand(previewKey, () =>
    numericInputsValid
      ? priceInstallment(principalCents, rateBps, term)
      : Promise.resolve(0),
  );
  const installmentCents = previewQ.data ?? 0;
  const totalPaidCents = installmentCents * term;
  const creditCostCents = totalPaidCents - principalCents;

  async function confirm() {
    if (
      !validInputs ||
      busy ||
      previewQ.loading ||
      previewQ.error !== null ||
      previewQ.data === undefined
    )
      return;
    // Salvar uma edição restaura o que a lixeira removeu: o primeiro clique arma o aviso; só
    // o clique com o aviso à vista aplica.
    if (editing && editing.missingRows > 0 && !confirmRestore) {
      setConfirmRestore(true);
      return;
    }
    setBusy(true);
    setError(null);
    const input = {
      scenarioId,
      principalCents,
      termMonths: term,
      rateBps,
      disbursementDate,
      firstInstallmentDate: firstDate,
      description: description.trim() || "Empréstimo",
    };
    try {
      if (editing) {
        await updateScenarioLoan(editing.loan.id, input);
        invalidateCommands();
        onSaved({ kind: "updated", loanId: editing.loan.id });
      } else {
        const loanId = await createScenarioLoan(input);
        invalidateCommands();
        setPrincipal("");
        onSaved({ kind: "created", loanId });
      }
      setBusy(false);
    } catch (err) {
      setBusy(false);
      setError(scenarioErrorMessage(err));
    }
  }

  return (
    <section aria-labelledby="scn-loan-title" ref={sectionRef}>
      <p className="scn-section-title" id="scn-loan-title">
        {editing ? "Editar empréstimo" : "Dimensionar um empréstimo"}
      </p>
      <div className="scn-field">
        <label htmlFor="scn-loan-desc">Descrição</label>
        <input
          id="scn-loan-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </div>
      <div className="scn-row3" style={{ marginTop: 8 }}>
        <div className="scn-field">
          <label htmlFor="scn-loan-principal">Valor</label>
          <input
            id="scn-loan-principal"
            inputMode="decimal"
            placeholder="0,00"
            value={principal}
            onChange={(e) => setPrincipal(e.target.value)}
          />
        </div>
        <div className="scn-field">
          <label htmlFor="scn-loan-term">Nº parcelas</label>
          <input
            id="scn-loan-term"
            type="number"
            min={1}
            max={480}
            value={termMonths}
            onChange={(e) => setTermMonths(e.target.value)}
            aria-invalid={termShowError || undefined}
            aria-describedby={termShowError ? "scn-loan-term-err" : undefined}
          />
          {termShowError && (
            <p id="scn-loan-term-err" className="scn-error">
              Prazo inválido — use um número inteiro de 1 a 480.
            </p>
          )}
        </div>
        <div className="scn-field">
          <label htmlFor="scn-loan-rate">Juros a.m. (%)</label>
          <input
            id="scn-loan-rate"
            inputMode="decimal"
            value={ratePct}
            onChange={(e) => setRatePct(e.target.value)}
            aria-invalid={rateShowError || undefined}
            aria-describedby={rateShowError ? "scn-loan-rate-err" : undefined}
          />
          {rateShowError && (
            <p id="scn-loan-rate-err" className="scn-error">
              Taxa inválida — use um número, ex.: 1,8.
            </p>
          )}
        </div>
      </div>
      <div className="scn-row2" style={{ marginTop: 8 }}>
        <div className="scn-field">
          {/* O trigger do popover é um <button> nativo — vive AO LADO do label, nunca dentro
              (label com controle aninhado disputa o clique com o foco do input). O texto do
              trigger é só para leitor de tela; visualmente fica o marcador "i". */}
          <span className="scn-label-row">
            <label htmlFor="scn-loan-disb">Data do desembolso</label>
            <InfoPopover
              term={{
                title: "Desembolso",
                body: "O dia em que o dinheiro do empréstimo entra na sua conta. A partir dessa data o valor recebido soma na trajetória do cenário — as parcelas seguem o próprio calendário.",
              }}
            >
              <span style={SR_ONLY}>O que é a data do desembolso</span>
            </InfoPopover>
          </span>
          <input
            id="scn-loan-disb"
            type="date"
            value={disbursementDate}
            onChange={(e) => setDisbursementDate(e.target.value)}
            required
            aria-invalid={disbursementDateShowError || undefined}
            aria-describedby={
              disbursementDateShowError ? "scn-loan-disb-err" : undefined
            }
          />
          {disbursementDateShowError && (
            <p id="scn-loan-disb-err" className="scn-error">
              Informe a data em que o dinheiro entra.
            </p>
          )}
        </div>
        <div className="scn-field">
          <label htmlFor="scn-loan-first">Data da 1ª parcela</label>
          <input
            id="scn-loan-first"
            type="date"
            value={firstDate}
            onChange={(e) => setFirstDate(e.target.value)}
            required
            aria-invalid={firstDateShowError || undefined}
            aria-describedby={firstDateShowError ? "scn-loan-first-err" : undefined}
          />
          {firstDateShowError && (
            <p id="scn-loan-first-err" className="scn-error">
              Informe a data da primeira parcela.
            </p>
          )}
        </div>
      </div>
      {numericInputsValid && previewQ.loading && (
        <p className="scn-preview" aria-live="polite">
          Calculando a prévia do empréstimo…
        </p>
      )}
      {numericInputsValid && previewQ.error !== null && (
        <div className="scn-preview-error" role="alert">
          <p>Não foi possível calcular a prévia do empréstimo.</p>
          <Button size="sm" variant="secondary" onClick={() => invalidateCommands()}>
            Tentar novamente
          </Button>
        </div>
      )}
      {numericInputsValid &&
        !previewQ.loading &&
        previewQ.error === null &&
        previewQ.data !== undefined && (
          <div className="scn-loan-summary">
            <div className="scn-loan-summary__row">
              <span>Parcela</span>
              <Money cents={installmentCents} size="sm" />
            </div>
            <div className="scn-loan-summary__row">
              <span>Total pago</span>
              <Money cents={totalPaidCents} size="sm" />
            </div>
            <div className="scn-loan-summary__row">
              <span>Custo do crédito</span>
              <Money cents={creditCostCents} size="sm" />
            </div>
          </div>
        )}
      {error && (
        <p role="alert" className="scn-error">
          {error}
        </p>
      )}
      {confirmRestore && editing && (
        <p role="alert" className="scn-hint scn-loan-restore-note">
          Este empréstimo tem {editing.missingRows}{" "}
          {editing.missingRows === 1 ? "linha removida" : "linhas removidas"} à mão —
          salvar regenera a série completa e as restaura.
        </p>
      )}
      <div className="scn-loan-form-actions">
        <Button
          size="sm"
          variant="primary"
          iconLeft={<Landmark size={14} strokeWidth={1.75} />}
          onClick={() => void confirm()}
          disabled={
            busy ||
            !validInputs ||
            previewQ.loading ||
            previewQ.error !== null ||
            previewQ.data === undefined
          }
          className="scn-loan-confirm"
        >
          {busy
            ? editing
              ? "Salvando…"
              : "Criando…"
            : editing
              ? confirmRestore
                ? "Restaurar e salvar"
                : "Salvar alterações"
              : "Adicionar empréstimo ao cenário"}
        </Button>
        {editing && (
          <Button size="sm" variant="ghost" onClick={onCancelEdit} disabled={busy}>
            Cancelar edição
          </Button>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Superfície de comparação (canvas principal do Horizonte em modo comparação)
// ---------------------------------------------------------------------------

type DeltaSense = "higher-better" | "lower-better";

// ---------------------------------------------------------------------------
// Estados do método (Nível 2) — a HERO de cada card de KPI é o ESTADO (ícone + palavra + cor),
// nunca só cor; o valor compacto desce a evidência. Os rótulos
// vêm SEMPRE de um helper do método (`saldoBand`/`performanceStatus`/`custoVidaStatus`), nunca
// de texto solto aqui. A fidelidade literal preserva a linguagem canônica do método.
// ---------------------------------------------------------------------------

/** `key` decide TRANSIÇÃO (comparar real × cenário) — pode divergir do `label` renderizado
 * quando o rótulo embute um valor que muda toda hora sem ser uma mudança de ESTADO (ex.:
 * "Pode gastar hoje": "Livre até R$X" tem `key = "livre"` fixo). `line` é uma frase adicional
 * DATA-DERIVADA para a situação (nunca copy fixa de conceito — essa mora só no InfoPopover). */
interface MethodState {
  key: string;
  label: string;
  color: string;
  Icon: typeof CheckCircle2;
  line?: string;
}

/** Buraco do futuro & Saldo no fim: o Termômetro canônico (`saldoBand`, limiares ABSOLUTOS,
 * nunca relativos ao baseline) — rótulos e cores usados verbatim. */
function saldoState(cents: number): MethodState {
  const band = saldoBand(cents);
  const ok = band.key === "comfortable" || band.key === "ok";
  return {
    key: band.key,
    label: band.label,
    color: band.text,
    Icon: ok ? CheckCircle2 : AlertTriangle,
  };
}

/** Performance: `performanceStatus` verbatim ("Sobrou dinheiro"/"Faltou dinheiro" — ambos
 * método). "Faltou dinheiro" é uma quebra real de limiar (disciplina do vermelho: cor cheia). */
function performanceState(cents: number): MethodState {
  const s = performanceStatus(cents);
  const ok = s.level === "strong";
  return {
    key: s.label,
    label: s.label,
    color: ok ? "var(--success-400)" : "var(--danger-400)",
    Icon: ok ? CheckCircle2 : AlertTriangle,
  };
}

/** Custo de vida: `custoVidaStatus` verbatim ("Dentro da renda" é método; "Acima da renda" é
 * copy do Neko para o estado ruim — ver totaisStatus.ts). Nesta superfície de decisão de alto
 * risco, "Acima da renda" é tratada como quebra real de limiar (disciplina do vermelho: cor
 * cheia) — mais rígida que o âmbar ambiente do card "Este mês" (TotaisScreen). */
function custoVidaState(cost: number, income: number): MethodState {
  const s = custoVidaStatus(cost, income);
  const ok = s.label === "Dentro da renda";
  return {
    key: s.label,
    label: s.label,
    color: ok ? "var(--success-400)" : "var(--danger-400)",
    Icon: ok ? CheckCircle2 : AlertTriangle,
  };
}

/** Pode gastar hoje: sem helper de método pronto — estado por valor+régua. `cents` nunca é
 * negativo (o motor já despeja no piso 0), então só há duas categorias. */
function podeGastarState(cents: number, guardrail: "cash" | "savings"): MethodState {
  if (cents > 0) {
    return {
      key: "livre",
      label: `Livre até ${fmtCompactBRL(cents)}`,
      color: "var(--success-400)",
      Icon: CheckCircle2,
    };
  }
  return {
    key: "segure",
    label: "Segure hoje",
    color: "var(--warning-400)",
    Icon: AlertTriangle,
    line:
      guardrail === "savings"
        ? "Limitado pela régua de poupança (20–30% ao ano), não pelo caixa."
        : "Limitado pelo caixa do mês, não pela régua de poupança.",
  };
}

/**
 * Semáforo de meses de reserva PÓS-financiamento (`LoanBreakdown.reserve_months_after_
 * financing`) — a regra de reserva do método: mínimo 6 meses; 6–8 = zona amarela;
 * 12+ = verde/"paz". A faixa 8–12 fecha a progressão entre o amarelo e a paz sem nome
 * verbatim na fonte — "Confortável" é a leitura neutra do meio. Fronteiras com limite
 * SUPERIOR inclusivo (mesma convenção do Termômetro em `saldoBand`): 6–8 cobre até 8,0 exato;
 * 8–12 cobre de 8,0+ até 12,0 exato; abaixo de 6 é sempre abaixo do mínimo; acima de 12 é paz.
 * `--jade-400` cru falha contraste no tema claro (comentário em colors.css) — `--primary-
 * quiet-text` é o alias já testado pra texto jade legível nos dois temas (ver TotaisScreen).
 */
function reserveMonthsState(months: number): MethodState {
  if (months < 6) {
    return {
      key: "below-min",
      label: "Abaixo do mínimo",
      color: "var(--danger-400)",
      Icon: AlertTriangle,
    };
  }
  if (months <= 8) {
    return {
      key: "amber",
      label: "Zona amarela",
      color: "var(--warning-400)",
      Icon: AlertTriangle,
    };
  }
  if (months <= 12) {
    return {
      key: "comfortable",
      label: "Confortável",
      color: "var(--success-400)",
      Icon: CheckCircle2,
    };
  }
  return {
    key: "peace",
    label: "Paz",
    color: "var(--primary-quiet-text)",
    Icon: CheckCircle2,
  };
}

function ReserveMonthsBadge({ months }: { months: number }) {
  const state = reserveMonthsState(months);
  const { Icon } = state;
  const monthsLabel = months.toLocaleString("pt-BR", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  return (
    <span className="scn-loan-summary__reserve" style={{ color: state.color }}>
      <Icon size={13} strokeWidth={1.75} aria-hidden="true" />
      {state.label} · {monthsLabel} meses
    </span>
  );
}

/** Abaixo de R$1 de diferença é ruído de arredondamento, não um resultado — um card mostrando
 * "−R$ 0,09" em vermelho alarma por nada. Este limiar é sobre MATERIALIDADE (existe mudança
 * que importa?), então usa o valor absoluto em centavos direto, sem depender do sentido
 * (`sense`) — que só decide se um delta material é bom ou ruim, não se ele é relevante. */
const DELTA_MATERIALITY_CENTS = 100;

function deltaChip(deltaCents: number, sense: DeltaSense) {
  if (Math.abs(deltaCents) <= DELTA_MATERIALITY_CENTS) {
    return <span className="scn-kpi__delta scn-kpi__delta--quiet">≈ Sem mudança</span>;
  }
  // O glifo/ícone vem de better/worse (o que o `sense` deste KPI considera bom), NUNCA do
  // sinal cru do delta — o mesmo ▲ não pode significar "melhorou" num card e "piorou" noutro
  // só porque a métrica é "menor é melhor" (custo de vida). Cor+ícone+sinal sempre concordam.
  const better = sense === "higher-better" ? deltaCents > 0 : deltaCents < 0;
  const cls = better
    ? "scn-kpi__delta scn-kpi__delta--better"
    : "scn-kpi__delta scn-kpi__delta--worse";
  const Icon = better ? TrendingUp : TrendingDown;
  return (
    <span className={cls}>
      <Icon size={12} strokeWidth={1.75} aria-hidden="true" />
      <SignedMoney cents={deltaCents} size="inherit" />
    </span>
  );
}

function KpiCard({
  label,
  term,
  realCents,
  scenarioCents,
  deltaCents,
  sense,
  realState,
  scenarioState,
  emptyScenario = false,
}: {
  label: string;
  term: { title: string; body: string };
  realCents: number;
  scenarioCents: number;
  deltaCents: number;
  sense: DeltaSense;
  realState: MethodState;
  scenarioState: MethodState;
  /** Cenário sem NENHUM ponto de projeção: quando
   * `true`, `scenarioCents`/`deltaCents` são ruído (`?? 0` do chamador) e nunca aparecem — o
   * card renderiza um vazio neutro ("—", sem cor de estado) em vez de fingir "Apertado R$ 0".
   * `scenarioState` ainda deve chegar neutra (ver `EMPTY_SCENARIO_STATE`); só ela controla a
   * cor/ícone do Nível 2, mas o headline/evidência/delta são sempre suprimidos aqui. */
  emptyScenario?: boolean;
}) {
  // STATE TRANSITIONS: quando o estado do cenário DIFERE do estado real, a
  // hero usa o estado do cenário com a origem numa linha discreta empilhada abaixo — nunca
  // inline (rótulos de estado são compridos; inline quebraria feio numa coluna estreita com o
  // ícone órfão — o inline "velho → novo" fica reservado à linha numérica de evidência). Compara
  // por `key` (categoria), não pelo `label` renderizado: "Pode gastar hoje" embute o valor no
  // label ("Livre até R$X"), que muda toda hora sem ser uma mudança de ESTADO. Sem projeção
  // nenhuma do cenário, "real vs vazio" não é uma transição de estado — é ausência de dado.
  const isTransition = !emptyScenario && realState.key !== scenarioState.key;
  const { Icon } = scenarioState;
  const stateAnnouncement = isTransition
    ? `${scenarioState.label} (antes ${realState.label})`
    : scenarioState.label;
  // O PORQUÊ do estado ("Limitado pela régua de poupança…") entra no anúncio: a linha visual é
  // aria-hidden (fonte única no aria-label), então sem isto o leitor de tela ouviria "Segure
  // hoje" sem a razão que a tela mostra.
  const stateWithReason = scenarioState.line
    ? `${stateAnnouncement} — ${scenarioState.line}`
    : stateAnnouncement;

  return (
    <article
      className="scn-kpi"
      aria-label={
        emptyScenario
          ? `${label}: sem dados de projeção do cenário, real ${fmtBRL(realCents)}`
          : `${label}: ${stateWithReason}, real ${fmtBRL(realCents)}, cenário ${fmtBRL(scenarioCents)}`
      }
    >
      <span className="scn-kpi__label">
        <InfoPopover term={term} hideMarker>
          {label}
        </InfoPopover>
      </span>
      {/* Estado (Nível 2): a HERO do card é o estado do método — ícone + palavra + cor, nunca só
          cor. Visual-only: o aria-label do article (acima) já anuncia o estado por extenso. */}
      <div
        className="scn-kpi__state"
        style={{ color: scenarioState.color }}
        aria-hidden="true"
      >
        <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
        <span className="scn-kpi__state-word">{scenarioState.label}</span>
      </div>
      {isTransition && (
        <span className="scn-kpi__state-origin" aria-hidden="true">
          Antes: {realState.label}
        </span>
      )}
      {scenarioState.line && (
        <p className="scn-kpi__state-line" aria-hidden="true">
          {scenarioState.line}
        </p>
      )}
      {/* Manchete (Nível 3): só o valor do CENÁRIO, compacto — nunca dois valores de precisão
          cheia numa linha sem quebra, pois eles não cabem em cards estreitos. Precisão
          cheia continua acessível: no aria-label do próprio article (acima). */}
      <span className="scn-kpi__headline" aria-hidden="true">
        {emptyScenario ? "—" : fmtCompactBRL(scenarioCents)}
      </span>
      {/* Evidência visual-only: o aria-label do article já anuncia real e cenário em precisão
          cheia — sem o aria-hidden, os dois <Money> anunciariam os MESMOS valores de novo
          (leitor de tela ouvindo tudo em dobro). */}
      <div className="scn-kpi__evidence" aria-hidden="true">
        <Money cents={realCents} size="inherit" />
        <ArrowRight size={12} strokeWidth={2} className="scn-kpi__arrow" />
        {emptyScenario ? "—" : <Money cents={scenarioCents} size="inherit" />}
      </div>
      {!emptyScenario && deltaChip(deltaCents, sense)}
    </article>
  );
}

/** Estado neutro para quando o cenário não tem NENHUM ponto de projeção —
 * nem `deepest_deficit` diário, nem `month_end` mensal. Nunca reutilizar `saldoState(0)` aqui:
 * 0 cai na banda "apertado" do Termômetro por coincidência aritmética do `?? 0`, não porque o
 * cenário tenha de fato um menor saldo — mostraria "Apertado" colorido sobre um dado inexistente.
 * `--text-faint` é a MESMA cor "sem valor" que `saldoBand(null)` já usa (nkFormat.ts). */
const EMPTY_SCENARIO_STATE: MethodState = {
  key: "none",
  label: "—",
  color: "var(--text-faint)",
  Icon: Minus,
};

/** Remove marcas + limita a 60 chars para caber na linha do chip de mudança. */
function changeLabel(desc: string): string {
  const clean = stripScenarioMarker(desc) || "Sem descrição";
  return clean.length > 60 ? `${clean.slice(0, 57)}…` : clean;
}

type VerdictTier = "risk" | "tight" | "ok";

interface ScenarioVerdict {
  tier: VerdictTier;
  headline: string;
  subline: string;
}

/** Menor saldo do CENÁRIO + mês (0–11) na melhor resolução disponível: `deepest_deficit`
 * (diária) quando o motor o tem; quando null, o mínimo do `scenario_month_end` (mensal — o
 * mesmo dado do gráfico); `null` sem projeção nenhuma. FONTE ÚNICA do banner de veredito E do
 * card "Buraco do futuro". Uma fonte única impede que o card use o fallback `?? 0` enquanto o
 * banner usa o mínimo mensal, evitando que ambos discordem sobre o mesmo dado. */
function scenarioDeepestPoint(
  compare: ScenarioCompareDto,
): { minCents: number; monthIdx: number } | null {
  const deficit = compare.scenario_deepest_deficit;
  if (deficit) {
    return { minCents: deficit.balance_cents, monthIdx: monthOf(deficit.date) };
  }
  if (compare.scenario_month_end.length > 0) {
    const worst = compare.scenario_month_end.reduce((a, b) =>
      b.balance_cents < a.balance_cents ? b : a,
    );
    return { minCents: worst.balance_cents, monthIdx: worst.month - 1 };
  }
  return null;
}

/** Veredito (Nível 1): a resposta a "é seguro?" de relance, ANTES da grade de
 * KPIs — determinístico a partir do menor saldo do CENÁRIO (`scenarioDeepestPoint`, o mesmo dado
 * que alimenta o card "Buraco do futuro" e o gráfico — nunca um número novo). O TOM vem do MESMO
 * predicado do card (`saldoBand`, o Termômetro canônico), em três níveis: banda negativa/crítica
 * → risco (vermelho); banda apertada → intermediário honesto (âmbar) — sem isto o banner diria
 * "no azul o ano todo" enquanto o card logo abaixo mostra "Apertado" em âmbar sobre o MESMO
 * número; banda ok/folga → azul (verde). Tom GPS-não-ameaça: cada ramo ruim sugere uma ação,
 * não um alarme. Sem NENHUM ponto de projeção: nível ok com a subline dizendo isso, em vez de
 * inventar um menor saldo. */
function scenarioVerdict(compare: ScenarioCompareDto): ScenarioVerdict {
  const point = scenarioDeepestPoint(compare);
  if (point == null) {
    return {
      tier: "ok",
      headline: "Este cenário se mantém no azul o ano todo.",
      subline: "Sem pontos de projeção no horizonte para apontar um menor saldo.",
    };
  }
  const { minCents, monthIdx } = point;
  const band = saldoBand(minCents);
  const monthLabel = (MES[monthIdx] ?? "").toLowerCase();
  if (band.key === "negative" || band.key === "critical") {
    return {
      tier: "risk",
      headline: `Fura o caixa em ${monthLabel} — faltam ${fmtCompactBRL(Math.abs(minCents))}.`,
      subline:
        "Antecipe uma entrada, reduza uma parcela ou cubra com um empréstimo antes desse mês.",
    };
  }
  if (band.key === "tight") {
    return {
      tier: "tight",
      headline: `Fica apertado em ${monthLabel} — menor saldo ${fmtCompactBRL(minCents)}.`,
      subline: "Segure gastos grandes perto dessa data ou reforce o colchão antes.",
    };
  }
  return {
    tier: "ok",
    headline: "Este cenário se mantém no azul o ano todo.",
    subline: `Menor saldo no período: ${fmtBRL(minCents)} — ${band.label}.`,
  };
}

/** Gêmeo VISÍVEL da região aria-live (que continua existindo e anunciando cada recomputo) —
 * ícone + palavra + cor, nunca só cor; borda tintada reforça sem depender só do texto. */
function ScenarioVerdictBanner({ compare }: { compare: ScenarioCompareDto }) {
  const verdict = scenarioVerdict(compare);
  return (
    <div className={`scn-verdict scn-verdict--${verdict.tier}`}>
      <span className="scn-verdict__icon" aria-hidden="true">
        {verdict.tier === "ok" ? (
          <CheckCircle2 size={20} strokeWidth={1.75} />
        ) : (
          <AlertTriangle size={20} strokeWidth={1.75} />
        )}
      </span>
      <div>
        <p className="scn-verdict__headline">{verdict.headline}</p>
        <p className="scn-verdict__subline">{verdict.subline}</p>
      </div>
    </div>
  );
}

export function ScenarioCompare({
  compare,
  onClose,
}: {
  compare: ScenarioCompareDto;
  /** Sai do modo comparação (deseleciona o cenário) — sem isto o único caminho de volta ao
   *  Horizonte normal era reabrir o sheet e des-clicar o cenário (ou trocar de tela). */
  onClose?: () => void;
}) {
  const lastMonthEnd = compare.month_end[compare.month_end.length - 1] ?? null;
  const endRealCents = lastMonthEnd?.real_balance_cents ?? 0;
  const endScenarioCents = lastMonthEnd?.scenario_balance_cents ?? 0;
  const endDeltaCents = lastMonthEnd?.delta_cents ?? 0;

  const realDeficit = compare.real_deepest_deficit?.balance_cents ?? 0;
  // Menor saldo do cenário pela MESMA derivação do banner (`scenarioDeepestPoint`) — nunca o
  // `?? 0` cru sobre o deficit diário: com deficit nulo mas `scenario_month_end` presente, o
  // card fabricava "cenário R$ 0,00" (+ delta fake) enquanto o banner logo acima caía
  // honestamente no mínimo mensal — banner e card discordando sobre o MESMO dado. Só sem
  // projeção NENHUMA (deficit E month_end vazios) o card rende o vazio neutro.
  const scenarioPoint = scenarioDeepestPoint(compare);
  const noScenarioProjection = scenarioPoint == null;
  // O 0 do fallback nunca renderiza: `emptyScenario` suprime manchete/evidência/delta.
  const scenarioDeficit = scenarioPoint?.minCents ?? 0;
  // Delta do backend quando existe (deficit diário nos DOIS ramos); senão derivado dos mesmos
  // números que a linha de evidência mostra — o chip nunca pode discordar da evidência.
  const deficitDelta =
    compare.deepest_deficit_delta_cents ??
    (scenarioPoint != null ? scenarioPoint.minCents - realDeficit : 0);

  return (
    <section className="card" aria-label="Comparação real × cenário">
      {/* Região live: só anuncia quando o TEXTO muda, então o anúncio precisa carregar um valor
          que muda a cada recomputo (o delta do saldo final muda em qualquer edição — lançamento,
          override, empréstimo), não apenas o nome do cenário selecionado. */}
      <div
        className="scn-compare-live"
        aria-live="polite"
        data-testid="scn-live-region"
      >
        Comparação atualizada: {compare.scenario_name}, saldo final{" "}
        <SignedMoney cents={endDeltaCents} size="sm" /> versus o real
      </div>
      <div className="card__head">
        <span className="card__title">
          <CircleDollarSign size={16} strokeWidth={1.75} className="ic" />
          Cenário: {compare.scenario_name}
        </span>
        {onClose && (
          <Button variant="ghost" size="sm" onClick={onClose}>
            <X size={14} strokeWidth={1.75} aria-hidden="true" />
            Fechar comparação
          </Button>
        )}
      </div>
      <div
        className="card__body"
        style={{ display: "flex", flexDirection: "column", gap: 20 }}
      >
        <ScenarioVerdictBanner compare={compare} />

        {/* Ordem por prioridade de decisão (padrão-Z): Buraco do futuro, Saldo no fim, Pode
            gastar hoje, Performance, Custo de vida. */}
        <div className="scn-kpis">
          <KpiCard
            label="Buraco do futuro"
            term={{
              title: "Buraco do futuro",
              body: "O menor saldo que sua projeção alcança daqui pra frente — o pior momento de caixa. Se ele fica negativo, você precisa de um plano antes de chegar lá.",
            }}
            realCents={realDeficit}
            scenarioCents={scenarioDeficit}
            deltaCents={deficitDelta}
            sense="higher-better"
            realState={saldoState(realDeficit)}
            scenarioState={
              noScenarioProjection ? EMPTY_SCENARIO_STATE : saldoState(scenarioDeficit)
            }
            emptyScenario={noScenarioProjection}
          />
          <KpiCard
            label="Saldo no fim do horizonte"
            term={{
              title: "Saldo no fim",
              body: "O saldo projetado no último mês do horizonte se nada mudar.",
            }}
            realCents={endRealCents}
            scenarioCents={endScenarioCents}
            deltaCents={endDeltaCents}
            sense="higher-better"
            realState={saldoState(endRealCents)}
            scenarioState={saldoState(endScenarioCents)}
          />
          <KpiCard
            label="Pode gastar hoje"
            term={{
              title: "Pode gastar hoje",
              body: "Quanto dá pra gastar agora sem furar o caixa do mês nem a régua de poupança de 20–30%.",
            }}
            realCents={compare.real_safe_to_spend_today_cents}
            scenarioCents={compare.scenario_safe_to_spend_today_cents}
            deltaCents={compare.safe_to_spend_delta_cents}
            sense="higher-better"
            realState={podeGastarState(
              compare.real_safe_to_spend_today_cents,
              compare.real_binding_guardrail,
            )}
            scenarioState={podeGastarState(
              compare.scenario_safe_to_spend_today_cents,
              compare.scenario_binding_guardrail,
            )}
          />
          <KpiCard
            label="Performance · mês atual"
            term={{
              title: "Performance",
              body: "Entradas menos as saídas do mês — fixas, diário, economia, cartão e a previsão do diário que ainda falta. A economia e essa previsão contam como saída, então o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
            }}
            realCents={compare.real_performance_cents}
            scenarioCents={compare.scenario_performance_cents}
            deltaCents={compare.performance_delta_cents}
            sense="higher-better"
            realState={performanceState(compare.real_performance_cents)}
            scenarioState={performanceState(compare.scenario_performance_cents)}
          />
          <KpiCard
            label="Custo de vida"
            term={{
              title: "Custo de vida",
              body: "Quanto sai por mês pra manter sua vida — fixas + diário + cartão. Não inclui economia (poupança não é custo), e é sobre ele que a reserva se dimensiona.",
            }}
            realCents={compare.real_cost_of_living_cents}
            scenarioCents={compare.scenario_cost_of_living_cents}
            deltaCents={compare.cost_of_living_delta_cents}
            sense="lower-better"
            realState={custoVidaState(
              compare.real_cost_of_living_cents,
              compare.real_income_cents,
            )}
            scenarioState={custoVidaState(
              compare.scenario_cost_of_living_cents,
              compare.scenario_income_cents,
            )}
          />
        </div>

        {compare.month_end.length > 0 && <DualLineChart compare={compare} />}
        {compare.month_end.length > 0 && <DiffSparkline monthEnd={compare.month_end} />}

        <ChangesList changes={compare.changes} />

        {compare.loan && (
          <div className="scn-loan-summary">
            <p className="scn-section-title" style={{ margin: 0 }}>
              Empréstimo simulado
            </p>
            <div className="scn-loan-summary__row">
              <span>Custo do crédito</span>
              <Money cents={compare.loan.loan_total_cost_cents} size="sm" />
            </div>
            <div className="scn-loan-summary__row">
              <span>Total pago</span>
              <Money cents={compare.loan.loan_total_paid_cents} size="sm" />
            </div>
            {compare.loan.reserve_months_after_financing != null && (
              <div className="scn-loan-summary__row">
                <span>
                  <InfoPopover
                    hideMarker
                    term={{
                      title: "Reserva após financiar",
                      body: "Quantos meses de custo de vida a sua reserva cobriria depois de assumir o financiamento. A régua: abaixo de 6 meses é abaixo do mínimo; de 6 a 8, zona amarela; de 8 a 12, confortável; acima de 12, paz — folga de sobra para financiar sem ansiedade.",
                    }}
                  >
                    Reserva após financiar
                  </InfoPopover>
                </span>
                <ReserveMonthsBadge
                  months={compare.loan.reserve_months_after_financing}
                />
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

/**
 * `ScenarioChange` não expõe `kind`, portanto os valores desta lista permanecem sem a cor de
 * movimento de `TYPE_META`. O DTO contém apenas `op`, `description`, `from_date`,
 * `old_amount_cents` e `new_amount_cents`.
 *
 * Derivar o tipo no cliente exigiria um join por descrição, data e valor, uma chave sintética sem
 * identidade estável, e ainda deixaria trocas de recorrência sem classificação. Colorir com
 * segurança exige que o backend inclua `kind` em `ScenarioChange`, a partir da mesma origem que
 * preenche `HypoTxnRow.ttype` e `Obligation.kind`.
 */
function ChangesList({ changes }: { changes: ScenarioCompareDto["changes"] }) {
  if (changes.length === 0) {
    return <p className="scn-empty">Nenhuma mudança neste cenário ainda.</p>;
  }
  return (
    <div>
      <p className="scn-section-title">O que mudou</p>
      <div className="scn-changes">
        {changes.map((c) => (
          <div
            className="scn-change-row"
            key={`${c.op}:${c.description}:${c.from_date}:${c.old_amount_cents ?? ""}:${c.new_amount_cents ?? ""}`}
          >
            <span
              className={
                "scn-chip " +
                (c.op === "add"
                  ? "scn-chip--add"
                  : c.op === "remove"
                    ? "scn-chip--remove"
                    : "scn-chip--replace")
              }
            >
              {c.op === "add"
                ? "+ Adicionado"
                : c.op === "remove"
                  ? "− Removido"
                  : "↔ Alterado"}
            </span>
            <span className="scn-change-row__desc">{changeLabel(c.description)}</span>
            <span className="scn-change-row__amt">
              {c.op === "replace" ? (
                <>
                  {c.old_amount_cents != null ? (
                    <Money cents={c.old_amount_cents} size="inherit" />
                  ) : (
                    "—"
                  )}{" "}
                  →{" "}
                  {c.new_amount_cents != null ? (
                    <Money cents={c.new_amount_cents} size="inherit" />
                  ) : (
                    "—"
                  )}
                </>
              ) : (
                <Money
                  cents={
                    (c.old_amount_cents ?? c.new_amount_cents ?? 0) *
                    (c.op === "remove" ? -1 : 1)
                  }
                  size="inherit"
                />
              )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Gutter reservado à direita do plot para os rótulos de fim de linha (~72px): sem ele "Real"/
 *  "Simulação" caem sobre o traço quando as linhas convergem no fim do horizonte. O plot termina
 *  antes do gutter; o texto começa dentro dele.
 *  (O vão vertical mínimo entre os rótulos vive em `scenarioHelpers.CHART_LABEL_MIN_GAP`.) */
const CHART_LABEL_GUTTER = 72;

/** Largura REAL do container (ResizeObserver) para desenhar o SVG 1:1 em pixels. Um viewBox
 *  fixo escala a tipografia interna junto com a largura — texto gigante em janela cheia OU
 *  minúsculo com o side-sheet aberto, nunca os dois certos. Medindo, 11px são 11px sempre;
 *  só o PLOT estica. Antes da primeira medição rende o fallback (evita flash de layout). */
function useMeasuredWidth(
  ref: React.RefObject<HTMLDivElement | null>,
  fallback: number,
) {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? 0;
      if (w > 0) setWidth(Math.round(w));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return width || fallback;
}

/**
 * Trajetória mensal real × simulação. Três decisões estruturais:
 * - Domínio Y "nice" (`niceChartScale`) SÓ sobre os pontos mensais desenhados — nunca forçado
 *   ao zero: um gráfico de linha mostra VARIAÇÃO, e ancorar um saldo de R$ 30 mil no zero
 *   esmagava as duas linhas numa faixa de pixels. O zero entra como LIMIAR condicional
 *   (tracejado de perigo) apenas quando o domínio o contém.
 * - Resolução honesta: este gráfico é o SALDO NO FIM DE CADA MÊS. O menor saldo DIÁRIO
 *   (buraco do futuro) tem outra resolução e NÃO ganha coordenada aqui — plotá-lo na linha
 *   mensal (o defeito anterior) desenhava um ponto órfão fora das linhas. O marcador do
 *   gráfico é o pior FIM DE MÊS da simulação; o buraco diário vive no veredito/KPI acima.
 * - Grade horizontal + ticks Y (`fmtAxisBRL`) + meses no eixo X + hover com leitura exata —
 *   a linguagem de gráfico do DS (readme §8), a mesma do `BalanceTrajectory`.
 */
function DualLineChart({ compare }: { compare: ScenarioCompareDto }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const points = compare.month_end;
  const W = useMeasuredWidth(wrapRef, 960);
  const H = 220;
  const padLeft = 72; // gutter dos ticks Y — "R$ 35 mil" em mono 10.5px ocupa ~56px + respiro
  const padRight = 12 + CHART_LABEL_GUTTER;
  const padTop = 14;
  const padBottom = 26;
  const innerW = W - padLeft - padRight;
  const innerH = H - padTop - padBottom;

  const allVals = points.flatMap((p) => [
    p.real_balance_cents,
    p.scenario_balance_cents,
  ]);
  const scale = niceChartScale(allVals);
  const range = scale.max - scale.min || 1;
  const x = (i: number) =>
    padLeft + (points.length === 1 ? innerW / 2 : (i / (points.length - 1)) * innerW);
  const y = (cents: number) => padTop + innerH - ((cents - scale.min) / range) * innerH;

  const realPts = points.map((p, i) => `${x(i)},${y(p.real_balance_cents)}`).join(" ");
  const scenarioPts = points
    .map((p, i) => `${x(i)},${y(p.scenario_balance_cents)}`)
    .join(" ");

  // Pior FIM DE MÊS da simulação — o marcador na resolução DESTE gráfico. Só rende quando
  // é de fato o vale da série (não o último ponto de uma linha monotônica, que já tem os
  // rótulos de fim de linha ao lado).
  let worstIdx = 0;
  points.forEach((p, i) => {
    if (p.scenario_balance_cents < (points[worstIdx]?.scenario_balance_cents ?? 0)) {
      worstIdx = i;
    }
  });
  const worst = points[worstIdx];
  const showWorst = points.length > 1 && worstIdx !== points.length - 1;

  const hasNegative = allVals.some((v) => v < 0);
  const zeroInDomain = scale.min <= 0 && scale.max >= 0;

  const lastPoint = points[points.length - 1];
  const ariaLabel =
    `Saldo no fim de cada mês, real versus simulação — resolução mensal. Real termina em ${fmtBRL(
      lastPoint?.real_balance_cents ?? 0,
    )}. Simulação termina em ${fmtBRL(lastPoint?.scenario_balance_cents ?? 0)}.` +
    (worst
      ? ` Pior fim de mês da simulação: ${fmtBRL(worst.scenario_balance_cents)} em ${MES[worst.month - 1]}.`
      : "") +
    " O menor saldo diário (buraco do futuro) aparece no veredito e no cartão acima, não nesta linha.";

  let labelX = 0;
  let realLabelY = 0;
  let scenarioLabelY = 0;
  if (lastPoint) {
    labelX = x(points.length - 1) + 12;
    // Colocação direction-aware + clamp do PAR (nunca de cada rótulo isolado, que comprimia
    // o vão de volta perto das bordas) — geometria pura e testada em `scenarioHelpers`.
    const placed = placeChartEndLabels(
      y(lastPoint.real_balance_cents),
      y(lastPoint.scenario_balance_cents),
      padTop + 8,
      H - padBottom - 2,
    );
    realLabelY = placed.realLabelY;
    scenarioLabelY = placed.scenarioLabelY;
  }

  // Hover por proximidade horizontal (mesmo gesto do BalanceTrajectory): o índice mais
  // próximo do cursor no espaço do PLOT (não do wrapper — o padLeft dos ticks descontado).
  const onMove = (e: React.MouseEvent) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || points.length === 0) return;
    const fx = ((e.clientX - rect.left) / rect.width) * W;
    const i = Math.max(
      0,
      Math.min(
        points.length - 1,
        Math.round(((fx - padLeft) / innerW) * (points.length - 1)),
      ),
    );
    setHover(i);
  };
  const hovered = hover != null ? points[hover] : null;
  const hoverFrac = hover != null ? x(hover) / W : 0;

  return (
    <div>
      <div className="scn-dualchart__head">
        <p className="scn-section-title" style={{ margin: 0 }}>
          Trajetória — saldo no fim de cada mês
        </p>
        {/* Redundância: a legenda repete cor+traço com texto — nunca só a cor conta a
            história (regra do DS: status nunca é só cor). */}
        <div className="scn-dualchart__legend">
          <span className="scn-dualchart__legend-item">
            <span className="scn-dualchart__legend-swatch scn-dualchart__legend-swatch--real" />
            Real
          </span>
          <span className="scn-dualchart__legend-item">
            <span className="scn-dualchart__legend-swatch scn-dualchart__legend-swatch--scenario" />
            Simulação
          </span>
        </div>
      </div>
      <div
        ref={wrapRef}
        className="scn-dualchart-wrap"
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
      >
        <svg
          className="scn-dualchart"
          viewBox={`0 0 ${W} ${H}`}
          role="img"
          aria-label={ariaLabel}
        >
          {/* Grade horizontal + ticks Y: só linhas horizontais (--chart-grid), rótulos em
              mono micro (--chart-axis) — Y implícito pela grade, sem eixo pesado (DS §8). */}
          {scale.ticks.map((t) => (
            <g key={t}>
              <line
                className="scn-dualchart__grid"
                x1={padLeft}
                x2={W - padRight + 6}
                y1={y(t)}
                y2={y(t)}
              />
              <text className="scn-dualchart__tick" x={padLeft - 8} y={y(t) + 3.5}>
                {fmtAxisBRL(t)}
              </text>
            </g>
          ))}
          {/* Zero como LIMIAR (não baseline): tracejado de perigo quando há mês negativo;
              com domínio contendo 0 mas tudo no azul, a grade comum já o mostra. */}
          {zeroInDomain && hasNegative && (
            <line
              className="scn-dualchart__zero"
              x1={padLeft}
              x2={W - padRight + 6}
              y1={y(0)}
              y2={y(0)}
            />
          )}
          {/* Meses no eixo X — âncora das pontas para dentro (mesma regra do DiffSparkline). */}
          {points.map((p, i) => (
            <text
              key={`${p.year}-${p.month}`}
              className="scn-dualchart__xlabel"
              x={x(i)}
              y={H - 8}
              textAnchor={
                i === 0 ? "start" : i === points.length - 1 ? "end" : "middle"
              }
            >
              {MES_ABBR[p.month - 1]}
            </text>
          ))}

          <polyline className="scn-dualchart__real" points={realPts} />
          <polyline className="scn-dualchart__scenario" points={scenarioPts} />

          {/* Pontos vazados em cada mês (hollow dots do DS): tornam a resolução mensal
              legível — a linha é uma interpolação entre 12 fatos, não um contínuo diário. */}
          {points.map((p, i) => (
            <g key={`dot-${p.year}-${p.month}`} aria-hidden="true">
              <circle
                className="scn-dualchart__dot scn-dualchart__dot--real"
                cx={x(i)}
                cy={y(p.real_balance_cents)}
                r={hover === i ? 4 : 2.75}
              />
              <circle
                className="scn-dualchart__dot scn-dualchart__dot--scenario"
                cx={x(i)}
                cy={y(p.scenario_balance_cents)}
                r={hover === i ? 4 : 2.75}
              />
            </g>
          ))}

          {/* Crosshair do hover */}
          {hovered && (
            <line
              aria-hidden="true"
              className="scn-dualchart__crosshair"
              x1={x(hover!)}
              x2={x(hover!)}
              y1={padTop}
              y2={H - padBottom}
            />
          )}

          {/* Pior fim de mês da SIMULAÇÃO — valor na resolução do gráfico, cor pela regra
              de sinal do dinheiro (nunca cor de série para sinal, DS §8). */}
          {showWorst && worst && (
            <g aria-hidden="true">
              <circle
                cx={x(worstIdx)}
                cy={y(worst.scenario_balance_cents)}
                r={3.5}
                fill={
                  worst.scenario_balance_cents < 0
                    ? "var(--danger-400)"
                    : "var(--text-faint)"
                }
              />
              <text
                className="scn-dualchart__worst"
                x={
                  x(worstIdx) < padLeft + 60
                    ? x(worstIdx) + 8
                    : Math.min(W - padRight - 20, x(worstIdx))
                }
                y={Math.min(y(worst.scenario_balance_cents) + 18, H - padBottom - 4)}
                textAnchor={x(worstIdx) < padLeft + 60 ? "start" : "middle"}
                fill={
                  worst.scenario_balance_cents < 0
                    ? "var(--danger-400)"
                    : "var(--text-muted)"
                }
              >
                {fmtAxisBRL(worst.scenario_balance_cents)}
              </text>
            </g>
          )}

          {lastPoint && (
            <>
              {/* Halo (paint-order: stroke) como segunda defesa, MELHOR-ESFORÇO: o suporte a
                  paint-order em <text> é irregular fora de Chromium/WebView2 (ex.: WebKitGTK) —
                  a defesa primária é o GUTTER à direita do plot, que vale em qualquer engine. */}
              <text
                className="scn-dualchart__label"
                x={labelX}
                y={realLabelY}
                textAnchor="start"
                fontSize="11"
                fontWeight="600"
                fill="var(--primary)"
                stroke="var(--surface)"
                strokeWidth={3}
                paintOrder="stroke"
              >
                Real
              </text>
              <text
                className="scn-dualchart__label"
                x={labelX}
                y={scenarioLabelY}
                textAnchor="start"
                fontSize="11"
                fontWeight="600"
                fill="var(--sim-scenario)"
                stroke="var(--surface)"
                strokeWidth={3}
                paintOrder="stroke"
              >
                Simulação
              </text>
            </>
          )}
        </svg>

        {/* Tooltip de hover (HTML sobre o gráfico, mesmo vocabulário do BalanceTrajectory).
            aria-hidden: leitura exata já está no aria-label do SVG + cards acima. */}
        {hovered && (
          <div
            className="nk-spark__tip"
            aria-hidden="true"
            style={{
              left: `${hoverFrac * 100}%`,
              transform: `translateX(${hoverFrac > 0.82 ? "-100%" : hoverFrac < 0.18 ? "0" : "-50%"})`,
            }}
          >
            <span className="nk-spark__tip-day">
              {MES[hovered.month - 1]} {hovered.year}
            </span>
            <span className="scn-dualchart__tip-row">
              <span className="scn-dualchart__legend-swatch scn-dualchart__legend-swatch--real" />
              <span className="nk-spark__tip-val">
                {formatBRL(hovered.real_balance_cents)}
              </span>
            </span>
            <span className="scn-dualchart__tip-row">
              <span className="scn-dualchart__legend-swatch scn-dualchart__legend-swatch--scenario" />
              <span className="nk-spark__tip-val">
                {formatBRL(hovered.scenario_balance_cents)}
              </span>
            </span>
            <span className="scn-dualchart__tip-delta">
              Δ {fmtSigned(hovered.delta_cents)}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

function DiffSparkline({ monthEnd }: { monthEnd: ScenarioCompareDto["month_end"] }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const W = useMeasuredWidth(wrapRef, 960);
  const H = 150;
  // Gutter um pouco maior que o mínimo geométrico: com `textAnchor="middle"` nos rótulos do
  // meio, o mês nas duas pontas (jan/dez) ainda teria metade do texto pra fora do viewBox só
  // com 12px — o mesmo defeito de colisão de borda do DualLineChart, em miniatura.
  const padX = 18;
  const padTop = 16;
  const padBottom = 22;
  const innerW = W - padX * 2;
  const innerH = H - padTop - padBottom;

  const vals = monthEnd.map((m) => m.delta_cents);
  const max = Math.max(...vals, 0);
  const min = Math.min(...vals, 0);
  const range = max - min || 1;
  const x = (i: number) =>
    padX + (monthEnd.length === 1 ? innerW / 2 : (i / (monthEnd.length - 1)) * innerW);
  const y = (cents: number) => padTop + innerH - ((cents - min) / range) * innerH;
  const zeroY = y(0);

  const linePts = monthEnd.map((m, i) => `${x(i)},${y(m.delta_cents)}`).join(" ");
  const areaPathTop = `M ${x(0)},${zeroY} L ${linePts.replace(/ /g, " L ")} L ${x(monthEnd.length - 1)},${zeroY} Z`;

  let worstIdx = 0;
  for (let i = 1; i < monthEnd.length; i++) {
    if ((monthEnd[i]?.delta_cents ?? 0) < (monthEnd[worstIdx]?.delta_cents ?? 0)) {
      worstIdx = i;
    }
  }
  const worst = monthEnd[worstIdx];
  const gid = "scn-diff-grad";

  // O gradiente divergente troca de cor NO ZERO REAL, não no meio do quadro: com
  // `userSpaceOnUse` o eixo do gradiente é fixado no espaço do viewBox (padTop → base do
  // plot) e o ponto de virada é a fração exata do zeroY. Com stops a 50% (o defeito), um ano
  // todo-negativo ganhava metade superior VERDE — cor afirmando "melhor" onde tudo era pior.
  // Nos extremos a fração satura em 0/1 e o lado ausente colapsa (sem faixa fantasma).
  const zeroFrac = Math.max(0, Math.min(1, (zeroY - padTop) / (innerH || 1)));

  // Rótulos de ZONA dentro do plot — a resposta didática a "o que este gráfico quer dizer?":
  // posição acima/abaixo do zero É a mensagem, então ela vira texto onde acontece. Cada
  // rótulo só aparece quando a zona tem altura para ele (não espreme em cima da linha).
  const aboveH = zeroY - padTop;
  const belowH = padTop + innerH - zeroY;
  const ZONE_LABEL_MIN_H = 26;

  // Alternativa textual do gráfico (mesmo padrão do ariaLabel do DualLineChart): sempre
  // presente — inclusive quando o cenário é melhor em todos os meses e a nota visual de
  // "Pior mês" não renderiza. A frase didática das zonas viaja junto.
  const ariaLabel =
    "Diferença mês a mês entre simulação e real — acima de zero a simulação deixa mais dinheiro; abaixo, menos. " +
    (worst && worst.delta_cents < 0
      ? `Pior mês: ${MES[worst.month - 1]} ${fmtBRL(worst.delta_cents)}.`
      : "A simulação fica igual ou melhor que o real em todos os meses.");

  const onMove = (e: React.MouseEvent) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || monthEnd.length === 0) return;
    const fx = ((e.clientX - rect.left) / rect.width) * W;
    const i = Math.max(
      0,
      Math.min(
        monthEnd.length - 1,
        Math.round(((fx - padX) / innerW) * (monthEnd.length - 1)),
      ),
    );
    setHover(i);
  };
  const hovered = hover != null ? monthEnd[hover] : null;
  const hoverFrac = hover != null ? x(hover) / W : 0;

  return (
    <div>
      {/* Mesmo padrão de título do `DualLineChart` logo acima (`__head` com margem zerada) —
          gráfico de UMA série (a diferença): sem legenda de cor; o zero tracejado + rótulos
          de zona contam a história por posição E por palavra. */}
      <div className="scn-diffchart__head">
        <p className="scn-section-title" style={{ margin: 0 }}>
          <InfoPopover
            term={{
              title: "Diferença mês a mês",
              body: "Cada ponto é o saldo da simulação menos o saldo real no fim daquele mês. Acima da linha do zero, o cenário te deixa com mais dinheiro que hoje; abaixo, com menos. Quanto mais longe do zero, maior o impacto.",
            }}
            hideMarker
          >
            Diferença mês a mês (simulação − real)
          </InfoPopover>
        </p>
      </div>
      <div
        ref={wrapRef}
        className="scn-dualchart-wrap"
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
      >
        <svg
          className="scn-diffchart"
          viewBox={`0 0 ${W} ${H}`}
          role="img"
          aria-label={ariaLabel}
        >
          <defs>
            <linearGradient
              id={gid}
              gradientUnits="userSpaceOnUse"
              x1="0"
              y1={padTop}
              x2="0"
              y2={padTop + innerH}
            >
              <stop offset="0" stopColor="var(--success-400)" stopOpacity="0.32" />
              <stop
                offset={zeroFrac}
                stopColor="var(--success-400)"
                stopOpacity="0.04"
              />
              <stop
                offset={zeroFrac}
                stopColor="var(--danger-400)"
                stopOpacity="0.04"
              />
              <stop offset="1" stopColor="var(--danger-400)" stopOpacity="0.32" />
            </linearGradient>
          </defs>
          <line
            className="scn-diffchart__zero"
            x1={padX}
            x2={W - padX}
            y1={zeroY}
            y2={zeroY}
          />
          {/* O zero nomeado: a âncora de leitura do gráfico inteiro. */}
          <text
            className="scn-diffchart__zerolabel"
            x={W - padX}
            y={zeroY - 5}
            textAnchor="end"
          >
            R$ 0
          </text>
          {aboveH >= ZONE_LABEL_MIN_H && (
            <text
              className="scn-diffchart__zone scn-diffchart__zone--better"
              x={padX + 2}
              y={padTop + 11}
            >
              Sobra mais que no real
            </text>
          )}
          {belowH >= ZONE_LABEL_MIN_H && (
            <text
              className="scn-diffchart__zone scn-diffchart__zone--worse"
              x={padX + 2}
              y={padTop + innerH - 6}
            >
              Sobra menos que no real
            </text>
          )}
          <path d={areaPathTop} fill={`url(#${gid})`} />
          <polyline className="scn-diffchart__line" points={linePts} />
          {monthEnd.map((m, i) => {
            // Ponta esquerda ancora à direita do próprio x (nunca vaza pra fora à esquerda);
            // ponta direita ancora à esquerda (nunca vaza à direita) — só o meio centraliza.
            const anchor =
              i === 0 ? "start" : i === monthEnd.length - 1 ? "end" : "middle";
            return (
              <text
                key={`${m.year}-${m.month}`}
                x={x(i)}
                y={H - 4}
                textAnchor={anchor}
                fontSize="11"
                fill="var(--text-faint)"
              >
                {MES_ABBR[m.month - 1]}
              </text>
            );
          })}
          {hovered && (
            <g aria-hidden="true">
              <line
                className="scn-dualchart__crosshair"
                x1={x(hover!)}
                x2={x(hover!)}
                y1={padTop}
                y2={H - padBottom}
              />
              <circle
                cx={x(hover!)}
                cy={y(hovered.delta_cents)}
                r={4}
                fill={
                  hovered.delta_cents < 0 ? "var(--danger-400)" : "var(--success-400)"
                }
                stroke="var(--surface)"
                strokeWidth={2}
              />
            </g>
          )}
          {/* Marcador de PERIGO só quando o pior mês é de fato pior (delta negativo) — um
              ponto vermelho sobre um delta positivo afirmaria risco onde só há folga. */}
          {worst && worst.delta_cents < 0 && (
            <circle
              cx={x(worstIdx)}
              cy={y(worst.delta_cents)}
              r={3.5}
              fill="var(--danger-400)"
            />
          )}
        </svg>
        {hovered && (
          <div
            className="nk-spark__tip"
            aria-hidden="true"
            style={{
              left: `${hoverFrac * 100}%`,
              transform: `translateX(${hoverFrac > 0.82 ? "-100%" : hoverFrac < 0.18 ? "0" : "-50%"})`,
            }}
          >
            <span className="nk-spark__tip-day">
              {MES[hovered.month - 1]} {hovered.year}
            </span>
            <span
              className="nk-spark__tip-val"
              style={{
                color:
                  hovered.delta_cents < 0
                    ? "var(--money-neg)"
                    : hovered.delta_cents > 0
                      ? "var(--money-pos)"
                      : undefined,
              }}
            >
              {fmtSigned(hovered.delta_cents)}
            </span>
          </div>
        )}
      </div>
      {/* Compacto aqui (mesmo registro da manchete dos cards de KPI) — a precisão cheia já
          mora no `aria-label` do SVG acima (`fmtBRL`), nunca só aqui. */}
      {worst && worst.delta_cents < 0 && (
        <p className="scn-worst-note">
          Pior mês: {MES[worst.month - 1]} {fmtCompactBRL(worst.delta_cents)}
        </p>
      )}
    </div>
  );
}
