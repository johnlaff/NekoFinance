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
  fmtCompactBRL,
  saldoBand,
  monthOf,
  MES,
  MES_ABBR,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { performanceStatus, custoVidaStatus } from "./totaisStatus";
import { kindToFields } from "../lib/movement";
import {
  stripScenarioMarker,
  addMonthsISO,
  placeChartEndLabels,
} from "../lib/scenarioHelpers";
import { Money, SignedMoney } from "../design-system/components/Money";
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

// ---------------------------------------------------------------------------
// Estados do método (Nível 2, plano 074/fatia B) — a HERO de cada card de KPI passa a ser o
// ESTADO (ícone + palavra + cor), nunca só cor; o valor compacto desce a evidência. Os rótulos
// vêm SEMPRE de um helper do método (`saldoBand`/`performanceStatus`/`custoVidaStatus`), nunca
// de texto solto aqui — fidelidade verbatim é o requisito duro do plano.
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
    return <span className="scn-kpi__delta scn-kpi__delta--quiet">≈ sem mudança</span>;
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
  /** Cenário sem NENHUM ponto de projeção (plano 074/fatia C — residual da fatia B): quando
   * `true`, `scenarioCents`/`deltaCents` são ruído (`?? 0` do chamador) e nunca aparecem — o
   * card renderiza um vazio neutro ("—", sem cor de estado) em vez de fingir "Apertado R$ 0".
   * `scenarioState` ainda deve chegar neutra (ver `EMPTY_SCENARIO_STATE`); só ela controla a
   * cor/ícone do Nível 2, mas o headline/evidência/delta são sempre suprimidos aqui. */
  emptyScenario?: boolean;
}) {
  // STATE TRANSITIONS (plano 074/fatia B): quando o estado do cenário DIFERE do estado real, a
  // hero vira o estado NOVO (cenário) com a origem numa linha discreta empilhada abaixo — nunca
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
          cheia numa linha sem quebra (essa era a causa do estouro medido em dogfooding). Precisão
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

/** Estado neutro (plano 074/fatia C) para quando o cenário não tem NENHUM ponto de projeção —
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
 * card "Buraco do futuro" (plano 074/fatia C): com derivações separadas, o card caía no `?? 0`
 * e fabricava "cenário R$ 0,00" enquanto o banner logo acima mostrava o mínimo mensal — banner
 * e card discordando sobre o MESMO dado, a mesma classe de contradição que a fatia B eliminou. */
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

/** Veredito (Nível 1, plano 074/fatia B): a resposta a "é seguro?" de relance, ANTES da grade de
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

export function ScenarioCompare({ compare }: { compare: ScenarioCompareDto }) {
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
                <span>Reserva após financiar</span>
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
 * SKIP (plano 074, fatia C, item 1): a intenção era colorir o valor de cada linha por TIPO de
 * movimento via `TYPE_META` (`entrada`/`saida`/`diario`/`economia`/`cartao`). `ScenarioChange`
 * (api.ts) só expõe `op`/`description`/`from_date`/`old_amount_cents`/`new_amount_cents` — sem
 * `kind`. E não dá pra derivar no cliente com o que já existe: uma troca (`replace`/`remove`)
 * nasce de um `scenario_override` sobre uma OBRIGAÇÃO ou uma RECORRÊNCIA (scenarios.rs, ~1145-
 * 1177) — o backend só busca o NOME da obrigação para o rótulo, nunca o `kind`; para uma
 * recorrência sem obrigação a "descrição" nem chega a ser legível (é o `recurrence_id` cru). As
 * únicas linhas com tipo conhecido no backend são as hipotéticas "add" (`HypoTxnRow.ttype`), mas
 * mesmo essas não serializam o tipo pro DTO. Um join no cliente (casar `changes` com
 * `listScenarioTransactions`/`listObligations` por descrição+data+valor) seria frágil (chave
 * sintética, sem `id`) e ainda deixaria as trocas de recorrência sem cor nenhuma — pior que não
 * colorir. Regra do plano: não estender o DTO nesta fatia. Retomar quando `ScenarioChange`
 * ganhar `kind` no backend (mesma origem que já preenche `HypoTxnRow.ttype`/`Obligation.kind`).
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
                ? "+ adicionou"
                : c.op === "remove"
                  ? "− removeu"
                  : "↔ alterou"}
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

/** Gutter reservado à direita do plot para os rótulos de fim de linha (~72px): sem ele "Real"/
 *  "Simulação" caem EM CIMA do traço quando as duas linhas convergem no fim do horizonte — o
 *  defeito medido em dogfooding. O plot termina antes do gutter; o texto começa dentro dele.
 *  (O vão vertical mínimo entre os rótulos vive em `scenarioHelpers.CHART_LABEL_MIN_GAP`.) */
const CHART_LABEL_GUTTER = 72;

function DualLineChart({ compare }: { compare: ScenarioCompareDto }) {
  const points = compare.month_end;
  const W = 720;
  const H = 200;
  const padLeft = 12;
  const padRight = 12 + CHART_LABEL_GUTTER;
  const padTop = 20;
  const padBottom = 24;
  const innerW = W - padLeft - padRight;
  const innerH = H - padTop - padBottom;

  const allVals = points.flatMap((p) => [
    p.real_balance_cents,
    p.scenario_balance_cents,
  ]);
  const max = Math.max(...allVals, 0);
  const min = Math.min(...allVals, 0);
  const range = max - min || 1;
  const x = (i: number) =>
    padLeft + (points.length === 1 ? innerW / 2 : (i / (points.length - 1)) * innerW);
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

  const last = points[points.length - 1];
  let labelX = 0;
  let realLabelY = 0;
  let scenarioLabelY = 0;
  if (last) {
    labelX = x(points.length - 1) + 12;
    // Colocação direction-aware + clamp do PAR (nunca de cada rótulo isolado, que comprimia
    // o vão de volta perto das bordas) — geometria pura e testada em `scenarioHelpers`.
    const placed = placeChartEndLabels(
      y(last.real_balance_cents),
      y(last.scenario_balance_cents),
      padTop + 8,
      H - 6,
    );
    realLabelY = placed.realLabelY;
    scenarioLabelY = placed.scenarioLabelY;
  }

  return (
    <div>
      <div className="scn-dualchart__head">
        <p className="scn-section-title" style={{ margin: 0 }}>
          Trajetória: real × simulação
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
      <svg
        className="scn-dualchart"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={ariaLabel}
      >
        <polyline className="scn-dualchart__real" points={realPts} />
        <polyline className="scn-dualchart__scenario" points={scenarioPts} />
        {last && (
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

  // Alternativa textual do gráfico (mesmo padrão do ariaLabel do DualLineChart): sempre
  // presente — inclusive quando o cenário é melhor em todos os meses e a nota visual de
  // "Pior mês" não renderiza.
  const ariaLabel =
    worst && worst.delta_cents < 0
      ? `Diferença mês a mês entre simulação e real. Pior mês: ${MES[worst.month - 1]} ${fmtBRL(worst.delta_cents)}.`
      : "Diferença mês a mês entre simulação e real. A simulação fica igual ou melhor que o real em todos os meses.";

  return (
    <div>
      {/* Mesmo padrão de título do `DualLineChart` logo acima (`__head` com margem zerada) —
          os dois gráficos empilhados na mesma superfície tinham ritmo vertical diferente
          (8px do `.scn-section-title` cru vs 4px do `__head`), um polimento sem função nova:
          este gráfico é de UMA série só (a diferença), então sem legenda de cor — o zero
          tracejado já separa "melhor"/"pior" por POSIÇÃO, não só por cor. */}
      <div className="scn-diffchart__head">
        <p className="scn-section-title" style={{ margin: 0 }}>
          Diferença mês a mês (simulação − real)
        </p>
      </div>
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
        {monthEnd.map((m, i) => {
          // Ponta esquerda ancora à direita do próprio x (nunca vaza pra fora à esquerda);
          // ponta direita ancora à esquerda (nunca vaza à direita) — só o meio centraliza.
          // Mesma lógica do gutter do DualLineChart, resolvida por âncora em vez de espaço
          // reservado (aqui o rótulo é curto — 3 letras — e já cabe dentro do padX).
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
        {worst && (
          <circle
            cx={x(worstIdx)}
            cy={y(worst.delta_cents)}
            r={3.5}
            fill="var(--danger-400)"
          />
        )}
      </svg>
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
