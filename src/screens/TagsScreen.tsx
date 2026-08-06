import "./tags.css";
import { useEffect, useReducer, useRef, useState } from "react";
import {
  ChevronRight,
  Pencil,
  SlidersHorizontal,
  Tags as TagsIcon,
  Users,
} from "lucide-react";
import { isTauri } from "../lib/env";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { safeErrorMessage } from "../lib/errors";
import { syncRecencyLabel } from "../lib/syncRecency";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Meter } from "../design-system/components/Meter";
import { Money } from "../design-system/components/Money";
import { MonthNav } from "../design-system/components/MonthNav";
import { Switch } from "../design-system/components/Switch";
import { SR_ONLY } from "../design-system/srOnly";
import { setCrumb } from "../shell/crumbStore";
import { monthTitle } from "./lancamentosView";
import {
  RULER_LABEL,
  RULER_ORDER,
  createTagCmd,
  exceptionSummary,
  labelFraction,
  maxLabelTotal,
  personRow,
  pluralLancamentos,
  pluralPessoas,
  pluralRotulos,
  resolveHeadline,
  rulerEffect,
  rulerMeasures,
  rulerSwitchLabel,
  splitExceptionsAndLabels,
  tagsScreenCacheKey,
  tagsScreenFetcher,
  toggleTagRuler,
  updateTagCmd,
  verdictLabel,
  monthLabelLower,
  type RulerEffect,
  type RulerKey,
  type TagsHeadline,
  type TagsScreenTag,
  type TagsScreenThirdParty,
} from "./tagsView";

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

// ---------------------------------------------------------------------------
// Formulário de criar/editar tag (nome + emoji + cor) — a única capacidade que o
// protótipo não cobre; preservada da tela atual, gramática nova.
// ---------------------------------------------------------------------------

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
  | { type: "toggleNew" }
  | { type: "openEdit"; id: string; name: string; emoji: string; color: string }
  | { type: "close" }
  | { type: "setName"; value: string }
  | { type: "setEmoji"; value: string }
  | { type: "setColor"; value: string }
  | { type: "submitStart" }
  | { type: "submitSuccess" }
  | { type: "submitError"; error: string };

function formReducer(s: FormState, a: FormAction): FormState {
  switch (a.type) {
    case "toggleNew":
      return s.open && s.editingId === null
        ? { ...initialForm, color: s.color }
        : { ...initialForm, open: true, color: s.color };
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
    case "close":
      return { ...initialForm, color: s.color };
    case "setName":
      return { ...s, name: a.value, error: null };
    case "setEmoji":
      return { ...s, emoji: a.value };
    case "setColor":
      return { ...s, color: a.value };
    case "submitStart":
      return { ...s, saving: true, error: null };
    case "submitSuccess":
      return { ...initialForm, color: s.color };
    case "submitError":
      return { ...s, saving: false, error: a.error };
  }
}

function TagFormPanel({
  form,
  dispatch,
  onSubmit,
}: {
  form: FormState;
  dispatch: React.Dispatch<FormAction>;
  onSubmit: () => void;
}) {
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

  return (
    <div className="tags__form">
      <div className="tags__form-row">
        <input
          aria-label="Nome da tag"
          placeholder="Nome (ex.: Mercado, ! Pagar)"
          value={form.name}
          onChange={(e) => dispatch({ type: "setName", value: e.target.value })}
          className="tags__form-input"
        />
        <input
          aria-label="Emoji da tag"
          placeholder="Emoji"
          value={form.emoji}
          onChange={(e) => dispatch({ type: "setEmoji", value: e.target.value })}
          className="tags__form-input tags__form-input--emoji"
        />
      </div>
      <div role="radiogroup" aria-label="Cor da tag" className="tags__swatches">
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
            className={"tags__swatch" + (form.color === c.value ? " is-on" : "")}
            style={{ background: c.value }}
          />
        ))}
      </div>
      {form.error && (
        <p role="alert" className="tags__form-error">
          {form.error}
        </p>
      )}
      <div className="tags__form-actions">
        <Button
          size="sm"
          onClick={onSubmit}
          disabled={!form.name.trim() || form.saving}
        >
          {form.saving ? "Salvando…" : form.editingId ? "Salvar tag" : "Criar tag"}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => dispatch({ type: "close" })}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Painel de réguas — compartilhado por linhas de Exceções E de Movimentação por
