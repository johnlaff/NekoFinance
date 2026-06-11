/* Neko Finance — Transactions / import review. Master table + detail panel +
   Google Sheets column mapping with AI confidence. Exposes window.TransactionsApp. */
const TX_NS = window.NekoFinanceDesignSystem_9bd1cd;
const { TransactionRow, OwnerChip, Badge, SegmentedControl, Button, Input } = TX_NS;
const TxIcon = window.Icon;

const txCSS = `
.tx{display:flex;flex-direction:column;gap:14px;}
.tx-banner{display:flex;align-items:center;gap:13px;padding:13px 16px;background:var(--info-tint);
  border:1px solid rgba(79,166,206,.25);border-radius:var(--radius-md);}
.tx-banner__ic{width:32px;height:32px;border-radius:9px;background:var(--surface);color:var(--info-400);
  display:flex;align-items:center;justify-content:center;flex:none;}
.tx-banner__t{font-size:13.5px;font-weight:600;color:var(--text);}
.tx-banner__s{font-size:12px;color:var(--text-muted);margin-top:1px;}
.tx-banner__s b{color:var(--warning-400);font-weight:600;}
.tx-tools{display:flex;align-items:center;gap:10px;}
.tx-tools__sp{flex:1;}
.tx-grid{display:grid;grid-template-columns:1fr 384px;gap:14px;align-items:start;}
.tx-tablewrap{border:1px solid var(--border);border-radius:var(--radius-md);overflow:hidden;background:var(--surface);}
.tx-thead{display:grid;grid-template-columns:84px minmax(0,1fr) auto auto 132px;gap:14px;padding:9px 14px;
  border-bottom:1px solid var(--border);background:var(--bg-subtle);font-size:10.5px;font-weight:700;
  letter-spacing:.06em;text-transform:uppercase;color:var(--text-faint);}
.tx-thead span:last-child{text-align:right;}
.tx-thead span:nth-child(3),.tx-thead span:nth-child(4){text-align:right;}
/* detail panel */
.tx-detail{border:1px solid var(--border);border-radius:var(--radius-md);background:var(--surface);
  box-shadow:var(--shadow-1);position:sticky;top:0;overflow:hidden;}
.tx-d__head{padding:15px 16px;border-bottom:1px solid var(--border);}
.tx-d__merchant{font-size:16px;font-weight:700;color:var(--text-strong);letter-spacing:-0.01em;}
.tx-d__meta{display:flex;align-items:center;gap:8px;margin-top:6px;}
.tx-d__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:26px;font-weight:600;
  color:var(--text-strong);margin-top:10px;}
.tx-d__src{font-family:var(--font-money);font-size:10.5px;color:var(--text-faint);margin-top:7px;display:flex;align-items:center;gap:6px;}
.tx-d__body{padding:14px 16px;display:flex;flex-direction:column;gap:16px;}
.tx-field__lbl{font-size:11px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;color:var(--text-faint);
  margin-bottom:8px;display:flex;align-items:center;justify-content:space-between;}
.tx-sugg{display:inline-flex;align-items:center;gap:5px;font-size:10.5px;font-weight:600;color:var(--primary);}
.tx-opts{display:flex;flex-wrap:wrap;gap:7px;}
.tx-opt{display:inline-flex;align-items:center;gap:7px;padding:7px 11px;border-radius:var(--radius-sm);
  border:1px solid var(--border);background:var(--surface-elevated);font-size:12.5px;font-weight:600;color:var(--text-muted);
  cursor:pointer;transition:var(--t-hover);}
.tx-opt:hover{border-color:var(--border-strong);color:var(--text);}
.tx-opt--on{border-color:var(--primary);background:var(--primary-quiet);color:var(--text-strong);}
.tx-opt__dot{width:8px;height:8px;border-radius:50%;}
.tx-roles{display:flex;flex-direction:column;gap:8px;}
.tx-role{display:flex;align-items:center;justify-content:space-between;gap:10px;}
.tx-role__k{font-size:12px;color:var(--text-muted);}
.tx-map{border-top:1px solid var(--border);}
.tx-map__row{display:grid;grid-template-columns:1fr auto 1fr auto;gap:10px;align-items:center;padding:8px 16px;
  font-size:12px;border-bottom:1px solid var(--border);}
.tx-map__row:last-child{border-bottom:none;}
.tx-map__col{font-family:var(--font-money);color:var(--text-muted);}
.tx-map__arrow{color:var(--text-faint);}
.tx-map__field{font-weight:600;color:var(--text);}
.tx-d__foot{display:flex;gap:8px;padding:14px 16px;border-top:1px solid var(--border);background:var(--bg-subtle);}
.tx-map__head{padding:12px 16px 6px;font-size:11px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;color:var(--text-faint);}
@media (max-width:1180px){
  .tx-grid{grid-template-columns:1fr;}
  .tx-detail{position:static;}
}
`;
function injectTx() {
  if (document.getElementById("tx-css")) return;
  const s = document.createElement("style");
  s.id = "tx-css";
  s.textContent = txCSS;
  document.head.appendChild(s);
}

