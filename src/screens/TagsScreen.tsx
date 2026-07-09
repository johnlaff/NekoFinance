import "./tags.css";
import { useReducer, useRef, useState } from "react";
import { Tags, Sparkles, Plus, Pencil } from "lucide-react";
import {
  createTag,
  updateTag,
  tagTotalsForMonth,
  updateTagExclude,
  isTauri,
} from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { MES } from "../lib/nkFormat";
import { safeErrorMessage } from "../lib/errors";
import { MonthNav } from "../design-system/components/MonthNav";
import { Button } from "../design-system/components/Button";
import { Money } from "../design-system/components/Money";

/** Paleta de cores da tag (tokens do DS). O nome humano vira o aria-label do swatch. */
const PALETTE: { value: string; name: string }[] = [
  { value: "var(--cat-jade)", name: "Verde" },
  { value: "var(--cat-sky)", name: "Azul" },
  { value: "var(--cat-orchid)", name: "Orquídea" },
  { value: "var(--cat-violet)", name: "Violeta" },
  { value: "var(--cat-teal)", name: "Turquesa" },
  { value: "var(--cat-amber)", name: "Âmbar" },
  { value: "var(--cat-coral)", name: "Coral" },
];

/** Próximo/anterior "YYYY-MM" (delta em meses). */
function shiftYm(ym: string, delta: number): string {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(Date.UTC(y!, m! - 1 + delta, 1));
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

// Estado do formulário de tag (criar OU editar) num reducer (uma atualização lógica = um
// render), em vez de vários useState relacionados. `editingId` distingue os dois modos.
interface FormState {
  open: boolean;
  editingId: string | null;
  name: string;
  emoji: string;
  color: string;
  saving: boolean;
  error: string | null;
}

const initialForm: FormState = {
  open: false,
  editingId: null,
  name: "",
  emoji: "",
  color: PALETTE[0]!.value,
  saving: false,
  error: null,
};

type FormAction =
  | { type: "toggle" }
  | { type: "openEdit"; id: string; name: string; emoji: string; color: string }
  | { type: "setName"; value: string }
  | { type: "setEmoji"; value: string }
  | { type: "setColor"; value: string }
  | { type: "submitStart" }
  | { type: "submitSuccess" }
  | { type: "submitError"; error: string };

function toggleExclude(tagId: string, currentValue: boolean) {
  void updateTagExclude(tagId, !currentValue)
    .then(() => {
      invalidateCommands();
    })
    .catch(() => {
      // Silencioso — alternar é best-effort; o próximo refetch reflete o estado real.
    });
}

function formReducer(s: FormState, a: FormAction): FormState {
  switch (a.type) {
    case "toggle":
      return s.open
        ? { ...initialForm, color: s.color }
        : { ...s, open: true, editingId: null, error: null };
    case "openEdit":
      return {
        open: true,
        editingId: a.id,
        name: a.name,
        emoji: a.emoji,
        color: a.color,
        saving: false,
        error: null,
      };
    case "setName":
      return { ...s, name: a.value, error: null };
    case "setEmoji":
      return { ...s, emoji: a.value };
    case "setColor":
      return { ...s, color: a.value };
    case "submitStart":
      return { ...s, saving: true, error: null };
    case "submitSuccess":
      return { ...s, name: "", emoji: "", open: false, editingId: null, saving: false };
    case "submitError":
      return { ...s, saving: false, error: a.error };
  }
}

export function TagsScreen() {
  const now = new Date();
  const todayYm = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  const [ym, setYm] = useState(todayYm);
  const [year, month] = ym.split("-").map(Number);
  const monthIndex = month! - 1; // 0-based for MES[]
  const [form, dispatch] = useReducer(formReducer, initialForm);

  // Padrão WAI-ARIA radiogroup: roving tabindex (só o selecionado é tabbable) + setas navegam.
  const swatchRefs = useRef<(HTMLButtonElement | null)[]>([]);
  function onSwatchKey(e: React.KeyboardEvent, i: number) {
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
  }

  // Chave inclui o ym (navegar de mês refetcha); escritas refetcham via invalidateCommands(),
  // que notifica os hooks montados — sem key artificial de reload.
  const key = `tag_totals:${ym}`;
  const totalsQ = useCommand(key, () => tagTotalsForMonth(year!, month!));

  // Ordena por gasto decrescente (maior primeiro), como a lista de barras espera.
  const sorted = (totalsQ.data ?? [])
    .slice()
    .sort((a, b) => b.total_cents - a.total_cents);

  const max = sorted.length > 0 ? sorted[0]!.total_cents : 1;
  const grand = sorted.reduce((s, t) => s + t.total_cents, 0);

  async function submit() {
    const trimmed = form.name.trim();
    if (!trimmed || form.saving) return;
    dispatch({ type: "submitStart" });
    try {
      if (form.editingId) {
        // Renomear/recolorir; is_special re-deriva da convenção "!" no backend.
        await updateTag(form.editingId, trimmed, form.color, form.emoji.trim() || null);
      } else {
        // Convenção do método: nomes começando com "!" (ex.: "! Pagar") ficam fixados no topo.
        await createTag(
          trimmed,
          form.color,
          form.emoji.trim() || null,
          trimmed.startsWith("!"),
        );
      }
      invalidateCommands();
      dispatch({ type: "submitSuccess" });
    } catch (e) {
      dispatch({
        type: "submitError",
        error: safeErrorMessage(e, "Não foi possível salvar a tag. Tente novamente."),
      });
    }
  }

  return (
    <div className="xs">
      <div className="xs-head">
        <div className="xs-title">Tags · {MES[monthIndex]}</div>
        <div className="xs-head__actions">
          <MonthNav
            label={`${MES[monthIndex]} de ${year}`}
            onPrev={() => setYm((v) => shiftYm(v, -1))}
            onNext={() => setYm((v) => shiftYm(v, 1))}
            onToday={() => setYm(todayYm)}
            atToday={ym === todayYm}
            prevLabel="Mês anterior"
            nextLabel="Próximo mês"
          />
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<Plus size={13} strokeWidth={2} />}
            onClick={() => dispatch({ type: "toggle" })}
          >
            {form.open ? "Cancelar" : "Nova tag"}
          </Button>
        </div>
      </div>

      {form.open && (
        <section className="card tg-form">
          <div className="card__body">
            <div className="tg-form__row">
              <input
                aria-label="Nome da tag"
                placeholder="Nome (ex.: Mercado, ! Pagar)"
                value={form.name}
                onChange={(e) => dispatch({ type: "setName", value: e.target.value })}
                className="tg-form__input"
              />
              <input
                aria-label="Emoji da tag"
                placeholder="Emoji"
                value={form.emoji}
                onChange={(e) => dispatch({ type: "setEmoji", value: e.target.value })}
                className="tg-form__input tg-form__input--emoji"
              />
            </div>
            <div
              role="radiogroup"
              aria-label="Cor da tag"
              className="tg-form__swatches"
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
                  className={"tg-swatch" + (form.color === c.value ? " is-on" : "")}
                  style={{ background: c.value }}
                />
              ))}
            </div>
            {form.error && (
              <p role="alert" className="tg-form__error">
                {form.error}
              </p>
            )}
            <div>
              <Button
                size="sm"
                onClick={() => void submit()}
                disabled={!form.name.trim() || form.saving}
              >
                {form.saving
                  ? "Salvando…"
                  : form.editingId
                    ? "Salvar tag"
                    : "Criar tag"}
              </Button>
            </div>
          </div>
        </section>
      )}

      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Tags size={16} strokeWidth={1.75} className="ic" />
            Gasto por tag
          </span>
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontSize: 12.5,
              color: "var(--text-faint)",
            }}
          >
            Total <Money cents={grand} size="inherit" />
          </span>
        </div>

        <div className="card__body">
          {totalsQ.loading ? (
            /* Loading skeleton — quiet, no flash */
            <p style={{ color: "var(--text-faint)", fontSize: 13 }}>Carregando…</p>
          ) : sorted.length === 0 ? (
            <p style={{ color: "var(--text-faint)", fontSize: 13 }}>
              {isTauri
                ? 'Nenhuma tag ainda. Crie a primeira com "Nova tag".'
                : "Preview web — abra o app desktop para ver seus dados."}
            </p>
          ) : (
            sorted.map((tag) => {
              const color = tag.color || "var(--cat-jade)";
              const pct = max > 0 ? (tag.total_cents / max) * 100 : 0;
              return (
                <div
                  className={"tg-row" + (tag.exclude_from_totals ? " is-excluded" : "")}
                  key={tag.id}
                >
                  <span
                    className="tg-dot"
                    style={{ background: color }}
                    aria-hidden="true"
                  />
                  <span className="tg-name" title={tag.name}>
                    {tag.emoji ? `${tag.emoji} ` : ""}
                    {tag.name}
                  </span>
                  <span className="tg-track" aria-hidden="true">
                    <span
                      className="tg-fill"
                      style={{ width: `${pct}%`, background: color }}
                    />
                  </span>
                  <span className="tg-amt">
                    <Money cents={tag.total_cents} size="inherit" />
                  </span>
                  <button
                    type="button"
                    aria-label={`Editar tag "${tag.name}"`}
                    className="tg-edit"
                    onClick={() =>
                      dispatch({
                        type: "openEdit",
                        id: tag.id,
                        name: tag.name,
                        emoji: tag.emoji ?? "",
                        color: tag.color || "var(--cat-jade)",
                      })
                    }
                  >
                    <Pencil size={13} strokeWidth={1.75} aria-hidden="true" />
                  </button>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={tag.exclude_from_totals}
                    aria-label={
                      tag.exclude_from_totals
                        ? `Incluir "${tag.name}" nos cálculos`
                        : `Ignorar "${tag.name}" nos cálculos`
                    }
                    className={
                      "tg-toggle" + (tag.exclude_from_totals ? " is-excluded" : "")
                    }
                    onClick={() => toggleExclude(tag.id, tag.exclude_from_totals)}
                  >
                    {tag.exclude_from_totals ? "Ignorada" : "Incluída"}
                  </button>
                </div>
              );
            })
          )}
        </div>
      </section>

      <p className="tg-hint">
        <Sparkles size={14} strokeWidth={1.75} />
        {/* Prosa num único filho: .tg-hint é flex (ícone + texto) e nós de texto soltos
            viravam colunas separadas em volta do <em>. */}
        <span>
          Tags classificam um lançamento. Uma saída pode ter várias, e a Mia sugere tags
          ao importar. Marcar uma tag como <em>ignorada</em> tira seus lançamentos dos
          totais e projeções — o Saldo continua contando.
        </span>
      </p>
    </div>
  );
}
