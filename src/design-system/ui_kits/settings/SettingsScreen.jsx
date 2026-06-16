/* Neko Finance — Settings / privacy screen. Local data, Google OAuth,
   AI provider keys, people/ownership, update channel. window.SettingsApp. */
const SET_NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Switch, SegmentedControl, Input, Button, Badge, OwnerChip } = SET_NS;
const SetIcon = window.Icon;

const setCSS = `
.set{max-width:760px;margin:0 auto;display:flex;flex-direction:column;gap:28px;}
.set-sec__head{margin-bottom:11px;}
.set-sec__title{font-size:15px;font-weight:700;color:var(--text-strong);letter-spacing:-0.005em;display:flex;align-items:center;gap:9px;}
.set-sec__ic{color:var(--text-faint);}
.set-sec__sub{font-size:12.5px;color:var(--text-muted);margin-top:3px;margin-left:25px;}
.set-panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);overflow:hidden;}
.set-row{display:flex;align-items:center;gap:14px;padding:14px 16px;border-bottom:1px solid var(--border);}
.set-row:last-child{border-bottom:none;}
.set-row__main{flex:1;min-width:0;}
.set-row__t{font-size:13.5px;font-weight:600;color:var(--text);}
.set-row__d{font-size:12px;color:var(--text-muted);margin-top:2px;line-height:1.4;}
.set-row__d code{font-family:var(--font-mono);font-size:11px;background:var(--surface-2);padding:1px 5px;border-radius:4px;color:var(--text);}
.set-row__ctl{flex:none;display:flex;align-items:center;gap:8px;}
.set-conn{display:flex;align-items:center;gap:12px;padding:15px 16px;background:var(--bg-subtle);border-bottom:1px solid var(--border);}
.set-conn__logo{width:38px;height:38px;border-radius:10px;background:var(--surface);border:1px solid var(--border);display:flex;align-items:center;justify-content:center;color:var(--success-500);flex:none;}
.set-conn__t{font-size:14px;font-weight:700;color:var(--text-strong);}
.set-conn__s{font-size:12px;color:var(--text-muted);margin-top:2px;display:flex;align-items:center;gap:6px;}
.set-key{display:flex;align-items:center;gap:8px;background:var(--surface-2);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0 10px;height:34px;font-family:var(--font-mono);font-size:12.5px;color:var(--text-muted);}
.set-people{display:flex;flex-direction:column;}
.set-danger{border-color:rgba(203,70,62,.28);}
[data-theme="light"] .set-danger,.set-danger{border-color:color-mix(in srgb,var(--danger-500) 30%,var(--border));}
.set-danger .set-row__t{color:var(--danger-500);}
.set-meta{display:flex;align-items:center;gap:8px;font-size:11.5px;color:var(--text-faint);font-family:var(--font-mono);}
`;
function injectSet() {
  if (document.getElementById("set-css")) return;
  const s = document.createElement("style");
  s.id = "set-css";
  s.textContent = setCSS;
  document.head.appendChild(s);
}

function Section({ icon, title, sub, children }) {
  return (
    <section>
      <div className="set-sec__head">
        <div className="set-sec__title">
          <SetIcon name={icon} size={17} className="set-sec__ic" />
          {title}
        </div>
        {sub ? <div className="set-sec__sub">{sub}</div> : null}
      </div>
      {children}
    </section>
  );
}

