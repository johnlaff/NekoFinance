import "./config.css";
import { useEffect, useState } from "react";
import {
  Bell,
  Database,
  FileUp,
  HardDrive,
  Landmark,
  Link,
  Lock,
  Palette,
  RefreshCw,
  CircleGauge,
  Settings,
  Shield,
  Sparkles,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { PocketsManager } from "../features/pockets/PocketsManager";
import { ConflictGate } from "../features/reconcile/ConflictGate";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import { WriteBackPending } from "./dashboard/WriteBackPending";
import { useWriteBackPending } from "../hooks/useWriteBackPending";
import { ACCENTS, applyAccent, getStoredAccent, type Accent } from "../lib/accent";
import { useNekoApp } from "../shell/appContext";
import {
  backupDatabase,
  checkAuthStatus,
  getAppInfo,
  getAppSetting,
  getDailyBudget,
  GOOGLE_CLIENT_ID,
  isTauri,
  registerOsReminder,
  setAppSetting,
  startOAuthFlow,
  unregisterOsReminder,
  type AuthStatus,
} from "../lib/api";
import {
  motionEnabled,
  setMotionPreference,
  systemPrefersReducedMotion,
} from "../lib/motion";
import { playThemeReveal, readMotionLog } from "../shell/themeReveal";
import { safeErrorMessage } from "../lib/errors";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { Button } from "../design-system/components/Button";
import { SegmentedControl } from "../design-system/components/SegmentedControl";
import { Money } from "../design-system/components/Money";

// ---------------------------------------------------------------------------
// Inline styles (hoisted — React Compiler: never inline in JSX)
// ---------------------------------------------------------------------------

const TIME_INPUT_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-money)",
  fontSize: "var(--fs-body)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  padding: "4px 8px",
  height: "var(--hit-min)",
};

// ---------------------------------------------------------------------------
// Toggle switch — matches the redesign's .sw/.sw__k exactly
// ---------------------------------------------------------------------------

function Toggle({
  on,
  onClick,
  label,
}: {
  on: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      className={"sw " + (on ? "on" : "off")}
      onClick={onClick}
      role="switch"
      aria-checked={on}
      aria-label={label}
    >
      <span className="sw__k" />
    </button>
  );
}

// ---------------------------------------------------------------------------
// cfg-item row helper — mirrors the redesign's `item()` closure
// ---------------------------------------------------------------------------

