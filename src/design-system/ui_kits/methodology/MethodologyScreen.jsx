/* Neko Finance — Methodology / insights screen. Private, source-neutral rules
   in the editorial (Newsreader) voice + derived insights. window.MethodologyApp. */
const MET_NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Button, Badge, OwnerChip } = MET_NS;
const MetIcon = window.Icon;

const metCSS = `
.met{max-width:980px;margin:0 auto;display:grid;grid-template-columns:1fr 244px;gap:34px;align-items:start;}
.met-main{min-width:0;}
.met-eyebrow{font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--primary);margin-bottom:12px;}
.met-title{font-family:var(--font-serif);font-size:34px;font-weight:500;line-height:1.12;letter-spacing:-0.01em;color:var(--text-strong);margin:0 0 12px;}
.met-lede{font-family:var(--font-serif);font-size:17px;line-height:1.6;color:var(--text-muted);max-width:60ch;}
.met-lede em{color:var(--text);font-style:italic;}
.met-private{display:inline-flex;align-items:center;gap:8px;margin-top:16px;padding:7px 12px;border-radius:var(--radius-pill);background:var(--primary-quiet);border:1px solid rgba(63,191,143,.22);font-size:12px;font-weight:600;color:var(--primary);}
.met-insights{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin:26px 0 30px;}
.met-ins{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;box-shadow:var(--shadow-1);}
.met-ins__v{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:22px;font-weight:600;color:var(--text-strong);}
.met-ins__l{font-size:11.5px;color:var(--text-muted);margin-top:4px;line-height:1.35;}
.met-rule{border-top:1px solid var(--border);padding:22px 0;}
.met-rule:first-of-type{border-top:none;}
.met-rule__num{font-family:var(--font-mono);font-size:11px;color:var(--text-faint);}
.met-rule__h{display:flex;align-items:center;gap:10px;margin:6px 0 9px;}
.met-rule__title{font-size:17px;font-weight:700;color:var(--text-strong);letter-spacing:-0.01em;}
.met-rule__body{font-family:var(--font-serif);font-size:15.5px;line-height:1.62;color:var(--text-muted);max-width:62ch;}
.met-rule__body b{color:var(--text);font-weight:600;}
.met-rule__body em{font-style:italic;color:var(--text);}
.met-eg{margin-top:13px;display:flex;align-items:stretch;gap:0;background:var(--bg-subtle);border:1px solid var(--border);border-radius:var(--radius-sm);overflow:hidden;max-width:560px;}
.met-eg__tag{display:flex;align-items:center;padding:0 11px;background:var(--surface-2);border-right:1px solid var(--border);font-size:9.5px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;color:var(--text-faint);}
.met-eg__body{padding:10px 13px;display:flex;flex-direction:column;gap:5px;flex:1;}
.met-eg__line{display:flex;justify-content:space-between;gap:14px;font-size:12.5px;color:var(--text-muted);align-items:baseline;}
.met-eg__line span:first-child{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0;}
.met-eg__line span:last-child{font-family:var(--font-money);font-variant-numeric:tabular-nums;color:var(--text);flex:none;}
.met-eg__tot{border-top:1px solid var(--border);padding-top:6px;margin-top:1px;font-weight:700;color:var(--text)!important;}
.met-eg__tot span:last-child{color:var(--primary)!important;font-weight:700;}
.met-rule__foot{margin-top:13px;display:flex;align-items:center;gap:10px;}
.met-rail{position:sticky;top:0;display:flex;flex-direction:column;gap:8px;}
.met-rail__h{font-size:10.5px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);padding:0 4px 4px;}
.met-toc{display:flex;flex-direction:column;}
.met-toc__i{display:flex;align-items:center;gap:9px;padding:7px 10px;border-radius:var(--radius-sm);font-size:12.5px;color:var(--text-muted);cursor:pointer;border:none;background:none;text-align:left;width:100%;transition:var(--t-hover);}
.met-toc__i:hover{background:var(--surface-hover);color:var(--text);}
.met-toc__i--on{background:var(--surface-selected);color:var(--text-strong);font-weight:600;}
.met-toc__n{font-family:var(--font-mono);font-size:10.5px;color:var(--text-faint);width:16px;flex:none;}
.met-note{margin-top:14px;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;}
.met-note__t{font-size:12px;font-weight:700;color:var(--text);display:flex;align-items:center;gap:7px;}
.met-note__d{font-size:11.5px;color:var(--text-muted);line-height:1.5;margin-top:6px;}
@media (max-width:920px){ .met{grid-template-columns:1fr;} .met-rail{position:static;} .met-insights{grid-template-columns:1fr 1fr;} }
`;
function injectMet() {
  if (document.getElementById("met-css")) return;
  const s = document.createElement("style");
  s.id = "met-css";
  s.textContent = metCSS;
  document.head.appendChild(s);
}