// rótulo: é o mesmo painel que faz um rótulo virar exceção.
// ---------------------------------------------------------------------------

function EffectText({ effect }: { effect: RulerEffect }) {
  if (effect.kind === "text") return <>{effect.text}</>;
  return (
    <>
      <Money cents={effect.cents} size="inherit" />
      {effect.suffix}
    </>
  );
}

function RulerRow({
  ruler,
  tag,
  busy,
  onToggle,
}: {
  ruler: RulerKey;
  tag: TagsScreenTag;
  busy: boolean;
  onToggle: () => void;
}) {
  const on = tag.counts_in[ruler];
  const effect = rulerEffect(ruler, on, tag.effects);
  return (
    <div className="tags__rule">
      <div className="tags__rwhat">
        <b>{RULER_LABEL[ruler]}</b>
        <small className={"tags__rwhat-note" + (effect ? " is-off" : "")}>
          {rulerMeasures(ruler)}
          {effect ? (
            <>
              {" "}
              Fora — <EffectText effect={effect} />
            </>
          ) : null}
        </small>
      </div>
      <Switch
        on={on}
        onChange={() => onToggle()}
        label={rulerSwitchLabel(ruler, tag.name)}
        disabled={busy}
      />
    </div>
  );
}

function TagPanel({
  tag,
  busy,
  failed,
  onToggleRuler,
  onEdit,
  editing,
  form,
  dispatch,
  onSubmit,
}: {
  tag: TagsScreenTag;
  busy: boolean;
  failed: boolean;
  onToggleRuler: (ruler: RulerKey) => void;
  onEdit: () => void;
  editing: boolean;
  form: FormState;
  dispatch: React.Dispatch<FormAction>;
  onSubmit: () => void;
}) {
  return (
    <div className="tags__panel">
      {failed ? (
        <p role="alert" className="tags__toggle-error">
          Não foi possível salvar — o interruptor voltou ao estado real. Tente de novo.
        </p>
      ) : null}
      {RULER_ORDER.map((ruler) => (
        <RulerRow
          key={ruler}
          ruler={ruler}
          tag={tag}
          busy={busy}
          onToggle={() => onToggleRuler(ruler)}
        />
      ))}
      {editing ? (
        <TagFormPanel form={form} dispatch={dispatch} onSubmit={onSubmit} />
      ) : (
        <div className="tags__rowactions">
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<Pencil size={13} strokeWidth={1.75} />}
            onClick={onEdit}
          >
            Editar tag
            {/* Nome acessível distinto por linha (regra 17) — "Editar tag" sozinho repete
                em toda linha; o nome real, oculto visualmente, desambigua para o leitor de tela. */}
            <span style={SR_ONLY}> "{tag.name}"</span>
          </Button>
        </div>
      )}
    </div>
  );
}

function TagChip({ tag }: { tag: TagsScreenTag }) {
  const color = tag.color || "var(--cat-jade)";
  return (
    <span className="tags__chip" style={{ "--tc": color } as React.CSSProperties}>
      <span className="tags__tdot" aria-hidden="true" />
      {tag.emoji ? `${tag.emoji} ` : ""}
      {tag.name}
    </span>
  );
}

/** Contexto compartilhado por toda linha de Exceções/Rótulo — evita repetir a
 * mesma dúzia de props em cada card (o painel de réguas é o mesmo em ambos). */
