import "./config.css";
import { useEffect, useState } from "react";
import {
  Bell,
  CircleGauge,
  FileUp,
  Landmark,
  Link,
  Lock,
  MessagesSquare,
  Palette,
  Receipt,
  RefreshCw,
  Table2,
  Shield,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { PocketsManager } from "../features/pockets/PocketsManager";
import { ConflictGate } from "../features/reconcile/ConflictGate";
import { UpdateSettingsBlock } from "../features/updater/UpdateSettingsBlock";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import {
  connectGoogleCmd,
  fetchGoogleAuthStatus,
  type AuthStatus,
} from "../features/sheets/sheetsView";
import { WriteBackPending } from "./dashboard/WriteBackPending";
import { useWriteBackPending } from "../hooks/useWriteBackPending";
import { ACCENTS, applyAccent, getStoredAccent, type Accent } from "../lib/accent";
import { useNekoApp } from "../shell/appContext";
import { GOOGLE_CLIENT_ID, isAndroid, isTauri, SHOW_RECEIPT } from "../lib/env";
import {
  motionEnabled,
  setMotionPreference,
  systemPrefersReducedMotion,
} from "../lib/motion";
import { playThemeReveal, readMotionLog } from "../shell/themeReveal";
import { useThemeSwitch } from "../shell/ThemeToggle";
import { errorText, safeErrorMessage } from "../lib/errors";
import { syncRecencyLabel } from "../lib/syncRecency";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Money } from "../design-system/components/Money";
import { Switch } from "../design-system/components/Switch";
import { fetchDailyBudget } from "./tetoView";
import {
  backupDatabaseCmd,
  driveCheckinCmd,
  driveCheckinErrorMessage,
  driveCheckinLabel,
  CHECKIN_REFUSED_CONFLICT,
  driveCheckoutLabel,
  driveCheckoutOutcomeWarning,
  DRIVE_CHECKIN_UP_TO_DATE_NOTE,
  fetchAppInfo,
  fetchConfigSetting,
  fetchLastDriveCheckin,
  fetchLastSyncAt,
  fetchMiaConsent,
  fetchShowReceiptFlag,
  grantMiaConsentCmd,
  greetState,
  NEEDS_DRIVE_REAUTH,
  registerOsReminderCmd,
  revokeMiaConsentCmd,
  setConfigSetting,
  setMiaApiKeyCmd,
  unregisterOsReminderCmd,
  type MiaConsentView,
} from "./configView";
import { Line, SecHead } from "./configLine";
import { openSnapshotConflict } from "../features/snapshot-conflict/snapshotConflictStore";

// A promessa de privacidade é uma leitura do estado, não um texto fixo: com a conversa ligada, a
// linha da Mia deixaria de ser verdadeira se continuasse afirmando que nada sai do aparelho.
function PrivacySection({
  dbPath,
  miaLinked,
  miaOperator,
}: {
  dbPath: string | null;
  miaLinked: boolean;
  miaOperator: string;
}) {
  return (
    <section className="config__card" aria-labelledby="config-privacidade">
      <SecHead icon={Shield} id="config-privacidade" title="Privacidade" />
      <Line
        icon={Lock}
        title="Seus dados"
        sub="Guardados só neste aparelho — nada de uso é enviado."
        subExtra={
          <div className="config__path" title={dbPath ?? undefined}>
            <code>{dbPath ?? "—"}</code>
          </div>
        }
        right={<span className="config__pill">Local</span>}
      />
      <BackupLine />
      <Line
        title="A Mia"
        sub={
          miaLinked
            ? `Conversa aberta autorizada — suas perguntas e os lançamentos necessários podem ir para OpenRouter e ${miaOperator}.`
            : "Responde local. Nada sai deste aparelho."
        }
        right={<span className="config__pill">{miaLinked ? "Nuvem" : "Local"}</span>}
      />
      <Line
        title="Conta Google"
        sub="Token no chaveiro do sistema."
        right={<span className="config__pill">Local</span>}
      />
    </section>
  );
}