function CfgItem({
  icon: Icon,
  title,
  sub,
  right,
}: {
  icon: React.ComponentType<{ size?: number; strokeWidth?: number }>;
  title: React.ReactNode;
  sub: string;
  right?: React.ReactNode;
}) {
  return (
    <div className="cfg-item">
      <span className="cfg-item__ic">
        <Icon size={17} strokeWidth={1.75} />
      </span>
      <div>
        <div className="cfg-item__t">{title}</div>
        <div className="cfg-item__s">{sub}</div>
      </div>
      {right != null ? <span className="cfg-item__r">{right}</span> : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// MotionDiagnostics — autoteste visível de animação (Aparência)
// ---------------------------------------------------------------------------

/**
 * Discrimina, na máquina do usuário e sem devtools, as três causas possíveis de
 * "animações não aparecem": (a) o motor não executa animações (WAAPI com duração
 * FIXA nunca termina/termina errado), (b) os tokens `--dur-*` estão colapsados
 * (animação CSS com duração por token termina cedo demais), (c) motor e tokens
 * ok — o problema está em quem dispara cada animação. Duas bolinhas varrem a
 * tela ~1,5s; o resultado fica legível no próprio item.
 */
function MotionDiagnostics() {
  const [verdict, setVerdict] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const facts = [
    `sistema ${systemPrefersReducedMotion() ? "reduzido" : "normal"}`,
    `toggle ${document.documentElement.getAttribute("data-motion") ?? "system"}`,
    `tokens ${
      getComputedStyle(document.documentElement)
        .getPropertyValue("--dur-base")
        .trim() || "?"
    }`,
    `view transitions ${
      typeof document.startViewTransition === "function" ? "sim" : "não"
    }`,
  ].join(" · ");

  function runTest() {
    if (running) return;
    setRunning(true);
    setVerdict(null);

    const mkDot = (bottom: number, className?: string) => {
      const dot = document.createElement("div");
      dot.style.cssText =
        `position:fixed;left:16px;bottom:${bottom}px;width:14px;height:14px;` +
        "border-radius:50%;background:var(--primary);z-index:9999;pointer-events:none;";
      if (className) dot.className = className;
      document.body.appendChild(dot);
      return dot;
    };

    const results: string[] = [];
    let pending = 3;
    const t0 = performance.now();
    const finish = () => {
      pending -= 1;
      if (pending > 0) return;
      setRunning(false);
      setVerdict(results.join(" · "));
    };

    // (a) WAAPI com duração FIXA — independe de tokens/CSS: o motor executa?
    const waapiDot = mkDot(16);
    if (typeof waapiDot.animate === "function") {
      const anim = waapiDot.animate(
        [{ transform: "translateX(0)" }, { transform: "translateX(220px)" }],
        { duration: 600, iterations: 3, direction: "alternate", easing: "ease-in-out" },
      );
      anim.onfinish = () => {
        const dt = Math.round(performance.now() - t0);
        waapiDot.remove();
        results.push(
          dt < 900 ? `WAAPI anormal (${dt}ms; esperado ~1800ms)` : `WAAPI ok (${dt}ms)`,
        );
        finish();
      };
    } else {
      waapiDot.remove();
      results.push("WAAPI indisponível");
      finish();
    }

    // (b) Animação CSS com duração por TOKEN — tokens colapsados terminam cedo.
    const cssDot = mkDot(40, "nk-diag-dot");
    cssDot.onanimationend = () => {
      const dt = Math.round(performance.now() - t0);
      cssDot.remove();
      results.push(
        dt < 700 ? `CSS anormal (${dt}ms — tokens zerados?)` : `CSS ok (${dt}ms)`,
      );
      finish();
    };

    // (c) clip-path animado via WAAPI — alguns compositors executam a animação
    // (finish no tempo certo) sem PINTAR a interpolação; aqui só medimos o tempo,
    // e o usuário reporta se VIU o quadrado alargar (a parte visual é o teste).
    const clipDot = mkDot(64);
    clipDot.style.borderRadius = "0";
    clipDot.style.width = "220px";
    clipDot.style.clipPath = "inset(0 206px 0 0)";
    if (typeof clipDot.animate === "function") {
      const clipAnim = clipDot.animate(
        [{ clipPath: "inset(0 206px 0 0)" }, { clipPath: "inset(0 0 0 0)" }],
        { duration: 600, iterations: 3, direction: "alternate", easing: "ease-in-out" },
      );
      clipAnim.onfinish = () => {
        const dt = Math.round(performance.now() - t0);
        clipDot.remove();
        results.push(`clip-path terminou (${dt}ms) — viu a barra alargar?`);
        finish();
      };
    } else {
      clipDot.remove();
      results.push("clip-path não testável");
      finish();
    }

    // Guarda: animação que nunca dispara/termina = motor não executa animações.
    window.setTimeout(() => {
      if (document.body.contains(waapiDot)) {
        waapiDot.remove();
        results.push("WAAPI nunca terminou — motor não executa animações");
        finish();
      }
      if (document.body.contains(cssDot)) {
        cssDot.remove();
        results.push("CSS nunca disparou — animações CSS não executam");
        finish();
      }
      if (document.body.contains(clipDot)) {
        clipDot.remove();
        results.push("clip-path nunca terminou");
        finish();
      }
    }, 6000);
  }

  // Roda o MESMO caminho do reveal de produção, do centro da tela, SEM trocar o
  // tema (apply vazio): o disco da cor do tema oposto cresce e se dissolve sobre a
  // UI atual. O verdict vira o log real do caminho (início/cresceu/cancelado).
  function runRevealTest() {
    if (running) return;
    setRunning(true);
    const cx = window.innerWidth / 2;
    const cy = window.innerHeight / 2;
    const next =
      document.documentElement.getAttribute("data-theme") === "light"
        ? "dark"
        : "light";
    playThemeReveal(cx, cy, Math.hypot(cx, cy), next, () => {
      // Só teste visual — o tema real não muda.
    });
    window.setTimeout(() => {
      setRunning(false);
      setVerdict(readMotionLog().slice(-2).join(" · ") || "sem log");
    }, 1100);
  }

  return (
    <CfgItem
      icon={Sparkles}
      title="Diagnóstico de animações"
      sub={verdict ?? facts}
      right={
        <span style={{ display: "inline-flex", gap: 8 }}>
          <Button variant="secondary" onClick={runTest} disabled={running}>
            {running ? "Testando…" : "Testar"}
          </Button>
          <Button variant="secondary" onClick={runRevealTest} disabled={running}>
            Testar reveal
          </Button>
        </span>
      }
    />
  );
}

// ---------------------------------------------------------------------------
// DailyReminderSection — OS-level reminder toggle + time picker
// ---------------------------------------------------------------------------

function DailyReminderSection() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [time, setTime] = useState("20:00");
  const [saving, setSaving] = useState(false);
  const [osWarn, setOsWarn] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const [en, t] = await Promise.all([
          getAppSetting("daily_reminder_enabled"),
          getAppSetting("daily_reminder_time"),
        ]);
        setEnabled(en !== "false");
        if (t) setTime(t);
      } catch {
        setEnabled(true);
      }
    })();
  }, []);

  async function syncOsReminder(on: boolean, at: string) {
    try {
      if (on) await registerOsReminder(at);
      else await unregisterOsReminder();
      setOsWarn(null);
    } catch (e) {
      setOsWarn(
        safeErrorMessage(
          e,
          "Não foi possível agendar no sistema; o lembrete ainda dispara com o app aberto.",
        ),
      );
    }
  }

  async function handleToggle(val: string) {
    const next = val === "on";
    setEnabled(next);
    setSaving(true);
    await setAppSetting("daily_reminder_enabled", next ? "true" : "false");
    await syncOsReminder(next, time);
    setSaving(false);
  }

  async function handleTimeChange(e: React.ChangeEvent<HTMLInputElement>) {
    const val = e.currentTarget.value;
    setTime(val);
    setSaving(true);
    await setAppSetting("daily_reminder_time", val);
    if (enabled) await syncOsReminder(true, val);
    setSaving(false);
  }

  if (!isTauri || enabled === null) return null;

  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <Bell size={16} strokeWidth={1.75} className="ic" />
          Notificações
        </span>
      </div>
      <div className="cfg-sec">
        <div className="cfg-item">
          <span className="cfg-item__ic">
            <Bell size={17} strokeWidth={1.75} />
          </span>
          <div>
            <div className="cfg-item__t">Lembrete diário</div>
            <div className="cfg-item__s">
              Notificação nativa no horário escolhido — no Windows, dispara mesmo com o
              app fechado.
              {osWarn ? (
                <strong role="alert" style={{ color: "var(--warning-400)" }}>
                  {" "}
                  {osWarn}
                </strong>
              ) : null}
            </div>
          </div>
          <span className="cfg-item__r">
            <SegmentedControl
              options={[
                { value: "on", label: "Ligado" },
                { value: "off", label: "Desligado" },
              ]}
              value={enabled ? "on" : "off"}
              onChange={(val) => void handleToggle(val)}
              size="sm"
              disabled={saving}
              ariaLabel="Ativar ou desativar lembrete diário"
            />
          </span>
        </div>
        {enabled ? (
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <Bell size={17} strokeWidth={1.75} />
            </span>
            <div>
              <div className="cfg-item__t">Horário</div>
              <div className="cfg-item__s">Hora local (24 h) para receber o aviso.</div>
            </div>
            <span className="cfg-item__r">
              <input
                type="time"
                value={time}
                onChange={(e) => void handleTimeChange(e)}
                disabled={saving}
                style={TIME_INPUT_STYLE}
                aria-label="Horário do lembrete diário"
              />
            </span>
          </div>
        ) : null}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// TetoLinkSection — resumo do teto do Diário com link para a tela própria
