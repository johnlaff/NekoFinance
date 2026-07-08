/**
 * Cenários "e se" (plano 072, fatia C) — o FRONTEND do que-se-fosse: um side-sheet para
 * montar/gerenciar o cenário (lançamentos hipotéticos, alterações sobre obrigações reais,
 * dimensionamento de empréstimo) + a superfície de comparação (real × cenário) que aparece no
 * Horizonte quando um cenário está selecionado. Nada aqui grava no livro-razão real nem no
 * Sheets — todo write passa pelos comandos `scenario_*`/`add_scenario_transaction` (fatia B),
 * que já isolam a linha hipotética via `scenario_id`.
 */
import { useEffect, useRef, useState } from "react";
import {
  X,
  Plus,
  Trash2,
  Repeat,
  Landmark,
  CircleDollarSign,
  ArrowRight,
} from "lucide-react";
import {
  createScenario,
  deleteScenario,
  listScenarios,
  addScenarioTransaction,
  deleteScenarioTransaction,
  listScenarioTransactions,
  setScenarioOverride,
  listObligations,
  obligationItems,
  priceInstallment,
  type Scenario,
  type ScenarioCompareDto,
  type ScenarioTransactionRow,
  type Obligation,
} from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { todayISO } from "../lib/format";
import {
  fmtBRL,
  fmtSigned,
  MES,
  MES_ABBR,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { kindToFields } from "../lib/movement";
import { stripScenarioMarker, addMonthsISO } from "../lib/scenarioHelpers";
import { Money } from "../design-system/components/Money";
import { Button } from "../design-system/components/Button";
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

/** `<dialog>` nativo (não uma `role="dialog"` sobre uma `<div>`): dá foco preso, Escape-para-
 * fechar e o `::backdrop` de graça (ver `.scn-sheet::backdrop` em scenarios.css) — nenhuma dessas
 * três coisas precisa ser reimplementada à mão aqui. */
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
    if (open && !el.open) el.showModal();
    else if (!open && el.open) el.close();
  }, [open]);

  // Light-dismiss no clique do ::backdrop: registrado via listener nativo (não `onClick` no JSX)
  // porque um clique no backdrop cai no próprio elemento `<dialog>` — não é uma interação do
  // CONTEÚDO do diálogo, é o gesto padrão de fechar um modal por fora, o mesmo que Escape já faz
  // (evento `close` nativo, tratado no `onClose` abaixo).
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onBackdropClick = (e: MouseEvent) => {
      if (e.target === el) onClose();
    };
    el.addEventListener("click", onBackdropClick);
    return () => el.removeEventListener("click", onBackdropClick);
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
          <>
            <AddHypotheticalSection scenarioId={activeScenarioId} />
            <HypotheticalList scenarioId={activeScenarioId} />
            <OverrideSection scenarioId={activeScenarioId} />
            <LoanSection scenarioId={activeScenarioId} />
          </>
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

