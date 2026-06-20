import { useReducer, useRef, useState, type CSSProperties } from "react";
import { createTag, tagTotalsForMonth, updateTagExclude } from "../lib/api";
import { monthNamePtBR } from "../lib/format";
import { safeErrorMessage } from "../lib/errors";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { MonthNav } from "../design-system/components/MonthNav";

/** Próximo/anterior "YYYY-MM". */
function shiftYm(ym: string, delta: number): string {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(Date.UTC(y!, m! - 1 + delta, 1));
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

// Cor + nome humano (o aria-label precisa de um nome legível, não da string do token CSS).
const PALETTE: { value: string; name: string }[] = [
  { value: "var(--cat-jade)", name: "Verde" },
  { value: "var(--cat-sky)", name: "Azul" },
  { value: "var(--cat-orchid)", name: "Orquídea" },
  { value: "var(--cat-violet)", name: "Violeta" },
  { value: "var(--cat-teal)", name: "Turquesa" },
  { value: "var(--cat-amber)", name: "Âmbar" },
  { value: "var(--cat-coral)", name: "Coral" },
];

// Estado do formulário "Nova tag" agrupado num reducer (uma atualização lógica = um render), em vez
// de seis useState relacionados.
interface FormState {
  open: boolean;
  name: string;
  emoji: string;
  color: string;
  saving: boolean;
  error: string | null;
}

const initialForm: FormState = {
  open: false,
  name: "",
  emoji: "",
  color: PALETTE[0]!.value,
  saving: false,
  error: null,
};

type FormAction =
  | { type: "toggle" }
  | { type: "setName"; value: string }
  | { type: "setEmoji"; value: string }
  | { type: "setColor"; value: string }
  | { type: "submitStart" }
  | { type: "submitSuccess" }
  | { type: "submitError"; error: string };

function formReducer(s: FormState, a: FormAction): FormState {
  switch (a.type) {
    case "toggle":
      return { ...s, open: !s.open, error: null };
    case "setName":
      return { ...s, name: a.value, error: null };
    case "setEmoji":
      return { ...s, emoji: a.value };
    case "setColor":
      return { ...s, color: a.value };
    case "submitStart":
      return { ...s, saving: true, error: null };
    case "submitSuccess":
      return { ...s, name: "", emoji: "", open: false, saving: false };
    case "submitError":
      return { ...s, saving: false, error: a.error };
  }
}

const FORM_PANEL_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-4)",
  padding: "var(--space-6)",
  marginBottom: "var(--space-6)",
  background: "var(--surface)",
  border: "var(--bw-hair) solid var(--border)",
  borderRadius: "var(--radius-md)",
};

// Botão-switch "Ignorar nos cálculos" por tag. Dois estilos estáticos (incluído/ignorado) hasteados
// p/ o React Compiler — só a escolha entre eles depende da linha.
const TOGGLE_BASE_STYLE: CSSProperties = {
  padding: "var(--space-1) var(--space-2)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  fontSize: "var(--fs-xs)",
  fontFamily: "var(--font-sans)",
  cursor: "pointer",
  flexShrink: 0,
};
const TOGGLE_INCLUDED_STYLE: CSSProperties = {
  ...TOGGLE_BASE_STYLE,
  background: "transparent",
  color: "var(--text)",
};
const TOGGLE_EXCLUDED_STYLE: CSSProperties = {
  ...TOGGLE_BASE_STYLE,
  background: "var(--surface-2)",
  color: "var(--text-muted)",
};

