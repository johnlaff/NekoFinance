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
          <div className="cpd-priv__t">Privado e local</div>
          <div className="cpd-priv__s">
            A Mia roda no dispositivo. Nada sai da sua máquina sem aprovação.
          </div>
        </div>
      </div>
      <div>
        <div className="cpd-h" style={{ marginBottom: 8 }}>
          O que a Mia enxerga
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
              <div className="cpd-row__t">Despesas 2025</div>
              <div className="cpd-row__s">
                Somente leitura · 248 linhas · sincronizado há 2 min
              </div>
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
              <div className="cpd-row__t">Escritas precisam de aprovação</div>
              <div className="cpd-row__s">Toda mudança aparece como diff antes.</div>
            </div>
          </div>
        </div>
      </div>
      <div>
        <div className="cpd-h" style={{ marginBottom: 8 }}>
          Sugestões
        </div>
        <div
          className="cpd-card"
          style={{ display: "flex", flexDirection: "column", gap: 7 }}
        >
          {[
            "Por que o fluxo de junho caiu?",
            "Dividir o aluguel 60/40 daqui pra frente",
            "Achar assinaturas esquecidas",
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
      title="Pergunte à Mia"
      crumb="Privado · roda localmente"
      dock={dock}
      flush={true}
    >
      <div className="cp">
        <div className="cp-scroll" ref={scrollRef}>
          <div className="cp-thread">
            <div className="cp-day">Hoje</div>

            <ChatBubble from="user" userInitials="JA">
              O mês passado tem vários gastos com comida sem categoria. Pode
              categorizá-los e marcar os compartilhados como casa?
            </ChatBubble>

            <ChatBubble from="mia">
              <p>
                Encontrei <b>3 gastos com comida sem categoria</b> em maio, somando{" "}
                <span className="nk-chat__money">R$ 131,70</span>{" "}
                <Citation index={1} source="linhas 1204–1206" />. Dois são em lugares
                que você costuma dividir.
              </p>
              <Citation
                variant="tool"
                fn="filtro(maio 2025, categoria = ∅, tipo = comida)"
                lines={[
                  { label: "Supermercado · 12 mai", value: "78,00" },
                  { label: "Cafeteria · 19 mai", value: "9,50" },
                  { label: "Restaurante · 24 mai", value: "44,20" },
                ]}
                total={{ label: "Correspondentes", value: "R$ 131,70" }}
                source="Planilha ‘Despesas 2025’ · 3 linhas · determinístico"
              />
            </ChatBubble>

            <ChatBubble from="mia">
              <p>
                Esta é a mudança que eu faria. Precisa da sua aprovação antes de eu
                tocar na planilha:
              </p>
            </ChatBubble>

            <div style={{ paddingLeft: 42 }}>
              <ApprovalDiffCard
                title="Categorizar e atribuir 3 linhas de comida"
                sheet="Despesas 2025"
                range="E1204:F1206"
                status={status}
                changes={[
                  { field: "Categoria", before: "—", after: "Comida" },
                  {
                    field: "Titular (2 linhas)",
                    before: "Sem titular",
                    after: "Casa (compartilhado)",
                  },
                  {
                    field: "Titular (1 linha)",
                    before: "Sem titular",
                    after: "João (pessoal)",
                  },
                ]}
                note={
                  <span>
                    <b>Supermercado</b> e <b>Restaurante</b> batem com sua regra de
                    comida compartilhada; <b>Cafeteria</b> foi um gasto individual perto
                    do escritório.
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
                        Aprovar e gravar
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        iconLeft={<CpIcon name="pencil" size={14} />}
                      >
                        Editar
                      </Button>
                      <span style={{ flex: 1 }} />
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => setStatus("rejected")}
                      >
                        Rejeitar
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
                        ? "Gravado em Despesas 2025 · 3 linhas atualizadas"
                        : "Rejeitado — nenhuma mudança feita"}
                    </span>
                  )
                }
              />
            </div>

            {status === "approved" ? (
              <ChatBubble from="mia">
                <p>
                  Pronto — atualizei <b>3 linhas</b> e sua comida de maio agora soma{" "}
                  <span className="nk-chat__money">R$ 486,20</span>. Quer que eu crie
                  uma regra para sugerir “Casa” automaticamente em gastos de lugares
                  compartilhados?
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
                placeholder="Pergunte sobre seu dinheiro — a Mia cita cada número…"
              />
              <button className="cp-send">
                <CpIcon name="send" size={16} />
              </button>
            </div>
            <div className="cp-foot">
              <span className="cp-foot__dot" /> Modelo local · lê sua planilha somente
              leitura · escritas sempre precisam de aprovação
            </div>
          </div>
        </div>
      </div>
    </window.AppShell>
  );
}
window.CopilotApp = CopilotApp;
