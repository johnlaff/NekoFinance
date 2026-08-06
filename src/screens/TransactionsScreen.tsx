import "./lancamentos.css";
import { useEffect, useReducer, useRef, useState } from "react";
import {
  ArrowDownLeft,
  CreditCard,
  Landmark,
  Pencil,
  PiggyBank,
  ReceiptText,
  Search,
  Tags,
  Trash2,
  TriangleAlert,
  Wallet,
} from "lucide-react";
import { Button } from "../design-system/components/Button";
import { Disclosure } from "../design-system/components/Disclosure";
import { EmptyState } from "../design-system/components/EmptyState";
import { Money, SignedMoney } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import {
  deleteSeriesAll,
  deleteSeriesFrom,
  deleteTransaction,
  getDashboardSummary,
  getForecast,
  getMonthGrid,
  getMonthTransactions,
  listTags,
  setTransactionTags,
  type LineItemKind,
  type MonthGridDay,
  type Tag,
  type TransactionRow,
} from "../lib/api";
import { isTauri } from "../lib/env";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { fmtBRL, MES, saldoBand } from "../lib/nkFormat";
import { todayISO } from "../lib/format";
import { useNekoApp } from "../shell/appContext";
import { setCrumb } from "../shell/crumbStore";
import { MarkObligationAction, ObligationsCard } from "./ObligationsPanel";
import {
  applySearch,
  buildDayGroups,
  countRows,
  daymarkLabel,
  daysSummary,
  emptyListCopy,
  LINE_ITEM_KIND_META,
  monthTitle,
  splitAroundToday,
  toMovementType,
  type CellGroup,
  type DayGroup,
  type DisplayRow,
  type FilterKey,
} from "./lancamentosView";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** ISO today, computed once per module load (stable for a day). */
const TODAY = todayISO();

/** Ícone semântico por kind — traço na cor do tipo sobre círculo neutro. */
const KIND_ICON: Record<LineItemKind, typeof Wallet> = {
  entrada: ArrowDownLeft,
  saida: ReceiptText,
  diario: Wallet,
  economia: PiggyBank,
  cartao: CreditCard,
  patrimonio: Landmark,
  ajuste: TriangleAlert,
};

// Ordem canônica dos 5 tipos (a mesma de FORM_KINDS) e rótulos no singular, iguais
// aos nomes de TYPE_META — o mesmo vocabulário em todo seletor de tipo do app.
const FILTER_CHIPS: { key: FilterKey; label: string; hint: string }[] = [
  { key: "todos", label: "Todos", hint: "Tudo o que entrou e saiu" },
  { key: "entrada", label: "Entrada", hint: "Dinheiro que chegou" },
  { key: "saida", label: "Saída", hint: "Conta fixa do mês" },
  { key: "diario", label: "Diário", hint: "O variável do dia" },
  { key: "economia", label: "Economia", hint: "O que você guardou" },
  { key: "cartao", label: "Cartão", hint: "Soma na fatura e vira Saída no vencimento" },
];

// ---------------------------------------------------------------------------
// Pure helpers (screen-local)
// ---------------------------------------------------------------------------

function monthKey(iso: string): string {
  return iso.slice(0, 7);
}

// Stable per-(year,month) fetchers for useCommand (its effect captures the first
// fetcher ref; an inline arrow would fetch with a stale closure — see useCommand).
const _monthGridFetchers = new Map<string, () => Promise<MonthGridDay[]>>();
function monthGridFetcher(year: number, month: number): () => Promise<MonthGridDay[]> {
  const key = `${year}-${month}`;
  const cached = _monthGridFetchers.get(key);
  if (cached) return cached;
  const fn = () => getMonthGrid(year, month);
  _monthGridFetchers.set(key, fn);
  return fn;
}