interface TagRowCtx {
  /** Tag com escrita de réguas em voo: as 4 réguas DELA travam juntas — o UPDATE
   *  grava as quatro colunas de uma vez, então um segundo clique montado da base
   *  velha desfaria o primeiro em silêncio (lost update). */
  busyTagId: string | null;
  /** Tag cuja última escrita falhou — o painel dela mostra o aviso. */
  failedTagId: string | null;
  onToggleRuler: (tag: TagsScreenTag, ruler: RulerKey) => void;
  form: FormState;
  dispatch: React.Dispatch<FormAction>;
  onSubmit: () => void;
  onEdit: (tag: TagsScreenTag) => void;
}

function TagPanelFor({ tag, ctx }: { tag: TagsScreenTag; ctx: TagRowCtx }) {
  return (
    <TagPanel
      tag={tag}
      busy={ctx.busyTagId === tag.id}
      failed={ctx.failedTagId === tag.id}
      onToggleRuler={(r) => ctx.onToggleRuler(tag, r)}
      onEdit={() => ctx.onEdit(tag)}
      editing={ctx.form.open && ctx.form.editingId === tag.id}
      form={ctx.form}
      dispatch={ctx.dispatch}
      onSubmit={ctx.onSubmit}
    />
  );
}

// ---------------------------------------------------------------------------
// Veredito — os 4 estados de conteúdo (A/B/C/F); D e o esqueleto/erro vivem
// no componente principal, que decide qual seção deste grupo mostrar.
// ---------------------------------------------------------------------------