const RULES = [
  {
    id: "split",
    title: "How shared expenses are split",
    body: (
      <>
        A shared expense is divided by <em>responsibility</em>, not by who paid. The{" "}
        <b>payer</b> is recorded so accounts reconcile; the <b>beneficiary</b> decides
        whose budget it lands in; the <b>responsible owner</b> is who ultimately carries
        it. By default, household charges split <b>50 / 50</b> unless a line sets its
        own ratio.
      </>
    ),
    eg: {
      tag: "Rent",
      lines: [
        { l: "Paid by Alex", v: "2,150.00" },
        { l: "Alex's share (50%)", v: "1,075.00" },
        { l: "Sam owes Alex", v: "1,075.00" },
      ],
      tot: { l: "Household total", v: "$2,150.00" },
    },
  },
  {
    id: "income",
    title: "What counts as income",
    body: (
      <>
        Only <b>realized inflows</b> to an owned account count as income — salary,
        interest, reimbursements received. Internal <em>transfers</em> between your own
        accounts are netted to zero so they never inflate cashflow, and a reimbursement
        is matched back to the expense it offsets rather than counted twice.
      </>
    ),
    eg: {
      tag: "June",
      lines: [
        { l: "Salary", v: "6,200.00" },
        { l: "Transfer in (own)", v: "0.00" },
        { l: "Reimbursement", v: "48.00" },
      ],
      tot: { l: "Counted income", v: "$6,248.00" },
    },
  },
  {
    id: "savings",
    title: "How the savings rate is measured",
    body: (
      <>
        Savings rate is <b>net saved ÷ net income</b> over the period, where net saved
        is income minus all spending including shared responsibility. It is a{" "}
        <em>source-neutral</em> definition — it does not assume any particular budgeting
        framework, only your own categorized rows.
      </>
    ),
    eg: {
      tag: "June",
      lines: [
        { l: "Net income", v: "6,248.00" },
        { l: "Total spending", v: "4,120.00" },
      ],
      tot: { l: "Savings rate", v: "34%" },
    },
  },
  {
    id: "confidence",
    title: "When Mia asks before classifying",
    body: (
      <>
        Every imported row gets a category and owner with a <b>confidence</b> score.
        High-confidence matches apply silently; <b>medium and low</b> ones are flagged
        for your review and never written back to the sheet until you confirm.
        Confidence comes from your <em>own</em> prior decisions, not an external
        dataset.
      </>
    ),
    eg: {
      tag: "Rule",
      lines: [
        { l: "Merchant match", v: "high" },
        { l: "New merchant", v: "low" },
        { l: "Asks before write", v: "yes" },
      ],
      tot: { l: "Rows flagged · June", v: "6" },
    },
  },
];

function MethodologyApp() {
  injectMet();
  const [nav, setNav] = React.useState("methodology");
  const [active, setActive] = React.useState("split");
  const refs = React.useRef({});
  const go = (id) => {
    setActive(id);
    const el = refs.current[id];
    const body = el && el.closest(".ak-body");
    if (el && body) {
      body.scrollTo({ top: el.offsetTop - 16, behavior: "smooth" });
    }
  };

  return (
    <window.AppShell
      active={nav}
      onNav={(k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k))}
      title="Methodology"
      crumb="Private rules · this ledger"
    >
      <div className="met">
        <div className="met-main">
          <div className="met-eyebrow">Methodology</div>
          <h1 className="met-title">The rules behind every number</h1>
          <p className="met-lede">
            Neko explains your money with rules you can read and change. They are{" "}
            <em>private and source-neutral</em> — derived from how you categorize your
            own ledger, never from a public course or a shared model.
          </p>
          <span className="met-private">
            <MetIcon name="lock" size={13} />
            Private to this ledger · editable
          </span>

          <div className="met-insights">
            <div className="met-ins">
              <div className="met-ins__v">$642</div>
              <div className="met-ins__l">
                Shared this month, split by responsibility
              </div>
            </div>
            <div className="met-ins">
              <div className="met-ins__v">34%</div>
              <div className="met-ins__l">Savings rate, by your definition</div>
            </div>
            <div className="met-ins">
              <div className="met-ins__v">6</div>
              <div className="met-ins__l">Rows held for your review</div>
            </div>
          </div>

          {RULES.map((r, i) => (
            <div
              className="met-rule"
              key={r.id}
              ref={(el) => (refs.current[r.id] = el)}
            >
              <div className="met-rule__num">Rule {String(i + 1).padStart(2, "0")}</div>
              <div className="met-rule__h">
                <div className="met-rule__title">{r.title}</div>
              </div>
              <div className="met-rule__body">{r.body}</div>
              <div className="met-eg">
                <div className="met-eg__tag">{r.eg.tag}</div>
                <div className="met-eg__body">
                  {r.eg.lines.map((l, j) => (
                    <div className="met-eg__line" key={j}>
                      <span>{l.l}</span>
                      <span>{l.v}</span>
                    </div>
                  ))}
                  <div className="met-eg__line met-eg__tot">
                    <span>{r.eg.tot.l}</span>
                    <span>{r.eg.tot.v}</span>
                  </div>
                </div>
              </div>
              <div className="met-rule__foot">
                <Button
                  variant="ghost"
                  size="sm"
                  iconLeft={<MetIcon name="pencil" size={14} />}
                >
                  Edit rule
                </Button>
                <Badge tone="neutral">Applied automatically</Badge>
              </div>
            </div>
          ))}
        </div>

        <aside className="met-rail">
          <div className="met-rail__h">On this page</div>
          <div className="met-toc">
            {RULES.map((r, i) => (
              <button
                key={r.id}
                className={"met-toc__i" + (active === r.id ? " met-toc__i--on" : "")}
                onClick={() => go(r.id)}
              >
                <span className="met-toc__n">{String(i + 1).padStart(2, "0")}</span>
                {r.title}
              </button>
            ))}
          </div>
          <div className="met-note">
            <div className="met-note__t">
              <MetIcon name="shield" size={14} style={{ color: "var(--primary)" }} />
              Source-neutral by design
            </div>
            <div className="met-note__d">
              These rules reference only your ledger. Neko never cites a public
              methodology or sends your rules anywhere.
            </div>
          </div>
        </aside>
      </div>
    </window.AppShell>
  );
}
window.MethodologyApp = MethodologyApp;
