import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { AlertTriangle, Check, CreditCard, Pencil } from "lucide-react";
import { Badge } from "../design-system/components/Badge";
import { Button } from "../design-system/components/Button";
import { Disclosure } from "../design-system/components/Disclosure";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Money } from "../design-system/components/Money";
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
  type CardPurchase,
  type InvoiceDetail,
  type InvoiceSummary,
} from "../lib/api";
import { safeErrorMessage } from "../lib/errors";
import { centsToBRLInput, parseBRLToCents } from "../lib/format";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import "./cartoes.css";

const FORM_FIELD: CSSProperties = {
  width: "100%",
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  font: "inherit",
};

const LABEL: CSSProperties = {
  display: "block",
  marginBottom: "var(--space-1)",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  color: "var(--text-muted)",
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
    open_invoice: {
      id: "demo-invoice",
      cycle_month: "2026-07",
      closing_date: "2026-06-20",
      due_date: "2026-07-10",
      status: "aberta",
      stated_total_cents: 428_900,
      purchases_sum_cents: 403_900,
      effective_total_cents: 428_900,
      reconciliation_delta_cents: 25_000,
    },
    next_due: {
      id: "demo-invoice",
      cycle_month: "2026-07",
      closing_date: "2026-06-20",
      due_date: "2026-07-10",
      status: "aberta",
      stated_total_cents: 428_900,
      purchases_sum_cents: 403_900,
      effective_total_cents: 428_900,
      reconciliation_delta_cents: 25_000,
    },
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
];

const DEMO_DETAIL: InvoiceDetail = {
  id: "demo-invoice",
  cycle_month: "2026-07",
  closing_date: "2026-06-20",
  due_date: "2026-07-10",
  status: "aberta",
  stated_total_cents: 428_900,
  purchases_sum_cents: 403_900,
  effective_total_cents: 428_900,
  reconciliation_delta_cents: 25_000,
  purchases: [
    {
      txn_id: "purchase-1",
      date: "2026-06-12",
      description: "Mercado",
      amount_cents: 239_900,
      owner_name: "Eu",
      series_id: null,
      installment_label: null,
      is_projection: false,
    },
    {
      txn_id: "purchase-2",
      date: "2026-06-15",
      description: "Assinatura",
      amount_cents: 164_000,
      owner_name: "Eu",
      series_id: "series-1",
      installment_label: null,
      is_projection: false,
    },
  ],
  refunds: [
    {
      txn_id: "refund-1",
      date: "2026-07-10",
      amount_cents: 35_000,
      description: "Parte compartilhada",
      is_projection: true,
    },
  ],
  sub_invoices: [
    {
      account_id: "demo-additional",
      card_name: "Cartão adicional",
      owner_name: "Parceiro(a)",
      effective_total_cents: 86_000,
    },
  ],
  emitter_total_cents: 514_900,
};

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
  card_gate_reserve: "below" as const,
};

type GateState = "alive" | "below" | "unknown";
type CardGateSummary = Pick<
  Awaited<ReturnType<typeof getDashboardSummary>>,
  "card_gate_economy" | "card_gate_reserve"
>;
interface CardSeries {
  id: string;
  occurrence: CardPurchase;
}

function statusTone(
  status: InvoiceSummary["status"],
): "info" | "primary" | "warning" | "success" {
  return status === "paga"
    ? "success"
    : status === "fechada"
      ? "warning"
      : status === "aberta"
        ? "primary"
        : "info";
}

function monthLabel(value: string) {
  return new Intl.DateTimeFormat("pt-BR", { month: "long", year: "numeric" }).format(
    new Date(`${value}-01T12:00:00`),
  );
}

function dateLabel(value: string) {
  return new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "short" }).format(
    new Date(`${value}T12:00:00`),
  );
}

function ownerKind(name: string): "personal" | "partner" | "shared" {
  return name === "Eu"
    ? "personal"
    : name.toLowerCase().includes("compart")
      ? "shared"
      : "partner";
}