function SettingsApp() {
  injectSet();
  const [nav, setNav] = React.useState("settings");
  const [approve, setApprove] = React.useState(true);
  const [autoCat, setAutoCat] = React.useState(true);
  const [telemetry, setTelemetry] = React.useState(false);
  const [provider, setProvider] = React.useState("local");
  const [channel, setChannel] = React.useState("stable");
  const [revealKey, setRevealKey] = React.useState(false);

  return (
    <window.AppShell
      active={nav}
      onNav={(k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k))}
      title="Settings & privacy"
      crumb="Local · this device"
    >
      <div className="set">
        <Section
          icon="link"
          title="Connection"
          sub="Neko reads your Google Sheet. It never writes without your approval."
        >
          <div className="set-panel">
            <div className="set-conn">
              <span className="set-conn__logo">
                <SetIcon name="table" size={19} />
              </span>
              <div style={{ flex: 1 }}>
                <div className="set-conn__t">Google Sheets</div>
                <div className="set-conn__s">
                  <span
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: "50%",
                      background: "var(--success-500)",
                      display: "inline-block",
                    }}
                  />
                  conta-google-conectada · read-only scope
                </div>
              </div>
              <Badge tone="success" dot>
                Connected
              </Badge>
              <Button variant="secondary" size="sm">
                Reconnect
              </Button>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Active sheet</div>
                <div className="set-row__d">
                  Workbook <code>Expenses 2025</code> · 248 rows · synced 2 min ago
                </div>
              </div>
              <div className="set-row__ctl">
                <Button
                  variant="ghost"
                  size="sm"
                  iconLeft={<SetIcon name="refresh" size={14} />}
                >
                  Re-sync
                </Button>
                <Button variant="ghost" size="sm">
                  Change
                </Button>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Write access</div>
                <div className="set-row__d">
                  Neko proposes edits as a diff. Nothing is written until you approve.
                </div>
              </div>
              <div className="set-row__ctl">
                <Badge tone="primary">Approval required</Badge>
              </div>
            </div>
          </div>
        </Section>

        <Section
          icon="sparkles"
          title="AI copilot (Mia)"
          sub="Choose where Mia's model runs. Local keeps everything on this device."
        >
          <div className="set-panel">
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Provider</div>
                <div className="set-row__d">
                  {provider === "local"
                    ? "On-device model — no data leaves your machine."
                    : "Calls an external API. Your sheet rows are sent to the provider."}
                </div>
              </div>
              <div className="set-row__ctl">
                <SegmentedControl
                  value={provider}
                  onChange={setProvider}
                  options={[
                    { value: "local", label: "Local" },
                    { value: "api", label: "API key" },
                  ]}
                />
              </div>
            </div>
            {provider === "api" ? (
              <div className="set-row">
                <div className="set-row__main">
                  <div className="set-row__t">API key</div>
                  <div className="set-row__d">
                    Stored encrypted in your local keychain — never synced.
                  </div>
                </div>
                <div className="set-row__ctl">
                  <span className="set-key">
                    {revealKey ? "sk-neko-7f3a9c21b8e4" : "sk-neko-••••••••••••"}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setRevealKey((v) => !v)}
                  >
                    {revealKey ? "Hide" : "Reveal"}
                  </Button>
                </div>
              </div>
            ) : null}
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Auto-categorize on import</div>
                <div className="set-row__d">
                  Mia suggests a category & owner for each new row. You confirm
                  low-confidence ones.
                </div>
              </div>
              <div className="set-row__ctl">
                <Switch checked={autoCat} onChange={setAutoCat} />
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Require approval for sheet writes</div>
                <div className="set-row__d">
                  Strongly recommended. Disabling lets Mia write approved rule-matches
                  directly.
                </div>
              </div>
              <div className="set-row__ctl">
                <Switch checked={approve} onChange={setApprove} />
              </div>
            </div>
          </div>
        </Section>

        <Section
          icon="shield"
          title="Privacy & data"
          sub="Neko is local-first. There is no Neko account and no backend."
        >
          <div className="set-panel">
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Data location</div>
                <div className="set-row__d">
                  Encrypted SQLite at <code>~/Library/Neko/neko.db</code> on this device
                  only.
                </div>
              </div>
              <div className="set-row__ctl">
                <Button
                  variant="ghost"
                  size="sm"
                  iconLeft={<SetIcon name="download" size={14} />}
                >
                  Export
                </Button>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Anonymous diagnostics</div>
                <div className="set-row__d">
                  Off by default. Neko sends no usage data unless you opt in.
                </div>
              </div>
              <div className="set-row__ctl">
                <Switch checked={telemetry} onChange={setTelemetry} />
              </div>
            </div>
          </div>
        </Section>

        <Section
          icon="settings"
          title="People & ownership"
          sub="Who shares this ledger, and the default owner for new shared expenses."
        >
          <div className="set-panel set-people">
            <div className="set-row">
              <div className="set-row__main">
                <OwnerChip name="Alex Tan" type="personal" note="You" />
              </div>
              <div className="set-row__ctl">
                <Badge tone="neutral">Owner</Badge>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <OwnerChip name="Sam Okafor" type="partner" note="Partner" />
              </div>
              <div className="set-row__ctl">
                <Badge tone="neutral">Can view & assign</Badge>
                <Button variant="ghost" size="sm">
                  Manage
                </Button>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Default owner for shared expenses</div>
                <div className="set-row__d">
                  Applied when a new charge matches a shared-venue rule.
                </div>
              </div>
              <div className="set-row__ctl">
                <SegmentedControl
                  value="shared"
                  onChange={() => {}}
                  options={[
                    {
                      value: "personal",
                      label: "Personal",
                      dot: "var(--owner-personal)",
                    },
                    { value: "shared", label: "Household", dot: "var(--owner-shared)" },
                  ]}
                />
              </div>
            </div>
          </div>
        </Section>

        <Section icon="refresh" title="Updates" sub="Neko 0.4.2 · Tauri desktop build.">
          <div className="set-panel">
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Update channel</div>
                <div className="set-row__d">
                  Stable ships monthly. Beta gets new copilot tools earlier.
                </div>
              </div>
              <div className="set-row__ctl">
                <SegmentedControl
                  value={channel}
                  onChange={setChannel}
                  options={[
                    { value: "stable", label: "Stable" },
                    { value: "beta", label: "Beta" },
                  ]}
                />
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Current version</div>
                <div className="set-row__d">
                  <span className="set-meta">
                    <SetIcon
                      name="check"
                      size={13}
                      style={{ color: "var(--success-500)" }}
                    />
                    v0.4.2 · up to date · checked today
                  </span>
                </div>
              </div>
              <div className="set-row__ctl">
                <Button variant="secondary" size="sm">
                  Check for updates
                </Button>
              </div>
            </div>
          </div>
        </Section>

        <Section
          icon="alertTriangle"
          title="Danger zone"
          sub="These actions affect only this device's local data."
        >
          <div className="set-panel set-danger">
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Disconnect Google Sheets</div>
                <div className="set-row__d">
                  Removes the OAuth token. Your local data stays.
                </div>
              </div>
              <div className="set-row__ctl">
                <Button variant="secondary" size="sm">
                  Disconnect
                </Button>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Erase local data</div>
                <div className="set-row__d">
                  Permanently deletes the local database and cached rules.
                </div>
              </div>
              <div className="set-row__ctl">
                <Button variant="danger" size="sm">
                  Erase…
                </Button>
              </div>
            </div>
          </div>
        </Section>
      </div>
    </window.AppShell>
  );
}
window.SettingsApp = SettingsApp;
