/* Neko Finance — Configurações e privacidade. Fiel ao SettingsScreen.tsx de produção:
   conexão Google Sheets, importação .xlsx local, bolsos (Pockets), lembrete diário,
   teto do Diário, categorias do Diário e seus dados (local-first, backup, versão).
   Expõe window.SettingsScreen — o index.html envolve com window.AppShell. */
const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { SegmentedControl, Button, Badge } = NS;
const Icon = window.Icon;

(function injectSettingsCSS() {
  if (document.getElementById("settings-css")) return;
  const s = document.createElement("style");
  s.id = "settings-css";
  s.textContent = `
.set{max-width:760px;margin:0 auto;display:flex;flex-direction:column;gap:28px;}

/* cabeçalho de seção */
.set-sec__head{margin-bottom:11px;}
.set-sec__title{font-size:15px;font-weight:700;color:var(--text-strong);letter-spacing:-0.005em;
  display:flex;align-items:center;gap:9px;margin:0;}
.set-sec__ic{color:var(--text-faint);flex:none;}
.set-sec__sub{font-size:12.5px;color:var(--text-muted);margin:3px 0 0 26px;line-height:1.45;}

/* painel de cartão */
.set-panel{background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-md);box-shadow:var(--shadow-1);overflow:hidden;}
.set-panel--pad{padding:0;}

/* linha de configuração */
.set-row{display:flex;align-items:center;gap:14px;padding:14px 16px;
  border-bottom:1px solid var(--border);}
.set-row:last-child{border-bottom:none;}
.set-row__main{flex:1;min-width:0;}
.set-row__t{font-size:13.5px;font-weight:600;color:var(--text);}
.set-row__d{font-size:12px;color:var(--text-muted);margin-top:2px;line-height:1.45;}
.set-row__d code{font-family:var(--font-mono);font-size:11px;background:var(--surface-2);
  padding:1px 5px;border-radius:4px;color:var(--text);}
.set-row__ctl{flex:none;display:flex;align-items:center;gap:8px;}

/* bloco de conexão com logo */
.set-conn{display:flex;align-items:center;gap:12px;padding:15px 16px;
  background:var(--bg-subtle);border-bottom:1px solid var(--border);}
.set-conn__logo{width:38px;height:38px;border-radius:10px;background:var(--surface);
  border:1px solid var(--border);display:flex;align-items:center;justify-content:center;
  color:var(--success-500);flex:none;}
.set-conn__t{font-size:14px;font-weight:700;color:var(--text-strong);}
.set-conn__s{font-size:12px;color:var(--text-muted);margin-top:2px;
  display:flex;align-items:center;gap:6px;}
.set-conn__dot{width:6px;height:6px;border-radius:50%;display:inline-block;flex:none;}
.set-conn__dot--ok{background:var(--success-500);}
.set-conn__dot--off{background:var(--text-faint);}

/* bolsos (pockets) */
.set-pockets{display:flex;flex-direction:column;}
.set-pocket-row{display:flex;align-items:center;gap:10px;padding:12px 16px;
  border-bottom:1px solid var(--border);}
.set-pocket-row:last-child{border-bottom:none;}
.set-pocket__ic{width:32px;height:32px;border-radius:8px;background:var(--surface-elevated);
  border:1px solid var(--border);display:flex;align-items:center;justify-content:center;
  color:var(--text-muted);flex:none;}
.set-pocket__nm{font-size:13px;font-weight:600;color:var(--text);}
.set-pocket__sub{font-size:11px;color:var(--text-faint);}
.set-pocket__amt{margin-left:auto;font-family:var(--font-money);font-variant-numeric:tabular-nums;
  font-size:14px;font-weight:600;color:var(--text);}
.set-pocket__badge{flex:none;}

/* categorias do Diário */
.set-cats{padding:14px 16px;}
.set-cat-row{display:flex;align-items:center;gap:8px;margin-bottom:8px;}
.set-cat-row:last-child{margin-bottom:0;}
.set-cat__name{flex:1;font-size:13px;font-weight:500;color:var(--text);}
.set-cat__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;
  font-size:13px;color:var(--text-muted);min-width:80px;text-align:right;}
.set-cat__bar{height:4px;border-radius:2px;background:var(--surface-2);margin-top:4px;overflow:hidden;}
.set-cat__fill{height:100%;border-radius:2px;background:var(--primary);}
.set-cats__total{margin-top:10px;padding-top:10px;border-top:1px solid var(--border);
  display:flex;justify-content:space-between;font-size:12px;color:var(--text-muted);}
.set-cats__total-amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;
  font-weight:600;color:var(--text);}

/* zona de perigo */
.set-danger{border-color:color-mix(in srgb,var(--danger-500) 30%,var(--border));}
.set-danger .set-row__t{color:var(--danger-400);}

/* meta da versão */
.set-meta{display:flex;align-items:center;gap:6px;font-size:11.5px;
  color:var(--text-faint);font-family:var(--font-mono);}

/* lembrete: campo de horário */
.set-time-ctl{display:flex;align-items:center;gap:8px;}
.set-time-input{font-family:var(--font-money);font-size:var(--fs-body);
  background:var(--bg-subtle);border:1px solid var(--border-input);
  border-radius:var(--radius-xs);color:var(--text);padding:4px 8px;
  height:var(--hit-min);}
`;
  document.head.appendChild(s);
})();

