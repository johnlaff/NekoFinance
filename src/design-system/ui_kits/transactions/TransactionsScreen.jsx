/* Neko Finance — Livro-razão (Lançamentos). Tabela de histórico com filtros, tipos de
   movimento (MovBadge), proveniência (ProvBadge), tags, titulares (OwnerChip) e painel
   de ações inline. Expõe window.TransactionsScreen. */
const NS = window.NekoFinanceDesignSystem_9bd1cd;
const {
  Badge,
  Button,
  SegmentedControl,
  OwnerChip,
  MovBadge,
  ProvBadge,
  Money,
  EmptyState,
} = NS;
const Icon = window.Icon;

(function injectCSS() {
  if (document.getElementById("txs-css")) return;
  const s = document.createElement("style");
  s.id = "txs-css";
  s.textContent = `
.dash{display:flex;flex-direction:column;gap:14px;}
.dash-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);}
.dash-card__body{padding:8px 16px 16px;}

/* ---- toolbar ---- */
.txs-tools{display:flex;align-items:center;gap:10px;flex-wrap:wrap;}
.txs-tools__sp{flex:1;}

/* ---- ledger table ---- */
.txn-table{width:100%;border-collapse:collapse;font-size:var(--fs-sm);font-family:var(--font-sans);}
.txn-table thead tr{border-bottom:1px solid var(--border);background:var(--bg-subtle);}
.txn-table th{padding:8px 12px;text-align:left;font-size:10.5px;font-weight:700;letter-spacing:.06em;
  text-transform:uppercase;color:var(--text-faint);white-space:nowrap;}
.txn-table th:last-child{width:32px;padding-right:8px;}
.txn-table td{padding:9px 12px;vertical-align:middle;border-bottom:1px solid var(--border);color:var(--text);}
.txn-table tr:last-child td{border-bottom:none;}
.txn-table tr.projection td{opacity:.65;}
.txn-table tr:hover td{background:var(--surface-hover);}
.txn-table td:nth-child(5){text-align:right;font-family:var(--font-money);font-variant-numeric:tabular-nums;
  white-space:nowrap;}
.txn-table td:last-child{text-align:right;padding-right:8px;}

/* month separator */
.txn-month-sep th{padding:10px 12px 6px;font-size:11px;font-weight:700;letter-spacing:.05em;
  text-transform:uppercase;color:var(--text-faint);border-bottom:1px solid var(--border);
  background:var(--bg);}

/* expandable sub-rows */
.txn-tag-editor td{padding:6px 12px 10px;background:var(--bg-subtle);border-bottom:1px solid var(--border);}

/* tag chip */
.txn-chip{display:inline-flex;align-items:center;gap:4px;height:18px;padding:0 7px 0 5px;
  border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid currentColor;
  font-size:var(--fs-micro);font-weight:var(--fw-medium);margin-left:5px;vertical-align:middle;}
.txn-tag-dot{width:6px;height:6px;border-radius:50%;flex:none;}

/* inline action buttons */
.txn-tag-btn{border:none;background:none;cursor:pointer;color:var(--text-faint);padding:2px 4px;
  border-radius:var(--radius-xs);line-height:1;vertical-align:middle;transition:var(--t-hover);}
.txn-tag-btn:hover{color:var(--text);background:var(--surface-hover);}

/* method text */
.txn-method{color:var(--text-muted);font-size:var(--fs-sm);}

/* tag picker */
.txn-tag-picker{display:flex;flex-wrap:wrap;gap:6px;}
.txn-tag-opt{display:inline-flex;align-items:center;gap:5px;padding:4px 9px;border-radius:var(--radius-sm);
  border:var(--bw-hair) solid var(--border);background:var(--surface-elevated);
  font-size:var(--fs-sm);font-weight:var(--fw-medium);color:var(--text-muted);
  cursor:pointer;transition:var(--t-hover);}
.txn-tag-opt.is-on{border-color:var(--primary);background:var(--primary-quiet);color:var(--text-strong);}
.txn-tag-opt:hover:not(.is-on){border-color:var(--border-strong);color:var(--text);}

/* inline error */
.txs-inline-error{margin:0 0 6px;font-size:var(--fs-sm);color:var(--danger-400);}

/* action panel */
.txn-imported-notice{margin:0 0 6px;font-size:var(--fs-micro);color:var(--text-faint);}

/* due date chip */
.txn-due-chip{display:inline-flex;align-items:center;gap:5px;height:20px;margin-left:6px;padding:0 8px;
  border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid var(--border);
  font-size:var(--fs-micro);font-weight:var(--fw-medium);color:var(--text-muted);
  white-space:nowrap;vertical-align:middle;}

/* installment badge */
.txn-inst-badge{display:inline-flex;align-items:center;height:20px;margin-left:6px;padding:0 8px;
  border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid var(--border);
  font-size:var(--fs-micro);font-weight:var(--fw-medium);color:var(--text-muted);
  white-space:nowrap;vertical-align:middle;}

/* line items list */
.txn-items-list{display:flex;flex-direction:column;gap:var(--space-1);margin:0;
  padding-left:var(--space-6);list-style:none;}
.txn-item-row{display:flex;gap:var(--space-3);align-items:baseline;font-size:var(--fs-sm);
  color:var(--text-muted);}

/* generic / italic description */
.txn-desc-faint{color:var(--text-faint);font-style:italic;}

@media (prefers-reduced-motion:reduce){
  .txn-tag-btn,.txn-tag-opt{transition:none;}
}
`;
  document.head.appendChild(s);
})();

