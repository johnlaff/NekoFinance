import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  AlertTriangle,
  ArrowLeft,
  Check,
  ChevronLeft,
  ChevronRight,
  ListChecks,
  Pencil,
  Repeat,
  ShoppingBag,
  Undo2,
  Users,
} from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Meter } from "../design-system/components/Meter";
import { Money } from "../design-system/components/Money";
import { NekoMark } from "../design-system/components/NekoMark";
import { NoRecordDash } from "../design-system/components/NoRecordDash";
import { OwnerChip } from "../design-system/components/OwnerChip";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import {
  acceptCardProposal,
  cancelCardSeries,
  createCardAccount,
  dismissCardProposal,
  getDashboardSummary,
  getInvoice,
  isTauri,
  listCardProposals,
  listCards,
  listInvoices,
  moveCardPurchase,
  setInvoiceStatedTotal,
  updateCardAccount,
  updateCardSeries,
  type Card,
  type CardProposal,
  type InvoiceDetail,
  type InvoiceSummary,
} from "../lib/api";
import { shiftCycleMonth, validateCardCycle } from "../lib/cardCycle";
import { safeErrorMessage } from "../lib/errors";
import { centsToBRLInput, parseBRLToCents } from "../lib/format";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import {
  buildBars,
  cycleOptions,
  cycleStateLabel,
  cycleWindow,
  dateLabel,
  defaultInvoiceId,
  groupSeries,
  heroSubtitle,
  installmentProgress,
  metaLine,
  monthLabelLower,
  netOfRefunds,
  ownerKind,
  subscriptionCadence,
  verdictLine,
} from "./cartoesView";
import "./cartoes.css";

// A tela Cartões é o sub-ledger de faturas: cada fatura é um lump por cartão
// que vira Saída no vencimento. Toda derivação de exibição vive no view-model
// puro `cartoesView`; aqui é a superfície — card-face, drill da fatura como
// herói e os gestos do domínio (ajuste, remanejo, séries, proposta).

const INVOICE_TERM = {
  title: "Como a fatura entra no mês",
  body: "Cada fatura é uma linha de Saída no vencimento — um lump por cartão, do jeito que a planilha registra. As compras itemizadas explicam a fatura por dentro; as séries pré-lançam nas faturas futuras; o reembolso vinculado volta como Entrada no vencimento. O limite fica discreto de propósito: não é régua.",
};

const GATE_TERM = {
  title: "Modo cartão",
  body: "A economia anual e a reserva de seis meses são as duas pernas observadas. Sem pressa para o próximo objetivo patrimonial. Se quiser conferir seu padrão, vale testar 2–3 meses no débito — o app acompanha do mesmo jeito.",
};

/** Piso do método: economia ≥ 20% da renda anual (`SAVINGS_FLOOR_BPS` no motor). */
const ECONOMY_GATE_TARGET_PCT = 20;
/** Piso do método: reserva ≥ 6 meses de custo de vida (`RESERVE_MIN_MONTHS` no motor). */
const RESERVE_GATE_TARGET_MONTHS = 6;

// ------------------------------------------------------------ demo (web) --

/** Dia fixo do fallback web: mantém "Fecha em N dias" determinístico nos baselines. */
const DEMO_TODAY = "2026-07-15";

const DEMO_AGO: InvoiceSummary = {
  id: "demo-ago",
  cycle_month: "2026-08",
  closing_date: "2026-07-20",
  due_date: "2026-08-10",
  status: "aberta",
  stated_total_cents: 428_900,
  purchases_sum_cents: 403_900,
  effective_total_cents: 428_900,
  reconciliation_delta_cents: 25_000,
};

const DEMO_INVOICES: InvoiceSummary[] = [
  { ...DEMO_AGO, id: "demo-mar", cycle_month: "2026-03", closing_date: "2026-02-20", due_date: "2026-03-10", status: "paga", stated_total_cents: 163_000, purchases_sum_cents: 163_000, effective_total_cents: 163_000, reconciliation_delta_cents: null },
  { ...DEMO_AGO, id: "demo-abr", cycle_month: "2026-04", closing_date: "2026-03-20", due_date: "2026-04-10", status: "paga", stated_total_cents: 223_000, purchases_sum_cents: 223_000, effective_total_cents: 223_000, reconciliation_delta_cents: null },
  { ...DEMO_AGO, id: "demo-mai", cycle_month: "2026-05", closing_date: "2026-04-20", due_date: "2026-05-10", status: "paga", stated_total_cents: 189_000, purchases_sum_cents: 189_000, effective_total_cents: 189_000, reconciliation_delta_cents: null },
  { ...DEMO_AGO, id: "demo-jun", cycle_month: "2026-06", closing_date: "2026-05-20", due_date: "2026-06-10", status: "paga", stated_total_cents: 270_000, purchases_sum_cents: 270_000, effective_total_cents: 270_000, reconciliation_delta_cents: null },
  { ...DEMO_AGO, id: "demo-jul", cycle_month: "2026-07", closing_date: "2026-06-20", due_date: "2026-07-10", status: "fechada", stated_total_cents: 249_000, purchases_sum_cents: 249_000, effective_total_cents: 249_000, reconciliation_delta_cents: null },
  DEMO_AGO,
];

