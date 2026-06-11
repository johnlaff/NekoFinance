/* Neko Finance — Copilot approval flow. Chat with cited/deterministic answers
   and a live human-approved sheet diff. Exposes window.CopilotApp. */
const CP_NS = window.NekoFinanceDesignSystem_9bd1cd;
const { ChatBubble, Citation, ApprovalDiffCard, Button, Badge, Input } = CP_NS;
const CpIcon = window.Icon;

const cpCSS = `
.cp{display:flex;flex-direction:column;height:100%;min-height:0;}
.cp-scroll{flex:1;overflow:auto;display:flex;flex-direction:column;align-items:center;padding:8px 0 18px;}
.cp-thread{width:100%;max-width:720px;display:flex;flex-direction:column;gap:16px;padding:0 22px;}
.cp-day{align-self:center;font-size:11px;color:var(--text-faint);background:var(--surface);border:1px solid var(--border);
  padding:3px 11px;border-radius:999px;margin:4px 0;}
.cp-approved{display:flex;align-items:center;gap:8px;font-size:12.5px;color:var(--success-400);font-weight:600;
  padding-left:42px;}
.cp-composer{flex:none;border-top:1px solid var(--border);background:var(--bg-subtle);padding:14px 22px;}
.cp-composer__inner{max-width:720px;margin:0 auto;display:flex;flex-direction:column;gap:8px;}
.cp-inrow{display:flex;align-items:flex-end;gap:9px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-md);padding:8px 8px 8px 14px;transition:border-color var(--dur-fast) var(--ease-standard);}
.cp-inrow:focus-within{border-color:var(--border-focus);}
.cp-inrow textarea{flex:1;resize:none;border:none;outline:none;background:none;color:var(--text);font-family:var(--font-sans);
  font-size:14px;line-height:1.45;max-height:120px;padding:5px 0;}
.cp-inrow textarea::placeholder{color:var(--text-faint);}
.cp-send{width:34px;height:34px;border-radius:var(--radius-sm);border:none;background:var(--primary);color:var(--text-on-primary);
  display:flex;align-items:center;justify-content:center;cursor:pointer;flex:none;transition:var(--t-hover);}
.cp-send:hover{background:var(--primary-hover);}
.cp-foot{display:flex;align-items:center;gap:8px;justify-content:center;font-size:11px;color:var(--text-faint);}
.cp-foot__dot{width:5px;height:5px;border-radius:50%;background:var(--success-500);}
/* dock */
.cpd{padding:16px;display:flex;flex-direction:column;gap:16px;}
.cpd-h{font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);}
.cpd-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;}
.cpd-row{display:flex;align-items:flex-start;gap:10px;padding:7px 0;}
.cpd-row__ic{width:26px;height:26px;border-radius:7px;display:flex;align-items:center;justify-content:center;flex:none;}
.cpd-row__t{font-size:12.5px;font-weight:600;color:var(--text);}
.cpd-row__s{font-size:11px;color:var(--text-faint);margin-top:1px;line-height:1.35;}
.cpd-priv{display:flex;align-items:center;gap:9px;padding:11px 13px;background:var(--primary-quiet);
  border:1px solid rgba(63,191,143,.22);border-radius:var(--radius-md);}
.cpd-priv__t{font-size:12px;font-weight:700;color:var(--text-strong);}
.cpd-priv__s{font-size:11px;color:var(--text-muted);margin-top:1px;}
`;
function injectCp() {
  if (document.getElementById("cp-css")) return;
  const s = document.createElement("style");
  s.id = "cp-css";
  s.textContent = cpCSS;
  document.head.appendChild(s);
}