// O gesto que fala com o backend vive fora do componente porque o compilador do React não otimiza
// função com `finally` — e devolver o botão ao estado normal aconteça o que acontecer é
// justamente para o que o `finally` existe aqui.
async function consentGesture(
  gesture: () => Promise<MiaConsentView>,
  fallback: string,
  setBusy: (busy: boolean) => void,
  setError: (message: string | null) => void,
): Promise<MiaConsentView | null> {
  setBusy(true);
  setError(null);
  try {
    return await gesture();
  } catch (cause) {
    setError(safeErrorMessage(cause, fallback));
    return null;
  } finally {
    setBusy(false);
  }
}

function ConversationConsent({
  consent,
  loading,
  error,
  onConsentChange,
}: {
  consent: MiaConsentView | null;
  loading: boolean;
  error: unknown;
  onConsentChange: (consent: MiaConsentView) => void;
}) {
  const [open, setOpen] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [replaceKey, setReplaceKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  if (loading && !consent) {
    return (
      <section className="config__card" aria-labelledby="config-conversa">
        <SecHead icon={MessagesSquare} id="config-conversa" title="Conversa" />
        <EmptyState variant="skeleton" skeletonRows={3} />
      </section>
    );
  }

  if (error && !consent) {
    return (
      <section className="config__card" aria-labelledby="config-conversa">
        <SecHead icon={MessagesSquare} id="config-conversa" title="Conversa" />
        <EmptyState
          variant="error"
          title="Não foi possível ler a conversa"
          description="Tente abrir Configurações de novo para consultar o consentimento."
        />
      </section>
    );
  }

  if (!consent) return null;

  const operator = consent.text.processors[1]?.name ?? "o provedor";
  const linked = consent.linked;
  const needsRenewal = consent.needs_renewal;
  // O vocabulário é de AUTORIZAÇÃO, não de ligar: o que este gesto faz é abrir a porta, e a
  // conversa aberta passa a responder quando o app souber usá-la. Chamar isto de "ligada"
  // contradiria a própria conversa, que continua dizendo que ainda não está.
  const invitation = needsRenewal
    ? { sub: "O texto mudou — leia de novo para seguir autorizada.", label: "Rever" }
    : linked
      ? { sub: `Autorizada · OpenRouter e ${operator}`, label: "Revogar" }
      : consent.granted
        ? {
            sub: "Falta a chave — guarde a sua chave do provedor para autorizar.",
            label: "Continuar",
          }
        : consent.has_key
          ? {
              sub: "Falta o consentimento — leia o que sai do aparelho para autorizar.",
              label: "Continuar",
            }
          : {
              sub: "Sem autorização — a Mia responde só o que ela calcula aqui dentro.",
              label: "Autorizar",
            };

  const canRegister = consent.has_key || apiKey.trim().length > 0;

  async function register() {
    if (!canRegister || busy) return;
    const key = apiKey.trim();
    const consent = await consentGesture(
      async () => {
        if (key) await setMiaApiKeyCmd(key);
        return grantMiaConsentCmd();
      },
      "Não foi possível registrar o consentimento.",
      setBusy,
      setActionError,
    );
    if (!consent) return;
    onConsentChange(consent);
    invalidateCommands();
    setApiKey("");
    setReplaceKey(false);
  }

  async function revoke() {
    if (busy) return;
    const consent = await consentGesture(
      revokeMiaConsentCmd,
      "Não foi possível revogar a conversa.",
      setBusy,
      setActionError,
    );
    if (!consent) return;
    onConsentChange(consent);
    invalidateCommands();
    setOpen(false);
  }

  return (
    <section className="config__card" aria-labelledby="config-conversa">
      <SecHead icon={MessagesSquare} id="config-conversa" title="Conversa" />
      <ShowReceiptLine />
      <Line
        title="Conversa aberta"
        sub={invitation.sub}
        right={
          <Button
            variant={linked ? "ghost" : "primary"}
            size="sm"
            onClick={() => setOpen((current) => !current)}
            aria-expanded={open}
            aria-controls="config-conversa-porta"
          >
            {invitation.label}
          </Button>
        }
      />
      <div
        className="config__door"
        id="config-conversa-porta"
        data-open={open}
        inert={!open}
        role="region"
        aria-label="Consentimento da conversa"
      >
        <div className="config__doorin">
          <div className="config__consent">
            <h3>{consent.text.headline}</h3>
            {consent.text.paragraphs.map((paragraph) => (
              <p key={paragraph}>{paragraph}</p>
            ))}
            {/* Os dois grupos são de naturezas diferentes — um é fato, o outro é tarefa — e
                sem rótulo visível correriam como mais um bloco de texto do consentimento. */}
            <div className="config__consent-group">
              <h4 id="config-conversa-processadores">Quem processa</h4>
              <ul
                className="config__consent-list"
                aria-labelledby="config-conversa-processadores"
              >
                {consent.text.processors.map((processor) => (
                  <li key={processor.name}>
                    <strong>{processor.name}</strong>
                    <span>{processor.role}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="config__consent-group">
              <h4 id="config-conversa-optins">
                Antes de autorizar, na sua conta do provedor
              </h4>
              <ul
                className="config__consent-list"
                aria-labelledby="config-conversa-optins"
              >
                {consent.text.checklist.map((item) => (
                  <li key={item.title}>
                    <strong>{item.title}</strong>
                    <span>{item.detail}</span>
                  </li>
                ))}
              </ul>
            </div>
            {linked ? (
              <div className="config__consent-action">
                <p>
                  Revogar apaga o consentimento e a chave, e a conversa volta a
                  responder só o que ela calcula aqui dentro.
                </p>
                <Button variant="danger" onClick={() => void revoke()} disabled={busy}>
                  Revogar e apagar a chave
                </Button>
              </div>
            ) : (
              <div className="config__consent-action">
                {consent.has_key && !replaceKey ? (
                  <div className="config__consent-key-state">
                    <span>Chave guardada</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setReplaceKey(true)}
                    >
                      Trocar
                    </Button>
                  </div>
                ) : (
                  // A ajuda fica FORA do rótulo: dentro dele, o nome acessível do campo
                  // passaria a ser o rótulo somado à frase inteira da ajuda.
                  <div className="config__consent-key">
                    <label htmlFor="mia-api-key">Sua chave do provedor</label>
                    <input
                      id="mia-api-key"
                      type="password"
                      aria-describedby="mia-api-key-help"
                      value={apiKey}
                      onChange={(event) => setApiKey(event.target.value)}
                      autoComplete="off"
                    />
                    <small id="mia-api-key-help">
                      Fica no cofre do sistema. Você pode apagar quando quiser.
                    </small>
                  </div>
                )}
                <Button onClick={() => void register()} disabled={busy || !canRegister}>
                  Registrar consentimento
                </Button>
              </div>
            )}
            {actionError ? (
              <EmptyState
                variant="error"
                title="Não foi possível concluir"
                description={actionError}
              />
            ) : null}
          </div>
        </div>
      </div>
    </section>
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
  const [diagOpen, setDiagOpen] = useState(false);

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

  // Ferramenta de depuração, não didática: fica atrás de porta para o jargão
  // do motor (WAAPI, tokens, View Transitions) não ocupar a leitura padrão.
  return (
    <>
      <Line
        title="Diagnóstico de animações"
        sub="Teste se este dispositivo executa as animações do app."
        right={
          <button
            type="button"
            className="config__more"
            aria-expanded={diagOpen}
            aria-controls="config-motion-diag"
            onClick={() => setDiagOpen((o) => !o)}
          >
            {diagOpen ? "Fechar" : "Abrir"}
          </button>
        }
      />
      <div
        className="config__door"
        id="config-motion-diag"
        data-open={diagOpen}
        inert={!diagOpen}
        role="region"
        aria-label="Diagnóstico de animações"
      >
        <div className="config__doorin">
          <div className="config__diag">
            {/* aria-live: o resultado chega ~1,8s após o clique — o leitor de
                tela precisa ser avisado sem re-navegar até a linha. */}
            <p className="config__diag-verdict" aria-live="polite">
              {verdict ?? facts}
            </p>
            <span className="config__btns">
              <Button variant="ghost" size="sm" onClick={runTest} disabled={running}>
                {running ? "Testando…" : "Testar"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={runRevealTest}
                disabled={running}
              >
                Testar reveal
              </Button>
            </span>
          </div>
        </div>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Conta sempre à mostra — a preferência de exibição do recibo em todo o app
// ---------------------------------------------------------------------------

function ShowReceiptLine() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        setEnabled(await fetchShowReceiptFlag(SHOW_RECEIPT, true));
      } catch {
        setEnabled(true);
      }
    })();
  }, []);

  async function handleToggle(next: boolean) {
    setEnabled(next);
    setSaving(true);
    try {
      await setConfigSetting(SHOW_RECEIPT, next ? "true" : "false");
      setSaving(false);
    } catch {
      setSaving(false);
    }
  }

  if (!isTauri || enabled === null) return null;

  return (
    <Line
      icon={Receipt}
      title="Conta sempre à mostra"
      sub="Desligada, a tela traz só o resultado, e a conta abre sob demanda onde ela estiver."
      right={
        <Switch
          on={enabled}
          onChange={(next) => void handleToggle(next)}
          label="Conta sempre à mostra"
          disabled={saving}
        />
      }
    />
  );
}

// ---------------------------------------------------------------------------
// Rotina — lembrete diário (toggle + horário) e atalho do teto
// ---------------------------------------------------------------------------

function ReminderLines() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [time, setTime] = useState("20:00");
  const [saving, setSaving] = useState(false);
  const [osWarn, setOsWarn] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const [en, t] = await Promise.all([
          fetchConfigSetting("daily_reminder_enabled"),
          fetchConfigSetting("daily_reminder_time"),
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
      if (on) await registerOsReminderCmd(at);
      else await unregisterOsReminderCmd();
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

  async function handleToggle(next: boolean) {
    setEnabled(next);
    setSaving(true);
    // Reset espelhado nos dois caminhos (não `finally`: o React Compiler não
    // suporta finalizer e o componente perderia a memoização automática).
    try {
      await setConfigSetting("daily_reminder_enabled", next ? "true" : "false");
      await syncOsReminder(next, time);
      setSaving(false);
    } catch {
      // Persistência falhou; o estado local já reflete a escolha e o próximo load relê.
      setSaving(false);
    }
  }

  async function handleTimeChange(e: React.ChangeEvent<HTMLInputElement>) {
    const val = e.currentTarget.value;
    setTime(val);
    setSaving(true);
    try {
      await setConfigSetting("daily_reminder_time", val);
      if (enabled) await syncOsReminder(true, val);
      setSaving(false);
    } catch {
      // Persistência falhou; o horário local segue editável e o próximo load relê.
      setSaving(false);
    }
  }

  if (!isTauri || enabled === null) return null;

  return (
    <>
      <Line
        icon={Bell}
        title="Lembrete diário"
        sub={
          <>
            Notificação nativa no horário escolhido — no Windows, dispara mesmo com o
            app fechado.
            {osWarn ? (
              <strong role="alert" className="config__warn">
                {" "}
                {osWarn}
              </strong>
            ) : null}
          </>
        }
        right={
          <Switch
            on={enabled}
            onChange={(next) => void handleToggle(next)}
            label="Lembrete diário"
            disabled={saving}
          />
        }
      />
      {enabled ? (
        <Line
          title="Horário"
          sub="Hora local (24 h) para receber o aviso."
          right={
            <input
              type="time"
              className="config__time"
              value={time}
              onChange={(e) => void handleTimeChange(e)}
              disabled={saving}
              aria-label="Horário do lembrete diário"
            />
          }
        />
      ) : null}
    </>
  );
}

/** O teto edita-se em UMA fonte só: a tela "Teto do diário" (cerimônia por itens + divisor).
 * Aqui fica o resumo do estado atual e o caminho até lá. */
function TetoLine() {
  const { navigate } = useNekoApp();
  const budgetQ = useCommand("get_daily_budget", fetchDailyBudget);
  const budget = budgetQ.data;
  return (
    <Line
      icon={CircleGauge}
      title="Teto do diário"
      sub={
        budget == null ? (
          // Fetch ainda no ar: sem negativo fabricado ("sem teto") antes do dado chegar.
          "A cerimônia do gasto variável."
        ) : budget.per_day_cents > 0 ? (
          <>
            Teto estipulado: <Money cents={budget.per_day_cents} size="inherit" /> por
            dia.
          </>
        ) : (
          "Sem teto estipulado."
        )
      }
      right={
        <button
          type="button"
          className="config__more"
          aria-label="Abrir teto do diário"
          onClick={() => navigate("teto")}
        >
          Abrir →
        </button>
      }
    />
  );
}

// ---------------------------------------------------------------------------
// Privacidade — backup do banco
// ---------------------------------------------------------------------------

function BackupLine() {
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
      await backupDatabaseCmd(dest);
      setBusy(false);
      setMsg("Backup salvo.");
    } catch (e) {
      setBusy(false);
      setErr(safeErrorMessage(e, "Não foi possível fazer o backup."));
    }
  }

  return (
    <Line
      title="Backup do banco"
      sub={
        <>
          Salva uma cópia íntegra (.db) onde você escolher.{" "}
          {msg ? <strong>{msg}</strong> : null}
          {err ? (
            <strong role="alert" className="config__err">
              {err}
            </strong>
          ) : null}
        </>
      }
      right={
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void doBackup()}
          disabled={busy || !isTauri}
        >
          {busy ? "Salvando…" : "Fazer backup"}
        </Button>
      }
    />
  );
}