function groupSeries(purchases: CardPurchase[]): CardSeries[] {
  const series = new Map<string, CardPurchase>();
  purchases.forEach((purchase) => {
    if (purchase.series_id && !series.has(purchase.series_id)) {
      series.set(purchase.series_id, purchase);
    }
  });
  return Array.from(series, ([id, occurrence]) => ({ id, occurrence }));
}

export function shiftCycleMonth(cycle: string, delta: number): string {
  const match = /^(\d{4})-(\d{2})$/.exec(cycle);
  if (!match) return cycle;

  const year = Number(match[1]);
  const monthIndex = Number(match[2]) - 1 + delta;
  const shiftedYear = year + Math.floor(monthIndex / 12);
  const shiftedMonth = (((monthIndex % 12) + 12) % 12) + 1;
  return `${shiftedYear}-${String(shiftedMonth).padStart(2, "0")}`;
}

export function validateCardCycle(closing: string, due: string): string | null {
  const closingDay = Number(closing);
  const dueDay = Number(due);
  if (!Number.isInteger(closingDay) || closingDay < 1 || closingDay > 28) {
    return "Fechamento deve ser entre 1 e 28.";
  }
  if (!Number.isInteger(dueDay) || dueDay < 1 || dueDay > 31) {
    return "Vencimento deve ser entre 1 e 31.";
  }
  return null;
}

export function CartoesScreen() {
  const cardsQ = useCommand("list_cards", listCards);
  const proposalsQ = useCommand("list_card_proposals", listCardProposals);
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const cards = isTauri ? (cardsQ.data ?? []) : DEMO_CARDS;
  const proposals = isTauri ? (proposalsQ.data ?? []) : DEMO_PROPOSALS;
  const gateSummary = isTauri ? summaryQ.data : DEMO_CARD_GATE;
  const [form, setForm] = useState<{ proposal?: CardProposal; card?: Card } | null>(
    null,
  );
  const holderCards = cards.filter((card) => !card.linked_account_id);
  const additions = cards.reduce((byHolder, card) => {
    if (!card.linked_account_id) return byHolder;
    const linkedCards = byHolder.get(card.linked_account_id) ?? [];
    linkedCards.push(card);
    byHolder.set(card.linked_account_id, linkedCards);
    return byHolder;
  }, new Map<string, Card[]>());

  return (
    <div className="cartoes">
      {proposals.map((proposal) => (
        <ProposalBanner
          key={proposal.id}
          proposal={proposal}
          onCreate={() => setForm({ proposal })}
        />
      ))}
      {cards.length > 0 ? <CardGate summary={gateSummary} /> : null}
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
      {holderCards.map((card) => (
        <CardPanel
          key={card.id}
          card={card}
          additionals={additions.get(card.id) ?? []}
          onEdit={() => setForm({ card })}
        />
      ))}
      {form ? (
        <CardForm
          {...(form.card ? { initial: form.card } : {})}
          {...(form.proposal ? { proposal: form.proposal } : {})}
          holders={holderCards}
          onClose={() => setForm(null)}
        />
      ) : null}
    </div>
  );
}

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
    <section className="card cartoes__proposal">
      <div className="card__body">
        <p className="cartoes__proposal-copy">
          A planilha menciona um cartão que o app não conhece:{" "}
          <strong>{proposal.display_name}</strong>.
        </p>
        <div className="cartoes__actions">
          <Button variant="primary" disabled={busy} onClick={onCreate}>
            Cadastrar cartão
          </Button>
          <Button variant="ghost" disabled={busy} onClick={dismiss}>
            Dispensar
          </Button>
        </div>
      </div>
    </section>
  );
}

