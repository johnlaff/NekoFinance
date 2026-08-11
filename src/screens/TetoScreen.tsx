import "./teto.css";
import { useEffect, useRef, useState } from "react";
import { Clock3, Gauge, ListChecks, Plus, Trash2 } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import {
  CollapsedReceipt,
  Receipt,
  type ReceiptLine,
} from "../design-system/components/Receipt";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { ModeChip } from "../design-system/components/ModeChip";
import { Money } from "../design-system/components/Money";
import { isTauri } from "../lib/env";
import { centsToBRLInput, formatBRL, parseBRLToCents, todayISO } from "../lib/format";
import { safeErrorMessage } from "../lib/errors";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { useShowReceipt } from "../hooks/useShowReceipt";
import {
  GUIDED_QUESTIONS,
  acceptCeilingProposalCmd,
  buildTetoView,
  categoriesFromDraft,
  ceilingPerDayCents,
  ceremonyAgeLabel,
  ceremonyMonthLabel,
  dismissCeilingProposalCmd,
  divisorFromText,
  draftTotalCents,
  fetchCeilingProposal,
  fetchDailyBudget,
  fetchDashboardSummary,
  guardTriggered,
  monthYearLabel,
  recalibrationCaption,
  saveDailyBudgetCmd,
  type DailyBudget,
  type DraftItem,
  type TetoProof,
  type TetoView,
} from "./tetoView";

// A tela do teto é o REGISTRO de uma decisão com prova: o número decidido, a cerimônia que o
// produziu (itens, fórmula e a nota original da planilha), a idade dessa cerimônia e como o dia
// lê o teto. Editar é um rito raro em três batidas na própria superfície — nunca um modal, nunca
// um formulário permanente. A composição vive no view-model puro `tetoView`.

const CEREMONY_TERM = {
  title: "Cerimônia do teto",
  body: "Liste o que o mês variável comporta por categoria, some e divida pelos dias. O resultado, arredondado para cima, é o teto diário — o número que o dia inteiro respeita. O método pede que ela se refaça de três em três meses.",
};

const SPEEDOMETER_TERM = {
  title: "O velocímetro do dia",
  body: "É a barra da Hoje que compara o gasto do dia com a régua vigente. No modo cartão a régua são as faturas em aberto — cada compra no crédito soma nelas, e o teto fica de referência. Se o gasto variável voltar para o débito, o velocímetro passa a medir o Diário contra o teto sozinho, sem nenhum ajuste seu.",
};

const FREE_MONEY_TERM = {
  title: "O mês variável",
  body: "É o dinheiro livre: o gasto que varia com a sua escolha do dia — comida, transporte, saúde, lazer, compras. Contas fixas, cartão e economia ficam de fora, cada um com a sua própria régua.",
};

/** Uma linha em branco do rito, com identidade estável para o React. */
let draftRowSeq = 0;
function blankItem(name = ""): DraftItem {
  draftRowSeq += 1;
  return { key: `draft-${draftRowSeq}`, name, amountText: "" };
}

function itemsFromBudget(budget: DailyBudget | undefined): DraftItem[] {
  const rows = budget?.categories ?? [];
  if (rows.length === 0) return [blankItem()];
  return rows.map((c) => ({
    key: c.id,
    name: c.name,
    amountText: centsToBRLInput(c.amount_cents),
  }));
}

type RiteStep = "items" | "guided" | "divisor" | "accept" | "direct";

interface RiteState {
  step: RiteStep;
  items: DraftItem[];
  divisorText: string;
  directText: string;
  /** Pergunta corrente da cerimônia guiada. */
  guidedIndex: number;
  /** A guarda do "vença o dia" já foi lida e recusada. */
  guardAcknowledged: boolean;
  /** Batida que abriu o atalho do valor direto — para onde o "Voltar" retorna. */
  returnTo?: RiteStep | undefined;
}