// ---------------------------------------------------------------------------
// Conexão — check-in do snapshot no Drive
// ---------------------------------------------------------------------------

function DriveCheckinLine({ onNeedsReauth }: { onNeedsReauth: () => void }) {
  const { data: info } = useCommand("last_drive_checkin", fetchLastDriveCheckin);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [needsReauth, setNeedsReauth] = useState(false);
  const checkoutWarning = driveCheckoutOutcomeWarning(info);

  async function doCheckin() {
    if (!GOOGLE_CLIENT_ID) return;
    setErr(null);
    setNote(null);
    setNeedsReauth(false);
    setBusy(true);
    try {
      const result = await driveCheckinCmd(GOOGLE_CLIENT_ID);
      setBusy(false);
      // "Em dia" é sucesso (ADR-0015): nada foi publicado, mas o dono merece saber que o
      // clique não falhou — só não tinha nada de novo para subir.
      if (!result.published) setNote(DRIVE_CHECKIN_UP_TO_DATE_NOTE);
      invalidateCommands();
    } catch (e) {
      setBusy(false);
      // Escopo `drive.appdata` faltando: leva ao fluxo de reconexão, nunca a um erro cru.
      if (errorText(e) === NEEDS_DRIVE_REAUTH) {
        setNeedsReauth(true);
      } else if (errorText(e) === CHECKIN_REFUSED_CONFLICT) {
        // Divergência dupla: nunca um erro de linha — abre a tela de conflito (ADR-0015) com a
        // lista de gestos deste aparelho antes de qualquer escolha.
        openSnapshotConflict();
      } else {
        setErr(driveCheckinErrorMessage(e));
      }
    }
  }

  return (
    <>
      <Line
        title="Snapshot no Drive"
        sub={
          <>
            {needsReauth
              ? "Reautorize o escopo do Drive para publicar o snapshot."
              : driveCheckinLabel(info)}{" "}
            {err ? (
              <strong role="alert" className="config__err">
                {err}
              </strong>
            ) : note ? (
              // "Em dia" é sucesso (ADR-0015), não erro: role="status" anuncia o clique ao leitor
              // de tela mesmo com o rótulo do botão inalterado; classe própria separa
              // visualmente da linha do rótulo em vez de herdar `.config__what-s` de dentro dela.
              <span role="status" className="config__note">
                {note}
              </span>
            ) : null}
          </>
        }
        right={
          needsReauth ? (
            <Button size="sm" variant="ghost" onClick={onNeedsReauth}>
              Reconectar
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void doCheckin()}
              disabled={busy || !isTauri || !GOOGLE_CLIENT_ID}
            >
              {busy ? "Publicando…" : "Fazer check-in"}
            </Button>
          )
        }
      />
      {/* Check-out roda sozinho ao abrir o app (ADR-0015) — sem botão, só o registro de quando
          e de qual aparelho veio a última leitura, mais o aviso de um desfecho que mereça a
          atenção do dono (recusa por schema mais novo, falha de rede/integridade). */}
      <Line
        title="Última leitura do Drive"
        sub={
          <>
            {driveCheckoutLabel(info)}{" "}
            {checkoutWarning ? (
              <strong role="alert" className="config__err">
                {checkoutWarning}
              </strong>
            ) : null}
          </>
        }
      />
    </>
  );
}

