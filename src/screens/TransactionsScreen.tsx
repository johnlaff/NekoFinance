import "./lancamentos.css";
import { useState, useMemo } from "react";
import { CalendarRange, ChevronRight, Pencil, Plus, Search, Tags } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { getRecentTransactions, isTauri, type TransactionRow } from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import {
  fmtBRL,
  fmtSigned,
  MES,
  monthOf,
  TYPE_META,
  type MovementType,
} from "../lib/nkFormat";
import { todayISO, fmtDayMonth } from "../lib/format";
import { useNekoApp } from "../shell/appContext";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FETCH_LIMIT = 400;

/** ISO today, computed once per module load (stable for a day). */
const TODAY = todayISO();

type FilterKey = "todos" | MovementType;
type ViewMode = "monthOnly" | "anchor";

// ---------------------------------------------------------------------------
// Type mapping: TransactionRow → MovementType (the 5 pillars of the method)
// ---------------------------------------------------------------------------

/**
 * Maps a raw TransactionRow to one of the 5 MovementTypes used by the method.
 * income → entrada
 * transfer → economia
 * expense + is_fixed → saida
 * expense + payment_method === "credit" → cartao
 * expense + everything else → diario
 */
function toMovementType(t: TransactionRow): MovementType {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

function monthKey(iso: string): string {
  return iso.slice(0, 7);
}

function monthLabel(key: string): string {
  const [y, m] = key.split("-");
  const mIdx = parseInt(m ?? "1", 10) - 1;
  return `${MES[mIdx] ?? m} ${y}`;
}

/** Net signed cents for a row (income = positive, everything else = negative). */
function signedCents(t: TransactionRow): number {
  const abs = Math.abs(t.amount);
  return toMovementType(t) === "entrada" ? abs : -abs;
}

/** Group a list of rows into descending month order [["2026-06", [...]], ...]. */
function groupByMonth(rows: TransactionRow[]): [string, TransactionRow[]][] {
  const map = new Map<string, TransactionRow[]>();
  for (const t of rows) {
    const k = monthKey(t.date);
    if (!map.has(k)) map.set(k, []);
    map.get(k)!.push(t);
  }
  return Array.from(map.entries()).toSorted((a, b) => (a[0] < b[0] ? 1 : -1));
}

/** Current month index (0-based), relative to the current year month. */
function currentMonthIndex(): number {
  return new Date().getMonth();
}

// ---------------------------------------------------------------------------
// Filter chip definitions
// ---------------------------------------------------------------------------

const FILTER_CHIPS: { key: FilterKey; label: string; color: string }[] = [
  { key: "todos", label: "Todos", color: "var(--text-faint)" },
  { key: "entrada", label: "Entradas", color: "var(--type-entrada)" },
  { key: "saida", label: "Saídas", color: "var(--type-saida)" },
  { key: "cartao", label: "Cartão", color: "var(--type-cartao)" },
  { key: "diario", label: "Diário", color: "var(--type-diario)" },
  { key: "economia", label: "Economia", color: "var(--type-economia)" },
];

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** A single transaction row with optional expanded parts panel. */
function Row({
  t,
  open,
  onToggle,
  onEdit,
}: {
  t: TransactionRow;
  open: boolean;
  onToggle: () => void;
  onEdit: (t: TransactionRow) => void;
}) {
  const mvType = toMovementType(t);
  const tm = TYPE_META[mvType];
  const totalCents = Math.abs(t.amount);
  const isFuture = t.date > TODAY;
  const isToday = t.date === TODAY;
  const isEntrada = mvType === "entrada";
  const hasItems = t.line_items.length > 1;

  const installmentLabel =
    t.installment_index != null && t.installment_total != null
      ? `${t.installment_index}/${t.installment_total}`
      : null;

  return (
    <>
      <div
        className={"lc-row" + (isFuture ? " lc-row--future" : "")}
        onClick={onToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggle();
          }
        }}
        aria-expanded={open}
      >
        <span className="lc-row__date">{fmtDayMonth(t.date)}</span>
        <span className="lc-row__type" style={{ color: tm.color }}>
          <span className="dot" style={{ background: tm.color }}>
            {tm.glyph}
          </span>
          {tm.name}
        </span>
        <span className="lc-row__desc">
          <ChevronRight
            size={13}
            strokeWidth={1.75}
            className={"lc-chev" + (open ? " is-open" : "")}
          />
          <span className="lc-row__t" title={t.description}>
            {t.description || "—"}
          </span>
          {hasItems && <span className="lc-tag">{`${t.line_items.length} itens`}</span>}
          {installmentLabel && <span className="lc-tag">{installmentLabel}</span>}
          {isFuture && (
            <span
              className="lc-pill"
              style={{ background: "var(--brass-tint)", color: "var(--brass-400)" }}
            >
              Previsto
            </span>
          )}
          {isToday && (
            <span
              className="lc-pill"
              style={{ background: "var(--surface-selected)", color: "var(--primary)" }}
            >
              Hoje
            </span>
          )}
        </span>
        <span
          className="lc-row__amt"
          style={{
            color: isEntrada
              ? "var(--money-pos)"
              : isFuture
                ? "var(--text-faint)"
                : "var(--money-neg)",
          }}
        >
          {isEntrada ? "+" : "−"}
          {fmtBRL(totalCents)}
        </span>
      </div>
      {open && (
        <div className="lc-parts">
          {t.line_items.length > 0 ? (
            <>
              <p className="lc-parts__note">
                <Pencil size={11} strokeWidth={1.75} />
                {`${t.line_items.length} ${t.line_items.length === 1 ? "item" : "itens"}`}
                {hasItems ? " · viram a nota da célula na planilha" : ""}
              </p>
              {t.line_items.map((li, i) => (
                <div className="lc-part" key={li.id ?? i}>
                  <span className="lc-part__desc">{li.description}</span>
                  <span
                    className="lc-part__amt"
                    style={{
                      color: isEntrada ? "var(--money-pos)" : "var(--money-neg)",
                    }}
                  >
                    {isEntrada ? "+" : "−"}
                    {fmtBRL(Math.abs(li.amount_cents))}
                  </span>
                </div>
              ))}
            </>
          ) : (
            <p className="lc-parts__note">
              <Pencil size={11} strokeWidth={1.75} />
              Lançamento simples · sem itens detalhados
            </p>
          )}
          <div className="lc-part-actions">
            <Button
              size="sm"
              variant="ghost"
              iconLeft={<Pencil size={13} strokeWidth={1.75} />}
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
              onClick={(e?: React.MouseEvent) => e?.stopPropagation()}
            >
              Tags
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

/** A sticky day-group header with net sum. */
function GroupHeader({
  title,
  today,
  sum,
}: {
  title: string;
  today: boolean;
  sum: number;
}) {
  return (
    <div className={"lc-gh" + (today ? " lc-gh--today" : "")}>
      <span className="lc-gh__t">{title}</span>
      <span className="lc-gh__sum">{fmtSigned(sum)}</span>
    </div>
  );
}

/** A group of rows under a single GroupHeader. */
function Group({
  title,
  today,
  rows,
  openIds,
  toggle,
  onEdit,
}: {
  title: string;
  today: boolean;
  rows: TransactionRow[];
  openIds: ReadonlySet<string>;
  toggle: (id: string) => void;
  onEdit: (t: TransactionRow) => void;
}) {
  const sum = rows.reduce((s, t) => s + signedCents(t), 0);
  return (
    <>
      <GroupHeader title={title} today={today} sum={sum} />
      {rows.map((t) => (
        <Row
          key={t.id}
          t={t}
          open={openIds.has(t.id)}
          onToggle={() => toggle(t.id)}
          onEdit={onEdit}
        />
      ))}
    </>
  );
}

/** Skeleton placeholder while loading. */
function Skeleton() {
  return (
    <div className="lc-card">
      <div className="lc-skeleton">
        {Array.from({ length: 7 }).map((_, i) => (
          <div
            key={i}
            className="lc-skel-row"
            style={{ width: `${60 + (i % 3) * 15}%` }}
          />
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function TransactionsScreen() {
  const { openCompose } = useNekoApp();

  const {
    data: transactions,
    loading,
    error,
  } = useCommand("get_recent_transactions:lc", () =>
    getRecentTransactions(FETCH_LIMIT),
  );

  const [view, setView] = useState<ViewMode>("anchor");
  const [filter, setFilter] = useState<FilterKey>("todos");
  const [showFuture, setShowFuture] = useState(false);
  const [openIds, setOpenIds] = useState<ReadonlySet<string>>(() => new Set());
  const [mOffset, setMOffset] = useState(0);
  const [search, setSearch] = useState("");

  function toggle(id: string) {
    setOpenIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function handleEdit(t: TransactionRow) {
    openCompose({ mode: "edit", transactionId: t.id });
  }

  function handleNew() {
    invalidateCommands();
    openCompose({ mode: "new" });
  }

  // Derive filtered + searched rows
  const allRows: TransactionRow[] = useMemo(() => {
    const rows = transactions ?? [];
    let filtered = rows;
    if (filter !== "todos") {
      filtered = filtered.filter((t) => toMovementType(t) === filter);
    }
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      filtered = filtered.filter(
        (t) => t.description?.toLowerCase().includes(q) || t.date.includes(q),
      );
    }
    return filtered;
  }, [transactions, filter, search]);

  const futureRows = useMemo(() => allRows.filter((t) => t.date > TODAY), [allRows]);
  const todayRows = useMemo(() => allRows.filter((t) => t.date === TODAY), [allRows]);
  const pastRows = useMemo(() => allRows.filter((t) => t.date < TODAY), [allRows]);

  const futureSum = futureRows.reduce((s, t) => s + signedCents(t), 0);

  // Today label for group header
  const todayParts = TODAY.split("-");
  const todayDay = parseInt(todayParts[2] ?? "1", 10);
  const todayMonthName = MES[monthOf(TODAY)]?.toLowerCase() ?? "";
  const todayLabel = `Hoje · ${todayDay} de ${todayMonthName}`;

  // Month-only mode: which month to show
  const currentMonthIdx = currentMonthIndex();
  const targetMonthIdx = currentMonthIdx + mOffset;
  // Wrap around 0-11 for the month, adjust year for offset
  const targetDate = new Date(new Date().getFullYear(), targetMonthIdx, 1);
  const targetYear = String(targetDate.getFullYear());
  const targetMonth = String(targetDate.getMonth() + 1).padStart(2, "0");
  const targetKey = `${targetYear}-${targetMonth}`;
  const inMonthRows = useMemo(
    () => allRows.filter((t) => monthKey(t.date) === targetKey),
    [allRows, targetKey],
  );

  // Web-preview fallback
  if (!isTauri) {
    return (
      <div className="lc">
        <div className="lc-card">
          <div className="lc-empty">
            Preview web — abra o app desktop para ver seus lançamentos.
          </div>
        </div>
      </div>
    );
  }

  // -------------------------------------------------------------------------
  // Content area
  // -------------------------------------------------------------------------

  let content: React.ReactNode;

  if (loading && !transactions) {
    content = <Skeleton />;
  } else if (error && !transactions) {
    content = (
      <div className="lc-card">
        <div className="lc-empty">
          Não foi possível carregar os lançamentos.{" "}
          <button
            type="button"
            style={{
              color: "var(--primary)",
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: 0,
            }}
            onClick={() => {
              invalidateCommands();
            }}
          >
            Tentar novamente
          </button>
        </div>
      </div>
    );
  } else if (view === "monthOnly") {
    content = (
      <div className="lc-card">
        <Group
          title={monthLabel(targetKey)}
          today={targetKey === TODAY.slice(0, 7)}
          rows={inMonthRows}
          openIds={openIds}
          toggle={toggle}
          onEdit={handleEdit}
        />
        {inMonthRows.length === 0 && (
          <div className="lc-empty">
            Nenhum lançamento neste mês para o filtro atual.
          </div>
        )}
      </div>
    );
  } else {
    // anchor view: future collapsed at top, today highlighted, past below
    content = (
      <div className="lc-card">
        {futureRows.length > 0 && (
          <>
            <div
              className="lc-future"
              onClick={() => setShowFuture((v) => !v)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setShowFuture((v) => !v);
                }
              }}
              aria-expanded={showFuture}
            >
              <span className="lc-future__ic">
                <CalendarRange size={16} strokeWidth={1.75} />
              </span>
              <div>
                <div className="lc-future__t">
                  {futureRows.length} lançamentos futuros já previstos
                </div>
                <div className="lc-future__s">
                  {showFuture
                    ? "Toque para recolher"
                    : "Toque para ver. Não atrapalham aqui."}
                </div>
              </div>
              <span className="lc-future__amt">
                {fmtSigned(futureSum)}
                <br />
                <ChevronRight
                  size={14}
                  strokeWidth={1.75}
                  style={showFuture ? { transform: "rotate(90deg)" } : undefined}
                />
              </span>
            </div>
            {showFuture &&
              groupByMonth(futureRows)
                .slice()
                .reverse()
                .map(([k, rows]) => (
                  <Group
                    key={k}
                    title={"Futuro · " + monthLabel(k)}
                    today={false}
                    rows={rows.slice().sort((a, b) => (a.date < b.date ? -1 : 1))}
                    openIds={openIds}
                    toggle={toggle}
                    onEdit={handleEdit}
                  />
                ))}
          </>
        )}

        {/* Today section */}
        {todayRows.length > 0 ? (
          <Group
            title={todayLabel}
            today
            rows={todayRows}
            openIds={openIds}
            toggle={toggle}
            onEdit={handleEdit}
          />
        ) : (
          <div className="lc-gh lc-gh--today">
            <span className="lc-gh__t">{todayLabel}</span>
            <span className="lc-gh__sum" style={{ color: "var(--text-faint)" }}>
              sem lançamentos
            </span>
          </div>
        )}

        {/* Past months */}
        {groupByMonth(pastRows).map(([k, rows]) => (
          <Group
            key={k}
            title={monthLabel(k)}
            today={false}
            rows={rows}
            openIds={openIds}
            toggle={toggle}
            onEdit={handleEdit}
          />
        ))}

        {allRows.length === 0 && (
          <div className="lc-empty">
            {transactions?.length === 0
              ? "Importe sua planilha em Configurações para começar."
              : "Nenhum resultado para o filtro atual."}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="lc">
      {/* Toolbar */}
      <div className="lc-tools">
        <SegmentedControl
          size="sm"
          ariaLabel="Modo de visualização"
          value={view}
          onChange={(v) => setView(v as ViewMode)}
          options={[
            { value: "anchor", label: "Linha do tempo" },
            { value: "monthOnly", label: "Por mês" },
          ]}
        />
        {view === "monthOnly" && (
          <div style={{ display: "inline-flex", gap: 4 }}>
            <button
              className="sh-iconbtn"
              onClick={() => setMOffset((o) => o - 1)}
              aria-label="Mês anterior"
              type="button"
            >
              <ChevronRight
                size={15}
                strokeWidth={1.75}
                style={{ transform: "rotate(180deg)" }}
              />
            </button>
            <button
              className="sh-iconbtn"
              onClick={() => setMOffset((o) => o + 1)}
              aria-label="Próximo mês"
              type="button"
            >
              <ChevronRight size={15} strokeWidth={1.75} />
            </button>
          </div>
        )}
        <span className="lc-tools__sp" />
        <label className="lc-search">
          <Search size={14} strokeWidth={1.75} />
          <input
            placeholder="Buscar…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            aria-label="Buscar lançamentos"
          />
        </label>
        <Button
          size="sm"
          variant="primary"
          iconLeft={<Plus size={14} strokeWidth={1.75} />}
          onClick={handleNew}
        >
          Novo
        </Button>
      </div>

      {/* Filter chips */}
      <div className="lc-filters">
        {FILTER_CHIPS.map((f) => (
          <button
            key={f.key}
            type="button"
            className={"lc-fchip" + (filter === f.key ? " is-on" : "")}
            onClick={() => setFilter(f.key)}
            style={
              filter === f.key
                ? { background: `color-mix(in srgb,${f.color} 16%, transparent)` }
                : undefined
            }
          >
            <span className="lc-fchip__dot" style={{ background: f.color }} />
            {f.label}
          </button>
        ))}
      </div>

      {content}

      {!isTauri && (
        <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