function openRite(budget: DailyBudget | undefined, step: RiteStep): RiteState {
  return {
    step,
    items:
      step === "guided"
        ? GUIDED_QUESTIONS.map((q) => blankItem(q.category))
        : itemsFromBudget(budget),
    divisorText: String(budget?.divisor_days ?? 31),
    directText: "",
    guidedIndex: 0,
    guardAcknowledged: false,
  };
}

export function TetoScreen() {
  const budgetQ = useCommand("get_daily_budget", fetchDailyBudget);
  const proposalQ = useCommand("get_ceiling_proposal", fetchCeilingProposal);
  const summaryQ = useCommand("get_dashboard_summary", fetchDashboardSummary);
  const [rite, setRite] = useState<RiteState | null>(null);

  const view = buildTetoView({
    budget: budgetQ.data,
    proposal: proposalQ.data,
    summary: summaryQ.data,
    today: todayISO(),
  });

  if (budgetQ.error) {
    return (
      <div className="teto neko-app">
        <EmptyState
          variant="error"
          title="Não foi possível carregar o teto"
          description="Confira a conexão e tente de novo."
          action={
            <Button size="sm" variant="ghost" onClick={() => invalidateCommands()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  if (view.kind === "loading") {
    return (
      <div className="teto neko-app">
        <EmptyState variant="skeleton" skeletonRows={5} />
      </div>
    );
  }

  return (
    // Bento de colunas INDEPENDENTES no desktop (massas díspares nunca se alinham por linha);
    // no mobile as colunas se dissolvem e os cards fluem na ordem do DOM.
    <div className="teto neko-app">
      <Verdict
        view={view}
        onStartRite={(step) => setRite(openRite(budgetQ.data, step))}
      />
      <div className="teto__col">
        {rite ? (
          <Rite
            state={rite}
            view={view}
            onChange={setRite}
            onClose={() => setRite(null)}
          />
        ) : view.proof ? (
          <ProofCard proof={view.proof} view={view} />
        ) : null}
      </div>
      <div className="teto__col">
        {rite ? (
          // Durante o rito, a prova vigente segue à vista: é o "antes" que o aceite substitui.
          view.proof ? (
            <ProofCard proof={view.proof} view={view} />
          ) : null
        ) : (
          <>
            {view.kind === "chosen" ? (
              <AgeCard
                view={view}
                onRecalibrate={() => setRite(openRite(budgetQ.data, "items"))}
              />
            ) : null}
            <ReadingCard view={view} />
          </>
        )}
      </div>
    </div>
  );
}

// ----------------------------------------------------------------- veredito --

function Verdict({
  view,
  onStartRite,
}: {
  view: TetoView;
  onStartRite: (step: RiteStep) => void;
}) {
  const showReceipt = useShowReceipt();
  return (
    <div className="teto__verdict">
      <p className="teto__vlabel">
        {view.kind === "proposal"
          ? "Proposta encontrada"
          : view.kind === "estimate"
            ? "Pelo seu histórico"
            : view.kind === "none"
              ? "A régua do gasto livre"
              : ceremonyMonthLabel(view.ceremonyMonth)}
        {view.kind === "estimate" ? (
          <EstimateMark
            term={{
              title: "Número em estimativa",
              body: "É o gasto variável do mês anterior dividido pelos dias dele, exibido enquanto não há teto escolhido. A conta está à mostra na própria manchete. A cerimônia transforma a estimativa em decisão.",
            }}
          />
        ) : null}
      </p>
      <VerdictHeadline
        view={view}
        onStartRite={onStartRite}
        showReceipt={showReceipt}
      />
      {view.kind === "chosen" ? (
        /* Sem o gate do método: esta tela registra a decisão do dono, e o julgamento da
           legitimidade do modo cartão mora onde a economia é julgada. */
        <ModeChip className="teto__modechip" mode={view.mode} />
      ) : null}
    </div>
  );
}

/** A conta da estimativa. A preferência escolhe entre aberta e recolhida — nunca some. */
function EstimateReceipt({
  basis,
  perDayCents,
  showReceipt,
}: {
  basis: NonNullable<TetoView["estimateBasis"]>;
  perDayCents: number;
  showReceipt: boolean;
}) {
  const month = monthYearLabel(basis.month) ?? "o mês anterior";
  const lines: ReceiptLine[] = [
    { label: `Gasto variável de ${month}`, cents: basis.variableCents },
    { label: `Dias de ${month}`, text: String(basis.days), op: "div" },
    { label: "Cerca de, por dia", cents: perDayCents, op: "eq", result: true },
  ];
  return showReceipt ? <Receipt lines={lines} /> : <CollapsedReceipt lines={lines} />;
}

function VerdictHeadline({
  view,
  onStartRite,
  showReceipt,
}: {
  view: TetoView;
  onStartRite: (step: RiteStep) => void;
  showReceipt: boolean;
}) {
  switch (view.kind) {
    case "proposal":
      return <ProposalVerdict view={view} />;
    case "estimate":
      return (
        <>
          <h1 data-large-title>
            Cerca de <Money cents={view.perDayCents} size="inherit" hideCents /> por
            dia, pelo seu histórico.
          </h1>
          {/* A conta impressa no lugar da frase que a descrevia: uma frase pode divergir do
              motor — e esta divergia, falando em "meses" onde o cálculo usa um mês só. */}
          {view.estimateBasis ? (
            <EstimateReceipt
              basis={view.estimateBasis}
              perDayCents={view.perDayCents}
              showReceipt={showReceipt}
            />
          ) : null}
          <p>
            Não é um teto escolhido.{" "}
            <span className="teto__cf">
              A cerimônia transforma a estimativa em decisão.
            </span>
          </p>
          <div className="teto__vactions">
            <Button variant="primary" onClick={() => onStartRite("items")}>
              Estipular o teto
            </Button>
          </div>
        </>
      );
    case "none":
      return (
        <>
          <h1 data-large-title>Você ainda não tem um teto.</h1>
          <p>
            O teto nasce do seu extrato: some as despesas variáveis de um mês, divida
            pelos dias e arredonde para cima.{" "}
            <span className="teto__cf">Cinco perguntas e o número sai pronto.</span>
          </p>
          <div className="teto__vactions">
            <Button variant="primary" onClick={() => onStartRite("guided")}>
              Estipular o teto
            </Button>
            <button
              type="button"
              className="teto__quiet"
              onClick={() => onStartRite("direct")}
            >
              Já sei meu teto
            </button>
          </div>
        </>
      );
    default:
      // Manchete pura: o corpo morre pela regra 41 — o velocímetro e o modo cartão já
      // vivem nos popovers da própria tela ("Como o dia lê o teto").
      return view.mode === "card" ? (
        <h1 data-large-title>
          Seu teto é <Money cents={view.perDayCents} size="inherit" /> por dia.
        </h1>
      ) : (
        <h1 data-large-title>
          Seu dia comporta <Money cents={view.perDayCents} size="inherit" />.
        </h1>
      );
  }
}

function ProposalVerdict({ view }: { view: TetoView }) {
  const proposal = view.proposal;
  const [busy, setBusy] = useState(false);
  if (!proposal) return null;
  const totalCents = proposal.items.reduce((sum, it) => sum + it.amount_cents, 0);

  function resolve(action: "accept" | "dismiss") {
    if (!isTauri || !proposal) return;
    setBusy(true);
    const call =
      action === "accept"
        ? acceptCeilingProposalCmd(proposal.id)
        : dismissCeilingProposalCmd(proposal.id);
    call
      .then(() => invalidateCommands())
      // eslint-disable-next-line @typescript-eslint/no-empty-function
      .catch(() => {})
      .finally(() => setBusy(false));
  }

  return (
    <>
      <h1 data-large-title>
        Sua planilha propõe <Money cents={proposal.per_day_cents} size="inherit" /> por
        dia.
      </h1>
      <p>
        A cerimônia está anotada nas notas do Diário:{" "}
        {totalCents > 0 ? (
          <>
            <b>
              <Money cents={totalCents} size="inherit" /> ÷ {proposal.divisor_days} dias
            </b>
            ,{" "}
          </>
        ) : (
          <>
            <b>÷ {proposal.divisor_days} dias</b>,{" "}
          </>
        )}
        escrita em {monthYearLabel(proposal.source_month) ?? "outro mês"}.{" "}
        <span className="teto__cf">
          {view.currentPerDayCents > 0 ? (
            <>
              Usar este teto substitui o atual de{" "}
              <Money cents={view.currentPerDayCents} size="inherit" /> por dia.
            </>
          ) : (
            "Nada é gravado sem o seu aceite."
          )}
        </span>
      </p>
      <div className="teto__vactions">
        <Button variant="primary" onClick={() => resolve("accept")} disabled={busy}>
          Usar este teto
        </Button>
        <Button variant="ghost" onClick={() => resolve("dismiss")} disabled={busy}>
          Agora não
        </Button>
      </div>
    </>
  );
}

// ------------------------------------------------------------ prova e cards --

function ProofCard({ proof, view }: { proof: TetoProof; view: TetoView }) {
  return (
    <section className="teto__card" aria-labelledby="teto-prova">
      <div className="teto__cardhead">
        <ListChecks size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h2 id="teto-prova">A prova do número</h2>
        {proof.sourceNote ? (
          <span className="teto__note">Anotada na sua planilha</span>
        ) : null}
      </div>

      <ul className="teto__items">
        {proof.items.map((item) => (
          <li key={item.id} className="teto__item">
            <span className="teto__itemname">
              <b>{item.name}</b>
            </span>
            <span className="teto__itemamt">
              <Money cents={item.amountCents} size="inherit" />
              <small>/mês</small>
            </span>
          </li>
        ))}
      </ul>

      <div className="teto__formula">
        <div className="teto__frow">
          {/* O conceito ancora UMA vez, na linha que o soma — repetido em cada item, viraria
              ruído constante em vez de informação. */}
          <span>
            Total do <InfoPopover term={FREE_MONEY_TERM}>mês variável</InfoPopover>
          </span>
          <b>
            <Money cents={proof.totalCents} size="inherit" />
          </b>
        </div>
        <div className="teto__frow">
          <span>Dividido por</span>
          <b>{proof.divisorDays} dias</b>
        </div>
        <div className="teto__frow teto__frow--result">
          <span>Teto por dia</span>
          <b>
            <Money cents={proof.perDayCents} size="inherit" />
          </b>
        </div>
      </div>
      <p className="teto__round">Arredondado para cima.</p>

      {view.proofMatchesVerdict ? null : (
        <p className="teto__mismatch" role="note">
          O teto em vigor é <Money cents={view.currentPerDayCents} size="inherit" /> por
          dia — os itens acima somam outro número. Recalibre para os dois voltarem a
          contar a mesma história.
        </p>
      )}

      {proof.sourceNote ? (
        <details className="teto__fold">
          <summary>Ver a nota original da planilha</summary>
          <div className="teto__cite">
            <code>{proof.sourceNote}</code>
            <small>
              Reproduzida como está na nota da célula — a sua notação é o contrato: o
              que o app gravar de volta mantém esse formato.
            </small>
          </div>
        </details>
      ) : null}
    </section>
  );
}

function AgeCard({
  view,
  onRecalibrate,
}: {
  view: TetoView;
  onRecalibrate: () => void;
}) {
  const age = view.ageMonths;
  return (
    <section className="teto__card" aria-labelledby="teto-idade">
      <div className="teto__cardhead">
        <Clock3 size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        {/* O marcador "i" fica escondido: dentro do título ele partiria a frase — o pontilhado
            do termo já convida à didática. */}
        <h2 id="teto-idade">
          <InfoPopover term={CEREMONY_TERM} hideMarker>
            {age == null ? "A cerimônia do seu teto" : ceremonyAgeLabel(age)}
          </InfoPopover>
        </h2>
      </div>
      {/* A regra dos três meses mora no popover do título — aqui fica só o operando do
          prazo desta cerimônia e se ele já venceu. */}
      <p className="teto__cardbody">
        {recalibrationCaption(view.ceremonyMonth, view.recalibrationDue)}
      </p>
      <div className="teto__cardactions">
        <Button variant="primary" onClick={onRecalibrate}>
          Recalibrar o teto
        </Button>
      </div>
    </section>
  );
}

function ReadingCard({ view }: { view: TetoView }) {
  return (
    <section className="teto__card" aria-labelledby="teto-leitura">
      <div className="teto__cardhead">
        <Gauge size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h2 id="teto-leitura">Como o dia lê o teto</h2>
      </div>
      <p className="teto__cardbody">
        O <InfoPopover term={SPEEDOMETER_TERM}>velocímetro</InfoPopover> do dia está
        medindo{" "}
        {view.mode === "card" ? (
          <>
            as <b>faturas em aberto</b> — cada compra no crédito soma nelas.
          </>
        ) : (
          <>
            o <b>Diário lançado</b> contra este teto.
          </>
        )}
      </p>
    </section>
  );
}

// --------------------------------------------------------------------- rito --

function Rite({
  state,
  view,
  onChange,
  onClose,
}: {
  state: RiteState;
  view: TetoView;
  onChange: (next: RiteState) => void;
  onClose: () => void;
}) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  // Uma superfície por gesto: a batida que entra recebe o foco, para o teclado e o leitor de
  // tela seguirem a mesma sequência que o olho.
  useEffect(() => {
    headingRef.current?.focus();
  }, [state.step, state.guidedIndex]);

  const totalCents = draftTotalCents(state.items);
  const divisor = divisorFromText(state.divisorText);
  const newPerDayCents =
    state.step === "direct"
      ? (parseBRLToCents(state.directText) ?? 0)
      : ceilingPerDayCents(totalCents, divisor ?? 0);
  const showGuard =
    state.step === "accept" &&
    !state.guardAcknowledged &&
    guardTriggered(view.currentPerDayCents, newPerDayCents);

  function persist(perDayCents: number, itemized: boolean) {
    if (!isTauri) return;
    setError(null);
    if (perDayCents <= 0) {
      setError("O teto precisa ser maior que zero.");
      return;
    }
    const categories = itemized ? categoriesFromDraft(state.items) : [];
    setSaving(true);
    saveDailyBudgetCmd(perDayCents, categories, itemized ? (divisor ?? null) : null)
      .then(() => {
        invalidateCommands();
        onClose();
      })
      .catch((e: unknown) => setError(safeErrorMessage(e)))
      .finally(() => setSaving(false));
  }

  const stepLabel =
    state.step === "guided"
      ? `Pergunta ${state.guidedIndex + 1} de ${GUIDED_QUESTIONS.length}`
      : state.step === "direct"
        ? "Teto direto"
        : `Batida ${state.step === "items" ? 1 : state.step === "divisor" ? 2 : 3} de 3`;
  const dots = state.step === "guided" ? GUIDED_QUESTIONS.length : 3;
  const filled =
    state.step === "guided"
      ? state.guidedIndex + 1
      : state.step === "items"
        ? 1
        : state.step === "divisor"
          ? 2
          : 3;

  return (
    <section className="teto__rite" aria-labelledby="teto-batida">
      <p className="teto__step">{stepLabel}</p>
      <span className="teto__dots" aria-hidden="true">
        {Array.from({ length: dots }, (_, i) => (
          <i key={i} className={i < filled ? "on" : ""} />
        ))}
      </span>

      {showGuard ? (
        <Guard
          currentCents={view.currentPerDayCents}
          newCents={newPerDayCents}
          totalCents={totalCents}
          divisorDays={divisor ?? 0}
          headingRef={headingRef}
          onProceed={() => onChange({ ...state, guardAcknowledged: true })}
          onKeep={onClose}
        />
      ) : (
        <RiteStepBody
          state={state}
          view={view}
          totalCents={totalCents}
          divisor={divisor}
          newPerDayCents={newPerDayCents}
          headingRef={headingRef}
          saving={saving}
          onChange={onChange}
          onClose={onClose}
          onPersist={persist}
        />
      )}

      {error ? (
        <p className="teto__error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}

/** Contrato comum das batidas: o estado do rito e os derivados que a superfície exibe. */
interface BeatProps {
  state: RiteState;
  view: TetoView;
  totalCents: number;
  divisor: number | null;
  newPerDayCents: number;
  headingRef: React.RefObject<HTMLHeadingElement | null>;
  saving: boolean;
  onChange: (next: RiteState) => void;
  onClose: () => void;
  onPersist: (perDayCents: number, itemized: boolean) => void;
}

/** Uma batida por vez na superfície — o dispatcher só escolhe qual delas entra. */
function RiteStepBody(props: BeatProps) {
  switch (props.state.step) {
    case "guided":
      return <GuidedBeat {...props} />;
    case "items":
      return <ItemsBeat {...props} />;
    case "divisor":
      return <DivisorBeat {...props} />;
    case "direct":
      return <DirectBeat {...props} />;
    default:
      return <AcceptBeat {...props} />;
  }
}

/** A cerimônia guiada: as cinco perguntas do método, uma por vez. */
function GuidedBeat({ state, headingRef, onChange, onClose }: BeatProps) {
  const q = GUIDED_QUESTIONS[state.guidedIndex];
  if (!q) return null;
  const last = state.guidedIndex === GUIDED_QUESTIONS.length - 1;
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        {q.question}
      </h2>
      <p className="teto__sub">{q.hint}</p>
      <div className="teto__field">
        <label htmlFor="teto-guided">{q.category} por mês (R$)</label>
        <input
          id="teto-guided"
          className="teto__input teto__input--amount"
          inputMode="decimal"
          value={state.items[state.guidedIndex]?.amountText ?? ""}
          onChange={(e) =>
            onChange({
              ...state,
              items: state.items.map((it, i) =>
                i === state.guidedIndex ? { ...it, amountText: e.target.value } : it,
              ),
            })
          }
        />
      </div>
      <p className="teto__escape">
        <button
          type="button"
          className="teto__quiet"
          onClick={() => onChange({ ...state, step: "direct", returnTo: "guided" })}
        >
          Já sei meu teto
        </button>
      </p>
      <div className="teto__ritefoot">
        <Button
          variant="ghost"
          onClick={() =>
            state.guidedIndex === 0
              ? onClose()
              : onChange({ ...state, guidedIndex: state.guidedIndex - 1 })
          }
        >
          Voltar
        </Button>
        <Button
          variant="primary"
          onClick={() =>
            onChange(
              last
                ? { ...state, step: "divisor" }
                : { ...state, guidedIndex: state.guidedIndex + 1 },
            )
          }
        >
          {last ? "Definir os dias" : "Próxima pergunta"}
        </Button>
      </div>
    </>
  );
}

/** Batida 1: os itens do mês variável, revisados pelo extrato. */
function ItemsBeat({ state, totalCents, headingRef, onChange, onClose }: BeatProps) {
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        O que o seu mês variável comporta?
      </h2>
      <p className="teto__sub">
        Ajuste pelo extrato, não pela esperança — ele é a melhor testemunha do seu
        hábito.
      </p>
      <ul className="teto__rows">
        {state.items.map((item, i) => (
          <li key={item.key} className="teto__row">
            <input
              className="teto__input"
              aria-label={`Nome da categoria ${i + 1}`}
              placeholder="Categoria"
              value={item.name}
              onChange={(e) =>
                onChange({
                  ...state,
                  items: state.items.map((it, j) =>
                    j === i ? { ...it, name: e.target.value } : it,
                  ),
                })
              }
            />
            <input
              className="teto__input teto__input--amount"
              inputMode="decimal"
              aria-label={`Valor mensal da categoria ${i + 1}`}
              placeholder="R$ mensal"
              value={item.amountText}
              onChange={(e) =>
                onChange({
                  ...state,
                  items: state.items.map((it, j) =>
                    j === i ? { ...it, amountText: e.target.value } : it,
                  ),
                })
              }
            />
            <button
              type="button"
              className="teto__del"
              aria-label={`Remover ${item.name || `categoria ${i + 1}`}`}
              onClick={() =>
                onChange({
                  ...state,
                  items: state.items.filter((_, j) => j !== i),
                })
              }
            >
              <Trash2 size={14} strokeWidth={1.75} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
      <div className="teto__addrow">
        <Button
          variant="ghost"
          onClick={() => onChange({ ...state, items: [...state.items, blankItem()] })}
        >
          <Plus size={14} strokeWidth={1.75} />
          Adicionar categoria
        </Button>
      </div>
      <p className="teto__runsum" aria-live="polite">
        <span>Total do mês variável</span>
        <b>
          <Money cents={totalCents} size="inherit" />
        </b>
      </p>
      <p className="teto__escape">
        <button
          type="button"
          className="teto__quiet"
          onClick={() => onChange({ ...state, step: "direct", returnTo: "items" })}
        >
          Já sei meu teto
        </button>
      </p>
      <div className="teto__ritefoot">
        <Button variant="ghost" onClick={onClose}>
          Voltar
        </Button>
        <Button
          variant="primary"
          onClick={() => onChange({ ...state, step: "divisor" })}
        >
          Definir os dias
        </Button>
      </div>
    </>
  );
}

/** Batida 2: o divisor de dias — a régua que não se troca no meio do mês. */
function DivisorBeat({
  state,
  view,
  totalCents,
  divisor,
  headingRef,
  onChange,
}: BeatProps) {
  const invalid = divisor == null;
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        Por quantos dias dividir o total?
      </h2>
      <p className="teto__sub">
        A régua do método é fixar um número e mantê-lo o ano todo — trocar no meio do
        mês tira a comparação.
      </p>
      <div className="teto__divstage">
        <span className="teto__dividend">
          <Money cents={totalCents} size="inherit" />
        </span>
        <span className="teto__obelus" aria-hidden="true">
          ÷
        </span>
        <input
          id="teto-divisor"
          className="teto__input teto__input--divisor"
          inputMode="numeric"
          aria-label="Divisor de dias"
          aria-describedby={invalid ? "teto-divisor-err" : undefined}
          aria-invalid={invalid || undefined}
          value={state.divisorText}
          onChange={(e) => onChange({ ...state, divisorText: e.target.value })}
        />
        <span className="teto__unit">dias</span>
      </div>
      {invalid ? (
        <p className="teto__ferr" id="teto-divisor-err" role="alert">
          O divisor precisa de pelo menos 1 dia.
        </p>
      ) : (
        <p className="teto__divhint">
          O teto novo aparece no aceite, pronto e arredondado para cima.
        </p>
      )}
      <div className="teto__ritefoot">
        <Button
          variant="ghost"
          onClick={() =>
            onChange({
              ...state,
              step: view.kind === "none" ? "guided" : "items",
              guidedIndex: GUIDED_QUESTIONS.length - 1,
            })
          }
        >
          Voltar
        </Button>
        <Button
          variant="primary"
          disabled={invalid}
          onClick={() => onChange({ ...state, step: "accept" })}
        >
          Ver o teto novo
        </Button>
      </div>
    </>
  );
}

/** O atalho de quem já sabe o número: grava sem cerimônia, e sem prova a exibir. */
function DirectBeat({
  state,
  newPerDayCents,
  headingRef,
  saving,
  onChange,
  onClose,
  onPersist,
}: BeatProps) {
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        Qual é o seu teto por dia?
      </h2>
      <p className="teto__sub">
        Sem cerimônia: o número que você já sabe. A prova fica de fora até a próxima
        cerimônia.
      </p>
      <div className="teto__field">
        <label htmlFor="teto-direct">Teto por dia (R$)</label>
        <input
          id="teto-direct"
          className="teto__input teto__input--amount"
          inputMode="decimal"
          value={state.directText}
          onChange={(e) => onChange({ ...state, directText: e.target.value })}
        />
      </div>
      <div className="teto__ritefoot">
        <Button
          variant="ghost"
          onClick={() =>
            state.returnTo
              ? onChange({ ...state, step: state.returnTo, returnTo: undefined })
              : onClose()
          }
        >
          Voltar
        </Button>
        <Button
          variant="primary"
          disabled={saving}
          onClick={() => onPersist(newPerDayCents, false)}
        >
          Usar este teto
        </Button>
      </div>
    </>
  );
}

/** Batida 3: o aceite — antes → depois, prospectivo, e só então grava. */
function AcceptBeat({
  state,
  view,
  totalCents,
  divisor,
  newPerDayCents,
  headingRef,
  saving,
  onChange,
  onPersist,
}: BeatProps) {
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        O seu teto novo está pronto.
      </h2>
      <p className="teto__sub">
        <Money cents={totalCents} size="inherit" /> ÷ {divisor ?? 0} dias, arredondado
        para cima — teto é teto.
      </p>
      <div
        className="teto__accept"
        role="group"
        aria-label={
          view.currentPerDayCents > 0
            ? `Teto sai de ${brl(view.currentPerDayCents)} para ${brl(newPerDayCents)} por dia, válido daqui para frente`
            : `Teto de ${brl(newPerDayCents)} por dia, válido daqui para frente`
        }
      >
        <div className="teto__aline" aria-hidden="true">
          {view.currentPerDayCents > 0 ? (
            <>
              <span className="teto__aold">
                <Money cents={view.currentPerDayCents} size="inherit" />
              </span>
              <span className="teto__aarrow">→</span>
            </>
          ) : null}
          <span className="teto__anew">
            <Money cents={newPerDayCents} size="inherit" />
            <small>por dia</small>
          </span>
        </div>
        <p className="teto__ameta">Calculado agora, com os itens que você revisou</p>
        <p className="teto__avale">
          Vale daqui para frente — os dias já vividos não mudam.
        </p>
      </div>
      <div className="teto__ritefoot">
        <Button
          variant="ghost"
          onClick={() =>
            onChange({ ...state, step: "divisor", guardAcknowledged: false })
          }
        >
          Voltar
        </Button>
        <Button
          variant="primary"
          disabled={saving}
          onClick={() => onPersist(newPerDayCents, true)}
        >
          Usar este teto
        </Button>
      </div>
    </>
  );
}

function Guard({
  currentCents,
  newCents,
  totalCents,
  divisorDays,
  headingRef,
  onProceed,
  onKeep,
}: {
  currentCents: number;
  newCents: number;
  totalCents: number;
  divisorDays: number;
  headingRef: React.RefObject<HTMLHeadingElement | null>;
  onProceed: () => void;
  onKeep: () => void;
}) {
  return (
    <>
      <h2 id="teto-batida" tabIndex={-1} ref={headingRef}>
        Antes de baixar o teto, vença o dia primeiro.
      </h2>
      <div className="teto__guard">
        <p>
          Você está baixando o teto de <b>{brl(currentCents)}</b> para{" "}
          <b>{brl(newCents)}</b> — e o seu extrato ainda conta a história antiga. Baixar
          por esperança pinta a planilha de verde hoje, mas o extrato segue o mesmo, e o
          vermelho reaparece na frente. Mantenha a régua, gaste menos de verdade, e o
          número desce sozinho na próxima cerimônia.
        </p>
        <p className="teto__ameta">
          {brl(totalCents)} ÷ {divisorDays} dias, arredondado para cima
        </p>
      </div>
      <div className="teto__ritefoot">
        <Button variant="ghost" onClick={onProceed}>
          Baixar assim mesmo
        </Button>
        <Button variant="primary" onClick={onKeep}>
          Manter {brl(currentCents)} por dia
        </Button>
      </div>
    </>
  );
}

/** Dinheiro dentro de texto puro (aria-label, rótulo de botão) — o componente Money é JSX. */
function brl(cents: number): string {
  return formatBRL(cents);
}