/* ---- Componentes de suporte ---- */

function Section({ icon, title, sub, children }) {
  return (
    <section>
      <div className="set-sec__head">
        <h2 className="set-sec__title">
          <Icon name={icon} size={17} className="set-sec__ic" />
          {title}
        </h2>
        {sub ? <div className="set-sec__sub">{sub}</div> : null}
      </div>
      {children}
    </section>
  );
}

/* ---- Seção: Conexão Google Sheets ---- */
function ConexaoSection() {
  const [status, setStatus] = React.useState("connected"); // connected | disconnected | expired

  return (
    <Section
      icon="link"
      title="Conexão Google Sheets"
      sub="O Neko lê sua planilha. Nada é escrito sem a sua aprovação."
    >
      <div className="set-panel">
        <div className="set-conn">
          <span className="set-conn__logo">
            <Icon name="table" size={19} />
          </span>
          <div style={{ flex: 1 }}>
            <div className="set-conn__t">Google Sheets</div>
            <div className="set-conn__s">
              <span
                className={
                  "set-conn__dot " +
                  (status === "connected" ? "set-conn__dot--ok" : "set-conn__dot--off")
                }
                aria-hidden="true"
              />
              {status === "connected"
                ? "voce@gmail.com · somente leitura"
                : status === "expired"
                  ? "Sessão expirada — reconecte para sincronizar"
                  : "Desconectado"}
            </div>
          </div>
          {status === "connected" ? (
            <Badge tone="success" dot>
              Conectado
            </Badge>
          ) : (
            <Badge tone="warning">Desconectado</Badge>
          )}
          <Button
            variant="secondary"
            size="sm"
            onClick={() =>
              setStatus(status === "connected" ? "disconnected" : "connected")
            }
          >
            {status === "connected" ? "Reconectar" : "Conectar"}
          </Button>
        </div>

        {status === "connected" ? (
          <>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Planilha ativa</div>
                <div className="set-row__d">
                  Pasta de trabalho <code>Finanças 2025</code> · 226 lançamentos ·
                  sincronizada há 3 min
                </div>
              </div>
              <div className="set-row__ctl">
                <Button
                  variant="ghost"
                  size="sm"
                  iconLeft={<Icon name="refresh" size={14} />}
                >
                  Re-sincronizar
                </Button>
                <Button variant="ghost" size="sm">
                  Trocar
                </Button>
              </div>
            </div>
            <div className="set-row">
              <div className="set-row__main">
                <div className="set-row__t">Escrita na planilha</div>
                <div className="set-row__d">
                  O Neko propõe edições como um diff. Nada é gravado até você aprovar.
                </div>
              </div>
              <div className="set-row__ctl">
                <Badge tone="primary">Aprovação obrigatória</Badge>
              </div>
            </div>
          </>
        ) : null}
      </div>
    </Section>
  );
}