// O Livro-razão busca por MÊS (a janela recente pura cortaria meses antigos no
// limite e o mês navegado pareceria vazio). Mesmo padrão de fetcher estável.
const _monthTxFetchers = new Map<string, () => Promise<TransactionRow[]>>();
function monthTxFetcher(monthKey: string): () => Promise<TransactionRow[]> {
  const cached = _monthTxFetchers.get(monthKey);
  if (cached) return cached;
  const fn = () => getMonthTransactions(monthKey);
  _monthTxFetchers.set(monthKey, fn);
  return fn;
}

/**
 * Recupera o id da recorrência a partir do id de uma ocorrência ("uuid:index" → "uuid").
 * Sem dois-pontos = lançamento único (null). Evita ida ao backend só para saber se é série.
 */
function recurrenceIdOf(id: string): string | null {
  return id.includes(":") ? id.slice(0, id.lastIndexOf(":")) : null;
}

/** True quando a linha é uma ocorrência de série recorrente (com parcelas COM repetições). */
function isSeriesRow(t: TransactionRow): boolean {
  return (
    t.installment_index != null &&
    t.installment_total != null &&
    recurrenceIdOf(t.id) != null
  );
}

/** Refresca a lista após um delete bem-sucedido; loga falhas sem quebrar a UI. */
function afterDelete(run: Promise<unknown>): void {
  void run
    .then(() => invalidateCommands())
    .catch((err: unknown) => {
      console.error("Falha ao apagar lançamento:", err);
    });
}

/**
 * Apaga uma linha do Livro-razão. Numa ocorrência de série recorrente, oferece o escopo — toda a
 * série / desta ocorrência em diante / só esta — via dois confirms (espelha o escopo do editar-série).
 * Module-scope para o React Compiler vê-la como estável.
 */