const TX_DATA = [
  {
    id: 1,
    date: "08 Jun",
    merchant: "Whole Foods Market",
    cat: "Groceries",
    catC: "var(--chart-2)",
    ownerN: "Household",
    ownerT: "shared",
    amt: "642.18",
    status: "needs-owner",
    conf: "low",
    raw: "WHOLEFDS #1042 SF CA",
  },
  {
    id: 2,
    date: "08 Jun",
    merchant: "Acme Payroll",
    cat: "Income",
    catC: "var(--chart-1)",
    ownerN: "Alex Tan",
    ownerT: "personal",
    amt: "6,200.00",
    pos: true,
    status: "reconciled",
    conf: "high",
    raw: "ACME CORP DIR DEP",
  },
  {
    id: 3,
    date: "07 Jun",
    merchant: "Pacific Gas & Electric",
    cat: "Housing",
    catC: "var(--chart-1)",
    ownerN: "Household",
    ownerT: "shared",
    amt: "148.90",
    status: "reconciled",
    conf: "high",
    raw: "PG&E AUTOPAY",
  },
  {
    id: 4,
    date: "07 Jun",
    merchant: "Blue Bottle Coffee",
    cat: "Dining",
    catC: "var(--chart-5)",
    ownerN: "Sam Okafor",
    ownerT: "partner",
    amt: "9.50",
    status: "needs-owner",
    conf: "medium",
    raw: "SQ *BLUE BOTTLE",
  },
  {
    id: 5,
    date: "06 Jun",
    merchant: "Spotify",
    cat: "Subscriptions",
    catC: "var(--chart-4)",
    ownerN: "Sam Okafor",
    ownerT: "partner",
    amt: "14.99",
    status: "imported",
    conf: "high",
    raw: "SPOTIFY P0A1B2",
  },
  {
    id: 6,
    date: "06 Jun",
    merchant: "Shell",
    cat: "Transport",
    catC: "var(--chart-3)",
    ownerN: "—",
    ownerT: "personal",
    amt: "58.20",
    status: "needs-owner",
    conf: "low",
    raw: "SHELL OIL 5731",
  },
];
const CATS = [
  { n: "Groceries", c: "var(--chart-2)" },
  { n: "Dining", c: "var(--chart-5)" },
  { n: "Housing", c: "var(--chart-1)" },
  { n: "Transport", c: "var(--chart-3)" },
  { n: "Subscriptions", c: "var(--chart-4)" },
];