/* ---- Seção: Importar arquivo local ---- */
function ImportacaoLocalSection() {
  const [imported, setImported] = React.useState(false);

  return (
    <Section
      icon="download"
      title="Importar arquivo local"
      sub="Use uma cópia .xlsx da planilha quando não quiser conectar a conta Google."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Planilha .xlsx</div>
            <div className="set-row__d">
              Importa todas as abas, detectando o layout de blocos mensais
              automaticamente. Linhas já importadas antes são ignoradas.
              {imported ? <strong> Importado com sucesso.</strong> : null}
            </div>
          </div>
          <div className="set-row__ctl">
            <Button
              variant="secondary"
              size="sm"
              iconLeft={<Icon name="download" size={14} />}
              onClick={() => setImported(true)}
            >
              Escolher arquivo…
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/* ---- Seção: Bolsos ---- */
const BOLSOS = [
  {
    nm: "Conta corrente",
    sub: "Nubank ·· 4821",
    amt: "R$ 12.408,52",
    ic: "wallet",
    liquid: true,
  },
  {
    nm: "Poupança",
    sub: "Caixa ·· 9920",
    amt: "R$ 5.800,00",
    ic: "piggy",
    liquid: true,
  },
  {
    nm: "Vale-alimentação",
    sub: "Flash · cartão VA",
    amt: "R$ 620,00",
    ic: "creditCard",
    liquid: true,
  },
  {
    nm: "FGTS",
    sub: "Caixa · bloqueado",
    amt: "R$ 38.410,00",
    ic: "lock",
    liquid: false,
  },
  {
    nm: "Previdência",
    sub: "XP · longo prazo",
    amt: "R$ 22.900,00",
    ic: "shield",
    liquid: false,
  },
];

function BolsosSection() {
  return (
    <Section
      icon="wallet"
      title="Bolsos"
      sub="Conta, poupança, vale, previdência e FGTS: só dinheiro líquido entra no saldo projetado."
    >
      <div className="set-panel set-pockets">
        {BOLSOS.map((b) => (
          <div className="set-pocket-row" key={b.nm}>
            <span className="set-pocket__ic">
              <Icon name={b.ic} size={16} />
            </span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="set-pocket__nm">{b.nm}</div>
              <div className="set-pocket__sub">{b.sub}</div>
            </div>
            {b.liquid ? null : (
              <span className="set-pocket__badge">
                <Badge tone="neutral">Bloqueado</Badge>
              </span>
            )}
            <span
              className="set-pocket__amt"
              style={{ color: b.liquid ? "var(--text)" : "var(--text-faint)" }}
            >
              {b.amt}
            </span>
          </div>
        ))}
        <div className="set-row" style={{ borderTop: "1px solid var(--border)" }}>
          <div className="set-row__main">
            <div className="set-row__t">Saldo líquido projetado</div>
            <div className="set-row__d">
              Soma apenas os bolsos líquidos (conta, poupança, VA). FGTS e previdência
              ficam de fora.
            </div>
          </div>
          <div className="set-row__ctl">
            <Button variant="ghost" size="sm" iconLeft={<Icon name="plus" size={14} />}>
              Adicionar bolso
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/* ---- Seção: Lembrete diário ---- */
function LembreteDiarioSection() {
  const [enabled, setEnabled] = React.useState(true);
  const [time, setTime] = React.useState("20:00");

  return (
    <Section
      icon="bell"
      title="Lembrete diário"
      sub="Notificação nativa no horário escolhido — dispara mesmo com o app fechado."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Ativar lembrete</div>
            <div className="set-row__d">
              Envia uma notificação nativa no horário escolhido — agendada no sistema
              para disparar mesmo com o Neko fechado.
            </div>
          </div>
          <div className="set-row__ctl">
            <SegmentedControl
              options={[
                { value: "on", label: "Ligado" },
                { value: "off", label: "Desligado" },
              ]}
              value={enabled ? "on" : "off"}
              onChange={(val) => setEnabled(val === "on")}
              size="sm"
              ariaLabel="Ativar ou desativar lembrete diário"
            />
          </div>
        </div>
        {enabled ? (
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Horário</div>
              <div className="set-row__d">Hora local (24 h) para receber o aviso.</div>
            </div>
            <div className="set-row__ctl set-time-ctl">
              <input
                type="time"
                value={time}
                onChange={(e) => setTime(e.currentTarget.value)}
                className="set-time-input"
                aria-label="Horário do lembrete diário"
              />
            </div>
          </div>
        ) : null}
      </div>
    </Section>
  );
}

/* ---- Seção: Teto do Diário ---- */
function TetoDiarioSection() {
  const [raw, setRaw] = React.useState("50,00");
  const [saved, setSaved] = React.useState(false);

  function handleSave() {
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <Section
      icon="sliders"
      title="Teto do Diário"
      sub="Defina quanto pretende gastar por dia no variável. Deixe em branco para usar a média do mês anterior."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Teto diário (R$)</div>
            <div className="set-row__d">
              Orienta a barra de progresso do check-in e o forecast dos dias futuros do
              mês. Em branco = usar a média do mês anterior automaticamente.
              {saved ? <strong> Salvo.</strong> : null}
            </div>
          </div>
          <div className="set-row__ctl">
            <input
              type="text"
              inputMode="decimal"
              placeholder="ex.: 50,00"
              value={raw}
              onChange={(e) => {
                setRaw(e.currentTarget.value);
                setSaved(false);
              }}
              aria-label="Teto diário em reais"
              style={{
                fontFamily: "var(--font-money)",
                fontSize: "var(--fs-body)",
                background: "var(--bg-subtle)",
                border: "1px solid var(--border-input)",
                borderRadius: "var(--radius-xs)",
                color: "var(--text)",
                padding: "4px 8px",
                height: "var(--hit-min)",
                width: "10ch",
              }}
            />
            <Button variant="secondary" size="sm" onClick={handleSave}>
              Salvar
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/* ---- Seção: Categorias do Diário ---- */
const CAT_DEMO = [
  { name: "Alimentação", amount: "R$ 380,00", pct: 38 },
  { name: "Transporte", amount: "R$ 200,00", pct: 20 },
  { name: "Farmácia", amount: "R$ 150,00", pct: 15 },
  { name: "Lazer", amount: "R$ 150,00", pct: 15 },
  { name: "Outros", amount: "R$ 120,00", pct: 12 },
];

function CategoriasDiarioSection() {
  return (
    <Section
      icon="layoutList"
      title="Categorias do Diário"
      sub="Distribua o teto mensal do Diário entre categorias (ex.: Alimentação, Transporte). O teto por dia é a soma ÷ dias do mês."
    >
      <div className="set-panel">
        <div className="set-row" style={{ borderBottom: "1px solid var(--border)" }}>
          <div className="set-row__main">
            <div className="set-row__t">Teto mensal do Diário (R$)</div>
            <div className="set-row__d">
              Em branco = usar a soma das categorias abaixo como teto mensal.
            </div>
          </div>
          <div className="set-row__ctl">
            <input
              type="text"
              inputMode="decimal"
              placeholder="ex.: 1.250,00"
              defaultValue="1.000,00"
              aria-label="Teto mensal do Diário em reais"
              style={{
                fontFamily: "var(--font-money)",
                fontSize: "var(--fs-body)",
                background: "var(--bg-subtle)",
                border: "1px solid var(--border-input)",
                borderRadius: "var(--radius-xs)",
                color: "var(--text)",
                padding: "4px 8px",
                height: "var(--hit-min)",
                width: "12ch",
              }}
            />
          </div>
        </div>
        <div className="set-cats">
          {CAT_DEMO.map((c) => (
            <div className="set-cat-row" key={c.name}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    marginBottom: 4,
                  }}
                >
                  <span className="set-cat__name">{c.name}</span>
                  <span className="set-cat__amt">{c.amount}</span>
                </div>
                <div className="set-cat__bar">
                  <div className="set-cat__fill" style={{ width: c.pct + "%" }} />
                </div>
              </div>
            </div>
          ))}
          <div className="set-cats__total">
            <span>Total mensal · 30 dias no mês</span>
            <span className="set-cats__total-amt">
              R$&nbsp;1.000,00 &nbsp;·&nbsp; R$&nbsp;33,33/dia
            </span>
          </div>
        </div>
        <div className="set-row" style={{ borderTop: "1px solid var(--border)" }}>
          <div className="set-row__ctl" style={{ marginLeft: "auto" }}>
            <Button variant="ghost" size="sm" iconLeft={<Icon name="plus" size={14} />}>
              Adicionar categoria
            </Button>
            <Button variant="secondary" size="sm">
              Salvar categorias
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/* ---- Seção: Seus dados ---- */
function SeusDadosSection() {
  const [backupMsg, setBackupMsg] = React.useState(null);

  function doBackup() {
    setBackupMsg("Backup salvo.");
    setTimeout(() => setBackupMsg(null), 2500);
  }

  return (
    <Section
      icon="shield"
      title="Seus dados"
      sub="O Neko é local-first: não existe conta Neko nem backend."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Onde ficam os dados</div>
            <div className="set-row__d">
              Banco SQLite em <code>~/Library/Application Support/Neko/neko.db</code>,
              somente neste dispositivo.
            </div>
          </div>
        </div>
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Backup do banco</div>
            <div className="set-row__d">
              Salva uma cópia íntegra (.db) onde você escolher — leve para outro disco
              ou dispositivo.{backupMsg ? <strong> {backupMsg}</strong> : null}
            </div>
          </div>
          <div className="set-row__ctl">
            <Button
              variant="secondary"
              size="sm"
              iconLeft={<Icon name="download" size={14} />}
              onClick={doBackup}
            >
              Fazer backup
            </Button>
          </div>
        </div>
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Telemetria</div>
            <div className="set-row__d">
              O Neko não envia nenhum dado de uso. Suas finanças não saem da sua
              máquina.
            </div>
          </div>
          <div className="set-row__ctl">
            <Badge tone="neutral">Desativada</Badge>
          </div>
        </div>
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Versão</div>
            <div className="set-row__d">
              <span className="set-meta">
                <Icon name="check" size={13} style={{ color: "var(--success-500)" }} />
                Neko Finance v0.1.0 · Tauri desktop
              </span>
            </div>
          </div>
          <div className="set-row__ctl">
            <Button variant="ghost" size="sm">
              Verificar atualizações
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/* ---- Componente raiz da tela ---- */
function SettingsScreen() {
  return (
    <div className="set">
      <ConexaoSection />
      <ImportacaoLocalSection />
      <BolsosSection />
      <LembreteDiarioSection />
      <TetoDiarioSection />
      <CategoriasDiarioSection />
      <SeusDadosSection />
    </div>
  );
}

window.SettingsScreen = SettingsScreen;
