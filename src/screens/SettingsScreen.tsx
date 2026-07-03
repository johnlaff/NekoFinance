import "./config.css";
import { useEffect, useReducer, useState } from "react";
import {
  Bell,
  Database,
  FileUp,
  HardDrive,
  Landmark,
  Link,
  Lock,
  Plus,
  RefreshCw,
  Settings,
  Shield,
  Sparkles,
  X,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { PocketsCard } from "../features/pockets/PocketsCard";
import { PocketsManager } from "../features/pockets/PocketsManager";
import { ConflictGate } from "../features/reconcile/ConflictGate";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import { WriteBackPending } from "./dashboard/WriteBackPending";
import { useWriteBackPending } from "../hooks/useWriteBackPending";
import {
  backupDatabase,
  checkAuthStatus,
  getAppInfo,
  getAppSetting,
  getDailyBudgetCategories,
  GOOGLE_CLIENT_ID,
  isTauri,
  registerOsReminder,
  setAppSetting,
  startOAuthFlow,
  unregisterOsReminder,
  upsertDailyBudget,
  upsertDailyBudgetWithCategories,
  type DailyBudgetCategoryInput,
  type AuthStatus,
} from "../lib/api";
import { formatBRL, parseBRLToCents } from "../lib/format";
import {
  motionEnabled,
  setMotionPreference,
  systemPrefersReducedMotion,
} from "../lib/motion";
import { safeErrorMessage } from "../lib/errors";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { Button } from "../design-system/components/Button";
import { SegmentedControl } from "../design-system/components/SegmentedControl";

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

const TETO_CTL_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
};

const CAT_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
  marginBottom: "var(--space-2)",
};

const CAT_NAME_INPUT_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-body)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  padding: "4px 8px",
  height: "var(--hit-min)",
  flex: 1,
};

const CAT_AMOUNT_INPUT_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-money)",
  fontSize: "var(--fs-body)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  padding: "4px 8px",
  height: "var(--hit-min)",
  width: "12ch",
};

const CAT_SUMMARY_STYLE: React.CSSProperties = {
  margin: "var(--space-3) 0 0",
  fontSize: "var(--fs-sm)",
  color: "var(--text-muted)",
};

const CAT_WARN_STYLE: React.CSSProperties = {
  color: "var(--brass-400)",
};

const CAT_REMOVE_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  width: "var(--hit-min)",
  height: "var(--hit-min)",
  background: "transparent",
  border: "var(--bw-hair) solid var(--border)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text-muted)",
  cursor: "pointer",
  flex: "none",
};

const CAT_ACTIONS_STYLE: React.CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  alignItems: "center",
  marginTop: "var(--space-3)",
};

const CAT_LEGEND_STYLE: React.CSSProperties = {
  display: "block",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  marginBottom: "var(--space-2)",
  padding: 0,
};