function CopilotApp() {
  injectCp();
  const [nav, setNav] = React.useState("copilot");
  const [status, setStatus] = React.useState("pending"); // pending|approved|rejected
  const scrollRef = React.useRef(null);

  const dock = (
    <div className="cpd">
      <div className="cpd-priv">
        <span style={{ color: "var(--primary)" }}>
          <CpIcon name="lock" size={20} />
        </span>
        <div>
          <div className="cpd-priv__t">Private &amp; local</div>
          <div className="cpd-priv__s">
            Mia runs on-device. Nothing leaves your machine without approval.
          </div>
        </div>
      </div>
      <div>
        <div className="cpd-h" style={{ marginBottom: 8 }}>
          What Mia can see
        </div>
        <div className="cpd-card">
          <div className="cpd-row">
            <span
              className="cpd-row__ic"
              style={{ background: "var(--success-tint)", color: "var(--success-400)" }}
            >
              <CpIcon name="table" size={15} />
            </span>
            <div>
              <div className="cpd-row__t">Expenses 2025</div>
              <div className="cpd-row__s">Read-only · 248 rows · synced 2m ago</div>
            </div>
          </div>
          <div className="cpd-row">
            <span
              className="cpd-row__ic"
              style={{
                background: "var(--surface-elevated)",
                color: "var(--text-muted)",
              }}
            >
              <CpIcon name="key" size={15} />
            </span>
            <div>
              <div className="cpd-row__t">Writes need approval</div>
              <div className="cpd-row__s">Every change is shown as a diff first.</div>
            </div>
          </div>
        </div>
      </div>
      <div>
        <div className="cpd-h" style={{ marginBottom: 8 }}>
          Suggested
        </div>
        <div
          className="cpd-card"
          style={{ display: "flex", flexDirection: "column", gap: 7 }}
        >
          {[
            "Why was June cashflow lower?",
            "Split rent 60/40 going forward",
            "Find subscriptions we forgot",
          ].map((s) => (
            <div
              key={s}
              style={{ fontSize: 12.5, color: "var(--text-muted)", cursor: "pointer" }}
            >
              ↳ {s}
            </div>
          ))}
        </div>
      </div>
    </div>
  );

  return (
    <window.AppShell
      active={nav}
      onNav={(k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k))}
      title="Ask Mia"
      crumb="Private · runs locally"
      dock={dock}
      flush={true}
    >
      <div className="cp">
        <div className="cp-scroll" ref={scrollRef}>
          <div className="cp-thread">
            <div className="cp-day">Today</div>

            <ChatBubble from="user" userInitials="AT">
              Last month has a bunch of uncategorized dining. Can you categorize them
              and mark the shared ones as household?
            </ChatBubble>

            <ChatBubble from="mia">
              <p>
                I found <b>3 uncategorized dining charges</b> in May totaling{" "}
                <span className="nk-chat__money">$131.70</span>{" "}
                <Citation index={1} source="rows 1204–1206" />. Two are at venues you
                usually split with Sam.
              </p>
              <Citation
                variant="tool"
                fn="filter(May 2025, category = ∅, mcc = dining)"
                lines={[
                  { label: "Whole Foods · 12 May", value: "78.00" },
                  { label: "Blue Bottle · 19 May", value: "9.50" },
                  { label: "Bottega · 24 May", value: "44.20" },
                ]}
                total={{ label: "Matched", value: "$131.70" }}
                source="Sheet ‘Expenses 2025’ · 3 rows · deterministic"
              />
            </ChatBubble>

            <ChatBubble from="mia">
              <p>
                Here's the change I'd make. It needs your approval before I touch the
                sheet:
              </p>
            </ChatBubble>

            <div style={{ paddingLeft: 42 }}>
              <ApprovalDiffCard
                title="Categorize & assign 3 dining rows"
                sheet="Expenses 2025"
                range="E1204:F1206"
                status={status}
                changes={[
                  { field: "Category", before: "—", after: "Dining" },
                  {
                    field: "Owner (2 rows)",
                    before: "Unassigned",
                    after: "Household (shared)",
                  },
                  {
                    field: "Owner (1 row)",
                    before: "Unassigned",
                    after: "Alex (personal)",
                  },
                ]}
                note={
                  <span>
                    Venues <b>Whole Foods</b> &amp; <b>Bottega</b> match your
                    shared-dining rule; <b>Blue Bottle</b> was a solo charge near your
                    office.
                  </span>
                }
                actions={
                  status === "pending" ? (
                    <>
                      <Button
                        variant="primary"
                        size="sm"
                        iconLeft={<CpIcon name="check" size={15} />}
                        onClick={() => setStatus("approved")}
                      >
                        Approve &amp; write
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        iconLeft={<CpIcon name="pencil" size={14} />}
                      >
                        Edit
                      </Button>
                      <span style={{ flex: 1 }} />
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => setStatus("rejected")}
                      >
                        Reject
                      </Button>
                    </>
                  ) : (
                    <span
                      style={{
                        fontSize: 12.5,
                        color:
                          status === "approved"
                            ? "var(--success-400)"
                            : "var(--danger-400)",
                        fontWeight: 600,
                        display: "flex",
                        alignItems: "center",
                        gap: 7,
                      }}
                    >
                      <CpIcon
                        name={status === "approved" ? "checkCircle" : "x"}
                        size={15}
                      />
                      {status === "approved"
                        ? "Written to Expenses 2025 · 3 rows updated"
                        : "Rejected — no changes made"}
                    </span>
                  )
                }
              />
            </div>

            {status === "approved" ? (
              <ChatBubble from="mia">
                <p>
                  Done — I updated <b>3 rows</b> and your May dining now reads{" "}
                  <span className="nk-chat__money">$486.20</span>. Want me to set up a
                  rule so future shared-venue charges auto-suggest “Household”?
                </p>
              </ChatBubble>
            ) : null}
          </div>
        </div>

        <div className="cp-composer">
          <div className="cp-composer__inner">
            <div className="cp-inrow">
              <textarea
                rows="1"
                placeholder="Ask about your money — Mia cites every number…"
              />
              <button className="cp-send">
                <CpIcon name="send" size={16} />
              </button>
            </div>
            <div className="cp-foot">
              <span className="cp-foot__dot" /> Local model · reads your sheet read-only
              · writes always need approval
            </div>
          </div>
        </div>
      </div>
    </window.AppShell>
  );
}
window.CopilotApp = CopilotApp;
