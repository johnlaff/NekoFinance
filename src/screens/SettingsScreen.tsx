import { useEffect, useState } from "react";
import {
  Bell,
  FileUp,
  HardDrive,
  Landmark,
  Link2,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { PocketsManager } from "../features/pockets/PocketsManager";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import {
  backupDatabase,
  getAppInfo,
  getAppSetting,
  isTauri,
  registerOsReminder,
  setAppSetting,
  unregisterOsReminder,
  upsertDailyBudget,
  type AuthStatus,
} from "../lib/api";
import { parseBRLToCents } from "../lib/format";
import { safeErrorMessage } from "../lib/errors";
import { useCommand } from "../lib/useCommand";
import { Button } from "../design-system/components/Button";
import { SegmentedControl } from "../design-system/components/SegmentedControl";

// Estilo estático do campo de horário (React Compiler: nunca inline em JSX).
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

// Estilo estático do campo de teto diário (React Compiler: nunca inline em JSX).
const TETO_INPUT_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-money)",
  fontSize: "var(--fs-body)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  padding: "4px 8px",
  height: "var(--hit-min)",
  width: "10ch",
};

// Linha de controle do teto (input + botão lado a lado). Estática/hoistada.
const TETO_CTL_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
};

/**
 * Configurações do lembrete diário: liga/desliga e horário preferido.
 * Persiste em `app_setting` via os comandos existentes. Disponível só no shell
 * desktop (isTauri). Além do laço em-app (dispara com o app aberto), registra um
 * agendamento no nível do sistema (plano 039) para o aviso disparar mesmo com o
 * app fechado. O registro no sistema é melhor-esforço: o laço em-app é o fallback,
 * então uma falha ali apenas mostra um aviso, sem bloquear o salvamento.
 */
function DailyReminderSection() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [time, setTime] = useState("20:00");
  const [saving, setSaving] = useState(false);
  // Aviso não-bloqueante quando o agendamento no nível do sistema falha (ex.: plataforma
  // ainda sem suporte). O lembrete em-app continua funcionando como fallback.
  const [osWarn, setOsWarn] = useState<string | null>(null);

  // Carrega as configurações atuais na montagem. Uma falha de leitura mantém o
  // padrão (ligado, 20:00) em vez de quebrar a tela — leitura de KV não é crítica.
  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const [en, t] = await Promise.all([
          getAppSetting("daily_reminder_enabled"),
          getAppSetting("daily_reminder_time"),
        ]);
        setEnabled(en !== "false"); // ausente = ligado por padrão
        if (t) setTime(t);
      } catch {
        setEnabled(true); // assume o padrão e segue renderizando a seção
      }
    })();
  }, []);

  // Sincroniza o agendamento no nível do sistema com o estado atual. Melhor-esforço:
  // nunca lança; uma falha vira só um aviso (o laço em-app cobre o caso).
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
    // Liga → agenda no horário atual; desliga → remove o agendamento do sistema.
    await syncOsReminder(next, time);
    setSaving(false);
  }

  async function handleTimeChange(e: React.ChangeEvent<HTMLInputElement>) {
    const val = e.currentTarget.value;
    setTime(val);
    setSaving(true);
    await setAppSetting("daily_reminder_time", val);
    // Reagenda no novo horário (idempotente — sobrescreve a entrada existente).
    if (enabled) await syncOsReminder(true, val);
    setSaving(false);
  }

  if (!isTauri) return null;
  if (enabled === null) return null; // ainda carregando

  return (
    <Section
      icon={Bell}
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
              {osWarn ? (
                <strong role="alert" style={{ color: "var(--brass-400)" }}>
                  {" "}
                  {osWarn}
                </strong>
              ) : null}
            </div>
          </div>
          <div className="set-row__ctl">
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
          </div>
        </div>
        {enabled && (
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Horário</div>
              <div className="set-row__d">Hora local (24 h) para receber o aviso.</div>
            </div>
            <div className="set-row__ctl">
              <input
                type="time"
                value={time}
                onChange={(e) => void handleTimeChange(e)}
                disabled={saving}
                style={TIME_INPUT_STYLE}
                aria-label="Horário do lembrete diário"
              />
            </div>
          </div>
        )}
      </div>
    </Section>
  );
}

/**
 * Configura o teto de gasto Diário por dia (para quem gasta no variável, não só no crédito).
 * Persiste em `daily_budget` via `upsert_daily_budget`; quando zerado, o engine usa o
 * fallback de média do mês anterior — nenhum teto explícito. Um valor de exibição é guardado
 * em `app_setting` só para pré-preencher o campo no próximo mount.
 * Disponível somente no shell desktop (isTauri).
 */