// ---------------------------------------------------------------------------
// Atualizações — mesma máquina de estados do convite calmo (updaterView)
// ---------------------------------------------------------------------------

function UpdatesSection() {
  return (
    <section className="config__card" aria-labelledby="config-atualizacoes">
      <SecHead icon={RefreshCw} id="config-atualizacoes" title="Atualizações" />
      <UpdateSettingsBlock />
    </section>
  );
}

// ---------------------------------------------------------------------------
// Main exported component
// ---------------------------------------------------------------------------

/** Após iniciar o OAuth, o token chega de forma assíncrona (consentimento no navegador). Sonda o
 *  status até conectar (≤ 2 min). Recursão com setTimeout evita await-dentro-de-loop. Module-scope:
 *  não usa estado local. */
async function pollConnected(attempt: number): Promise<AuthStatus> {
  if (attempt >= 60) return fetchGoogleAuthStatus();
  await new Promise((resolve) => setTimeout(resolve, 2000));
  const status = await fetchGoogleAuthStatus();
  return status === "connected" ? status : pollConnected(attempt + 1);
}

interface GoogleReconnect {
  reconnecting: boolean;
  /** Tentativas NESTE ciclo de vida do app (zera sozinho ao reabrir). */
  reconnectAttempts: number;
  handleReconnect: () => void;
}