// ---------------------------------------------------------------------------

/** O teto edita-se em UMA fonte só: a tela "Teto do diário" (cerimônia por itens + divisor).
 * Aqui fica o resumo do estado atual e o caminho até lá. */
function TetoLinkSection() {
  const { navigate } = useNekoApp();
  const budgetQ = useCommand("get_daily_budget", getDailyBudget);
  const budget = budgetQ.data;
  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <CircleGauge size={16} strokeWidth={1.75} className="ic" />
          Teto do Diário
        </span>
      </div>
      <div className="cfg-sec">
        <CfgItem
          icon={CircleGauge}
          title={
            budget == null ? (
              // Fetch ainda no ar: sem negativo fabricado ("sem teto") antes do dado chegar.
              "Teto do Diário"
            ) : budget.per_day_cents > 0 ? (
              <>
                Teto estipulado: <Money cents={budget.per_day_cents} size="inherit" />{" "}
                por dia
              </>
            ) : (
              "Sem teto estipulado"
            )
          }
          sub="A cerimônia (itens mensais ÷ dias) e a edição vivem na tela do teto."
          right={
            <Button variant="secondary" onClick={() => navigate("teto")}>
              Abrir teto do diário
            </Button>
          }
        />
      </div>
    </section>
  );
}

// DataBackupRow — backup button row inside Seus dados card
// ---------------------------------------------------------------------------