function DailyTetoCeilingSection() {
  const [raw, setRaw] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // Carrega o teto exibido na montagem para pré-preencher o campo.
  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const val = await getAppSetting("daily_diario_ceiling_display");
        if (val) setRaw(val);
      } catch {
        // leitura não-crítica; ignora
      }
    })();
  }, []);

  // Sem `finally` de propósito: o React Compiler não otimiza componentes com try/finally.
  async function handleSave() {
    const cents = parseBRLToCents(raw);
    // String em branco vira 0 (desativa o teto). Valor não-parseável é rejeitado.
    const cleared = raw.trim() === "";
    if (!cleared && (cents == null || cents < 0)) {
      setErr(
        "Informe um valor válido (ex.: 50,00) ou deixe em branco para usar a média.",
      );
      return;
    }
    const amountCents = cleared ? 0 : (cents ?? 0);
    setSaving(true);
    setErr(null);
    setSaved(false);
    try {
      await upsertDailyBudget(amountCents);
      // Guarda o display para restaurar no próximo mount (vazio quando desativado).
      await setAppSetting("daily_diario_ceiling_display", amountCents > 0 ? raw : "");
      setSaving(false);
      setSaved(true);
    } catch (e) {
      setSaving(false);
      setErr(safeErrorMessage(e, "Não foi possível salvar o teto."));
    }
  }

  if (!isTauri) return null;

  return (
    <Section
      icon={SlidersHorizontal}
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
              {err ? (
                <strong role="alert" style={{ color: "var(--danger-400)" }}>
                  {" "}
                  {err}
                </strong>
              ) : null}
            </div>
          </div>
          <div className="set-row__ctl" style={TETO_CTL_STYLE}>
            <input
              type="text"
              inputMode="decimal"
              placeholder="ex.: 50,00"
              value={raw}
              onChange={(e) => {
                setRaw(e.currentTarget.value);
                setSaved(false);
              }}
              disabled={saving}
              style={TETO_INPUT_STYLE}
              aria-label="Teto diário em reais"
            />
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void handleSave()}
              disabled={saving}
            >
              {saving ? "Salvando…" : "Salvar"}
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}

/** Backup local do banco: escolhe o destino no save dialog nativo e grava via VACUUM INTO. */
function DataBackupRow() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  // Sem `finally` de propósito: o React Compiler não otimiza componentes com try/finally.
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
    if (!dest) return; // usuário cancelou
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
    <div className="set-row">
      <div className="set-row__main">
        <div className="set-row__t">Backup do banco</div>
        <div className="set-row__d">
          Salva uma cópia íntegra (.db) onde você escolher — leve para outro disco ou
          dispositivo. {msg ? <strong>{msg}</strong> : null}
          {err ? (
            <strong role="alert" style={{ color: "var(--danger-400)" }}>
              {err}
            </strong>
          ) : null}
        </div>
      </div>
      <div className="set-row__ctl">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void doBackup()}
          disabled={busy || !isTauri}
        >
          {busy ? "Salvando…" : "Fazer backup"}
        </Button>
      </div>
    </div>
  );
}

function Section({
  icon: Icon,
  title,
  sub,
  children,
}: {
  icon: LucideIcon;
  title: string;
  sub?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="set-sec__head">
        <h2 className="set-sec__title">
          <Icon size={17} strokeWidth={1.75} className="set-sec__ic" />
          {title}
        </h2>
        {sub ? <div className="set-sec__sub">{sub}</div> : null}
      </div>
      {children}
    </section>
  );
}

export function SettingsScreen({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const appInfo = useCommand("get_app_info", getAppInfo).data ?? null;

  return (
    <div className="set">
      <Section
        icon={Link2}
        title="Conexão Google Sheets"
        sub="O Neko lê sua planilha. Nada é escrito sem a sua aprovação."
      >
        <div className="set-panel set-panel--pad">
          <GoogleSheetsPanel authStatus={authStatus} onAuthChange={onAuthChange} />
        </div>
      </Section>

      <Section
        icon={FileUp}
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
              </div>
            </div>
            <div className="set-row__ctl">
              <LocalXlsxImport />
            </div>
          </div>
        </div>
      </Section>

      <Section
        icon={Landmark}
        title="Bolsos"
        sub="Conta, poupança, vale, previdência e FGTS: só dinheiro líquido entra no saldo projetado."
      >
        <PocketsManager />
      </Section>

      <DailyReminderSection />

      <DailyTetoCeilingSection />

      <Section
        icon={HardDrive}
        title="Seus dados"
        sub="O Neko é local-first: não existe conta Neko nem backend."
      >
        <div className="set-panel">
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Onde ficam os dados</div>
              <div className="set-row__d">
                Banco SQLite em <code>{appInfo ? appInfo.db_path : "—"}</code>, somente
                neste dispositivo.
              </div>
            </div>
          </div>
          <DataBackupRow />
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Telemetria</div>
              <div className="set-row__d">
                O Neko não envia nenhum dado de uso. Suas finanças não saem da sua
                máquina.
              </div>
            </div>
          </div>
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Versão</div>
              <div className="set-row__d">
                Neko Finance {appInfo ? `v${appInfo.version}` : "—"} · Tauri desktop
              </div>
            </div>
          </div>
        </div>
      </Section>
    </div>
  );
}