const CAT_FIELDSET_STYLE: React.CSSProperties = {
  border: "none",
  margin: 0,
  padding: "var(--space-3) 0 0",
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
  title: string;
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
      anim.addEventListener("finish", () => {
        const dt = Math.round(performance.now() - t0);
        waapiDot.remove();
        results.push(
          dt < 900 ? `WAAPI anormal (${dt}ms; esperado ~1800ms)` : `WAAPI ok (${dt}ms)`,
        );
        finish();
      });
    } else {
      waapiDot.remove();
      results.push("WAAPI indisponível");
      finish();
    }

    // (b) Animação CSS com duração por TOKEN — tokens colapsados terminam cedo.
    const cssDot = mkDot(40, "nk-diag-dot");
    cssDot.addEventListener("animationend", () => {
      const dt = Math.round(performance.now() - t0);
      cssDot.remove();
      results.push(
        dt < 700 ? `CSS anormal (${dt}ms — tokens zerados?)` : `CSS ok (${dt}ms)`,
      );
      finish();
    });

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
      clipAnim.addEventListener("finish", () => {
        const dt = Math.round(performance.now() - t0);
        clipDot.remove();
        results.push(`clip-path terminou (${dt}ms) — viu a barra alargar?`);
        finish();
      });
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

  return (
    <CfgItem
      icon={Sparkles}
      title="Diagnóstico de animações"
      sub={verdict ?? facts}
      right={
        <Button variant="secondary" onClick={runTest} disabled={running}>
          {running ? "Testando…" : "Testar"}
        </Button>
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
                <strong role="alert" style={{ color: "var(--brass-400)" }}>
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
// DailyTetoCeilingSection — daily spend ceiling input
// ---------------------------------------------------------------------------

function DailyTetoCeilingSection() {
  const [raw, setRaw] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const val = await getAppSetting("daily_diario_ceiling_display");
        if (val) setRaw(val);
      } catch {
        // non-critical
      }
    })();
  }, []);

  async function handleSave() {
    const cents = parseBRLToCents(raw);
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
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <Settings size={16} strokeWidth={1.75} className="ic" />
          Teto do Diário
        </span>
      </div>
      <div className="cfg-sec">
        <div className="cfg-item">
          <span className="cfg-item__ic">
            <Settings size={17} strokeWidth={1.75} />
          </span>
          <div>
            <div className="cfg-item__t">Teto diário (R$)</div>
            <div className="cfg-item__s">
              Orienta o forecast dos dias futuros. Em branco = média do mês anterior.
              {saved ? <strong> Salvo.</strong> : null}
              {err ? (
                <strong role="alert" style={{ color: "var(--danger-400)" }}>
                  {" "}
                  {err}
                </strong>
              ) : null}
            </div>
          </div>
          <span className="cfg-item__r" style={TETO_CTL_STYLE}>
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
          </span>
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// DiarioCategorySection — per-category Diário budget breakdown
// ---------------------------------------------------------------------------

let catRowSeq = 0;
function nextCatKey(): string {
  catRowSeq += 1;
  return `cat-${catRowSeq}`;
}

interface CatDraft {
  key: string;
  name: string;
  amount: string;
}

interface DiarioCatState {
  total: string;
  rows: CatDraft[];
  loading: boolean;
  saving: boolean;
  saved: boolean;
  error: string | null;
}

type DiarioCatAction =
  | { type: "loaded"; total: string; rows: CatDraft[] }
  | { type: "setTotal"; value: string }
  | { type: "setName"; index: number; value: string }
  | { type: "setAmount"; index: number; value: string }
  | { type: "addRow" }
  | { type: "removeRow"; index: number }
  | { type: "saveStart" }
  | { type: "saveOk" }
  | { type: "saveErr"; error: string };

const DIARIO_CAT_INITIAL: DiarioCatState = {
  total: "",
  rows: [],
  loading: true,
  saving: false,
  saved: false,
  error: null,
};

function diarioCatReducer(s: DiarioCatState, a: DiarioCatAction): DiarioCatState {
  switch (a.type) {
    case "loaded":
      return { ...s, total: a.total, rows: a.rows, loading: false };
    case "setTotal":
      return { ...s, total: a.value, saved: false };
    case "setName":
      return {
        ...s,
        saved: false,
        rows: s.rows.map((r, i) => (i === a.index ? { ...r, name: a.value } : r)),
      };
    case "setAmount":
      return {
        ...s,
        saved: false,
        rows: s.rows.map((r, i) => (i === a.index ? { ...r, amount: a.value } : r)),
      };
    case "addRow":
      return {
        ...s,
        saved: false,
        rows: [...s.rows, { key: nextCatKey(), name: "", amount: "" }],
      };
    case "removeRow":
      return { ...s, saved: false, rows: s.rows.filter((_, i) => i !== a.index) };
    case "saveStart":
      return { ...s, saving: true, error: null, saved: false };
    case "saveOk":
      return { ...s, saving: false, saved: true };
    case "saveErr":
      return { ...s, saving: false, error: a.error };
  }
}

function centsToBRLInput(cents: number): string {
  return (cents / 100).toFixed(2).replace(".", ",");
}

const DIARIO_CAT_PLACEHOLDERS = [
  "Alimentação",
  "Transporte",
  "Farmácia",
  "Lazer",
  "Outros",
];

function DiarioCategorySection() {
  const [s, dispatch] = useReducer(diarioCatReducer, DIARIO_CAT_INITIAL);

  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const [cats, totalDisplay] = await Promise.all([
          getDailyBudgetCategories(),
          getAppSetting("daily_diario_ceiling_display"),
        ]);
        const rows: CatDraft[] = cats.map((c) => ({
          key: nextCatKey(),
          name: c.name,
          amount: centsToBRLInput(c.amount_cents),
        }));
        dispatch({ type: "loaded", total: totalDisplay ?? "", rows });
      } catch {
        dispatch({ type: "loaded", total: "", rows: [] });
      }
    })();
  }, []);

  const now = new Date();
  const daysInMonth = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();

  const totalCents = parseBRLToCents(s.total);
  const catSumCents = s.rows.reduce(
    (sum, r) => sum + (parseBRLToCents(r.amount) ?? 0),
    0,
  );
  const effectiveTotal =
    totalCents != null && totalCents > 0 ? totalCents : catSumCents;
  const dailyRate = daysInMonth > 0 ? Math.floor(effectiveTotal / daysInMonth) : 0;
  const mismatch =
    totalCents != null &&
    totalCents > 0 &&
    s.rows.length > 0 &&
    catSumCents !== totalCents;

  async function handleSave() {
    dispatch({ type: "saveStart" });
    const amountCents = effectiveTotal;
    const categories: DailyBudgetCategoryInput[] = [];
    for (let i = 0; i < s.rows.length; i++) {
      const r = s.rows[i]!;
      const cents = parseBRLToCents(r.amount);
      const name = r.name.trim();
      if (name === "" && (cents == null || cents <= 0)) continue;
      if (cents == null || cents <= 0) {
        dispatch({
          type: "saveErr",
          error: `Informe um valor válido para "${name || "categoria sem nome"}".`,
        });
        return;
      }
      categories.push({
        name: name || `Categoria ${i + 1}`,
        amount_cents: cents,
        position: categories.length,
      });
    }
    try {
      await upsertDailyBudgetWithCategories(amountCents, categories);
      await setAppSetting(
        "daily_diario_ceiling_display",
        amountCents > 0 ? centsToBRLInput(amountCents) : "",
      );
      dispatch({ type: "saveOk" });
    } catch (e) {
      dispatch({
        type: "saveErr",
        error: safeErrorMessage(e, "Não foi possível salvar as categorias."),
      });
    }
  }

  if (!isTauri || s.loading) return null;

  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <Settings size={16} strokeWidth={1.75} className="ic" />
          Categorias do Diário
        </span>
      </div>
      <div className="card__body">
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "flex-start",
            gap: 12,
            marginBottom: "var(--space-3)",
          }}
        >
          <div style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>
            Distribua o teto mensal entre categorias. Em branco = soma das categorias
            como teto.
          </div>
          <div style={TETO_CTL_STYLE}>
            <input
              type="text"
              inputMode="decimal"
              placeholder="ex.: 1.250,00"
              value={s.total}
              onChange={(e) =>
                dispatch({ type: "setTotal", value: e.currentTarget.value })
              }
              disabled={s.saving}
              style={TETO_INPUT_STYLE}
              aria-label="Teto mensal do Diário em reais"
            />
          </div>
        </div>

        <fieldset style={CAT_FIELDSET_STYLE}>
          <legend style={CAT_LEGEND_STYLE}>Categorias</legend>
          {s.rows.length === 0 ? (
            <p style={{ ...CAT_SUMMARY_STYLE, marginTop: 0 }}>
              Nenhuma categoria ainda. Adicione abaixo para acompanhar o gasto por
              categoria durante o mês.
            </p>
          ) : (
            s.rows.map((r, i) => (
              <div key={r.key} style={CAT_ROW_STYLE}>
                <input
                  type="text"
                  placeholder={
                    DIARIO_CAT_PLACEHOLDERS[i % DIARIO_CAT_PLACEHOLDERS.length]
                  }
                  value={r.name}
                  onChange={(e) =>
                    dispatch({
                      type: "setName",
                      index: i,
                      value: e.currentTarget.value,
                    })
                  }
                  disabled={s.saving}
                  style={CAT_NAME_INPUT_STYLE}
                  aria-label={`Nome da categoria ${i + 1}`}
                />
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder="ex.: 300,00"
                  value={r.amount}
                  onChange={(e) =>
                    dispatch({
                      type: "setAmount",
                      index: i,
                      value: e.currentTarget.value,
                    })
                  }
                  disabled={s.saving}
                  style={CAT_AMOUNT_INPUT_STYLE}
                  aria-label={`Valor mensal da categoria ${i + 1} em reais`}
                />
                <button
                  type="button"
                  onClick={() => dispatch({ type: "removeRow", index: i })}
                  disabled={s.saving}
                  style={CAT_REMOVE_BTN_STYLE}
                  aria-label={`Remover categoria ${i + 1}`}
                >
                  <X size={15} strokeWidth={1.75} />
                </button>
              </div>
            ))
          )}
        </fieldset>

        <p style={CAT_SUMMARY_STYLE}>
          Total {formatBRL(effectiveTotal)} — {formatBRL(dailyRate)}/dia ({daysInMonth}{" "}
          dias no mês atual).
          {mismatch ? (
            <strong role="status" style={CAT_WARN_STYLE}>
              {" "}
              A soma das categorias ({formatBRL(catSumCents)}) difere do teto informado.
            </strong>
          ) : null}
          {s.saved ? <strong> Salvo.</strong> : null}
          {s.error ? (
            <strong role="alert" style={{ color: "var(--danger-400)" }}>
              {" "}
              {s.error}
            </strong>
          ) : null}
        </p>

        <div style={CAT_ACTIONS_STYLE}>
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<Plus size={15} strokeWidth={2} />}
            onClick={() => dispatch({ type: "addRow" })}
            disabled={s.saving}
          >
            Adicionar categoria
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void handleSave()}
            disabled={s.saving}
          >
            {s.saving ? "Salvando…" : "Salvar categorias"}
          </Button>
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
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

      {/* ── Teto do Diário (desktop only) ─────────────────────── */}
      <DailyTetoCeilingSection />

      {/* ── Categorias do Diário (desktop only) ───────────────── */}
      <DiarioCategorySection />

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
