/* Neko Finance — Dashboard screen. Composes DS finance components +
   hand-built SVG charts using chart tokens. Exposes window.DashboardScreen. */
const DASH_NS = window.NekoFinanceDesignSystem_9bd1cd;
const {
  MetricTile,
  HealthBadge,
  OwnerChip,
  TransactionRow,
  SegmentedControl,
  Badge,
  Button,
} = DASH_NS;
const DashIcon = window.Icon;

const dashCSS = `
.dash{display:flex;flex-direction:column;gap:18px;max-width:1180px;}
.dash-hero{display:flex;align-items:center;gap:18px;padding:18px 20px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-1);}
.dash-hero__txt{flex:1;min-width:0;}
.dash-hero__line{font-size:15px;line-height:1.5;color:var(--text-muted);}
.dash-hero__line b{color:var(--text-strong);font-weight:700;}
.dash-hero__money{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;color:var(--text);}
.dash-grid4{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;}
.dash-2col{display:grid;grid-template-columns:1.6fr 1fr;gap:14px;}
.dash-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);}
.dash-card__head{display:flex;align-items:center;justify-content:space-between;gap:10px;padding:14px 16px 8px;}
.dash-card__title{font-size:14px;font-weight:700;color:var(--text-strong);display:flex;align-items:center;gap:8px;}
.dash-card__ic{color:var(--text-faint);}
.dash-card__body{padding:8px 16px 16px;}
.dash-legend{display:flex;flex-direction:column;gap:9px;}
.dash-leg{display:flex;align-items:center;gap:9px;font-size:12.5px;}
.dash-leg__dot{width:9px;height:9px;border-radius:3px;flex:none;}
.dash-leg__name{color:var(--text-muted);flex:1;}
.dash-leg__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;color:var(--text);}
.dash-leg__pct{color:var(--text-faint);font-size:11px;width:34px;text-align:right;}
.dash-acct{display:flex;align-items:center;gap:12px;padding:11px 0;border-bottom:1px solid var(--border);}
.dash-acct:last-child{border-bottom:none;}
.dash-acct__ic{width:34px;height:34px;border-radius:9px;background:var(--surface-elevated);border:1px solid var(--border);
  display:flex;align-items:center;justify-content:center;color:var(--text-muted);flex:none;}
.dash-acct__nm{font-size:13px;font-weight:600;color:var(--text);}
.dash-acct__sub{font-size:11px;color:var(--text-faint);}
.dash-acct__amt{margin-left:auto;font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;font-size:14px;}
.dash-split{display:flex;flex-direction:column;gap:13px;}
.dash-splitrow__top{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px;}
.dash-splitrow__lbl{display:flex;align-items:center;gap:8px;font-size:12.5px;font-weight:600;color:var(--text);}
.dash-splitrow__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:12.5px;color:var(--text-muted);}
.dash-bar{height:8px;border-radius:999px;background:var(--surface-2);overflow:hidden;}
.dash-bar__fill{height:100%;border-radius:999px;}
.dash-sectit{font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);}
@media (max-width:1080px){
  .dash-grid4{grid-template-columns:repeat(2,1fr);}
  .dash-2col{grid-template-columns:1fr;}
}`;

function injectDash() {
  if (document.getElementById("dash-css")) return;
  const s = document.createElement("style");
  s.id = "dash-css";
  s.textContent = dashCSS;
  document.head.appendChild(s);
}