function DataBackupRow() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  async function doBackup() {
    setMsg(null);
    setErr(null);
    let dest: string | null;
    try {
      dest = await save({
        title: "Salvar backup do Neko",
        defaultPath: "neko-finance-backup.db",
        filters: [{ name: "Banco SQLite", extensions: ["db"] }],
      });
    } catch (e) {
      setErr(safeErrorMessage(e, "Não foi possível abrir o seletor de arquivo."));
      return;
    }
    if (!dest) return;
    setBusy(true);
    try {
      await backupDatabase(dest);
      setBusy(false);
      setMsg("Backup salvo.");
    } catch (e) {
      setBusy(false);
      setErr(safeErrorMessage(e, "Não foi possível fazer o backup."));
    }
  }

  return (
    <div className="cfg-item">
      <span className="cfg-item__ic">
        <HardDrive size={17} strokeWidth={1.75} />
      </span>
      <div>
        <div className="cfg-item__t">Backup do banco</div>
        <div className="cfg-item__s">
          Salva uma cópia íntegra (.db) onde você escolher.{" "}
          {msg ? <strong>{msg}</strong> : null}
          {err ? (
            <strong role="alert" style={{ color: "var(--danger-400)" }}>
              {err}
            </strong>
          ) : null}
        </div>
      </div>
      <span className="cfg-item__r">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void doBackup()}
          disabled={busy || !isTauri}
        >
          {busy ? "Salvando…" : "Fazer backup"}
        </Button>
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main exported component
// ---------------------------------------------------------------------------

/** Após iniciar o OAuth, o token chega de forma assíncrona (consentimento no navegador). Sonda o
 *  status até conectar (≤ 2 min). Recursão com setTimeout evita await-dentro-de-loop. Module-scope:
 *  não usa estado local. */
async function pollConnected(attempt: number): Promise<AuthStatus> {
  if (attempt >= 60) return checkAuthStatus();
  await new Promise((resolve) => setTimeout(resolve, 2000));
  const status = await checkAuthStatus();
  return status === "connected" ? status : pollConnected(attempt + 1);
}