function CardGate({ summary }: { summary: CardGateSummary | undefined }) {
  return (
    <section className="cartoes__gate" aria-label="Gate do modo cartão">
      <GateLeg label="Economia viva" state={summary?.card_gate_economy} />
      <GateLeg label="Reserva de 6 meses" state={summary?.card_gate_reserve} />
      <InfoPopover
        term={{
          title: "Modo cartão",
          body: "A economia anual e a reserva de seis meses são as duas pernas observadas. Sem pressa para o próximo objetivo patrimonial. Se quiser conferir seu padrão, vale testar 2–3 meses no débito — o app acompanha do mesmo jeito.",
        }}
      >
        <span className="cartoes__gate-help">Como esta leitura funciona</span>
      </InfoPopover>
    </section>
  );
}

function GateLeg({ label, state }: { label: string; state: GateState | undefined }) {
  if (state === undefined) {
    return <span className="cartoes__gate-loading">Carregando…</span>;
  }
  if (state === "unknown") {
    return (
      <span className="cartoes__gate-leg">
        <NoRecordDash
          label="Sem registro"
          term={{ body: `${label}: faltam dados registrados para esta leitura.` }}
        />
      </span>
    );
  }

  const alive = state === "alive";
  return (
    <span className={`cartoes__gate-leg ${alive ? "is-ok" : "is-warn"}`}>
      {alive ? (
        <Check size={16} aria-hidden="true" />
      ) : (
        <AlertTriangle size={16} aria-hidden="true" />
      )}
      {label}
      {!alive ? " — falta" : ""}
    </span>
  );
}

function CardPanel({
  card,
  additionals,
  onEdit,
}: {
  card: Card;
  additionals: Card[];
  onEdit: () => void;
}) {
  const [invoices, setInvoices] = useState<InvoiceSummary[]>(() =>
    !isTauri && card.open_invoice ? [card.open_invoice] : [],
  );
  const [selected, setSelected] = useState<string | null>(
    card.open_invoice?.id ?? card.next_due?.id ?? null,
  );
  const selectedInvoice = useRef(selected);
  const [detail, setDetail] = useState<InvoiceDetail | null>(() =>
    !isTauri && card.id === "demo-holder" ? DEMO_DETAIL : null,
  );
  const load = useCallback(() => {
    if (!isTauri) return;
    listInvoices(card.id)
      .then((items) => {
        setInvoices(items);
        const id = selectedInvoice.current ?? items[0]?.id ?? null;
        selectedInvoice.current = id;
        setSelected(id);
        if (id) void getInvoice(id).then(setDetail);
      })
      .catch(() => setInvoices([]));
  }, [card.id]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!selected || !isTauri) return;
    getInvoice(selected)
      .then(setDetail)
      .catch(() => setDetail(null));
  }, [selected]);

  const next = card.next_due;
  const selectInvoice = (id: string) => {
    selectedInvoice.current = id;
    setSelected(id);
  };
  return (
    <section className="card cartoes__card">
      <div className="card__body">
        <div className="cartoes__card-head">
          <div>
            <h2>{card.name}</h2>
            <p>
              {card.institution ?? "Instituição não informada"}{" "}
              <OwnerChip who={ownerKind(card.owner_name)} />
            </p>
          </div>
          <div className="cartoes__actions">
            <Button variant="ghost" onClick={load}>
              Ver faturas
            </Button>
            <Button variant="ghost" onClick={onEdit}>
              <Pencil size={15} />
              Editar
            </Button>
          </div>
        </div>
        {next ? (
          <div className="cartoes__invoice-summary">
            <span>Próxima fatura: {dateLabel(next.due_date)}</span>
            <Money
              className="cartoes__invoice-money"
              cents={next.effective_total_cents}
            />
            <Badge tone={statusTone(next.status)}>{next.status}</Badge>
          </div>
        ) : (
          <NoRecordDash
            label="Sem fatura registrada"
            term={{
              body: "A primeira fatura aparece quando uma compra, uma série ou a planilha registra o ciclo.",
            }}
          />
        )}
        {card.open_invoice ? (
          <p className="cartoes__open">
            Fatura aberta acumulando{" "}
            <Money cents={card.open_invoice.effective_total_cents} size="inherit" />
          </p>
        ) : null}
        {card.credit_limit_cents != null ? (
          <p className="cartoes__limit">
            Limite <Money cents={card.credit_limit_cents} size="inherit" />
          </p>
        ) : null}
        {additionals.map((additional) => (
          <div key={additional.id} className="cartoes__additional">
            <OwnerChip who={ownerKind(additional.owner_name)} /> {additional.name}
          </div>
        ))}
        <Disclosure
          title="Faturas"
          summary={
            invoices.length ? `${invoices.length} ciclos` : "Sem ciclos registrados"
          }
          icon={<CreditCard size={16} />}
        >
          <InvoiceDrill
            invoices={invoices}
            selected={selected}
            onSelect={selectInvoice}
            detail={detail}
            onRefresh={load}
          />
        </Disclosure>
      </div>
    </section>
  );
}