function AddHypotheticalSection({ scenarioId }: { scenarioId: string }) {
  const [kind, setKind] = useState<MovementType>("saida");
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");
  const [date, setDate] = useState(() => todayISO());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const trimmedDesc = description.trim();
    const cents = Math.round(parseFloat(amount.replace(",", ".")) * 100);
    if (!trimmedDesc || !Number.isFinite(cents) || cents <= 0 || busy) return;
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

function HypotheticalList({ scenarioId }: { scenarioId: string }) {
  const listQ = useCommand(`scenario_transactions:${scenarioId}`, () =>
    listScenarioTransactions(scenarioId),
  );
  const rows = listQ.data ?? [];
  const [error, setError] = useState<string | null>(null);

  async function remove(txnId: string) {
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

  return (
    <section>
      <p className="scn-section-title">Lançamentos hipotéticos</p>
      {error && (
        <p role="alert" className="scn-error">
          {error}
        </p>
      )}
      <div className="scn-txn-list">
        {rows.map((r: ScenarioTransactionRow) => (
          <div className="scn-txn-row" key={r.id}>
            <span className="scn-txn-row__desc">
              {stripScenarioMarker(r.description)}
            </span>
            <Money
              cents={r.type === "income" ? Math.abs(r.amount) : -Math.abs(r.amount)}
              size="sm"
            />
            <button
              type="button"
              className="scn-txn-row__del"
              aria-label={`Remover "${stripScenarioMarker(r.description)}" do cenário`}
              onClick={() => void remove(r.id)}
            >
              <Trash2 size={13} strokeWidth={1.75} />
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}

function OverrideSection({ scenarioId }: { scenarioId: string }) {
  const obligationsQ = useCommand("obligations", listObligations);
  const obligations = obligationsQ.data ?? [];
  const [selectedId, setSelectedId] = useState("");
  const [action, setAction] = useState<"replace" | "suppress">("suppress");
  const [newAmount, setNewAmount] = useState("");
  const [fromDate, setFromDate] = useState(() => todayISO());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const previewQ = useCommand(
    selectedId ? `obligation_items:${selectedId}` : "obligation_items:none",
    () => (selectedId ? obligationItems(selectedId) : Promise.resolve([])),
  );
  const affectedCount = (previewQ.data ?? []).filter(
    (it) => it.date >= fromDate,
  ).length;

  async function confirm() {
    if (!selectedId || busy) return;
    if (action === "replace" && !newAmount.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const cents =
        action === "replace"
          ? Math.round(parseFloat(newAmount.replace(",", ".")) * 100)
          : 0;
      await setScenarioOverride({
        scenarioId,
        op: action,
        fromDate,
        obligationId: selectedId,
        replacement:
          action === "replace" ? { amount_cents: cents, date: fromDate } : null,
      });
      invalidateCommands();
      setSelectedId("");
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
        <label htmlFor="scn-ov-obligation">Obrigação recorrente</label>
        <select
          id="scn-ov-obligation"
          value={selectedId}
          onChange={(e) => setSelectedId(e.target.value)}
        >
          <option value="">Selecione…</option>
          {obligations.map((ob: Obligation) => (
            <option key={ob.id} value={ob.id}>
              {ob.name}
            </option>
          ))}
        </select>
      </div>
      {selectedId && (
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
          <p className="scn-preview" aria-live="polite">
            {previewQ.loading
              ? "Calculando ocorrências afetadas…"
              : `Isto afeta ${affectedCount} ${affectedCount === 1 ? "ocorrência" : "ocorrências"} a partir de ${fromDate}.`}
          </p>
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
              busy || previewQ.loading || (action === "replace" && !newAmount.trim())
            }
          >
            {busy ? "Aplicando…" : "Confirmar alteração"}
          </Button>
        </>
      )}
    </section>
  );
}

function LoanSection({ scenarioId }: { scenarioId: string }) {
  const [principal, setPrincipal] = useState("");
  const [termMonths, setTermMonths] = useState("12");
  const [ratePct, setRatePct] = useState("2");
  const [firstDate, setFirstDate] = useState(() => addMonthsISO(todayISO(), 1));
  const [description, setDescription] = useState("Empréstimo");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const principalCents = Math.round(parseFloat(principal.replace(",", ".")) * 100) || 0;
  const term = Math.max(1, parseInt(termMonths, 10) || 0);
  const rateBps = Math.round((parseFloat(ratePct.replace(",", ".")) || 0) * 100);
  const validInputs = principalCents > 0 && term > 0;

  const previewKey = validInputs
    ? `price_installment:${principalCents}:${rateBps}:${term}`
    : "price_installment:none";
  const previewQ = useCommand(previewKey, () =>
    validInputs ? priceInstallment(principalCents, rateBps, term) : Promise.resolve(0),
  );
  const installmentCents = previewQ.data ?? 0;
  const totalPaidCents = installmentCents * term;
  const creditCostCents = totalPaidCents - principalCents;

  async function confirm() {
    if (!validInputs || busy) return;
    setBusy(true);
    setError(null);
    const groupId =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const marker = ` #loan:${groupId}:${rateBps}`;
    const disbursementDate = todayISO();
    // As linhas do empréstimo são criadas em SEQUÊNCIA, parando na primeira falha (não em
    // Promise.all): num lote paralelo, uma rejeição no meio deixaria as irmãs commitarem
    // mesmo assim e o grupo ficaria pela metade sem controle de quantas entraram. Falha no
    // meio ainda deixa um grupo parcial persistido — por isso o catch SEMPRE invalida (a
    // lista de hipotéticos passa a mostrar as linhas órfãs, cada uma com o botão de excluir)
    // e a mensagem diz exatamente quantas entraram.
    let createdInstallments = 0;
    let principalCreated = false;
    try {
      await addScenarioTransaction({
        scenarioId,
        txnType: "income",
        amountCents: principalCents,
        description: `${description.trim() || "Empréstimo"}${marker}`,
        date: disbursementDate,
      });
      principalCreated = true;
      for (let i = 0; i < term; i++) {
        // react-doctor-disable-next-line react-doctor/async-await-in-loop -- sequencial de propósito (ver banner acima): parar na 1ª falha e saber exatamente quantas parcelas entraram; Promise.all deixaria as irmãs commitarem após uma rejeição no meio (grupo parcial sem contagem)
        await addScenarioTransaction({
          scenarioId,
          txnType: "expense",
          amountCents: installmentCents,
          description: `${description.trim() || "Empréstimo"} parcela ${i + 1}/${term}${marker}`,
          date: addMonthsISO(firstDate, i),
          isFixed: true,
        });
        createdInstallments += 1;
      }
      invalidateCommands();
      setPrincipal("");
      setBusy(false);
    } catch (err) {
      // Refetch SEMPRE: as linhas que já entraram precisam aparecer na lista de hipotéticos
      // para o usuário poder excluí-las — sem isso o grupo parcial ficaria invisível e um
      // retry criaria um SEGUNDO grupo sobreposto.
      invalidateCommands();
      setBusy(false);
      const partial = principalCreated
        ? ` O empréstimo ficou incompleto (${createdInstallments} de ${term} parcelas criadas) — exclua as linhas do empréstimo na lista acima e tente novamente.`
        : "";
      setError(`${scenarioErrorMessage(err)}${partial}`);
    }
  }

  return (
    <section aria-labelledby="scn-loan-title">
      <p className="scn-section-title" id="scn-loan-title">
        Dimensionar um empréstimo
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
            max={360}
            value={termMonths}
            onChange={(e) => setTermMonths(e.target.value)}
          />
        </div>
        <div className="scn-field">
          <label htmlFor="scn-loan-rate">Juros a.m. (%)</label>
          <input
            id="scn-loan-rate"
            inputMode="decimal"
            value={ratePct}
            onChange={(e) => setRatePct(e.target.value)}
          />
        </div>
      </div>
      <div className="scn-field" style={{ marginTop: 8 }}>
        <label htmlFor="scn-loan-first">Data da 1ª parcela</label>
        <input
          id="scn-loan-first"
          type="date"
          value={firstDate}
          onChange={(e) => setFirstDate(e.target.value)}
        />
      </div>
      {validInputs && (
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
      <Button
        size="sm"
        variant="primary"
        iconLeft={<Landmark size={14} strokeWidth={1.75} />}
        onClick={() => void confirm()}
        disabled={busy || !validInputs || previewQ.loading}
        className="scn-loan-confirm"
      >
        {busy ? "Criando…" : "Adicionar empréstimo ao cenário"}
      </Button>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Superfície de comparação (canvas principal do Horizonte em modo comparação)
// ---------------------------------------------------------------------------

type DeltaSense = "higher-better" | "lower-better";

function deltaChip(deltaCents: number, sense: DeltaSense) {
  const better = sense === "higher-better" ? deltaCents > 0 : deltaCents < 0;
  const worse = sense === "higher-better" ? deltaCents < 0 : deltaCents > 0;
  const cls = better
    ? "scn-kpi__delta scn-kpi__delta--better"
    : worse
      ? "scn-kpi__delta scn-kpi__delta--worse"
      : "scn-kpi__delta scn-kpi__delta--neutral";
  const arrow = deltaCents > 0 ? "▲" : deltaCents < 0 ? "▼" : "•";
  return (
    <span className={cls}>
      {arrow} {fmtSigned(deltaCents)}
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
}: {
  label: string;
  term: { title: string; body: string };
  realCents: number;
  scenarioCents: number;
  deltaCents: number;
  sense: DeltaSense;
}) {
  return (
    <article className="scn-kpi">
      <span className="scn-kpi__label">
        <InfoPopover term={term} hideMarker>
          {label}
        </InfoPopover>
      </span>
      <div className="scn-kpi__transition">
        {fmtBRL(realCents)}
        <ArrowRight
          size={12}
          strokeWidth={2}
          className="scn-kpi__arrow"
          aria-hidden="true"
        />
        <span className="scn-kpi__scenario-val">{fmtBRL(scenarioCents)}</span>
      </div>
      {deltaChip(deltaCents, sense)}
    </article>
  );
}

/** Remove marcas + limita a 60 chars para caber na linha do chip de mudança. */
function changeLabel(desc: string): string {
  const clean = stripScenarioMarker(desc) || "Sem descrição";
  return clean.length > 60 ? `${clean.slice(0, 57)}…` : clean;
}

export function ScenarioCompare({ compare }: { compare: ScenarioCompareDto }) {
  const lastMonthEnd = compare.month_end[compare.month_end.length - 1] ?? null;
  const endRealCents = lastMonthEnd?.real_balance_cents ?? 0;
  const endScenarioCents = lastMonthEnd?.scenario_balance_cents ?? 0;
  const endDeltaCents = lastMonthEnd?.delta_cents ?? 0;

  const realDeficit = compare.real_deepest_deficit?.balance_cents ?? 0;
  const scenarioDeficit = compare.scenario_deepest_deficit?.balance_cents ?? 0;
  const deficitDelta = compare.deepest_deficit_delta_cents ?? 0;

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
        {fmtSigned(endDeltaCents)} versus o real
      </div>
      <div className="card__head">
        <span className="card__title">
          <CircleDollarSign size={16} strokeWidth={1.75} className="ic" />
          Cenário: {compare.scenario_name}
        </span>
      </div>
      <div
        className="card__body"
        style={{ display: "flex", flexDirection: "column", gap: 20 }}
      >
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
          </div>
        )}
      </div>
    </section>
  );
}

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
                ? "+ adicionou"
                : c.op === "remove"
                  ? "− removeu"
                  : "↔ alterou"}
            </span>
            <span className="scn-change-row__desc">{changeLabel(c.description)}</span>
            <span className="scn-change-row__amt">
              {c.op === "replace"
                ? `${c.old_amount_cents != null ? fmtBRL(c.old_amount_cents) : "—"} → ${c.new_amount_cents != null ? fmtBRL(c.new_amount_cents) : "—"}`
                : fmtBRL(
                    (c.old_amount_cents ?? c.new_amount_cents ?? 0) *
                      (c.op === "remove" ? -1 : 1),
                  )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Aproxima uma data ("1º dia útil de projeção" do fim de mês) para posicionar o marcador do
 * menor saldo no eixo contínuo do gráfico mensal (a série do cenário só tem resolução MENSAL —
 * ver `ScenarioCompareDto.month_end` — então plotamos por mês, não por dia). */
function monthFraction(
  year: number,
  month: number,
  startYear: number,
  startMonth: number,
) {
  return (year - startYear) * 12 + (month - startMonth);
}

function DualLineChart({ compare }: { compare: ScenarioCompareDto }) {
  const points = compare.month_end;
  const W = 720;
  const H = 200;
  const padX = 12;
  const padTop = 20;
  const padBottom = 24;
  const innerW = W - padX * 2;
  const innerH = H - padTop - padBottom;

  const allVals = points.flatMap((p) => [
    p.real_balance_cents,
    p.scenario_balance_cents,
  ]);
  const max = Math.max(...allVals, 0);
  const min = Math.min(...allVals, 0);
  const range = max - min || 1;
  const x = (i: number) =>
    padX + (points.length === 1 ? innerW / 2 : (i / (points.length - 1)) * innerW);
  const y = (cents: number) => padTop + innerH - ((cents - min) / range) * innerH;

  const realPts = points.map((p, i) => `${x(i)},${y(p.real_balance_cents)}`).join(" ");
  const scenarioPts = points
    .map((p, i) => `${x(i)},${y(p.scenario_balance_cents)}`)
    .join(" ");

  const first = points[0];
  const startIdx = first
    ? monthFraction(first.year, first.month, first.year, first.month)
    : 0;
  const deficitDate = compare.scenario_deepest_deficit?.date;
  const deficitYear = deficitDate ? parseInt(deficitDate.slice(0, 4), 10) : null;
  const deficitMonth = deficitDate ? parseInt(deficitDate.slice(5, 7), 10) : null;
  const deficitIdx =
    first && deficitYear != null && deficitMonth != null
      ? Math.max(
          0,
          Math.min(
            points.length - 1,
            monthFraction(deficitYear, deficitMonth, first.year, first.month) +
              startIdx,
          ),
        )
      : null;

  const ariaLabel = `Trajetória real versus simulação. Real termina em ${fmtBRL(
    points[points.length - 1]?.real_balance_cents ?? 0,
  )}. Simulação termina em ${fmtBRL(
    points[points.length - 1]?.scenario_balance_cents ?? 0,
  )}. Buraco do futuro real: ${fmtBRL(compare.real_deepest_deficit?.balance_cents ?? 0)}. Buraco do futuro na simulação: ${fmtBRL(compare.scenario_deepest_deficit?.balance_cents ?? 0)}.`;

  return (
    <div>
      <p className="scn-section-title">Trajetória: real × simulação</p>
      <svg
        className="scn-dualchart"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={ariaLabel}
      >
        <polyline className="scn-dualchart__real" points={realPts} />
        <polyline className="scn-dualchart__scenario" points={scenarioPts} />
        {points.length > 0 && (
          <>
            <text
              x={x(points.length - 1) - 4}
              y={y(points[points.length - 1]!.real_balance_cents) - 8}
              textAnchor="end"
              fontSize="11"
              fontWeight="600"
              fill="var(--primary)"
            >
              Real
            </text>
            <text
              x={x(points.length - 1) - 4}
              y={y(points[points.length - 1]!.scenario_balance_cents) + 14}
              textAnchor="end"
              fontSize="11"
              fontWeight="600"
              fill="var(--sim-scenario)"
            >
              Simulação
            </text>
          </>
        )}
        {deficitIdx != null && (
          <circle
            cx={x(deficitIdx)}
            cy={y(compare.scenario_deepest_deficit!.balance_cents)}
            r={4}
            fill="var(--sim-scenario)"
            stroke="var(--surface)"
            strokeWidth={2}
          />
        )}
      </svg>
    </div>
  );
}

function DiffSparkline({ monthEnd }: { monthEnd: ScenarioCompareDto["month_end"] }) {
  const W = 720;
  const H = 120;
  const padX = 12;
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

  // Alternativa textual do gráfico (mesmo padrão do ariaLabel do DualLineChart): sempre
  // presente — inclusive quando o cenário é melhor em todos os meses e a nota visual de
  // "Pior mês" não renderiza.
  const ariaLabel =
    worst && worst.delta_cents < 0
      ? `Diferença mês a mês entre simulação e real. Pior mês: ${MES[worst.month - 1]} ${fmtBRL(worst.delta_cents)}.`
      : "Diferença mês a mês entre simulação e real. A simulação fica igual ou melhor que o real em todos os meses.";

  return (
    <div>
      <p className="scn-section-title">Diferença mês a mês (simulação − real)</p>
      <svg
        className="scn-diffchart"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={ariaLabel}
      >
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--success-400)" stopOpacity="0.32" />
            <stop offset="50%" stopColor="var(--success-400)" stopOpacity="0.04" />
            <stop offset="50%" stopColor="var(--danger-400)" stopOpacity="0.04" />
            <stop offset="100%" stopColor="var(--danger-400)" stopOpacity="0.32" />
          </linearGradient>
        </defs>
        <line
          className="scn-diffchart__zero"
          x1={padX}
          x2={W - padX}
          y1={zeroY}
          y2={zeroY}
        />
        <path d={areaPathTop} fill={`url(#${gid})`} />
        <polyline className="scn-diffchart__line" points={linePts} />
        {monthEnd.map((m, i) => (
          <text
            key={`${m.year}-${m.month}`}
            x={x(i)}
            y={H - 4}
            textAnchor="middle"
            fontSize="10"
            fill="var(--text-faint)"
          >
            {MES_ABBR[m.month - 1]}
          </text>
        ))}
        {worst && (
          <circle
            cx={x(worstIdx)}
            cy={y(worst.delta_cents)}
            r={3.5}
            fill="var(--danger-400)"
          />
        )}
      </svg>
      {worst && worst.delta_cents < 0 && (
        <p className="scn-worst-note">
          Pior mês: {MES[worst.month - 1]} {fmtBRL(worst.delta_cents)}
        </p>
      )}
    </div>
  );
}