export function SettingsScreen({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const appInfo = useCommand("get_app_info", getAppInfo).data ?? null;
  const writeBack = useWriteBackPending();

  // Persistido em localStorage e refletido em <html data-motion> (src/lib/motion.ts).
  // Ligar FORÇA animações mesmo com o SO em movimento reduzido (escolha explícita).
  const [animacoes, setAnimacoes] = useState(() => motionEnabled());
  const [accent, setAccent] = useState<Accent>(() => getStoredAccent());
  const [reconnecting, setReconnecting] = useState(false);

  const isConnected = authStatus === "connected";

  /** Força um novo fluxo OAuth (token novo). Necessário quando o app reporta "conectado" mas o
   *  refresh token está morto (HTTP 400) — sem isso não há como refazer a autenticação. */
  function handleReconnect() {
    if (!GOOGLE_CLIENT_ID || reconnecting) return;
    setReconnecting(true);
    startOAuthFlow(GOOGLE_CLIENT_ID)
      .then(() => pollConnected(0))
      .then((status) => {
        onAuthChange(status);
        invalidateCommands();
      })
      .catch(() => undefined)
      .finally(() => setReconnecting(false));
  }

  return (
    <div className="xs">
      <div className="xs-title">Configurações e privacidade</div>

      {/* ── Planilha conectada ─────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Database size={16} strokeWidth={1.75} className="ic" />
            Planilha conectada
          </span>
        </div>
        <div className="cfg-sec">
          <CfgItem
            icon={Link}
            title="Google Sheets"
            sub={
              isConnected
                ? "Conectado — dados sincronizados com a sua planilha"
                : authStatus === "loading"
                  ? "Verificando conexão…"
                  : "Desconectado"
            }
            right={
              <Button
                size="sm"
                variant="ghost"
                onClick={() => handleReconnect()}
                disabled={reconnecting || !GOOGLE_CLIENT_ID}
              >
                {reconnecting ? "Reconectando…" : "Reconectar"}
              </Button>
            }
          />
          <CfgItem
            icon={Lock}
            title="Escrita só com aprovação"
            sub="Toda escrita na planilha exige prévia com diff e a sua confirmação"
            right={<span className="cfg-badge cfg-badge--local">Ativo</span>}
          />
        </div>
        <div className="card__body">
          <GoogleSheetsPanel authStatus={authStatus} onAuthChange={onAuthChange} />
        </div>
      </section>

      {/* ── Importar arquivo local ─────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <FileUp size={16} strokeWidth={1.75} className="ic" />
            Importar arquivo local
          </span>
        </div>
        <div className="cfg-sec">
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <FileUp size={17} strokeWidth={1.75} />
            </span>
            <div>
              <div className="cfg-item__t">Planilha .xlsx</div>
              <div className="cfg-item__s">
                Importa todas as abas, detectando o layout automaticamente. Linhas já
                importadas são ignoradas.
              </div>
            </div>
            <span className="cfg-item__r">
              <LocalXlsxImport />
            </span>
          </div>
        </div>
      </section>

      {/* ── Sincronização ──────────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <RefreshCw size={16} strokeWidth={1.75} className="ic" />
            Sincronização
          </span>
        </div>
        <div className="card__body">
          <ConflictGate onResolved={writeBack.refresh} />
          <WriteBackPending writeBack={writeBack} />
          {!writeBack.loading &&
            writeBack.pendingCount === 0 &&
            writeBack.conflictCount === 0 && (
              <p
                style={{
                  fontSize: "var(--fs-sm)",
                  color: "var(--text-muted)",
                  margin: 0,
                }}
              >
                Nenhuma alteração pendente de envio para a planilha.
              </p>
            )}
        </div>
      </section>

      {/* ── Bolsos ─────────────────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Landmark size={16} strokeWidth={1.75} className="ic" />
            Bolsos
          </span>
        </div>
        <div className="card__body">
          <PocketsCard />
          <PocketsManager />
        </div>
      </section>

      {/* ── Lembrete diário (desktop only) ────────────────────── */}
      <DailyReminderSection />

      {/* ── Teto do Diário (resumo + link para a tela própria) ── */}
      <TetoLinkSection />

      {/* ── Privacidade ────────────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Shield size={16} strokeWidth={1.75} className="ic" />
            Privacidade
          </span>
        </div>
        <div className="cfg-sec">
          <CfgItem
            icon={Lock}
            title="Tudo neste dispositivo"
            sub="Seus dados não saem do computador"
            right={<span className="cfg-badge cfg-badge--local">Local</span>}
          />
          {/* Fato, não configuração: não existe caminho de nuvem para ligar/desligar. */}
          <CfgItem
            icon={Sparkles}
            title="Mia responde localmente"
            sub="Sem enviar dados financeiros para a nuvem"
            right={<span className="cfg-badge cfg-badge--local">Local</span>}
          />
        </div>
      </section>

      {/* ── Aparência ──────────────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Settings size={16} strokeWidth={1.75} className="ic" />
            Aparência
          </span>
        </div>
        <div className="cfg-sec">
          {/* O sub é um diagnóstico vivo: expõe o que o motor do WebView reporta
              (movimento reduzido? View Transitions?) para depurar sem devtools. */}
          <CfgItem
            icon={Sparkles}
            title="Animações"
            sub={[
              systemPrefersReducedMotion()
                ? animacoes
                  ? "Forçando animações (o sistema pede movimento reduzido)"
                  : "Seguindo o movimento reduzido do sistema"
                : animacoes
                  ? "Transições e gráficos animados"
                  : "Desligadas neste dispositivo",
              ...(typeof document.startViewTransition !== "function"
                ? ["sem View Transitions"]
                : []),
            ].join(" · ")}
            right={
              <Toggle
                on={animacoes}
                onClick={() => {
                  const next = !animacoes;
                  setAnimacoes(next);
                  setMotionPreference(next ? "on" : "off");
                }}
                label="Animações"
              />
            }
          />
          <MotionDiagnostics />
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <Palette size={17} strokeWidth={1.75} />
            </span>
            <div className="cfg-item__grow">
              <div className="cfg-item__t">Cor de destaque</div>
              <div className="cfg-item__s">
                Pinta o chrome, os botões e a seleção. As cores de status do método —
                paz, atenção, dinheiro — não mudam com a paleta.
              </div>
              <div className="cfg-accents" role="group" aria-label="Cor de destaque">
                {ACCENTS.map((a) => (
                  <button
                    key={a.key}
                    type="button"
                    className={`cfg-accent ${accent === a.key ? "cfg-accent--on" : ""}`}
                    aria-pressed={accent === a.key}
                    onClick={() => {
                      setAccent(a.key);
                      applyAccent(a.key);
                    }}
                  >
                    <i style={{ background: a.swatch }} aria-hidden="true" />
                    <span>{a.label}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Seus dados ─────────────────────────────────────────── */}
      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <HardDrive size={16} strokeWidth={1.75} className="ic" />
            Seus dados
          </span>
        </div>
        <div className="cfg-sec">
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <HardDrive size={17} strokeWidth={1.75} />
            </span>
            <div>
              <div className="cfg-item__t">Onde ficam os dados</div>
              <div className="cfg-item__s">
                Banco SQLite em <code>{appInfo ? appInfo.db_path : "—"}</code>, somente
                neste dispositivo.
              </div>
            </div>
          </div>
          <DataBackupRow />
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <Shield size={17} strokeWidth={1.75} />
            </span>
            <div>
              <div className="cfg-item__t">Telemetria</div>
              <div className="cfg-item__s">
                O Neko não envia nenhum dado de uso. Suas finanças não saem da sua
                máquina.
              </div>
            </div>
          </div>
          <div className="cfg-item">
            <span className="cfg-item__ic">
              <Settings size={17} strokeWidth={1.75} />
            </span>
            <div>
              <div className="cfg-item__t">Versão</div>
              <div className="cfg-item__s">
                Neko Finance {appInfo ? `v${appInfo.version}` : "—"} · Tauri desktop
              </div>
            </div>
          </div>
        </div>
      </section>

      {!isTauri ? (
        <p style={{ fontSize: 12, color: "var(--text-faint)" }}>
          Preview web — abra o app desktop para ver seus dados reais.
        </p>
      ) : null}
    </div>
  );
}