const DEMO_DETAIL: InvoiceDetail = {
  ...DEMO_AGO,
  purchases: [
    { txn_id: "p1", date: "2026-06-25", description: "Mercado", amount_cents: 239_900, owner_name: "Eu", series_id: null, installment_label: null, is_projection: false },
    { txn_id: "p2", date: "2026-07-02", description: "Notebook", amount_cents: 64_000, owner_name: "Eu", series_id: "s2", installment_label: "2/5", is_projection: false },
    { txn_id: "p3", date: "2026-07-10", description: "Farmácia", amount_cents: 95_010, owner_name: "Parceiro(a)", series_id: null, installment_label: null, is_projection: false },
    { txn_id: "p4", date: "2026-07-15", description: "Streaming", amount_cents: 4_990, owner_name: "Eu", series_id: "s1", installment_label: null, is_projection: false },
  ],
  refunds: [
    { txn_id: "r1", date: "2026-08-10", amount_cents: 35_000, description: "Parte compartilhada", is_projection: true },
  ],
  sub_invoices: [
    { account_id: "demo-additional", card_name: "Cartão adicional", owner_name: "Parceiro(a)", effective_total_cents: 86_000 },
  ],
  emitter_total_cents: 514_900,
};

const DEMO_CARDS: Card[] = [
  {
    id: "demo-holder",
    name: "Cartão principal",
    institution: "Instituição demo",
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: 1_200_000,
    aliases: ["Principal"],
    open_invoice: DEMO_AGO,
    next_due: DEMO_AGO,
  },
  {
    id: "demo-additional",
    name: "Cartão adicional",
    institution: "Instituição demo",
    owner_name: "Parceiro(a)",
    linked_account_id: "demo-holder",
    closing_day: 20,
    due_day: 10,
    credit_limit_cents: null,
    aliases: ["Adicional"],
    open_invoice: null,
    next_due: null,
  },
  {
    id: "demo-reserve",
    name: "Cartão reserva",
    institution: null,
    owner_name: "Eu",
    linked_account_id: null,
    closing_day: 3,
    due_day: 10,
    credit_limit_cents: null,
    aliases: [],
    open_invoice: null,
    next_due: null,
  },
];

const DEMO_PROPOSALS: CardProposal[] = [
  {
    id: "proposal-1",
    alias: "Cartão de viagens",
    display_name: "Cartão de viagens",
    source_month: "2026-06",
    status: "pending",
  },
];

const DEMO_CARD_GATE = {
  card_gate_economy: "alive" as const,
  card_gate_economy_bps: 2_400,
  card_gate_reserve: "below" as const,
  reserve_months: 4.2,
};

function demoDetailFor(invoiceId: string | null): InvoiceDetail | null {
  if (!invoiceId) return null;
  if (invoiceId === DEMO_DETAIL.id) return DEMO_DETAIL;
  const summary = DEMO_INVOICES.find((invoice) => invoice.id === invoiceId);
  if (!summary) return null;
  return {
    ...summary,
    purchases: [],
    refunds: [],
    sub_invoices: [],
    emitter_total_cents: summary.effective_total_cents,
  };
}