/* ---- Cashflow area + bars chart ---- */
function CashflowChart() {
  const W = 660,
    H = 180,
    pad = { l: 8, r: 8, t: 10, b: 22 };
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
  const income = [6200, 6200, 6450, 6200, 6800, 6200];
  const spend = [4100, 5200, 3900, 4600, 4200, 3142];
  const max = 7200;
  const iw = W - pad.l - pad.r;
  const ih = H - pad.t - pad.b;
  const x = (i) => pad.l + (iw / (months.length - 1)) * i;
  const y = (v) => pad.t + ih * (1 - v / max);
  const linePath = (arr) =>
    arr
      .map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)} ${y(v).toFixed(1)}`)
      .join(" ");
  const areaPath =
    linePath(income) +
    ` L${x(months.length - 1)} ${pad.t + ih} L${x(0)} ${pad.t + ih} Z`;
  const grid = [0, 0.25, 0.5, 0.75, 1];
  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      style={{ width: "100%", height: "auto", display: "block" }}
    >
      {grid.map((g, i) => (
        <line
          key={i}
          x1={pad.l}
          x2={W - pad.r}
          y1={pad.t + ih * g}
          y2={pad.t + ih * g}
          stroke="var(--chart-grid)"
          strokeWidth="1"
        />
      ))}
      <defs>
        <linearGradient id="cf" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--chart-1)" stopOpacity="0.22" />
          <stop offset="100%" stopColor="var(--chart-1)" stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={areaPath} fill="url(#cf)" />
      {spend.map((v, i) => (
        <rect
          key={i}
          x={x(i) - 7}
          y={y(v)}
          width="14"
          height={pad.t + ih - y(v)}
          rx="3"
          fill="var(--chart-2)"
          opacity="0.55"
        />
      ))}
      <path
        d={linePath(income)}
        fill="none"
        stroke="var(--chart-1)"
        strokeWidth="2.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      {income.map((v, i) => (
        <circle
          key={i}
          cx={x(i)}
          cy={y(v)}
          r="3"
          fill="var(--bg)"
          stroke="var(--chart-1)"
          strokeWidth="2"
        />
      ))}
      {months.map((m, i) => (
        <text
          key={i}
          x={x(i)}
          y={H - 6}
          textAnchor="middle"
          fontSize="10.5"
          fill="var(--chart-axis)"
          fontFamily="var(--font-mono)"
        >
          {m}
        </text>
      ))}
    </svg>
  );
}

/* ---- Category donut ---- */
function Donut({ data }) {
  const total = data.reduce((s, d) => s + d.value, 0);
  const R = 52,
    sw = 16,
    C = 2 * Math.PI * R;
  // Offsets cumulativos puramente funcionais (prefix-sum), sem reatribuir variável durante o
  // render — o React Compiler rejeita reatribuição capturada em closure.
  const offsets = data.map(
    (_, i) => -C * (data.slice(0, i).reduce((s, x) => s + x.value, 0) / total),
  );
  return (
    <svg viewBox="0 0 140 140" style={{ width: 140, height: 140 }}>
      <g transform="rotate(-90 70 70)">
        {data.map((d, i) => {
          const frac = d.value / total;
          const dash = `${(C * frac).toFixed(1)} ${(C * (1 - frac)).toFixed(1)}`;
          const off = offsets[i];
          return (
            <circle
              key={i}
              cx="70"
              cy="70"
              r={R}
              fill="none"
              stroke={d.color}
              strokeWidth={sw}
              strokeDasharray={dash}
              strokeDashoffset={off}
            />
          );
        })}
      </g>
      <text
        x="70"
        y="65"
        textAnchor="middle"
        fontSize="11"
        fill="var(--text-faint)"
        fontFamily="var(--font-sans)"
      >
        Spending
      </text>
      <text
        x="70"
        y="84"
        textAnchor="middle"
        fontSize="17"
        fontWeight="700"
        fill="var(--text-strong)"
        fontFamily="var(--font-money)"
      >
        $3,142
      </text>
    </svg>
  );
}

function DashboardScreen({ onAskMia = () => {} }) {
  injectDash();
  const cats = [
    { name: "Housing", value: 1450, color: "var(--chart-1)" },
    { name: "Groceries", value: 642, color: "var(--chart-2)" },
    { name: "Transport", value: 380, color: "var(--chart-3)" },
    { name: "Subscriptions", value: 270, color: "var(--chart-4)" },
    { name: "Dining", value: 400, color: "var(--chart-5)" },
  ];
  const total = cats.reduce((s, c) => s + c.value, 0);
  return (
    <div className="dash">
      <div className="dash-hero">
        <HealthBadge level="strong" sublabel="3.1 months runway" size="lg" />
        <div className="dash-hero__txt">
          <div className="dash-hero__line">
            You're <b>$1,678</b> ahead this month. Spending is <b>6% under</b> your
            average, with <span className="dash-hero__money">$642</span> in shared
            groceries awaiting an owner.
          </div>
        </div>
        <Button
          variant="secondary"
          iconLeft={<DashIcon name="sparkles" size={16} />}
          onClick={onAskMia}
        >
          Ask Mia
        </Button>
      </div>

      <div className="dash-grid4">
        <MetricTile
          label="Net worth"
          value="$182,400"
          icon={<DashIcon name="trendingUp" size={15} />}
          delta="+1.8%"
          deltaDir="up"
          sublabel="this quarter"
        />
        <MetricTile
          label="Net cashflow"
          value="$4,820.00"
          delta="+12.4%"
          deltaDir="up"
          sublabel="vs. last month"
          spark={[40, 55, 48, 70, 62, 88, 100]}
        />
        <MetricTile
          label="Spending"
          value="$3,142.18"
          delta="6.1%"
          deltaDir="down"
          sublabel="under budget"
        />
        <MetricTile
          label="Savings rate"
          value="34%"
          delta="+3 pts"
          deltaDir="up"
          sublabel="of net income"
        />
      </div>

      <div className="dash-2col">
        <div className="dash-card">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <DashIcon name="trendingUp" size={16} className="dash-card__ic" />
              Cashflow
            </span>
            <SegmentedControl
              size="sm"
              value="6m"
              onChange={() => {}}
              options={[
                { value: "3m", label: "3M" },
                { value: "6m", label: "6M" },
                { value: "1y", label: "1Y" },
              ]}
            />
          </div>
          <div className="dash-card__body">
            <CashflowChart />
            <div
              style={{
                display: "flex",
                gap: 18,
                marginTop: 6,
                fontSize: 11.5,
                color: "var(--text-muted)",
              }}
            >
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span
                  style={{
                    width: 14,
                    height: 3,
                    borderRadius: 2,
                    background: "var(--chart-1)",
                  }}
                />
                Income
              </span>
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span
                  style={{
                    width: 10,
                    height: 10,
                    borderRadius: 2,
                    background: "var(--chart-2)",
                    opacity: 0.55,
                  }}
                />
                Spending
              </span>
            </div>
          </div>
        </div>
        <div className="dash-card">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <DashIcon name="piggy" size={16} className="dash-card__ic" />
              By category
            </span>
          </div>
          <div
            className="dash-card__body"
            style={{ display: "flex", gap: 14, alignItems: "center" }}
          >
            <Donut data={cats} />
            <div className="dash-legend" style={{ flex: 1 }}>
              {cats.map((c) => (
                <div className="dash-leg" key={c.name}>
                  <span className="dash-leg__dot" style={{ background: c.color }} />
                  <span className="dash-leg__name">{c.name}</span>
                  <span className="dash-leg__amt">${c.value.toLocaleString()}</span>
                  <span className="dash-leg__pct">
                    {Math.round((c.value / total) * 100)}%
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="dash-2col">
        <div className="dash-card">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <DashIcon name="wallet" size={16} className="dash-card__ic" />
              Accounts & cards
            </span>
            <Button
              variant="ghost"
              size="sm"
              iconLeft={<DashIcon name="plus" size={15} />}
            >
              Add
            </Button>
          </div>
          <div className="dash-card__body" style={{ paddingTop: 0 }}>
            {[
              {
                nm: "Joint Checking",
                sub: "Chase ·· 4821",
                amt: "$12,408.52",
                ic: "wallet",
                owner: "shared",
              },
              {
                nm: "Alex — Savings",
                sub: "Ally ·· 9920",
                amt: "$96,140.00",
                ic: "piggy",
                owner: "personal",
              },
              {
                nm: "Sam — Amex Gold",
                sub: "Credit ·· 1007",
                amt: "−$1,344.18",
                ic: "creditCard",
                owner: "partner",
              },
            ].map((a) => (
              <div className="dash-acct" key={a.nm}>
                <span className="dash-acct__ic">
                  <DashIcon name={a.ic} size={17} />
                </span>
                <div>
                  <div className="dash-acct__nm">{a.nm}</div>
                  <div className="dash-acct__sub">{a.sub}</div>
                </div>
                <span
                  className="dash-acct__amt"
                  style={{
                    color: a.amt.startsWith("−") ? "var(--money-neg)" : "var(--text)",
                  }}
                >
                  {a.amt}
                </span>
              </div>
            ))}
          </div>
        </div>
        <div className="dash-card">
          <div className="dash-card__head">
            <span className="dash-card__title">
              <DashIcon name="shield" size={16} className="dash-card__ic" />
              Responsibility split
            </span>
          </div>
          <div className="dash-card__body">
            <div className="dash-split">
              {[
                {
                  lbl: "Personal — Alex",
                  amt: "$1,612",
                  pct: 51,
                  c: "var(--owner-personal)",
                  type: "personal",
                },
                {
                  lbl: "Partner — Sam",
                  amt: "$888",
                  pct: 28,
                  c: "var(--owner-partner)",
                  type: "partner",
                },
                {
                  lbl: "Shared household",
                  amt: "$642",
                  pct: 21,
                  c: "var(--owner-shared)",
                  type: "shared",
                },
              ].map((r) => (
                <div key={r.lbl}>
                  <div className="dash-splitrow__top">
                    <span className="dash-splitrow__lbl">
                      <span
                        style={{
                          width: 9,
                          height: 9,
                          borderRadius: 3,
                          background: r.c,
                        }}
                      />
                      {r.lbl}
                    </span>
                    <span className="dash-splitrow__amt">
                      {r.amt} · {r.pct}%
                    </span>
                  </div>
                  <div className="dash-bar">
                    <div
                      className="dash-bar__fill"
                      style={{ width: r.pct + "%", background: r.c }}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="dash-card">
        <div className="dash-card__head">
          <span className="dash-card__title">
            <DashIcon name="receipt" size={16} className="dash-card__ic" />
            Recent activity
          </span>
          <Badge tone="warning" dot>
            6 need an owner
          </Badge>
        </div>
        <div className="dash-card__body" style={{ padding: 0 }}>
          <TransactionRow
            date="08 Jun"
            merchant="Whole Foods Market"
            owner={<OwnerChip name="Household" type="shared" bare />}
            amount="642.18"
            status="needs-owner"
            confidence="low"
          />
          <TransactionRow
            date="08 Jun"
            merchant="Acme Payroll"
            owner={<OwnerChip name="Alex Tan" type="personal" bare />}
            amount="6,200.00"
            positive
            status="reconciled"
          />
          <TransactionRow
            date="07 Jun"
            merchant="Pacific Gas & Electric"
            owner={<OwnerChip name="Household" type="shared" bare />}
            amount="148.90"
            status="reconciled"
          />
          <TransactionRow
            date="06 Jun"
            merchant="Spotify"
            owner={<OwnerChip name="Sam Okafor" type="partner" bare />}
            amount="14.99"
            status="imported"
            confidence="high"
          />
        </div>
      </div>
    </div>
  );
}
window.DashboardScreen = DashboardScreen;