/* ---- Demo data ---- */
const DEMO_TRANSACTIONS = [
  /* Junho 2026 */
  {
    id: "t-001",
    date: "2026-06-20",
    type: "expense",
    is_fixed: false,
    payment_method: "credit",
    provenance: "manual",
    description: "iFood — Jantar",
    amount: -4750,
    owners: [],
    tags: [{ id: "tag-1", name: "Alimentação", color: "#e0a33e", emoji: "" }],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-002",
    date: "2026-06-18",
    type: "expense",
    is_fixed: true,
    payment_method: "debit",
    provenance: "importado",
    description: "Aluguel — Junho",
    amount: -180000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-003",
    date: "2026-06-17",
    type: "income",
    is_fixed: false,
    payment_method: null,
    provenance: "importado",
    description: "Salário — Empresa XYZ",
    amount: 620000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-004",
    date: "2026-06-15",
    type: "expense",
    is_fixed: false,
    payment_method: "pix",
    provenance: "manual",
    description: "Mercado Livre — Fone de ouvido",
    amount: -25990,
    owners: [],
    tags: [{ id: "tag-2", name: "Eletrônicos", color: "#5fa8dc", emoji: "" }],
    line_items: [],
    due_date: null,
    installment_index: 2,
    installment_total: 3,
    is_projection: false,
  },
  {
    id: "t-005",
    date: "2026-06-12",
    type: "expense",
    is_fixed: false,
    payment_method: "credit",
    provenance: "importado",
    description: "Supermercado Extra",
    amount: -38400,
    owners: [],
    tags: [],
    line_items: [
      { id: "li-1", description: "Hortifruti", amount_cents: 8900 },
      { id: "li-2", description: "Limpeza", amount_cents: 12300 },
      { id: "li-3", description: "Laticínios", amount_cents: 17200 },
    ],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-006",
    date: "2026-06-10",
    type: "transfer",
    is_fixed: false,
    payment_method: "pix",
    provenance: "manual",
    description: "Poupança — aporte mensal",
    amount: -50000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-007",
    date: "2026-06-05",
    type: "expense",
    is_fixed: true,
    payment_method: "debit",
    provenance: "importado",
    description: "Netflix",
    amount: -5490,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  /* Maio 2026 */
  {
    id: "t-008",
    date: "2026-05-30",
    type: "expense",
    is_fixed: false,
    payment_method: "pix",
    provenance: "importado",
    description: "Dentista",
    amount: -35000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-009",
    date: "2026-05-17",
    type: "income",
    is_fixed: false,
    payment_method: null,
    provenance: "importado",
    description: "Salário — Empresa XYZ",
    amount: 620000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  {
    id: "t-010",
    date: "2026-05-10",
    type: "expense",
    is_fixed: false,
    payment_method: "credit",
    provenance: "importado",
    description: "Posto Ipiranga",
    amount: -9200,
    owners: [],
    tags: [],
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    is_projection: false,
  },
  /* projetados */
  {
    id: "t-011",
    date: "2026-07-01",
    type: "expense",
    is_fixed: true,
    payment_method: "debit",
    provenance: "projetado",
    description: "Aluguel — Julho",
    amount: -180000,
    owners: [],
    tags: [],
    line_items: [],
    due_date: "2026-07-05",
    installment_index: null,
    installment_total: null,
    is_projection: true,
  },
];

const METHOD_LABELS = {
  debit: "Débito",
  credit: "Crédito",
  pix: "PIX",
  cash: "Dinheiro",
};

function methodLabel(t) {
  if (t.payment_method) return METHOD_LABELS[t.payment_method] ?? t.payment_method;
  return t.type === "income" ? "Entrada" : "—";
}

function movKind(t) {
  if (t.type === "income") return "entrada";
  if (t.type === "transfer") return "economia";
  if (t.is_fixed) return "saida";
  if (t.payment_method === "credit") return "cartao";
  return "diario";
}

function fmtDate(iso) {
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}

function monthLabel(ym) {
  const months = [
    "Janeiro",
    "Fevereiro",
    "Março",
    "Abril",
    "Maio",
    "Junho",
    "Julho",
    "Agosto",
    "Setembro",
    "Outubro",
    "Novembro",
    "Dezembro",
  ];
  const [y, m] = ym.split("-");
  return `${months[parseInt(m, 10) - 1]} de ${y}`;
}

function fmtBRL(cents) {
  const abs = Math.abs(cents);
  const reais = Math.floor(abs / 100);
  const centavos = String(abs % 100).padStart(2, "0");
  const formatted = reais.toLocaleString("pt-BR");
  const prefix = cents < 0 ? "− R$ " : "R$ ";
  return `${prefix}${formatted},${centavos}`;
}

/* ---- Tag chip (somente leitura) ---- */
function TagChip({ tag }) {
  return (
    <span className="txn-chip" style={{ borderColor: tag.color, color: "var(--text)" }}>
      <span
        aria-hidden="true"
        className="txn-tag-dot"
        style={{ background: tag.color }}
      />
      {tag.emoji ? `${tag.emoji} ` : ""}
      {tag.name}
    </span>
  );
}

/* ---- Painel de itens itemizados ---- */
function LineItemsPanel({ t }) {
  if (!t.line_items || t.line_items.length === 0) return null;
  const sign = t.type === "income" ? 1 : -1;
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        <ul
          className="txn-items-list"
          aria-label={`Itens de ${t.description || "lançamento"}`}
        >
          {t.line_items.map((li) => (
            <li key={li.id} className="txn-item-row">
              <span
                style={{
                  fontFamily: "var(--font-money)",
                  fontVariantNumeric: "tabular-nums",
                  fontSize: "var(--fs-sm)",
                  color: sign < 0 ? "var(--money-neg)" : "var(--money-pos)",
                  whiteSpace: "nowrap",
                }}
              >
                {fmtBRL(sign * Math.abs(li.amount_cents))}
              </span>
              <span>{li.description}</span>
            </li>
          ))}
        </ul>
      </td>
    </tr>
  );
}

/* ---- Painel de ações (Editar / Apagar) ---- */
function ActionPanel({ t, onClose }) {
  const isImported = t.provenance === "importado";
  const isRecurring = t.id.includes(":");
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        {isImported && (
          <p className="txn-imported-notice">
            Linha importada da planilha — edições ficam no app; um re-import pode
            sobrescrever o valor se a planilha mudou. Apagar aqui não apaga da planilha;
            o próximo import restaura a linha.
          </p>
        )}
        <div
          style={{
            display: "flex",
            gap: "var(--space-3)",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <Button size="sm" variant="ghost" onClick={onClose}>
            Editar
          </Button>
          {isRecurring ? (
            <Button size="sm" variant="ghost" onClick={onClose}>
              Apagar da série
            </Button>
          ) : (
            <Button size="sm" variant="ghost" onClick={onClose}>
              Apagar
            </Button>
          )}
        </div>
      </td>
    </tr>
  );
}

/* ---- Painel de editor de tags ---- */
function TagEditorPanel({ t, onClose }) {
  const allTags = [
    { id: "tag-1", name: "Alimentação", color: "#e0a33e", emoji: "" },
    { id: "tag-2", name: "Eletrônicos", color: "#5fa8dc", emoji: "" },
    { id: "tag-3", name: "Saúde", color: "#4fd39a", emoji: "" },
    { id: "tag-4", name: "Lazer", color: "#c98bd4", emoji: "" },
    { id: "tag-5", name: "Moradia", color: "#e0625b", emoji: "" },
  ];
  const activeIds = new Set(t.tags.map((x) => x.id));
  return (
    <tr className="txn-tag-editor">
      <td colSpan={6}>
        <span className="txn-tag-picker">
          {allTags.map((tag) => {
            const on = activeIds.has(tag.id);
            return (
              <button
                key={tag.id}
                type="button"
                aria-pressed={on}
                className={`txn-tag-opt${on ? " is-on" : ""}`}
              >
                <span
                  aria-hidden="true"
                  className="txn-tag-dot"
                  style={{ background: tag.color }}
                />
                {tag.name}
              </button>
            );
          })}
        </span>
      </td>
    </tr>
  );
}

/* ---- Linha do ledger ---- */
function LedgerRow({
  t,
  itemsOpen,
  actionOpen,
  tagOpen,
  onToggleItems,
  onToggleAction,
  onToggleTag,
}) {
  const hasItems = t.line_items && t.line_items.length > 0;
  const isGeneric =
    t.description && /^(Entrada|Saída|Diário) \d{4}-\d{2}-\d{2}$/.test(t.description);
  return (
    <tr className={t.is_projection ? "projection" : ""}>
      {/* Data */}
      <td style={{ whiteSpace: "nowrap", color: "var(--text-muted)" }}>
        {fmtDate(t.date)}
      </td>
      {/* Tipo */}
      <td>
        <MovBadge kind={movKind(t)} showLabel size={16} />
      </td>
      {/* Descrição */}
      <td>
        {hasItems && (
          <button
            type="button"
            className="txn-tag-btn"
            aria-label={`${itemsOpen ? "Fechar" : "Ver"} itens de ${t.description || "lançamento"}`}
            aria-expanded={itemsOpen}
            onClick={onToggleItems}
          >
            {itemsOpen ? (
              <Icon name="chevronDown" size={13} />
            ) : (
              <Icon name="chevronRight" size={13} />
            )}
          </button>
        )}{" "}
        {t.description ? (
          <span
            className={isGeneric ? "txn-desc-faint" : ""}
            title={
              isGeneric ? "Sem nota na célula — reimporte via Google Sheets" : undefined
            }
          >
            {t.description}
          </span>
        ) : (
          "—"
        )}{" "}
        <ProvBadge provenance={t.provenance} />
        {t.due_date && (
          <span
            className="txn-due-chip"
            aria-label={`Vencimento: ${fmtDate(t.due_date)}`}
          >
            <Icon name="calendar" size={11} />
            {fmtDate(t.due_date)}
          </span>
        )}
        {t.installment_index != null && t.installment_total != null && (
          <span
            className="txn-inst-badge"
            aria-label={`Parcela ${t.installment_index} de ${t.installment_total}`}
          >
            {t.installment_index}/{t.installment_total} parcelas
          </span>
        )}
        {t.owners && t.owners.length >= 2 && (
          <span
            style={{
              display: "inline-flex",
              gap: 4,
              marginLeft: 6,
              verticalAlign: "middle",
            }}
          >
            {t.owners.map((name) => (
              <OwnerChip key={name} name={name} />
            ))}
          </span>
        )}
        {t.tags && t.tags.map((tag) => <TagChip key={tag.id} tag={tag} />)}
        <button
          type="button"
          className="txn-tag-btn"
          aria-label={`Editar tags de ${t.description || "lançamento"}`}
          aria-expanded={tagOpen}
          onClick={onToggleTag}
          style={{ marginLeft: 4 }}
        >
          <Icon name="tags" size={13} />
        </button>
      </td>
      {/* Método */}
      <td>
        <span className="txn-method">{methodLabel(t)}</span>
      </td>
      {/* Valor */}
      <td style={{ textAlign: "right" }}>
        <span
          style={{
            fontFamily: "var(--font-money)",
            fontVariantNumeric: "tabular-nums",
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-semibold)",
            color:
              t.type === "income"
                ? "var(--money-pos)"
                : t.is_projection
                  ? "var(--text-faint)"
                  : "var(--money-neg)",
            whiteSpace: "nowrap",
          }}
        >
          {fmtBRL(t.type === "income" ? Math.abs(t.amount) : -Math.abs(t.amount))}
        </span>
      </td>
      {/* Ações */}
      <td style={{ width: 32, textAlign: "right", paddingRight: 8 }}>
        <button
          type="button"
          className="txn-tag-btn"
          aria-label={`Ações para ${t.description || "lançamento"}`}
          aria-expanded={actionOpen}
          onClick={onToggleAction}
        >
          <Icon name="more" size={13} />
        </button>
      </td>
    </tr>
  );
}

/* ---- Tabela do Livro-razão ---- */
function LedgerTable({ rows }) {
  const [itemsId, setItemsId] = React.useState(null);
  const [actionId, setActionId] = React.useState(null);
  const [tagId, setTagId] = React.useState(null);

  function toggleItems(id) {
    setItemsId((prev) => (prev === id ? null : id));
  }
  function toggleAction(id) {
    setActionId((prev) => (prev === id ? null : id));
  }
  function toggleTag(id) {
    setTagId((prev) => (prev === id ? null : id));
  }

  return (
    <table className="txn-table">
      <thead>
        <tr>
          <th scope="col">Data</th>
          <th scope="col">Tipo</th>
          <th scope="col">Descrição</th>
          <th scope="col">Método</th>
          <th scope="col">Valor</th>
          <th scope="col" aria-label="Ações" />
        </tr>
      </thead>
      <tbody>
        {rows.map((t, i) => {
          const ym = t.date.slice(0, 7);
          const showMonth = i === 0 || rows[i - 1].date.slice(0, 7) !== ym;
          return (
            <React.Fragment key={t.id}>
              {showMonth && (
                <tr className="txn-month-sep">
                  <th scope="colgroup" colSpan={6}>
                    {monthLabel(ym)}
                  </th>
                </tr>
              )}
              <LedgerRow
                t={t}
                itemsOpen={itemsId === t.id}
                actionOpen={actionId === t.id}
                tagOpen={tagId === t.id}
                onToggleItems={() => toggleItems(t.id)}
                onToggleAction={() => toggleAction(t.id)}
                onToggleTag={() => toggleTag(t.id)}
              />
              {itemsId === t.id && <LineItemsPanel t={t} />}
              {actionId === t.id && (
                <ActionPanel t={t} onClose={() => setActionId(null)} />
              )}
              {tagId === t.id && (
                <TagEditorPanel t={t} onClose={() => setTagId(null)} />
              )}
            </React.Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

/* ---- Formulário inline de novo lançamento (stub visual) ---- */
function NewLancamentoForm({ onClose }) {
  return (
    <div
      style={{
        marginBottom: "var(--space-4)",
        background: "var(--surface)",
        border: "1px solid var(--border)",
        borderRadius: "var(--radius-md)",
        padding: "var(--space-6)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-4)",
      }}
    >
      <div
        style={{
          fontWeight: "var(--fw-semibold)",
          fontSize: "var(--fs-sm)",
          color: "var(--text-strong)",
        }}
      >
        Novo lançamento
      </div>
      <div style={{ display: "flex", gap: "var(--space-4)", flexWrap: "wrap" }}>
        {[
          { label: "Tipo", placeholder: "Diário" },
          { label: "Valor (R$)", placeholder: "0,00" },
          { label: "Data", placeholder: "21/06/2026" },
          { label: "Descrição", placeholder: "Ex.: Farmácia" },
        ].map(({ label, placeholder }) => (
          <label
            key={label}
            style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 120 }}
          >
            <span
              style={{
                fontSize: "var(--fs-micro)",
                fontWeight: "var(--fw-bold)",
                textTransform: "uppercase",
                letterSpacing: ".06em",
                color: "var(--text-faint)",
              }}
            >
              {label}
            </span>
            <input
              placeholder={placeholder}
              style={{
                height: "var(--hit-min)",
                padding: "0 10px",
                background: "var(--surface-2)",
                border: "var(--bw-hair) solid var(--border)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text)",
                fontFamily: "var(--font-sans)",
                fontSize: "var(--fs-sm)",
                outline: "none",
              }}
            />
          </label>
        ))}
      </div>
      <div style={{ display: "flex", gap: "var(--space-3)" }}>
        <Button size="sm" variant="primary">
          Salvar
        </Button>
        <Button size="sm" variant="ghost" onClick={onClose}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

/* ---- Tela principal ---- */
function TransactionsScreen() {
  const [scope, setScope] = React.useState("all");
  const [showForm, setShowForm] = React.useState(false);

  const filtered = React.useMemo(() => {
    const txns = [...DEMO_TRANSACTIONS].sort((a, b) => b.date.localeCompare(a.date));
    if (scope === "credit") return txns.filter((t) => t.payment_method === "credit");
    if (scope === "future") return txns.filter((t) => t.is_projection);
    return txns;
  }, [scope]);

  return (
    <div className="dash">
      {/* Toolbar */}
      <div className="txs-tools">
        <SegmentedControl
          size="sm"
          ariaLabel="Filtrar lançamentos por escopo"
          value={scope}
          onChange={setScope}
          options={[
            { value: "all", label: "Todas" },
            { value: "credit", label: "Crédito" },
            { value: "future", label: "Futuro" },
          ]}
        />
        <span className="txs-tools__sp" />
        <Badge tone="secondary">
          {filtered.length} {filtered.length === 1 ? "exibida" : "exibidas"}
        </Badge>
        <Button
          size="sm"
          variant={showForm ? "ghost" : "primary"}
          iconLeft={<Icon name="plus" size={15} />}
          onClick={() => setShowForm((v) => !v)}
        >
          {showForm ? "Fechar" : "Novo lançamento"}
        </Button>
      </div>

      {/* Formulário inline */}
      {showForm && <NewLancamentoForm onClose={() => setShowForm(false)} />}

      {/* Tabela */}
      <div className="dash-card">
        <div className="dash-card__body" style={{ padding: 0 }}>
          {filtered.length === 0 ? (
            <EmptyState
              variant="empty"
              title="Nenhum lançamento encontrado"
              description="Nenhum resultado para o filtro atual."
            />
          ) : (
            <LedgerTable rows={filtered} />
          )}
        </div>
      </div>
    </div>
  );
}

window.TransactionsScreen = TransactionsScreen;