function handleDelete(t: TransactionRow): void {
  if (t.provenance === "importado") return; // guard: backend also rejects this
  const recId = recurrenceIdOf(t.id);
  if (isSeriesRow(t) && recId) {
    const all = window.confirm(
      "Apagar TODA a série recorrente?\n\nOK = apagar a série inteira\nCancela = escolher só esta ou as futuras",
    );
    if (all) {
      afterDelete(deleteSeriesAll(recId));
      return;
    }
    const fromHere = window.confirm(
      "OK = apagar esta e todas as futuras da série.\nCancela = apagar somente esta ocorrência.",
    );
    afterDelete(fromHere ? deleteSeriesFrom(t.id) : deleteTransaction(t.id));
    return;
  }
  const confirmed = window.confirm(
    `Apagar "${t.description || "lançamento"}"? Esta ação não pode ser desfeita.`,
  );
  if (!confirmed) return;
  afterDelete(deleteTransaction(t.id));
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Inline tag picker shown within the row's expanded panel. */
function TagPicker({
  transactionId,
  currentTagIds,
  allTags,
  onDone,
}: {
  transactionId: string;
  currentTagIds: string[];
  allTags: Tag[];
  onDone: () => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(() => new Set(currentTagIds));
  const [saving, setSaving] = useState(false);

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function save() {
    setSaving(true);
    setTransactionTags(transactionId, Array.from(selected))
      .then(() => {
        invalidateCommands();
        onDone();
      })
      .catch(() => undefined)
      .finally(() => setSaving(false));
  }

  return (
    <div className="lc-tagpicker">
      <div className="lc-tagpicker__chips">
        {allTags.length === 0 ? (
          <span className="lc-tagpicker__none">Nenhuma tag criada ainda.</span>
        ) : (
          allTags.map((tag) => {
            const on = selected.has(tag.id);
            return (
              <button
                key={tag.id}
                type="button"
                className={"lc-tagchip" + (on ? " is-on" : "")}
                style={
                  on
                    ? {
                        background: `color-mix(in srgb, ${tag.color} 20%, transparent)`,
                        borderColor: tag.color,
                        color: tag.color,
                      }
                    : undefined
                }
                onClick={() => toggle(tag.id)}
                aria-pressed={on}
              >
                {tag.emoji ? `${tag.emoji} ` : ""}
                {tag.name}
              </button>
            );
          })
        )}
      </div>
      <div className="lc-tagpicker__actions">
        <Button
          size="sm"
          variant="primary"
          onClick={() => void save()}
          disabled={saving}
        >
          Aplicar
        </Button>
        <Button size="sm" variant="ghost" onClick={onDone}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

/** Pílulas de metadado junto do nome — nunca na coluna do dinheiro. */
function RowPillsInline({ row }: { row: DisplayRow }) {
  const { pills } = row;
  return (
    <>
      {pills.installment && (
        <span className="lc-pill lc-pill--mono">{pills.installment}</span>
      )}
      {pills.refund && <span className="lc-pill lc-pill--ok">Reembolso</span>}
      {pills.previsto && <span className="lc-pill lc-pill--prev">Previsto</span>}
      {pills.tags.map((tag) => (
        <span
          key={tag.id}
          className="lc-pill lc-pill--tag"
          style={{
            borderColor: `color-mix(in srgb, ${tag.color} 40%, transparent)`,
            color: tag.color,
          }}
        >
          {tag.emoji ? `${tag.emoji} ` : ""}
          {tag.name}
        </span>
      ))}
    </>
  );
}

/** Uma linha do Livro-razão: ícone · nome · contexto · valor (colunas no desktop). */
function RowLine({
  row,
  open,
  onToggle,
}: {
  row: DisplayRow;
  open: boolean;
  onToggle: () => void;
}) {
  const Icon = KIND_ICON[row.kind];
  const kindColor = LINE_ITEM_KIND_META[row.kind].color;
  const positive = row.signedCents > 0;
  return (
    <button
      type="button"
      className={"lc-row" + (row.pills.previsto ? " lc-row--future" : "")}
      onClick={onToggle}
      aria-expanded={open}
      // Sem aria-label: o nome acessível é o conteúdo real da linha (nome,
      // pílulas, contexto e valor) — um rótulo só com o nome esconderia o
      // dinheiro do leitor de tela.
    >
      <span className="lc-row__ic" style={{ color: kindColor }} aria-hidden="true">
        <Icon size={17} strokeWidth={1.75} />
      </span>
      <span className="lc-row__name">
        {row.name}
        <RowPillsInline row={row} />
      </span>
      <span className="lc-row__ctx">{row.context}</span>
      <span
        className="lc-row__val"
        style={{
          color: positive
            ? "var(--money-pos)"
            : row.pills.previsto
              ? "var(--text-faint)"
              : "var(--money-neg)",
        }}
      >
        <SignedMoney cents={row.signedCents} size="inherit" />
      </span>
    </button>
  );
}

/** Painel de ações da linha (as ações são do lançamento dono; o item marca obrigação). */
function RowPanel({
  row,
  onEdit,
  allTags,
}: {
  row: DisplayRow;
  onEdit: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const t = row.txn;
  const isImported = t.provenance === "importado";
  const [showTagPicker, setShowTagPicker] = useState(false);
  return (
    <div className="lc-panel">
      {row.item && (
        <p className="lc-panel__note">
          Item da nota do dia — as ações valem para a célula inteira.
          <MarkObligationAction item={row.item} />
        </p>
      )}
      {isImported && (
        <p className="lc-panel__note lc-panel__note--warn">
          Lançamento importado · edição e exclusão via planilha
        </p>
      )}
      <div className="lc-panel__actions">
        <Button
          size="sm"
          variant="ghost"
          iconLeft={<Pencil size={13} strokeWidth={1.75} />}
          disabled={isImported}
          onClick={(e?: React.MouseEvent) => {
            e?.stopPropagation();
            onEdit(t);
          }}
        >
          Editar
        </Button>
        <Button
          size="sm"
          variant="ghost"
          iconLeft={<Tags size={13} strokeWidth={1.75} />}
          onClick={(e?: React.MouseEvent) => {
            e?.stopPropagation();
            setShowTagPicker((v) => !v);
          }}
        >
          Tags
        </Button>
        <Button
          size="sm"
          variant="ghost"
          iconLeft={<Trash2 size={13} strokeWidth={1.75} />}
          disabled={isImported}
          onClick={(e?: React.MouseEvent) => {
            e?.stopPropagation();
            handleDelete(t);
          }}
        >
          {isSeriesRow(t) ? "Apagar da série" : "Apagar"}
        </Button>
      </div>
      {showTagPicker && (
        <TagPicker
          transactionId={t.id}
          currentTagIds={t.tags.map((tag) => tag.id)}
          allTags={allTags}
          onDone={() => setShowTagPicker(false)}
        />
      )}
    </div>
  );
}

/** Cabeçalho de célula: coluna do método + total como autoridade (+ selo de diferença). */
function CellHead({ cell, searchActive }: { cell: CellGroup; searchActive: boolean }) {
  const showDiff = !searchActive && cell.diffCents !== 0;
  return (
    <h3 className="lc-cellhead">
      <span className="lc-cellhead__label">
        {cell.label}
        {showDiff && <i className="lc-selo">Com diferença</i>}
        {cell.refund && <i className="lc-pill lc-pill--ok">Reembolso</i>}
        {cell.tags.map((tag) => (
          <i
            key={tag.id}
            className="lc-pill lc-pill--tag"
            style={{
              borderColor: `color-mix(in srgb, ${tag.color} 40%, transparent)`,
              color: tag.color,
            }}
          >
            {tag.emoji ? `${tag.emoji} ` : ""}
            {tag.name}
          </i>
        ))}
      </span>
      <b className="lc-cellhead__total">
        <Money cents={cell.totalCents} size="inherit" />
      </b>
    </h3>
  );
}

/** Linha sintética de reconciliação — pertence à célula, nunca conta como item. */
function DiffLine({ diffCents }: { diffCents: number }) {
  const phrase =
    diffCents > 0
      ? "O total da célula é maior que a soma dos itens da nota."
      : "O total da célula é menor que a soma dos itens da nota.";
  return (
    <li className="lc-recdif" aria-disabled="true">
      <span className="lc-recdif__txt">
        <b>Diferença no detalhamento</b> — {phrase}
      </span>
      <span className="lc-recdif__val">
        <Money cents={Math.abs(diffCents)} size="inherit" />
      </span>
    </li>
  );
}

/** Grupo-célula: cabeçalho + linhas + reconciliação. */
function CellBlock({
  cell,
  searchActive,
  openKeys,
  toggle,
  onEdit,
  allTags,
}: {
  cell: CellGroup;
  searchActive: boolean;
  openKeys: ReadonlySet<string>;
  toggle: (key: string) => void;
  onEdit: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const showDiff = !searchActive && cell.diffCents !== 0;
  return (
    <section className="lc-cell" aria-label={cell.label}>
      <CellHead cell={cell} searchActive={searchActive} />
      <ul className="lc-rows">
        {cell.rows.map((row) => (
          <li key={row.key}>
            <RowLine
              row={row}
              open={openKeys.has(row.key)}
              onToggle={() => toggle(row.key)}
            />
            {openKeys.has(row.key) && (
              <RowPanel row={row} onEdit={onEdit} allTags={allTags} />
            )}
          </li>
        ))}
        {showDiff && <DiffLine diffCents={cell.diffCents} />}
      </ul>
    </section>
  );
}

/** Um dia do Livro-razão: daymark + grupos-célula. */
function DayBlock({
  day,
  balance,
  searchActive,
  openKeys,
  toggle,
  onEdit,
  allTags,
}: {
  day: DayGroup;
  /** Saldo encadeado do fim do dia (paridade com a coluna Saldo da planilha). */
  balance: number | null | undefined;
  searchActive: boolean;
  openKeys: ReadonlySet<string>;
  toggle: (key: string) => void;
  onEdit: (t: TransactionRow) => void;
  allTags: Tag[];
}) {
  const band = balance != null ? saldoBand(balance) : null;
  const isToday = day.date === TODAY;
  return (
    <section className="lc-day">
      <div className={"lc-daymark" + (isToday ? " lc-daymark--today" : "")}>
        <h2 className="lc-daymark__t">{daymarkLabel(day.date)}</h2>
        {isToday && <span className="lc-daymark__today">Hoje</span>}
        {band && (
          <span
            className="lc-daymark__saldo"
            style={{ background: band.fill, color: band.text }}
            aria-label={`Saldo do dia ${fmtBRL(balance!)}`}
          >
            {/* O rótulo já anuncia "Saldo do dia R$ X"; o valor visível fica aria-hidden
                para o leitor de tela não falar o número duas vezes. */}
            <span aria-hidden="true">
              <Money cents={balance!} size="inherit" />
            </span>
          </span>
        )}
      </div>
      {day.cells.map((cell) => (
        <CellBlock
          key={cell.key}
          cell={cell}
          searchActive={searchActive}
          openKeys={openKeys}
          toggle={toggle}
          onEdit={onEdit}
          allTags={allTags}
        />
      ))}
    </section>
  );
}

/** Bottom sheet de filtro por tipo (mobile) — dialog nativo, mesma gramática do Compose. */
function FilterSheet({
  open,
  filter,
  onPick,
  onClose,
}: {
  open: boolean;
  filter: FilterKey;
  onPick: (f: FilterKey) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      className="lc-sheet"
      aria-label="Filtrar por tipo"
      onClose={onClose}
      // Light-dismiss nativo (toque no backdrop fecha); Esc já é nativo do dialog.
      {...{ closedby: "any" }}
    >
      <div>
        <h3 className="lc-sheet__t">Filtrar por tipo</h3>
        {/* Sem parágrafo didático: os hints por opção já ensinam cada tipo. */}
        <div className="lc-sheet__opts">
          {FILTER_CHIPS.map((f) => (
            <button
              key={f.key}
              type="button"
              aria-pressed={filter === f.key}
              onClick={() => {
                onPick(f.key);
                onClose();
              }}
            >
              <span>
                {f.label}
                <small>{f.hint}</small>
              </span>
              {filter === f.key && <span aria-hidden="true">✓</span>}
            </button>
          ))}
        </div>
      </div>
    </dialog>
  );
}

// ---------------------------------------------------------------------------
// Main component state
// ---------------------------------------------------------------------------

interface LcState {
  filter: FilterKey;
  openKeys: ReadonlySet<string>;
  mOffset: number;
  search: string;
  sheetOpen: boolean;
}

type LcAction =
  | { type: "SET_FILTER"; filter: FilterKey }
  | { type: "TOGGLE_OPEN"; key: string }
  | { type: "MONTH_DELTA"; delta: number }
  | { type: "MONTH_TODAY" }
  | { type: "SET_SEARCH"; search: string }
  | { type: "SET_SHEET"; open: boolean };

function lcReducer(state: LcState, action: LcAction): LcState {
  switch (action.type) {
    case "SET_FILTER":
      return { ...state, filter: action.filter };
    case "TOGGLE_OPEN": {
      const next = new Set(state.openKeys);
      if (next.has(action.key)) next.delete(action.key);
      else next.add(action.key);
      return { ...state, openKeys: next };
    }
    case "MONTH_DELTA":
      return { ...state, mOffset: state.mOffset + action.delta };
    case "MONTH_TODAY":
      return { ...state, mOffset: 0 };
    case "SET_SEARCH":
      return { ...state, search: action.search };
    case "SET_SHEET":
      return { ...state, sheetOpen: action.open };
    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function TransactionsScreen() {
  const { openCompose } = useNekoApp();

  const { data: allTags } = useCommand("list_tags:lc", listTags);
  // Modo de gasto (copy do vazio) e custo de vida do mês — caches compartilhados do app.
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);

  const [state, dispatch] = useReducer(lcReducer, {
    filter: "todos",
    openKeys: new Set<string>(),
    mOffset: 0,
    search: "",
    sheetOpen: false,
  });
  const { filter, openKeys, mOffset, search, sheetOpen } = state;

  // Mês visto (offset a partir do corrente).
  const now = new Date();
  const targetDate = new Date(now.getFullYear(), now.getMonth() + mOffset, 1);
  const targetYear = targetDate.getFullYear();
  const targetMonth = targetDate.getMonth() + 1;
  const targetKey = `${targetYear}-${String(targetMonth).padStart(2, "0")}`;
  const monthName = (MES[targetMonth - 1] ?? "").toLowerCase();
  const crumbLabel = monthTitle(targetKey);

  // A lista do mês visto — escopada no backend, nunca cortada por janela recente.
  const {
    data: transactions,
    loading,
    error,
  } = useCommand(`get_month_transactions:${targetKey}`, monthTxFetcher(targetKey));

  // O crumb da appbar acompanha o mês visto; ao sair da tela, volta ao padrão.
  // `setCrumb` é função de módulo (identidade fixa) — o efeito só re-dispara
  // quando o rótulo muda de verdade.
  useEffect(() => {
    setCrumb("lancamentos", crumbLabel);
    return () => setCrumb("lancamentos", null);
  }, [crumbLabel]);

  function toggle(key: string) {
    dispatch({ type: "TOGGLE_OPEN", key });
  }

  function handleEdit(t: TransactionRow) {
    openCompose({
      mode: "edit",
      transactionId: t.id,
      type: toMovementType(t),
      date: t.date,
      description: t.description,
      amountCents: Math.abs(t.amount),
      provenance: t.provenance,
    });
  }

  // Pipeline: tipo → grupos célula×nota → busca → em torno de hoje. (A lista já
  // chega mês-escopada; o guard por monthKey só protege contra cache de outro mês.)
  const rows = transactions ?? [];
  const typed =
    filter !== "todos" ? rows.filter((t) => toMovementType(t) === filter) : rows;
  const inMonth = typed.filter((t) => monthKey(t.date) === targetKey);
  const grouped = buildDayGroups(inMonth, TODAY);
  const searchActive = search.trim().length > 0;
  const visible = applySearch(grouped, search);
  const { future, past } = splitAroundToday(visible, TODAY);
  const isCurrentMonth = targetKey === monthKey(TODAY);
  const futureSummary = daysSummary(future);

  // Saldo encadeado por dia (paridade com a coluna Saldo da planilha).
  const gridQ = useCommand(
    `month_grid:${targetKey}`,
    monthGridFetcher(targetYear, targetMonth),
  );
  const monthGrid = gridQ.data ?? [];
  const balanceByDate = new Map<string, number | null>(
    monthGrid.map((d) => [d.date, d.balance_cents]),
  );

  // Custo de vida do mês visto — só quando o motor cobre o mês (nunca fabricado).
  const monthMetric = forecastQ.data?.months.find(
    (m) => m.year === targetYear && m.month === targetMonth,
  );
  const costOfLiving = monthMetric?.cost_of_living_cents ?? null;

  const cardMode = summaryQ.data?.spending_mode === "card";
  const filterName =
    filter !== "todos"
      ? (FILTER_CHIPS.find((f) => f.key === filter)?.label ?? null)
      : null;

  // Web-preview fallback
  if (!isTauri) {
    return (
      <div className="lc">
        <EmptyState
          variant="empty"
          title="Preview web"
          description="Abra o app desktop para ver seus lançamentos."
        />
      </div>
    );
  }

  // Um único campo de busca nos dois viewports: ao lado do contexto no desktop,
  // largura cheia sob ele no mobile — sempre no fluxo, nunca flutuando sobre dado.
  const searchField = (
    <label className="lc-search">
      <Search size={15} strokeWidth={1.75} aria-hidden="true" />
      <input
        type="search"
        placeholder="Buscar lançamento"
        value={search}
        onChange={(e) => dispatch({ type: "SET_SEARCH", search: e.target.value })}
        aria-label="Buscar lançamento"
      />
    </label>
  );

  let content: React.ReactNode;
  if (loading && !transactions) {
    content = <EmptyState variant="skeleton" skeletonRows={7} />;
  } else if (error && !transactions) {
    content = (
      <EmptyState
        variant="error"
        title="Não foi possível carregar os lançamentos"
        description="Confira a conexão e tente de novo."
        action={
          <Button size="sm" variant="ghost" onClick={() => invalidateCommands()}>
            Tentar novamente
          </Button>
        }
      />
    );
  } else {
    const dayBlock = (day: DayGroup) => (
      <DayBlock
        key={day.date}
        day={day}
        balance={balanceByDate.get(day.date)}
        searchActive={searchActive}
        openKeys={openKeys}
        toggle={toggle}
        onEdit={handleEdit}
        allTags={allTags ?? []}
      />
    );
    content = (
      <div className="lc-list">
        {future.length > 0 &&
          (isCurrentMonth ? (
            <Disclosure
              className="lc-future"
              title="O que ainda vem neste mês"
              summary={
                <span className="lc-future__sum">
                  {futureSummary.txnCount}{" "}
                  {futureSummary.txnCount === 1 ? "lançamento" : "lançamentos"} ·{" "}
                  <SignedMoney cents={futureSummary.sumCents} size="inherit" />
                </span>
              }
            >
              {future.map(dayBlock)}
            </Disclosure>
          ) : (
            future.map(dayBlock)
          ))}
        {past.map(dayBlock)}
        {countRows(visible) === 0 && (
          <p className="lc-empty">
            {emptyListCopy({ query: search, filterName, monthName, cardMode })}
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="lc">
      {/* O título da tela vive no shell (sh-top/appbar, com o mês no crumb); aqui
          entram a frase de contexto e a busca — lado a lado no desktop, empilhadas
          com a busca em largura cheia no mobile. */}
      <header className="lc-head">
        {/* Sem alegar ordenação: num mês futuro a lista sobe do mais próximo. */}
        <p className="lc-head__teach">Tudo o que entrou e saiu, dia a dia.</p>
        {searchField}
      </header>

      <div className="lc-filters">
        <MonthNav
          label={crumbLabel}
          onPrev={() => dispatch({ type: "MONTH_DELTA", delta: -1 })}
          onNext={() => dispatch({ type: "MONTH_DELTA", delta: 1 })}
          onToday={() => dispatch({ type: "MONTH_TODAY" })}
          atToday={isCurrentMonth}
        />
        <button
          type="button"
          className="lc-ftrigger"
          onClick={() => dispatch({ type: "SET_SHEET", open: true })}
        >
          Tipo: <b>{FILTER_CHIPS.find((f) => f.key === filter)?.label}</b>{" "}
          <span aria-hidden="true">▾</span>
        </button>
        <div className="lc-chips" role="group" aria-label="Filtrar por tipo">
          {FILTER_CHIPS.map((f) => (
            <button
              key={f.key}
              type="button"
              className={"lc-chip" + (filter === f.key ? " is-on" : "")}
              aria-pressed={filter === f.key}
              onClick={() => dispatch({ type: "SET_FILTER", filter: f.key })}
            >
              {f.label}
            </button>
          ))}
        </div>
        {costOfLiving != null && (
          <span className="lc-filters__tot">
            Custo de vida — <Money cents={costOfLiving} size="inherit" /> no mês
          </span>
        )}
      </div>

      <ObligationsCard />

      {content}

      <FilterSheet
        open={sheetOpen}
        filter={filter}
        onPick={(f) => dispatch({ type: "SET_FILTER", filter: f })}
        onClose={() => dispatch({ type: "SET_SHEET", open: false })}
      />
    </div>
  );
}