function VerdictSection({
  monthKey,
  monthLabel,
  headline,
  onCreateNew,
}: {
  monthKey: string;
  monthLabel: string;
  headline: TagsHeadline;
  onCreateNew: () => void;
}) {
  return (
    <section className="tags__verdict" data-large-title aria-live="polite">
      <p className="tags__vlabel">{verdictLabel(monthKey)}</p>
      {headline.kind === "exceptions" && (
        <>
          <h1>
            <Money cents={headline.costCents} size="inherit" /> em {monthLabel}
          </h1>
          <p>
            O custo de vida já deixa de fora{" "}
            <b>
              <Money cents={headline.excludedCents} size="inherit" />
            </b>
            .{" "}
            <span className="tags__cf">
              Sem as exceções, suas réguas contariam{" "}
              <Money cents={headline.allOnCents} size="inherit" />.
            </span>
          </p>
        </>
      )}
      {headline.kind === "third-party" && (
        <>
          <h1>Suas réguas contam dinheiro que não é seu.</h1>
          <p>
            <Money cents={headline.avgCents} size="inherit" /> por mês
            <EstimateMark
              term={{
                title: "Estimativa",
                body: "Média do que terceiros movimentaram pela sua conta nos últimos 12 meses completos + o mês corrente, sobre os meses com movimento detectado.",
              }}
            />
            , em média, é movimentação de {pluralPessoas(headline.peopleCount)} ao longo
            dos meses.
          </p>
          <Button className="tags__cta" onClick={onCreateNew}>
            Tirar isso das réguas
          </Button>
        </>
      )}
      {headline.kind === "clean" && (
        <>
          <h1>
            <Money cents={headline.costCents} size="inherit" /> em {monthLabel}
          </h1>
          <p>
            Nenhuma exceção declarada — e nada a declarar.{" "}
            <span className="tags__cf">Suas réguas veem só o que é seu.</span>
          </p>
        </>
      )}
      {headline.kind === "stale" && (
        <>
          <h1>
            <Money cents={headline.costCents} size="inherit" /> em {monthLabel}
          </h1>
          <p role="status" className="tags__stale">
            Não foi possível ler a planilha agora. Este número foi atualizado{" "}
            <b>{syncRecencyLabel(headline.staleAt) ?? "há pouco"}</b> — pode estar
            desatualizado.{" "}
            <button
              type="button"
              className="tags__relink"
              onClick={() => invalidateCommands()}
            >
              Tentar de novo
            </button>
          </p>
        </>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Dinheiro de terceiros — o app DETECTOU (some quando não há pessoa conhecida).
// ---------------------------------------------------------------------------

function ThirdPartiesCard({
  people,
  monthLabel,
}: {
  people: TagsScreenThirdParty[];
  monthLabel: string;
}) {
  if (people.length === 0) return null;
  return (
    <section className="tags__card" aria-labelledby="tags-terceiros">
      <div className="tags__sechead">
        <Users size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h2 id="tags-terceiros">Dinheiro de terceiros</h2>
        <span className="tags__note">Detectado no import</span>
      </div>
      {people.map((p) => {
        const row = personRow(p, monthLabel);
        return (
          <div className="tags__prow" key={row.personId}>
            <span className="tags__pav" aria-hidden="true">
              {row.initials}
            </span>
            <span className="tags__pwho">
              <b>{row.name}</b>
              <small>{row.detail}</small>
            </span>
            <span className="tags__pend">
              <span className={`tags__pval tags__pval--${p.state}`}>
                {row.value.kind === "money" ? (
                  <Money cents={row.value.cents} size="inherit" />
                ) : (
                  row.value.text
                )}
              </span>
              <span className="tags__page">{row.tail}</span>
            </span>
          </div>
        );
      })}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Exceções — você DECLAROU. Botão "Nova tag" no sec-head; garantia do saldo
// uma vez no rodapé do card (nunca por tag).
// ---------------------------------------------------------------------------

function ExceptionsCard({
  exceptions,
  ctx,
  formOpenForNew,
  onToggleNew,
}: {
  exceptions: TagsScreenTag[];
  ctx: TagRowCtx;
  formOpenForNew: boolean;
  onToggleNew: () => void;
}) {
  return (
    <section className="tags__card" aria-labelledby="tags-excecoes">
      <div className="tags__sechead">
        <SlidersHorizontal
          size={16}
          strokeWidth={1.75}
          className="ic"
          aria-hidden="true"
        />
        <h2 id="tags-excecoes">Exceções</h2>
        <button
          type="button"
          className="tags__more"
          aria-expanded={formOpenForNew}
          onClick={onToggleNew}
        >
          {formOpenForNew ? "Cancelar" : "Nova tag"}
        </button>
      </div>
      {formOpenForNew && (
        <TagFormPanel form={ctx.form} dispatch={ctx.dispatch} onSubmit={ctx.onSubmit} />
      )}
      {exceptions.length === 0 ? (
        <p className="tags__exempty">
          Nenhuma tag foge das réguas — todas contam em Performance, Custo de vida,
          Economia e Diário médio.
        </p>
      ) : (
        exceptions.map((tag) => (
          <details className="tags__ex" key={tag.id}>
            <summary className="tags__exsum">
              <TagChip tag={tag} />
              <span className="tags__exmeta">
                {exceptionSummary(tag.counts_in)} · {pluralLancamentos(tag.txn_count)}
              </span>
              <span className="tags__exval">
                <Money cents={tag.month_total_cents} size="inherit" />
              </span>
              <ChevronRight className="tags__chev" size={15} aria-hidden="true" />
            </summary>
            <TagPanelFor tag={tag} ctx={ctx} />
          </details>
        ))
      )}
      <p className="tags__guar">
        <b>O saldo da conta sempre conta</b> — o dinheiro entrou e saiu de verdade.{" "}
        <InfoPopover
          term={{
            title: "Como os interruptores contam",
            body: "Os interruptores só mudam o que as réguas enxergam — nunca o Saldo. O valor à direita é o que cada tag movimentou no mês; na conta da manchete só entra o que sai do custo de vida.",
          }}
        >
          Como funciona?
        </InfoPopover>
      </p>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Movimentação por rótulo — consequência, atrás de disclosure; mesmo painel de
// interruptores é o caminho de um rótulo virar exceção.
// ---------------------------------------------------------------------------

/** No desktop o fold nasce aberto — a coluna larga merece o ranking à vista
 *  (densidade por ambiente); no polegar continua fechado, consequência no fim.
 *  Segue colapsável nos dois casos. */
function foldStartsOpen(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(min-width: 901px)").matches
  );
}

function LabelsFold({
  labels,
  monthLabel,
  ctx,
}: {
  labels: TagsScreenTag[];
  monthLabel: string;
  ctx: TagRowCtx;
}) {
  // Decisão de montagem: o valor não muda por render (viewport estável) e o
  // <details> segue não-controlado — o toggle manual do usuário persiste.
  const [startsOpen] = useState(foldStartsOpen);
  if (labels.length === 0) return null;
  const maxLabel = maxLabelTotal(labels);
  return (
    <section className="tags__card">
      <details className="tags__fold" open={startsOpen || undefined}>
        <summary>
          <TagsIcon size={16} strokeWidth={1.75} aria-hidden="true" />
          <b>Movimentação por rótulo</b>
          <span className="tags__fc">
            {pluralRotulos(labels.length)} · {monthLabel}
          </span>
          <ChevronRight className="tags__chev" size={15} aria-hidden="true" />
        </summary>
        {labels.map((tag) => (
          <details className="tags__lbl-row" key={tag.id}>
            <summary className="tags__lblsum">
              <TagChip tag={tag} />
              <Meter
                className="tags__lbar"
                fraction={labelFraction(tag, maxLabel)}
                color="var(--text-faint)"
                trackColor="var(--surface-2)"
              />
              <span className="tags__lval">
                <Money cents={tag.month_total_cents} size="inherit" />
              </span>
              <span className="tags__lcount">{pluralLancamentos(tag.txn_count)}</span>
              <ChevronRight className="tags__chev" size={15} aria-hidden="true" />
            </summary>
            <TagPanelFor tag={tag} ctx={ctx} />
          </details>
        ))}
      </details>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Componente principal — orquestra fetch/estado; a leitura vive nas seções.
// ---------------------------------------------------------------------------

export function TagsScreen() {
  const now = new Date();
  const todayYm = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  const [ym, setYm] = useState(todayYm);
  const [year, month] = ym.split("-").map(Number) as [number, number];
  const crumbLabel = monthTitle(ym);

  const {
    data: dto,
    loading,
    error,
  } = useCommand(tagsScreenCacheKey(ym), tagsScreenFetcher(year, month));

  const [form, dispatch] = useReducer(formReducer, initialForm);
  // Tag com escrita de réguas em voo — as 4 réguas dela travam juntas até o refetch
  // (o UPDATE grava as quatro colunas; base velha num 2º clique desfaria o 1º).
  const [busyTagId, setBusyTagId] = useState<string | null>(null);
  // Tag cuja última escrita FALHOU — o aviso fica no painel dela até o próximo gesto.
  const [failedTagId, setFailedTagId] = useState<string | null>(null);

  // O crumb da appbar acompanha o mês visto; ao sair da tela, volta ao padrão. `setCrumb`
  // é função de módulo (identidade fixa) — o efeito só re-dispara quando o rótulo muda.
  useEffect(() => {
    setCrumb("tags", crumbLabel);
    return () => setCrumb("tags", null);
  }, [crumbLabel]);

  function handleToggleRuler(tag: TagsScreenTag, ruler: RulerKey) {
    setBusyTagId(tag.id);
    setFailedTagId(null);
    toggleTagRuler(tag, ruler)
      .then(() => invalidateCommands())
      .catch(() => {
        // O refetch reflete o estado real; o usuário fica sabendo que o gesto não pegou.
        setFailedTagId(tag.id);
      })
      .finally(() => setBusyTagId((t) => (t === tag.id ? null : t)));
  }

  async function submitForm() {
    const trimmed = form.name.trim();
    if (!trimmed || form.saving) return;
    dispatch({ type: "submitStart" });
    try {
      if (form.editingId) {
        await updateTagCmd(
          form.editingId,
          trimmed,
          form.color,
          form.emoji.trim() || null,
        );
      } else {
        // Convenção do método: nomes começando com "!" (ex.: "! Pagar") ficam fixados no topo.
        await createTagCmd(
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

  // Web-preview fallback (mesmo padrão de Lançamentos): sem Tauri não há dado real, e
  // `useCommand` nunca sai de `loading:false`/`data:undefined` para buscar.
  if (!isTauri) {
    return (
      <div className="tags">
        <EmptyState
          variant="empty"
          title="Preview web"
          description="Abra o app desktop para ver suas tags."
        />
      </div>
    );
  }

  const monthNav = (
    <MonthNav
      className="tags__nav"
      label={crumbLabel}
      onPrev={() => setYm((v) => shiftYm(v, -1))}
      onNext={() => setYm((v) => shiftYm(v, 1))}
      onToday={() => setYm(todayYm)}
      atToday={ym === todayYm}
      prevLabel="Mês anterior"
      nextLabel="Próximo mês"
    />
  );

  let content: React.ReactNode;
  if (loading && !dto) {
    content = (
      <div className="tags__verdict">
        <p className="tags__vlabel">{verdictLabel(ym)}</p>
        <EmptyState variant="skeleton" skeletonRows={6} />
      </div>
    );
  } else if (error && !dto) {
    content = (
      <EmptyState
        variant="error"
        title="Não foi possível carregar as tags"
        description="Confira a conexão e tente de novo."
        action={
          <Button size="sm" variant="ghost" onClick={() => invalidateCommands()}>
            Tentar novamente
          </Button>
        }
      />
    );
  } else if (dto) {
    // Manchete F (stale) = a leitura ATUAL falhou com um DTO em cache: o número fica,
    // com a idade da última sincronização.
    const headline = resolveHeadline(dto, error != null);
    const monthLabel = monthLabelLower(dto.month);

    if (headline.kind === "empty-tags") {
      content = (
        <div className="tags__void" data-large-title>
          <h1>Tags não são categorias.</h1>
          <p>
            Categoria diz onde você gastou — e este método não decide nada a partir
            disso. Tag tem dois usos: tirar das réguas o dinheiro que não é seu, e
            marcar o que você quer encontrar depois — assinaturas para cancelar,
            reembolsos da empresa.
          </p>
          <Button
            variant="ghost"
            aria-expanded={form.open}
            onClick={() => dispatch({ type: "toggleNew" })}
          >
            Criar primeira tag
          </Button>
          {form.open && (
            <TagFormPanel
              form={form}
              dispatch={dispatch}
              onSubmit={() => void submitForm()}
            />
          )}
        </div>
      );
    } else {
      const { exceptions, labels } = splitExceptionsAndLabels(dto.tags);
      const ctx: TagRowCtx = {
        busyTagId,
        failedTagId,
        onToggleRuler: handleToggleRuler,
        form,
        dispatch,
        onSubmit: () => void submitForm(),
        onEdit: (tag) =>
          dispatch({
            type: "openEdit",
            id: tag.id,
            name: tag.name,
            emoji: tag.emoji ?? "",
            color: tag.color || "var(--cat-jade)",
          }),
      };

      content = (
        <>
          <VerdictSection
            monthKey={dto.month}
            monthLabel={monthLabel}
            headline={headline}
            onCreateNew={() => dispatch({ type: "toggleNew" })}
          />
          {/* Duas colunas independentes no desktop (terceiros | exceções + rótulos);
              no mobile os wrappers dissolvem (display: contents) e a pilha segue o
              DOM — que é sempre a ordem de leitura. */}
          <div className="tags__grid">
            <div className="tags__col">
              <ThirdPartiesCard people={dto.third_parties} monthLabel={monthLabel} />
            </div>
            <div className="tags__col">
              <ExceptionsCard
                exceptions={exceptions}
                ctx={ctx}
                formOpenForNew={form.open && form.editingId === null}
                onToggleNew={() => dispatch({ type: "toggleNew" })}
              />
              <LabelsFold labels={labels} monthLabel={monthLabel} ctx={ctx} />
            </div>
          </div>
        </>
      );
    }
  }

  return (
    <div className="tags">
      {monthNav}
      {content}
    </div>
  );
}