function InvoiceDrill({
  invoices,
  selected,
  onSelect,
  detail,
  onRefresh,
}: {
  invoices: InvoiceSummary[];
  selected: string | null;
  onSelect: (id: string) => void;
  detail: InvoiceDetail | null;
  onRefresh: () => void;
}) {
  const [adjusting, setAdjusting] = useState(false);
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [editingSeries, setEditingSeries] = useState<{
    id: string;
    description: string;
    amount: string;
  } | null>(null);
  const current = invoices.find((invoice) => invoice.id === selected);
  const net = detail
    ? detail.effective_total_cents -
      detail.refunds.reduce((sum, refund) => sum + refund.amount_cents, 0)
    : 0;

  function saveStatedTotal() {
    if (!detail || !isTauri) return;
    const cents = parseBRLToCents(amount);
    if (cents == null) return;
    setBusy(true);
    void setInvoiceStatedTotal(detail.id, cents)
      .then(() => {
        setAdjusting(false);
        onRefresh();
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  }

  function movePurchase(txnId: string, delta: number) {
    if (!isTauri || !detail) return;
    void moveCardPurchase(txnId, shiftCycleMonth(detail.cycle_month, delta))
      .then(onRefresh)
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
        onRefresh();
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  }

  function cancelSeries(seriesId: string) {
    if (
      !detail ||
      !isTauri ||
      !window.confirm("Cancelar esta assinatura a partir deste ciclo?")
    )
      return;
    void cancelCardSeries(seriesId, detail.cycle_month)
      .then(onRefresh)
      .catch(() => undefined);
  }

  if (!current || !detail) {
    return (
      <p className="cartoes__muted">
        Selecione um ciclo quando houver faturas registradas.
      </p>
    );
  }

  const series = groupSeries(detail.purchases);
  return (
    <div className="cartoes__drill">
      <SegmentedControl
        ariaLabel="Ciclo da fatura"
        size="sm"
        value={selected ?? ""}
        onChange={onSelect}
        options={invoices.map((invoice) => ({
          value: invoice.id,
          label: `${monthLabel(invoice.cycle_month)} · ${invoice.status}`,
        }))}
      />
      <div className="cartoes__totals">
        <span>
          Total declarado <Money cents={detail.effective_total_cents} size="inherit" />
        </span>
        <span>
          Compras <Money cents={detail.purchases_sum_cents} size="inherit" />
        </span>
        {adjusting ? (
          <span>
            <input
              aria-label="Total declarado"
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
              style={FORM_FIELD}
            />
            <Button variant="primary" disabled={busy} onClick={saveStatedTotal}>
              Confirmar
            </Button>
          </span>
        ) : (
          <Button
            variant="ghost"
            onClick={() => {
              setAmount(
                centsToBRLInput(
                  detail.stated_total_cents ?? detail.effective_total_cents,
                ),
              );
              setAdjusting(true);
            }}
          >
            Ajustar total
          </Button>
        )}
      </div>
      {detail.reconciliation_delta_cents != null ? (
        <div className="cartoes__reconciliation">
          Não itemizado —{" "}
          <Money cents={detail.reconciliation_delta_cents} size="inherit" />
        </div>
      ) : null}
      <div className="cartoes__purchases">
        {detail.purchases.map((purchase) => (
          <div key={purchase.txn_id} className="cartoes__purchase">
            <span>{dateLabel(purchase.date)}</span>
            <span>
              {purchase.description}{" "}
              {purchase.installment_label ? (
                <Badge tone="secondary">{purchase.installment_label}</Badge>
              ) : null}
            </span>
            <OwnerChip who={ownerKind(purchase.owner_name)} />
            <Money
              className="cartoes__purchase-money"
              cents={purchase.amount_cents}
              size="inherit"
            />
            {purchase.series_id ? null : (
              <div className="cartoes__actions">
                <Button
                  variant="ghost"
                  onClick={() => movePurchase(purchase.txn_id, -1)}
                >
                  Mover p/ ciclo anterior
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => movePurchase(purchase.txn_id, 1)}
                >
                  Mover p/ ciclo seguinte
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>
      {series.length ? (
        <section className="cartoes__series">
          <h3>Séries</h3>
          {series.map(({ id, occurrence }) => {
            const isSubscription = occurrence.installment_label === null;
            const isEditing = editingSeries?.id === id;
            return (
              <div key={id} className="cartoes__series-row">
                {isEditing ? (
                  <>
                    <label style={LABEL}>
                      Descrição
                      <input
                        aria-label="Descrição da série"
                        value={editingSeries.description}
                        onChange={(event) =>
                          setEditingSeries({
                            ...editingSeries,
                            description: event.target.value,
                          })
                        }
                        style={FORM_FIELD}
                      />
                    </label>
                    <label style={LABEL}>
                      Valor BRL
                      <input
                        aria-label="Valor da série"
                        inputMode="decimal"
                        value={editingSeries.amount}
                        onChange={(event) =>
                          setEditingSeries({
                            ...editingSeries,
                            amount: event.target.value,
                          })
                        }
                        style={FORM_FIELD}
                      />
                    </label>
                    <p className="cartoes__muted">Regenera as ocorrências futuras.</p>
                    <div className="cartoes__actions">
                      <Button variant="primary" disabled={busy} onClick={saveSeries}>
                        Salvar
                      </Button>
                      <Button variant="ghost" onClick={() => setEditingSeries(null)}>
                        Cancelar
                      </Button>
                    </div>
                  </>
                ) : (
                  <>
                    <span>{occurrence.description}</span>
                    <Money cents={occurrence.amount_cents} size="inherit" />
                    <span className="cartoes__muted">
                      {isSubscription
                        ? "Assinatura"
                        : `Parcelado ${occurrence.installment_label}`}
                    </span>
                    <div className="cartoes__actions">
                      <Button
                        variant="ghost"
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
                      {isSubscription ? (
                        <Button variant="ghost" onClick={() => cancelSeries(id)}>
                          Cancelar a partir deste ciclo
                        </Button>
                      ) : null}
                    </div>
                  </>
                )}
              </div>
            );
          })}
        </section>
      ) : null}
      {detail.refunds.length ? (
        <section>
          <h3>Reembolsos vinculados</h3>
          {detail.refunds.map((refund) => (
            <p key={refund.txn_id}>
              {dateLabel(refund.date)} ·{" "}
              <Money cents={refund.amount_cents} size="inherit" /> ·{" "}
              {refund.description}{" "}
              {refund.is_projection ? <Badge tone="info">Prevista</Badge> : null}
            </p>
          ))}
        </section>
      ) : null}
      {detail.sub_invoices.length ? (
        <section>
          <h3>Sub-faturas</h3>
          {detail.sub_invoices.map((sub) => (
            <p key={sub.account_id}>
              <OwnerChip who={ownerKind(sub.owner_name)} />{" "}
              <Money cents={sub.effective_total_cents} size="inherit" />
            </p>
          ))}
          <p>
            Total do emissor <Money cents={detail.emitter_total_cents} size="inherit" />
          </p>
        </section>
      ) : null}
      <p className="cartoes__net">
        Líquido de reembolsos: <Money cents={net} size="inherit" />
        <EstimateMark
          term={{
            body: "As réguas do método julgam o valor bruto; esta é uma leitura de conferência.",
          }}
        />
      </p>
    </div>
  );
}

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
  const [name, setName] = useState(initial?.name ?? proposal?.display_name ?? "");
  const [institution, setInstitution] = useState(initial?.institution ?? "");
  const [closing, setClosing] = useState(initial?.closing_day?.toString() ?? "");
  const [due, setDue] = useState(initial?.due_day?.toString() ?? "");
  const [limit, setLimit] = useState(
    initial?.credit_limit_cents
      ? (initial.credit_limit_cents / 100).toString().replace(".", ",")
      : "",
  );
  const [owner, setOwner] = useState(initial?.owner_name ?? "Eu");
  const [aliases, setAliases] = useState(initial?.aliases.join(", ") ?? "");
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
    <section className="card cartoes__form">
      <div className="card__body">
        <h2>{initial ? "Editar cartão" : "Cadastrar cartão"}</h2>
        <div className="cartoes__form-grid">
          <label style={LABEL}>
            Nome
            <input
              aria-label="Nome"
              value={name}
              onChange={(event) => setName(event.target.value)}
              style={FORM_FIELD}
            />
          </label>
          <label style={LABEL}>
            Instituição
            <input
              aria-label="Instituição"
              value={institution}
              onChange={(event) => setInstitution(event.target.value)}
              style={FORM_FIELD}
            />
          </label>
          <label style={LABEL}>
            Dono
            <input
              aria-label="Dono"
              value={owner}
              onChange={(event) => setOwner(event.target.value)}
              style={FORM_FIELD}
            />
          </label>
          <label style={LABEL}>
            Cartão adicional de
            <select
              aria-label="Cartão adicional de"
              value={linked}
              onChange={(event) => setLinked(event.target.value)}
              style={FORM_FIELD}
            >
              <option value="">Nenhum — titular</option>
              {holders
                .filter((holder) => holder.id !== initial?.id)
                .map((holder) => (
                  <option key={holder.id} value={holder.id}>
                    {holder.name}
                  </option>
                ))}
            </select>
          </label>
          {additional ? (
            <p className="cartoes__inherit">Herda o ciclo do titular.</p>
          ) : (
            <>
              <label style={LABEL}>
                Fechamento
                <input
                  aria-label="Fechamento"
                  type="number"
                  min="1"
                  max="28"
                  value={closing}
                  onChange={(event) => setClosing(event.target.value)}
                  style={FORM_FIELD}
                />
              </label>
              <label style={LABEL}>
                Vencimento
                <input
                  aria-label="Vencimento"
                  type="number"
                  min="1"
                  max="31"
                  value={due}
                  onChange={(event) => setDue(event.target.value)}
                  style={FORM_FIELD}
                />
              </label>
            </>
          )}
          <label style={LABEL}>
            Limite opcional
            <input
              aria-label="Limite opcional"
              inputMode="decimal"
              value={limit}
              onChange={(event) => setLimit(event.target.value)}
              style={FORM_FIELD}
            />
          </label>
          <label style={LABEL}>
            Aliases
            <input
              aria-label="Aliases"
              value={aliases}
              onChange={(event) => setAliases(event.target.value)}
              placeholder="Separe por vírgulas"
              style={FORM_FIELD}
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
      </div>
    </section>
  );
}