/** Força um novo fluxo OAuth (token novo). Necessário quando o app reporta "conectado" mas o
 *  refresh token está morto (HTTP 400) — sem isso não há como refazer a autenticação. No Android,
 *  o plugin de deep link tem uma limitação conhecida (upstream tauri-apps/plugins-workspace#2397):
 *  um SEGUNDO deep link no mesmo processo pode não retornar ao app — `reconnectAttempts` deixa a
 *  tela avisar a mitigação (reiniciar o app) a partir da segunda tentativa, antes de frustrar. */
function useGoogleReconnect(
  onAuthChange: (status: AuthStatus) => void,
): GoogleReconnect {
  const [reconnecting, setReconnecting] = useState(false);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);

  function handleReconnect() {
    if (!GOOGLE_CLIENT_ID || reconnecting) return;
    setReconnecting(true);
    setReconnectAttempts((n) => n + 1);
    connectGoogleCmd(GOOGLE_CLIENT_ID)
      .then(() => pollConnected(0))
      .then((status) => {
        onAuthChange(status);
        invalidateCommands();
      })
      .catch(() => undefined)
      .finally(() => setReconnecting(false));
  }

  return { reconnecting, reconnectAttempts, handleReconnect };
}

export function SettingsScreen({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const appInfo = useCommand("get_app_info", fetchAppInfo).data ?? null;
  const writeBack = useWriteBackPending();
  const { data: lastSync } = useCommand("last_sync_at", fetchLastSyncAt);
  const miaConsentQ = useCommand("get_mia_consent", fetchMiaConsent);
  const { theme, toggleTheme } = useThemeSwitch();

  // Persistido em localStorage e refletido em <html data-motion> (src/lib/motion.ts).
  // Ligar FORÇA animações mesmo com o SO em movimento reduzido (escolha explícita).
  const [animacoes, setAnimacoes] = useState(() => motionEnabled());
  const [accent, setAccent] = useState<Accent>(() => getStoredAccent());
  const { reconnecting, reconnectAttempts, handleReconnect } =
    useGoogleReconnect(onAuthChange);
  const [manageOpen, setManageOpen] = useState(false);
  // A resposta do gesto vale até o backend ser relido — e nem um instante além. Ela guarda a
  // leitura que substituiu; quando o comando traz outra, o override caduca sozinho. Um override
  // permanente sobreviveria a uma revogação vinda de fora, e a tela seguiria dizendo "autorizada"
  // enquanto o backend já recusa.
  const [miaConsent, setMiaConsent] = useState<{
    view: MiaConsentView;
    replaced: MiaConsentView | undefined;
  } | null>(null);

  const visibleMiaConsent =
    miaConsent && miaConsent.replaced === miaConsentQ.data
      ? miaConsent.view
      : (miaConsentQ.data ?? null);
  const miaLinked = visibleMiaConsent?.linked === true;
  const miaOperator = visibleMiaConsent?.text.processors[1]?.name ?? "o provedor";

  const isConnected = authStatus === "connected";
  const greet = greetState(
    authStatus,
    writeBack.pendingCount,
    writeBack.conflictCount,
    isConnected ? syncRecencyLabel(lastSync) : null,
  );

  return (
    <div className="config">
      {/* ── Veredito: título de identidade + linha de estado viva ── */}
      <section className="config__greet" data-large-title>
        <h1>Tudo neste dispositivo</h1>
        <span className="config__state" data-tone={greet.tone}>
          <span className="config__state-dot" aria-hidden="true" />
          <b>{greet.headline}</b>
          {greet.detail ? <> · {greet.detail}</> : null}
        </span>
      </section>

      {/* ── Conexão ────────────────────────────────────────────── */}
      <section className="config__card" aria-labelledby="config-conexao">
        <SecHead
          icon={Table2}
          id="config-conexao"
          title="Conexão"
          action={
            <button
              type="button"
              className="config__more"
              aria-expanded={manageOpen}
              aria-controls="config-manage"
              onClick={() => setManageOpen((o) => !o)}
            >
              Gerenciar
            </button>
          }
        />
        <Line
          icon={Link}
          title="Google Sheets"
          sub={
            isConnected
              ? writeBack.sheetName
                ? `Conta conectada · Aba ${writeBack.sheetName}`
                : "Conta conectada"
              : authStatus === "loading"
                ? "Verificando conexão…"
                : authStatus === "expired"
                  ? "Sessão expirada — reconecte para sincronizar"
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
        <Line
          title="Escrita só com aprovação"
          sub="Nenhuma mudança na planilha sem seu OK."
          right={<span className="config__pill">Sempre</span>}
        />
        <DriveCheckinLine onNeedsReauth={handleReconnect} />
        {isAndroid && reconnectAttempts >= 1 && (
          <p role="status" className="config__note">
            Uma nova reconexão nesta sessão pode não retornar ao app sozinha — se a tela
            não avançar depois de aprovar no navegador, feche e reabra o Neko Finance.
          </p>
        )}
        <div className="config__panel">
          <ConflictGate onResolved={writeBack.refresh} />
          <WriteBackPending writeBack={writeBack} />
        </div>
        <div
          className="config__door"
          id="config-manage"
          data-open={manageOpen}
          inert={!manageOpen}
          role="region"
          aria-label="Gerenciar conexão"
        >
          <div className="config__doorin">
            <GoogleSheetsPanel authStatus={authStatus} onAuthChange={onAuthChange} />
            <Line
              icon={FileUp}
              title="Importar planilha .xlsx"
              sub="Importa todas as abas, detectando o layout automaticamente. Linhas já importadas são ignoradas."
              right={<LocalXlsxImport />}
            />
          </div>
        </div>
      </section>

      <PrivacySection
        dbPath={appInfo ? appInfo.db_path : null}
        miaLinked={miaLinked}
        miaOperator={miaOperator}
      />

      <ConversationConsent
        consent={visibleMiaConsent}
        loading={miaConsentQ.loading === true}
        error={miaConsentQ.error}
        onConsentChange={(view) => setMiaConsent({ view, replaced: miaConsentQ.data })}
      />

      {/* ── Bolsos ─────────────────────────────────────────────── */}
      <section className="config__card" aria-labelledby="config-bolsos">
        <SecHead icon={Landmark} id="config-bolsos" title="Bolsos" />
        <div className="config__panel">
          <PocketsCard />
          <PocketsManager />
        </div>
      </section>

      {/* ── Aparência ──────────────────────────────────────────── */}
      <section className="config__card" aria-labelledby="config-aparencia">
        <SecHead icon={Palette} id="config-aparencia" title="Aparência" />
        <Line
          title="Tema escuro"
          sub="Feito para uso noturno."
          right={
            <Switch
              on={theme === "dark"}
              onChange={(_next, event) => toggleTheme(event)}
              label="Tema escuro"
            />
          }
        />
        {/* O sub é um diagnóstico vivo: expõe o que o motor do WebView reporta
            (movimento reduzido? View Transitions?) para depurar sem devtools. */}
        <Line
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
            <Switch
              on={animacoes}
              onChange={(next) => {
                setAnimacoes(next);
                setMotionPreference(next ? "on" : "off");
              }}
              label="Animações"
            />
          }
        />
        <MotionDiagnostics />
        <div className="config__line config__line--block">
          <div className="config__what">
            <div className="config__what-t">
              Cor de destaque{" "}
              <InfoPopover
                term={{
                  title: "Cor de destaque",
                  body: "Pinta o chrome, os botões e a seleção. As cores de status do método — paz, atenção, dinheiro — não mudam com a paleta.",
                }}
                hideMarker
              >
                <span className="config__how">Como funciona?</span>
              </InfoPopover>
            </div>
            <div className="config__what-s">
              A cor que o app usa nos seus destaques.
            </div>
            {/* Radiogroup APG com roving tabindex: uma parada de Tab; setas
                percorrem e selecionam (padrão de radio de seleção imediata). */}
            <div
              className="config__accents"
              role="radiogroup"
              aria-label="Cor de destaque"
              onKeyDown={(e) => {
                const delta =
                  e.key === "ArrowRight" || e.key === "ArrowDown"
                    ? 1
                    : e.key === "ArrowLeft" || e.key === "ArrowUp"
                      ? -1
                      : 0;
                if (delta === 0) return;
                e.preventDefault();
                const idx = ACCENTS.findIndex((a) => a.key === accent);
                const next = ACCENTS[(idx + delta + ACCENTS.length) % ACCENTS.length];
                if (!next) return;
                setAccent(next.key);
                applyAccent(next.key);
                const group = e.currentTarget;
                requestAnimationFrame(() => {
                  group
                    .querySelector<HTMLButtonElement>('[aria-checked="true"]')
                    ?.focus();
                });
              }}
            >
              {ACCENTS.map((a) => (
                <button
                  key={a.key}
                  type="button"
                  role="radio"
                  className="config__swatch"
                  style={{ background: a.swatch }}
                  aria-checked={accent === a.key}
                  tabIndex={accent === a.key ? 0 : -1}
                  aria-label={a.label}
                  title={a.label}
                  onClick={() => {
                    setAccent(a.key);
                    applyAccent(a.key);
                  }}
                />
              ))}
            </div>
          </div>
        </div>
      </section>

      {/* ── Rotina ─────────────────────────────────────────────── */}
      <section className="config__card" aria-labelledby="config-rotina">
        <SecHead icon={Bell} id="config-rotina" title="Rotina" />
        <ReminderLines />
        <TetoLine />
      </section>

      {/* ── Atualizações ───────────────────────────────────────── */}
      {isTauri ? <UpdatesSection /> : null}

      {/* ── Rodapé quieto ──────────────────────────────────────── */}
      <p className="config__foot">
        Neko Finance {appInfo ? `v${appInfo.version}` : "—"} ·{" "}
        {isAndroid ? "Tauri Android" : "Tauri desktop"}
        {!isTauri ? (
          <>
            <br />
            Preview web — abra o app desktop para ver seus dados reais.
          </>
        ) : null}
      </p>
    </div>
  );
}