function localTodayISO(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

// Fetchers estáveis por chave (padrão do YearGrid/Horizonte): o useCommand
// recebe sempre a MESMA função para a mesma chave.
const _invoiceListFetchers = new Map<string, () => Promise<InvoiceSummary[]>>();
function invoicesFetcher(cardId: string) {
  let fn = _invoiceListFetchers.get(cardId);
  if (!fn) {
    fn = () => listInvoices(cardId);
    _invoiceListFetchers.set(cardId, fn);
  }
  return fn;
}

const _invoiceDetailFetchers = new Map<string, () => Promise<InvoiceDetail | null>>();
function detailFetcher(invoiceId: string | null) {
  const key = invoiceId ?? "none";
  let fn = _invoiceDetailFetchers.get(key);
  if (!fn) {
    fn = () => (invoiceId ? getInvoice(invoiceId) : Promise.resolve(null));
    _invoiceDetailFetchers.set(key, fn);
  }
  return fn;
}

type GateState = "alive" | "below" | "unknown";
type CardGateSummary = Pick<
  Awaited<ReturnType<typeof getDashboardSummary>>,
  "card_gate_economy" | "card_gate_economy_bps" | "card_gate_reserve" | "reserve_months"
>;

// ------------------------------------------------------------------- tela --

export function CartoesScreen() {
  const cardsQ = useCommand("list_cards", listCards);
  const proposalsQ = useCommand("list_card_proposals", listCardProposals);
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const cards = isTauri ? (cardsQ.data ?? []) : DEMO_CARDS;
  const proposals = isTauri ? (proposalsQ.data ?? []) : DEMO_PROPOSALS;
  const gateSummary = isTauri ? summaryQ.data : DEMO_CARD_GATE;
  const todayISO = isTauri ? localTodayISO() : DEMO_TODAY;

  const [form, setForm] = useState<{ proposal?: CardProposal; card?: Card } | null>(
    null,
  );
  const [chosenCardId, setChosenCardId] = useState<string | null>(null);
  // Mobile: lista → drill como estado da tela; o DOM não muda, só a visibilidade.
  const [drilled, setDrilled] = useState(false);
  const detailRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const holderCards = cards.filter((card) => !card.linked_account_id);
  const additions = cards.reduce((byHolder, card) => {
    if (!card.linked_account_id) return byHolder;
    const linkedCards = byHolder.get(card.linked_account_id) ?? [];
    linkedCards.push(card);
    byHolder.set(card.linked_account_id, linkedCards);
    return byHolder;
  }, new Map<string, Card[]>());
  const selectedCard =
    holderCards.find((card) => card.id === chosenCardId) ?? holderCards[0] ?? null;

  if (isTauri && cardsQ.error) {
    return (
      <div className="cartoes">
        <EmptyState
          variant="error"
          title="Não foi possível carregar os cartões"
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

  if (isTauri && !cardsQ.data) {
    return (
      <div className="cartoes">
        <EmptyState variant="skeleton" skeletonRows={5} />
      </div>
    );
  }

  // No mobile o drill troca a vista: leva o topo do detalhe à viewport antes de
  // focar (focus sozinho rola só o suficiente e deixa o herói cortado).
  const isMobile = () => window.matchMedia("(max-width: 899px)").matches;
  const selectCard = (id: string) => {
    setChosenCardId(id);
    setDrilled(true);
    requestAnimationFrame(() => {
      if (isMobile()) detailRef.current?.scrollIntoView?.({ block: "start" });
      // O painel nasce display:none no mobile — o centering do ciclo no mount
      // não tem layout; refeito aqui, quando o drill acaba de ficar visível.
      detailRef.current
        ?.querySelector('[aria-checked="true"]')
        ?.scrollIntoView?.({ inline: "center", block: "nearest" });
      detailRef.current?.focus({ preventScroll: true });
    });
  };
  const backToList = () => {
    setDrilled(false);
    requestAnimationFrame(() => {
      if (isMobile()) listRef.current?.scrollIntoView?.({ block: "start" });
      listRef.current?.focus({ preventScroll: true });
    });
  };

  return (
    <div className={`cartoes${drilled ? " cartoes--drilled" : ""}`}>
      <section className="cartoes__head" data-large-title>
        <h1>{verdictLine(holderCards)}</h1>
        {cards.length > 0 ? <CardGate summary={gateSummary} /> : null}
      </section>

      {proposals.map((proposal) => (
        <ProposalBanner
          key={proposal.id}
          proposal={proposal}
          onCreate={() => setForm({ proposal })}
        />
      ))}

      {cards.length === 0 && proposals.length === 0 ? (
        <EmptyState
          title="Nenhum cartão cadastrado"
          description="Cadastre um cartão para acompanhar faturas, assinaturas e parcelas."
          action={
            <Button variant="primary" onClick={() => setForm({})}>
              Adicionar cartão
            </Button>
          }
        />
      ) : null}

      {form ? (
        <CardForm
          {...(form.card ? { initial: form.card } : {})}
          {...(form.proposal ? { proposal: form.proposal } : {})}
          holders={holderCards}
          onClose={() => setForm(null)}
        />
      ) : holderCards.length > 0 ? (
        <div className="cartoes__cols">
          <div
            className="cartoes__list"
            ref={listRef}
            tabIndex={-1}
            aria-label="Seus cartões"
          >
            {holderCards.map((card) => (
              <CardTile
                key={card.id}
                card={card}
                selected={card.id === selectedCard?.id}
                additionals={additions.get(card.id) ?? []}
                todayISO={todayISO}
                onSelect={() => selectCard(card.id)}
                onEdit={() => setForm({ card })}
              />
            ))}
            <Button
              variant="ghost"
              size="sm"
              className="cartoes__add"
              onClick={() => setForm({})}
            >
              Adicionar cartão
            </Button>
          </div>
          <div
            className="cartoes__detail"
            ref={detailRef}
            tabIndex={-1}
            aria-label={
              selectedCard ? `Faturas de ${selectedCard.name}` : "Faturas"
            }
          >
            {drilled ? (
              <div className="cartoes__back">
                <Button variant="ghost" size="sm" onClick={backToList}>
                  <ArrowLeft size={15} aria-hidden="true" />
                  Voltar
                </Button>
              </div>
            ) : null}
            {selectedCard ? (
              <InvoicePanel
                key={selectedCard.id}
                card={selectedCard}
                todayISO={todayISO}
              />
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------- proposta --

function ProposalBanner({
  proposal,
  onCreate,
}: {
  proposal: CardProposal;
  onCreate: () => void;
}) {
  const [busy, setBusy] = useState(false);

  function dismiss() {
    if (!isTauri) return;
    setBusy(true);
    void dismissCardProposal(proposal.id)
      .then(invalidateCommands)
      .catch(() => undefined)
      .finally(() => setBusy(false));
  }

  return (
    <section
      className="cartoes__proposal"
      aria-label={`Proposta de cartão: ${proposal.display_name}`}
    >
      <span className="cartoes__sniff">
        <Activity size={14} aria-hidden="true" />
        A Mia farejou um cartão na planilha
      </span>
      <p className="cartoes__proposal-copy">
        Uma linha da seção de cartões de {monthLabelLower(proposal.source_month)} não casa
        com nenhum cartão que o app conhece: <strong>{proposal.display_name}</strong>.
        Quer cadastrar para acompanhar a fatura?
      </p>
      <div className="cartoes__actions">
        <Button variant="primary" size="sm" disabled={busy} onClick={onCreate}>
          Cadastrar cartão
        </Button>
        <Button variant="ghost" size="sm" disabled={busy} onClick={dismiss}>
          Dispensar
        </Button>
      </div>
    </section>
  );
}

// -------------------------------------------------------------------- gate --

function CardGate({ summary }: { summary: CardGateSummary | undefined }) {
  const economyPct =
    summary?.card_gate_economy_bps != null ? summary.card_gate_economy_bps / 100 : null;
  return (
    <div className="cartoes__gate" role="group" aria-label="Gate do modo cartão">
      <GateLeg
        label="Economia viva"
        state={summary?.card_gate_economy}
        current={economyPct}
        target={ECONOMY_GATE_TARGET_PCT}
        decimals={0}
        unit="%"
      />
      <span className="cartoes__gate-sep" aria-hidden="true" />
      <GateLeg
        label="Reserva de 6 meses"
        state={summary?.card_gate_reserve}
        current={summary?.reserve_months ?? null}
        target={RESERVE_GATE_TARGET_MONTHS}
        decimals={1}
        unit=" meses"
      />
      <InfoPopover term={GATE_TERM}>
        <span className="cartoes__gate-help">Como esta leitura funciona</span>
      </InfoPopover>
    </div>
  );
}

function GateLeg({
  label,
  state,
  current,
  target,
  decimals,
  unit,
}: {
  label: string;
  state: GateState | undefined;
  /** Número atual já na unidade de exibição (percentual ou meses). */
  current: number | null;
  target: number;
  decimals: number;
  unit: string;
}) {
  if (state === undefined) {
    return <span className="cartoes__gate-loading">Carregando…</span>;
  }
  if (state === "unknown" || current == null) {
    return (
      <span className="cartoes__gate-leg">
        <NoRecordDash
          label="Sem registro"
          term={{ body: `${label}: faltam dados registrados para esta leitura.` }}
        />
      </span>
    );
  }

  const format = (value: number, fractionDigits: number) =>
    `${value.toLocaleString("pt-BR", {
      minimumFractionDigits: fractionDigits,
      maximumFractionDigits: fractionDigits,
    })}${unit}`;
  const alive = state === "alive";
  // O alvo (20%, 6 meses) é sempre um número redondo por desenho do método — mostrado sem casas
  // decimais mesmo quando a leitura atual usa precisão fina (ex.: "4,2 meses ... p/ 6").
  const missing = Math.max(0, target - current);
  return (
    <span className={`cartoes__gate-leg ${alive ? "is-ok" : "is-warn"}`}>
      {alive ? (
        <Check size={15} aria-hidden="true" />
      ) : (
        <AlertTriangle size={15} aria-hidden="true" />
      )}
      {label} — {format(current, decimals)}
      {!alive ? ` (falta ${format(missing, decimals)} p/ ${format(target, 0)})` : ""}
    </span>
  );
}

// ------------------------------------------------------------------- lista --

function StatusChip({ status }: { status: InvoiceSummary["status"] }) {
  const label = status.charAt(0).toUpperCase() + status.slice(1);
  return <span className={`cartoes__chip cartoes__chip--${status}`}>{label}</span>;
}

function CardTile({
  card,
  selected,
  additionals,
  todayISO,
  onSelect,
  onEdit,
}: {
  card: Card;
  selected: boolean;
  additionals: Card[];
  todayISO: string;
  onSelect: () => void;
  onEdit: () => void;
}) {
  const next = card.next_due;

  if (!selected) {
    return (
      <button
        type="button"
        className="cartoes__row"
        onClick={onSelect}
        aria-label={`Abrir as faturas de ${card.name}`}
      >
        <span className="cartoes__row-main">
          <b>{card.name}</b>
          <span className="cartoes__discrete">{metaLine(card)}</span>
        </span>
        {next ? (
          <StatusChip status={next.status} />
        ) : (
          <span className="cartoes__dash" aria-hidden="true">
            —
          </span>
        )}
      </button>
    );
  }

  return (
    <div className="cartoes__selected">
      <button
        type="button"
        className="cartoes__face"
        onClick={onSelect}
        aria-current="true"
        aria-label={`Faturas de ${card.name} — cartão selecionado`}
      >
        <span className="cartoes__face-top">
          <span>
            <span className="cartoes__face-name">{card.name}</span>
            <span className="cartoes__face-inst">
              Titular · {card.owner_name}
              {card.institution ? ` · ${card.institution}` : ""}
            </span>
          </span>
          <NekoMark width={26} height={26} className="cartoes__face-cat" />
        </span>
        <span className="cartoes__face-foot">
          {next ? (
            <>
              <span className="cartoes__face-due">
                <small>Próxima fatura</small>
                <b>Vence {dateLabel(next.due_date)}</b>
              </span>
              <span className="cartoes__face-amt">
                <Money cents={next.effective_total_cents} size="inherit" />
                <small>{cycleStateLabel(next, todayISO)}</small>
              </span>
            </>
          ) : (
            <span className="cartoes__face-due">
              <small>Próxima fatura</small>
              <b>Sem fatura registrada ainda</b>
            </span>
          )}
        </span>
      </button>

      <div className="cartoes__meta">
        <div className="cartoes__meta-row">
          <span className="cartoes__discrete">{metaLine(card)}</span>
          <Button
            variant="ghost"
            size="sm"
            aria-label={`Editar ${card.name}`}
            onClick={onEdit}
          >
            <Pencil size={14} aria-hidden="true" />
            Editar
          </Button>
        </div>
        {additionals.map((additional) => (
          <div key={additional.id} className="cartoes__additional">
            <div className="cartoes__additional-row">
              <span className="cartoes__additional-name">
                {additional.name} <OwnerChip who={ownerKind(additional.owner_name)} />
              </span>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Editar ${additional.name}`}
                onClick={onEdit}
              >
                <Pencil size={14} aria-hidden="true" />
              </Button>
            </div>
            <span className="cartoes__discrete">
              Herda o ciclo do titular · sub-fatura própria
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------- drill --

function InvoicePanel({ card, todayISO }: { card: Card; todayISO: string }) {
  const invoicesQ = useCommand(`list_invoices:${card.id}`, invoicesFetcher(card.id));
  const invoices = isTauri
    ? (invoicesQ.data ?? [])
    : card.id === "demo-holder"
      ? DEMO_INVOICES
      : [];
  const cycles = cycleWindow(invoices);
  const [chosen, setChosen] = useState<string | null>(null);
  const selectedId =
    chosen && cycles.some((invoice) => invoice.id === chosen)
      ? chosen
      : defaultInvoiceId(card, invoices);

  const detailQ = useCommand(
    `get_invoice:${selectedId ?? "none"}`,
    detailFetcher(selectedId),
  );
  const detail = isTauri ? (detailQ.data ?? null) : demoDetailFor(selectedId);

  if (cycles.length === 0 || !selectedId) {
    return (
      <section className="cartoes__panel">
        <NoRecordDash
          label="Sem fatura registrada"
          term={{
            body: "A primeira fatura aparece quando uma compra, uma série ou a planilha registra o ciclo.",
          }}
        />
        <p className="cartoes__discrete">
          A fatura nasce da planilha ou do primeiro lançamento no cartão.
        </p>
      </section>
    );
  }

  const bars = buildBars(cycles, selectedId);
  const selectedSummary = cycles.find((invoice) => invoice.id === selectedId);

  return (
    <section className="cartoes__panel" aria-label={`Fatura de ${card.name}`}>
      <CycleScroller selectedId={selectedId}>
        <SegmentedControl
          ariaLabel="Ciclo da fatura"
          size="sm"
          className="cartoes__cycles"
          value={selectedId}
          onChange={setChosen}
          options={cycleOptions(cycles)}
        />
      </CycleScroller>

      {detail ? (
        <InvoiceDetailBody
          card={card}
          detail={detail}
          bars={bars}
          todayISO={todayISO}
        />
      ) : selectedSummary ? (
        <EmptyState variant="skeleton" skeletonRows={4} />
      ) : null}
    </section>
  );
}

function InvoiceDetailBody({
  card,
  detail,
  bars,
  todayISO,
}: {
  card: Card;
  detail: InvoiceDetail;
  bars: ReturnType<typeof buildBars>;
  todayISO: string;
}) {
  const [adjusting, setAdjusting] = useState(false);
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [editingSeries, setEditingSeries] = useState<{
    id: string;
    description: string;
    amount: string;
  } | null>(null);

  const series = groupSeries(detail.purchases);
  const net = netOfRefunds(detail);

  function saveStatedTotal() {
    if (!isTauri) return;
    const cents = parseBRLToCents(amount);
    if (cents == null) return;
    setBusy(true);
    void setInvoiceStatedTotal(detail.id, cents)
      .then(() => {
        setAdjusting(false);
        invalidateCommands();
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  }

  function movePurchase(txnId: string, delta: number) {
    if (!isTauri) return;
    void moveCardPurchase(txnId, shiftCycleMonth(detail.cycle_month, delta))
      .then(invalidateCommands)
      .catch(() => undefined);
  }

  function saveSeries() {
    if (!editingSeries || !isTauri) return;
    const cents = parseBRLToCents(editingSeries.amount);
    if (cents == null) return;
    setBusy(true);
    void updateCardSeries(editingSeries.id, editingSeries.description, cents)
      .then(() => {
        setEditingSeries(null);
        invalidateCommands();
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  }

  function cancelSeries(seriesId: string, description: string) {
    if (
      !isTauri ||
      !window.confirm(`Cancelar "${description}" a partir deste ciclo?`)
    )
      return;
    void cancelCardSeries(seriesId, detail.cycle_month)
      .then(invalidateCommands)
      .catch(() => undefined);
  }

  return (
    <>
      <div className="cartoes__hero">
        <div className="cartoes__hero-amt">
          <Money cents={detail.effective_total_cents} size="display" />
          <small>{heroSubtitle(detail.stated_total_cents)}</small>
        </div>
        <div className="cartoes__hero-due">
          <StatusChip status={detail.status} />
          <b>Vence {dateLabel(detail.due_date)}</b>
          <small>{cycleStateLabel(detail, todayISO)}</small>
        </div>
      </div>
      <InfoPopover term={INVOICE_TERM}>
        <span className="cartoes__how">Como a fatura entra no mês</span>
      </InfoPopover>

      <div role="img" aria-label={bars.aria}>
        <div className="cartoes__bars">
          {bars.bars.map((bar) => (
            <i
              key={bar.id}
              className={bar.selected ? "is-selected" : undefined}
              style={{ height: `${bar.pct}%` }}
            />
          ))}
        </div>
        <div className="cartoes__bars-labels">
          {bars.bars.map((bar) => (
            <em key={bar.id}>{bar.label}</em>
          ))}
        </div>
      </div>
      <p className="cartoes__bars-cap">{bars.caption}</p>

      <SectionHead icon={<ListChecks size={15} aria-hidden="true" />} title="Totais" />
      <div className="cartoes__totline cartoes__totline--head">
        <span>Total declarado</span>
        <b>
          <Money cents={detail.effective_total_cents} size="inherit" />
        </b>
      </div>
      <div className="cartoes__totline">
        <span>Compras itemizadas</span>
        <Money cents={detail.purchases_sum_cents} size="inherit" />
      </div>
      {detail.reconciliation_delta_cents != null ? (
        <div className="cartoes__recon">
          <span>Não itemizado — parte da fatura sem linha</span>
          <Money cents={detail.reconciliation_delta_cents} size="inherit" />
        </div>
      ) : null}
      {adjusting ? (
        <div className="cartoes__adjust-row">
          <input
            aria-label="Total declarado"
            className="cartoes__field"
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
          />
          <Button variant="primary" size="sm" disabled={busy} onClick={saveStatedTotal}>
            Confirmar
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setAdjusting(false)}>
            Cancelar
          </Button>
        </div>
      ) : (
        <Button
          variant="ghost"
          size="sm"
          className="cartoes__adjust"
          onClick={() => {
            setAmount(
              centsToBRLInput(detail.stated_total_cents ?? detail.effective_total_cents),
            );
            setAdjusting(true);
          }}
        >
          Ajustar total declarado
        </Button>
      )}

      {detail.purchases.length > 0 ? (
        <>
          <SectionHead
            icon={<ShoppingBag size={15} aria-hidden="true" />}
            title="Compras"
          />
          <ul className="cartoes__rows">
            {detail.purchases.map((purchase) => (
              <li key={purchase.txn_id}>
                <span className="cartoes__what">
                  <b>
                    {purchase.description}
                    {purchase.installment_label ? (
                      <span className="cartoes__parc">{purchase.installment_label}</span>
                    ) : null}
                  </b>
                  <small>
                    {dateLabel(purchase.date)} ·{" "}
                    <OwnerChip who={ownerKind(purchase.owner_name)} />
                  </small>
                </span>
                {purchase.series_id ? null : (
                  <span className="cartoes__move">
                    <button
                      type="button"
                      className="cartoes__move-btn"
                      aria-label={`Mover ${purchase.description} para o ciclo anterior`}
                      onClick={() => movePurchase(purchase.txn_id, -1)}
                    >
                      <ChevronLeft size={15} aria-hidden="true" />
                    </button>
                    <button
                      type="button"
                      className="cartoes__move-btn"
                      aria-label={`Mover ${purchase.description} para o ciclo seguinte`}
                      onClick={() => movePurchase(purchase.txn_id, 1)}
                    >
                      <ChevronRight size={15} aria-hidden="true" />
                    </button>
                  </span>
                )}
                <span className="cartoes__val">
                  <Money cents={purchase.amount_cents} size="inherit" />
                </span>
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {series.length > 0 ? (
        <>
          <SectionHead icon={<Repeat size={15} aria-hidden="true" />} title="Séries" />
          <div className="cartoes__series">
            {series.map(({ id, kind, occurrence }) => {
              const isEditing = editingSeries?.id === id;
              const progress =
                kind === "installment"
                  ? installmentProgress(occurrence.installment_label, occurrence.amount_cents)
                  : null;
              return (
                <div key={id} className="cartoes__serie">
                  {isEditing ? (
                    <div className="cartoes__serie-edit">
                      <label className="cartoes__label">
                        Descrição
                        <input
                          className="cartoes__field"
                          value={editingSeries.description}
                          onChange={(event) =>
                            setEditingSeries({
                              ...editingSeries,
                              description: event.target.value,
                            })
                          }
                        />
                      </label>
                      <label className="cartoes__label">
                        Valor BRL
                        <input
                          className="cartoes__field"
                          inputMode="decimal"
                          value={editingSeries.amount}
                          onChange={(event) =>
                            setEditingSeries({
                              ...editingSeries,
                              amount: event.target.value,
                            })
                          }
                        />
                      </label>
                      <p className="cartoes__discrete">Regenera as ocorrências futuras.</p>
                      <div className="cartoes__actions">
                        <Button variant="primary" size="sm" disabled={busy} onClick={saveSeries}>
                          Salvar
                        </Button>
                        <Button variant="ghost" size="sm" onClick={() => setEditingSeries(null)}>
                          Cancelar
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <div className="cartoes__serie-head">
                        <b>
                          {occurrence.description} —{" "}
                          {kind === "installment" ? "parcelado" : "assinatura"}
                        </b>
                        <Money cents={occurrence.amount_cents} size="inherit" />
                      </div>
                      {progress ? (
                        <>
                          <Meter
                            className="cartoes__prog"
                            fraction={progress.fraction}
                            color="var(--accent)"
                          />
                          <span className="cartoes__prog-cap">
                            Parcela {progress.current} de {progress.total} · faltam{" "}
                            <Money cents={progress.remainingCents} size="inherit" /> em{" "}
                            {progress.remainingCycles}{" "}
                            {progress.remainingCycles === 1 ? "fatura" : "faturas"}
                          </span>
                        </>
                      ) : kind === "subscription" ? (
                        <span className="cartoes__prog-cap">
                          {subscriptionCadence(occurrence.date)}
                        </span>
                      ) : null}
                      <div className="cartoes__actions">
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Editar ${occurrence.description}`}
                          onClick={() =>
                            setEditingSeries({
                              id,
                              description: occurrence.description,
                              amount: centsToBRLInput(occurrence.amount_cents),
                            })
                          }
                        >
                          Editar
                        </Button>
                        {kind === "subscription" ? (
                          <Button
                            variant="ghost"
                            size="sm"
                            aria-label={`Cancelar ${occurrence.description} a partir deste ciclo`}
                            onClick={() => cancelSeries(id, occurrence.description)}
                          >
                            Cancelar a partir deste ciclo
                          </Button>
                        ) : null}
                      </div>
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </>
      ) : null}

      {detail.refunds.length > 0 ? (
        <>
          <SectionHead
            icon={<Undo2 size={15} aria-hidden="true" />}
            title="Reembolsos vinculados"
          />
          <ul className="cartoes__rows">
            {detail.refunds.map((refund) => (
              <li key={refund.txn_id}>
                <span className="cartoes__what">
                  <b>
                    {refund.description}
                    {refund.is_projection ? (
                      <span className="cartoes__chip cartoes__chip--prevista">
                        Prevista
                      </span>
                    ) : null}
                  </b>
                  <small>
                    Entra como Entrada no vencimento — a régua julga o valor cheio
                  </small>
                </span>
                <span className="cartoes__val cartoes__val--in">
                  + <Money cents={refund.amount_cents} size="inherit" />
                </span>
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {detail.sub_invoices.length > 0 ? (
        <>
          <SectionHead
            icon={<Users size={15} aria-hidden="true" />}
            title="Recorte por dono"
          />
          <div className="cartoes__subinv">
            <span>
              <OwnerChip who={ownerKind(card.owner_name)} /> Titular
            </span>
            <Money cents={detail.effective_total_cents} size="inherit" />
          </div>
          {detail.sub_invoices.map((sub) => (
            <div key={sub.account_id} className="cartoes__subinv">
              <span>
                <OwnerChip who={ownerKind(sub.owner_name)} /> {sub.card_name}
              </span>
              <Money cents={sub.effective_total_cents} size="inherit" />
            </div>
          ))}
          <div className="cartoes__emitter">
            <span>Total do emissor</span>
            <b>
              <Money cents={detail.emitter_total_cents} size="inherit" />
            </b>
          </div>
        </>
      ) : null}

      <p className="cartoes__net">
        Líquido de reembolsos <Money cents={net} size="inherit" />
        <InfoPopover
          term={{
            title: "Leitura de conferência",
            body: "As réguas do método julgam o valor bruto; o líquido de reembolsos existe só para conferir a divisão. Limite e pontos não entram em régua nenhuma.",
          }}
        >
          <span className="cartoes__badge">Conferência</span>
        </InfoPopover>
      </p>
    </>
  );
}

/** Scroller horizontal do seletor de ciclo: 6 ciclos não cabem em 390px; o
    selecionado entra à vista sozinho (o default é o ciclo mais recente). */
function CycleScroller({
  selectedId,
  children,
}: {
  selectedId: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current
      ?.querySelector('[aria-checked="true"]')
      ?.scrollIntoView?.({ inline: "center", block: "nearest" });
  }, [selectedId]);
  return (
    <div className="cartoes__cycles-scroll" ref={ref}>
      {children}
    </div>
  );
}

function SectionHead({ icon, title }: { icon: ReactNode; title: string }) {
  return (
    <div className="cartoes__sec">
      <span className="cartoes__sec-ic">{icon}</span>
      <h2>{title}</h2>
    </div>
  );
}

// -------------------------------------------------------------------- form --

function CardForm({
  initial,
  proposal,
  holders,
  onClose,
}: {
  initial?: Card;
  proposal?: CardProposal;
  holders: Card[];
  onClose: () => void;
}) {
  const [name, setName] = useState(() => initial?.name ?? proposal?.display_name ?? "");
  const [institution, setInstitution] = useState(() => initial?.institution ?? "");
  const [closing, setClosing] = useState(() => initial?.closing_day?.toString() ?? "");
  const [due, setDue] = useState(() => initial?.due_day?.toString() ?? "");
  const [limit, setLimit] = useState(() =>
    initial?.credit_limit_cents
      ? (initial.credit_limit_cents / 100).toString().replace(".", ",")
      : "",
  );
  const [owner, setOwner] = useState(initial?.owner_name ?? "Eu");
  const [aliases, setAliases] = useState(() => initial?.aliases.join(", ") ?? "");
  const [linked, setLinked] = useState(initial?.linked_account_id ?? "");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const additional = linked !== "";

  function save() {
    const cycleError = additional ? null : validateCardCycle(closing, due);
    if (cycleError) {
      setError(cycleError);
      return;
    }

    const limitCents = limit ? parseBRLToCents(limit) : null;
    const input = {
      name,
      institution,
      closingDay: additional ? null : Number(closing),
      dueDay: additional ? null : Number(due),
      creditLimitCents: limitCents,
      aliases: aliases
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
    };
    const promise = proposal
      ? acceptCardProposal({
          proposalId: proposal.id,
          closingDay: input.closingDay,
          dueDay: input.dueDay,
          ownerPersonName: owner,
          linkedAccountId: linked || null,
        })
      : initial
        ? updateCardAccount({ accountId: initial.id, ...input })
        : createCardAccount({
            ...input,
            ownerPersonName: owner,
            linkedAccountId: linked || null,
          });

    setBusy(true);
    promise
      .then(() => {
        invalidateCommands();
        onClose();
      })
      .catch((reason) => setError(safeErrorMessage(reason)))
      .finally(() => setBusy(false));
  }

  return (
    <section className="cartoes__form">
      <h2>{initial ? "Editar cartão" : "Cadastrar cartão"}</h2>
      <div className="cartoes__form-grid">
        <label className="cartoes__label">
          Nome
          <input
            className="cartoes__field"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="cartoes__label">
          Instituição
          <input
            className="cartoes__field"
            value={institution}
            onChange={(event) => setInstitution(event.target.value)}
          />
        </label>
        <label className="cartoes__label">
          Dono
          <input
            className="cartoes__field"
            value={owner}
            onChange={(event) => setOwner(event.target.value)}
          />
        </label>
        <label className="cartoes__label">
          Cartão adicional de
          <select
            className="cartoes__field"
            value={linked}
            onChange={(event) => setLinked(event.target.value)}
          >
            <option value="">Nenhum — titular</option>
            {holders.flatMap((holder) =>
              holder.id === initial?.id ? (
                []
              ) : (
                <option key={holder.id} value={holder.id}>
                  {holder.name}
                </option>
              ),
            )}
          </select>
        </label>
        {additional ? (
          <p className="cartoes__discrete cartoes__inherit">Herda o ciclo do titular.</p>
        ) : (
          <>
            <label className="cartoes__label">
              Fechamento
              <input
                className="cartoes__field"
                type="number"
                min="1"
                max="28"
                value={closing}
                onChange={(event) => setClosing(event.target.value)}
              />
            </label>
            <label className="cartoes__label">
              Vencimento
              <input
                className="cartoes__field"
                type="number"
                min="1"
                max="31"
                value={due}
                onChange={(event) => setDue(event.target.value)}
              />
            </label>
          </>
        )}
        <label className="cartoes__label">
          Limite opcional
          <input
            className="cartoes__field"
            inputMode="decimal"
            value={limit}
            onChange={(event) => setLimit(event.target.value)}
          />
        </label>
        <label className="cartoes__label">
          Aliases
          <input
            className="cartoes__field"
            value={aliases}
            onChange={(event) => setAliases(event.target.value)}
            placeholder="Separe por vírgulas"
          />
        </label>
      </div>
      {error ? (
        <p role="alert" className="cartoes__error">
          {error}
        </p>
      ) : null}
      <div className="cartoes__actions">
        <Button variant="primary" disabled={busy} onClick={save}>
          {busy ? "Salvando…" : "Salvar cartão"}
        </Button>
        <Button variant="ghost" onClick={onClose}>
          Cancelar
        </Button>
      </div>
    </section>
  );
}