function TransactionsApp() {
  injectTx();
  const [nav, setNav] = React.useState("transactions");
  const [selId, setSelId] = React.useState(1);
  const [scope, setScope] = React.useState("all");
  const sel = TX_DATA.find((t) => t.id === selId);
  const [cat, setCat] = React.useState(sel.cat);
  const [ownerType, setOwnerType] = React.useState(sel.ownerT);
  React.useEffect(() => {
    setCat(sel.cat);
    setOwnerType(sel.ownerT);
  }, [selId]);
  const rows = TX_DATA.filter((t) => scope === "all" || t.ownerT === scope);

  const right = React.createElement(
    Button,
    {
      variant: "secondary",
      size: "sm",
      iconLeft: React.createElement(TxIcon, { name: "refresh", size: 15 }),
    },
    "Re-sync sheet",
  );

  return (
    <window.AppShell
      active={nav}
      onNav={(k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k))}
      title="Transactions"
      crumb="Import review · Expenses 2025"
      right={right}
    >
      <div className="tx">
        <div className="tx-banner">
          <span className="tx-banner__ic">
            <TxIcon name="table" size={17} />
          </span>
          <div style={{ flex: 1 }}>
            <div className="tx-banner__t">
              Imported 248 rows from “Expenses 2025 → Aug”
            </div>
            <div className="tx-banner__s">
              <b>6 need an owner</b> · 12 low-confidence categories · mapped 8 of 8
              columns
            </div>
          </div>
          <Button variant="ghost" size="sm">
            Review mapping
          </Button>
          <Button
            variant="primary"
            size="sm"
            iconLeft={<TxIcon name="check" size={15} />}
          >
            Confirm all clean
          </Button>
        </div>

        <div className="tx-tools">
          <SegmentedControl
            value={scope}
            onChange={setScope}
            options={[
              { value: "all", label: "All" },
              { value: "personal", label: "Personal", dot: "var(--owner-personal)" },
              { value: "partner", label: "Partner", dot: "var(--owner-partner)" },
              { value: "shared", label: "Shared", dot: "var(--owner-shared)" },
            ]}
          />
          <Badge tone="neutral">{rows.length} shown</Badge>
          <span className="tx-tools__sp" />
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<TxIcon name="filter" size={15} />}
          >
            Confidence
          </Button>
        </div>

        <div className="tx-grid">
          <div className="tx-tablewrap">
            <div className="tx-thead">
              <span>Date</span>
              <span>Merchant</span>
              <span>Owner</span>
              <span>Status</span>
              <span>Amount</span>
            </div>
            {rows.map((t) => (
              <TransactionRow
                key={t.id}
                date={t.date}
                merchant={t.merchant}
                category={t.cat}
                categoryColor={t.catC}
                owner={
                  <OwnerChip
                    name={t.ownerN === "—" ? "Unassigned" : t.ownerN}
                    type={t.ownerT}
                    bare
                  />
                }
                amount={t.amt}
                positive={t.pos}
                status={t.status}
                confidence={t.conf}
                selected={t.id === selId}
                onClick={() => setSelId(t.id)}
              />
            ))}
          </div>

          <div className="tx-detail">
            <div className="tx-d__head">
              <div className="tx-d__merchant">{sel.merchant}</div>
              <div className="tx-d__meta">
                <Badge
                  tone={
                    sel.status === "reconciled"
                      ? "success"
                      : sel.status === "imported"
                        ? "info"
                        : "warning"
                  }
                  dot
                >
                  {sel.status === "needs-owner"
                    ? "Needs owner"
                    : sel.status === "imported"
                      ? "Imported"
                      : "Reconciled"}
                </Badge>
                <span style={{ fontSize: 11.5, color: "var(--text-faint)" }}>
                  {sel.date} · 2025
                </span>
              </div>
              <div
                className="tx-d__amt"
                style={{ color: sel.pos ? "var(--money-pos)" : "var(--text-strong)" }}
              >
                {sel.pos ? "+ " : "− "}${sel.amt}
              </div>
              <div className="tx-d__src">
                <TxIcon name="table" size={12} />
                Sheet ‘Expenses 2025’ · row 1,204 · “{sel.raw}”
              </div>
            </div>
            <div className="tx-d__body">
              <div>
                <div className="tx-field__lbl">
                  <span>Category</span>
                  <span className="tx-sugg">
                    <TxIcon name="sparkles" size={12} />
                    Mia: {sel.cat} ({sel.conf})
                  </span>
                </div>
                <div className="tx-opts">
                  {CATS.map((c) => (
                    <button
                      key={c.n}
                      className={"tx-opt" + (cat === c.n ? " tx-opt--on" : "")}
                      onClick={() => setCat(c.n)}
                    >
                      <span className="tx-opt__dot" style={{ background: c.c }} />
                      {c.n}
                    </button>
                  ))}
                </div>
              </div>
              <div>
                <div className="tx-field__lbl">
                  <span>Ownership</span>
                </div>
                <SegmentedControl
                  value={ownerType}
                  onChange={setOwnerType}
                  options={[
                    {
                      value: "personal",
                      label: "Personal",
                      dot: "var(--owner-personal)",
                    },
                    { value: "partner", label: "Partner", dot: "var(--owner-partner)" },
                    { value: "shared", label: "Shared", dot: "var(--owner-shared)" },
                  ]}
                />
              </div>
              <div>
                <div className="tx-field__lbl">
                  <span>Roles</span>
                </div>
                <div className="tx-roles">
                  <div className="tx-role">
                    <span className="tx-role__k">Payer</span>
                    <OwnerChip name="Sam Okafor" type="partner" bare />
                  </div>
                  <div className="tx-role">
                    <span className="tx-role__k">Beneficiary</span>
                    <OwnerChip
                      name={ownerType === "shared" ? "Household" : "Alex Tan"}
                      type={ownerType === "shared" ? "shared" : "personal"}
                      bare
                    />
                  </div>
                  <div className="tx-role">
                    <span className="tx-role__k">Responsible</span>
                    <OwnerChip
                      name={ownerType === "shared" ? "Household" : "Alex Tan"}
                      type={ownerType}
                      bare
                    />
                  </div>
                </div>
              </div>
            </div>
            <div className="tx-map">
              <div className="tx-map__head">Sheet column mapping</div>
              {[
                { col: "Col B · Date", field: "date", conf: "high" },
                { col: "Col C · Description", field: "merchant", conf: "high" },
                { col: "Col D · Amount", field: "amount", conf: "high" },
                { col: "Col F · Tag", field: "category", conf: "low" },
              ].map((m) => (
                <div className="tx-map__row" key={m.field}>
                  <span className="tx-map__col">{m.col}</span>
                  <span className="tx-map__arrow">→</span>
                  <span className="tx-map__field">{m.field}</span>
                  <Badge tone={m.conf === "high" ? "success" : "warning"}>
                    {m.conf}
                  </Badge>
                </div>
              ))}
            </div>
            <div className="tx-d__foot">
              <Button
                variant="primary"
                size="sm"
                fullWidth
                iconLeft={<TxIcon name="check" size={15} />}
              >
                Confirm & next
              </Button>
              <Button variant="ghost" size="sm">
                Skip
              </Button>
            </div>
          </div>
        </div>
      </div>
    </window.AppShell>
  );
}
window.TransactionsApp = TransactionsApp;