export function TagsScreen() {
  const now = new Date();
  const todayYm = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  const [ym, setYm] = useState(todayYm);
  const [year, month] = ym.split("-").map(Number);
  const [reload, setReload] = useState(0);
  const [form, dispatch] = useReducer(formReducer, initialForm);
  // Padrão WAI-ARIA radiogroup: roving tabindex (só o selecionado é tabbable) + setas navegam.
  const swatchRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const onSwatchKey = (e: React.KeyboardEvent, i: number) => {
    const last = PALETTE.length - 1;
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = i === last ? 0 : i + 1;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp")
      next = i === 0 ? last : i - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    if (next === null) return;
    e.preventDefault();
    dispatch({ type: "setColor", value: PALETTE[next]!.value });
    swatchRefs.current[next]?.focus();
  };

  const totalsQ = useCommand(`tag_totals:${ym}:${reload}`, () =>
    tagTotalsForMonth(year!, month!),
  );
  const tags = totalsQ.data ?? [];

  async function submit() {
    const trimmed = form.name.trim();
    if (!trimmed || form.saving) return;
    dispatch({ type: "submitStart" });
    try {
      await createTag(
        trimmed,
        form.color,
        form.emoji.trim() || null,
        trimmed.startsWith("!"),
      );
      invalidateCommands();
      dispatch({ type: "submitSuccess" });
      setReload((r) => r + 1);
    } catch (e) {
      dispatch({
        type: "submitError",
        error: safeErrorMessage(e, "Não foi possível criar a tag. Tente novamente."),
      });
    }
  }

  async function toggleExclude(tagId: string, currentValue: boolean) {
    try {
      await updateTagExclude(tagId, !currentValue);
      invalidateCommands();
      setReload((r) => r + 1);
    } catch {
      // Silencioso — alternar é best-effort; o próximo reload reflete o estado real.
    }
  }

  return (
    <div style={{ maxWidth: 720, margin: "0 auto", padding: "var(--space-2)" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-4)",
          marginBottom: "var(--space-6)",
        }}
      >
        <div>
          <h1
            style={{
              fontSize: "var(--fs-h2)",
              fontWeight: "var(--fw-bold)",
              letterSpacing: "var(--ls-tight)",
              margin: 0,
            }}
          >
            Tags
          </h1>
          <p
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-1) 0 0",
            }}
          >
            Totais de {monthNamePtBR(`${ym}-01`)} de {year}. Tags são diagnóstico — para
            onde foi o dinheiro, não orçamento; "! Pagar" e similares ficam no topo.
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}>
          <MonthNav
            label={`${monthNamePtBR(`${ym}-01`)} de ${year}`}
            onPrev={() => setYm((v) => shiftYm(v, -1))}
            onNext={() => setYm((v) => shiftYm(v, 1))}
            onToday={() => setYm(todayYm)}
            atToday={ym === todayYm}
            prevLabel="Mês anterior"
            nextLabel="Próximo mês"
          />
          <Button onClick={() => dispatch({ type: "toggle" })}>
            {form.open ? "Cancelar" : "Nova tag"}
          </Button>
        </div>
      </header>

      {form.open ? (
        <div style={FORM_PANEL_STYLE}>
          <div style={{ display: "flex", gap: "var(--space-3)", flexWrap: "wrap" }}>
            <input
              aria-label="Nome da tag"
              placeholder="Nome (ex.: Categoria demo, ! Pagar)"
              value={form.name}
              onChange={(e) => dispatch({ type: "setName", value: e.target.value })}
              style={inputStyle}
            />
            <input
              aria-label="Emoji da tag"
              placeholder="Emoji"
              value={form.emoji}
              onChange={(e) => dispatch({ type: "setEmoji", value: e.target.value })}
              style={{ ...inputStyle, width: 80 }}
            />
          </div>
          <div
            role="radiogroup"
            aria-label="Cor da tag"
            style={{ display: "flex", gap: "var(--space-2)" }}
          >
            {PALETTE.map((c, i) => (
              <button
                key={c.value}
                ref={(el) => {
                  swatchRefs.current[i] = el;
                }}
                type="button"
                role="radio"
                aria-checked={form.color === c.value}
                aria-label={c.name}
                tabIndex={form.color === c.value ? 0 : -1}
                onClick={() => dispatch({ type: "setColor", value: c.value })}
                onKeyDown={(e) => onSwatchKey(e, i)}
                style={{
                  width: 24,
                  height: 24,
                  borderRadius: "50%",
                  background: c.value,
                  border:
                    form.color === c.value
                      ? "2px solid var(--text)"
                      : "2px solid transparent",
                  cursor: "pointer",
                }}
              />
            ))}
          </div>
          {form.error ? (
            <p
              role="alert"
              style={{
                color: "var(--danger-400)",
                fontSize: "var(--fs-sm)",
                margin: 0,
              }}
            >
              {form.error}
            </p>
          ) : null}
          <div>
            <Button
              onClick={() => void submit()}
              disabled={!form.name.trim() || form.saving}
            >
              {form.saving ? "Criando…" : "Criar tag"}
            </Button>
          </div>
        </div>
      ) : null}

      {totalsQ.loading ? (
        <EmptyState variant="skeleton" skeletonRows={6} />
      ) : tags.length === 0 ? (
        <EmptyState
          title="Nenhuma tag ainda"
          description='Crie tags livres (com emoji e cor) para marcar lançamentos, como "! Pagar", "Categoria demo A", "Categoria demo B".'
        />
      ) : (
        <ul
          style={{
            listStyle: "none",
            margin: 0,
            padding: 0,
            display: "flex",
            flexDirection: "column",
            gap: "2px",
          }}
        >
          {tags.map((t) => (
            <li
              key={t.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
                padding: "var(--space-4) var(--space-3)",
                borderBottom: "var(--bw-hair) solid var(--border)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 14,
                  height: 22,
                  borderRadius: "3px 6px 6px 3px",
                  background: t.color,
                  flexShrink: 0,
                }}
              />
              {t.emoji ? <span aria-hidden="true">{t.emoji}</span> : null}
              <span
                style={{
                  flex: 1,
                  fontWeight: t.is_special ? "var(--fw-bold)" : "var(--fw-semibold)",
                  color: t.exclude_from_totals ? "var(--text-muted)" : "var(--text)",
                }}
              >
                {t.name}
              </span>
              <Money cents={t.total_cents} size="sm" />
              <button
                type="button"
                role="switch"
                aria-checked={t.exclude_from_totals}
                aria-label={
                  t.exclude_from_totals
                    ? `Incluir "${t.name}" nos cálculos`
                    : `Ignorar "${t.name}" nos cálculos`
                }
                onClick={() => void toggleExclude(t.id, t.exclude_from_totals)}
                style={
                  t.exclude_from_totals ? TOGGLE_EXCLUDED_STYLE : TOGGLE_INCLUDED_STYLE
                }
              >
                {t.exclude_from_totals ? "ignorado" : "incluído"}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 160,
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--surface-2)",
  color: "var(--text)",
  fontSize: "var(--fs-body)",
  fontFamily: "var(--font-sans)",
};
