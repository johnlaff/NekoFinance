/* Neko Finance — Tags screen (new).
   "Rótulos do mês" — lista de tags com totais mensais, controle de exclusão dos cálculos,
   e painel de criação de nova tag com paleta de cores e emoji.
   PT-BR copy · R$ em mono tabular · zero dependências externas.
   Expõe window.TagsScreen. */

const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Button, MonthNav, Money, EmptyState } = NS;
const Icon = window.Icon;

/* ---- CSS (once-only) ---- */
(function injectTagsCSS() {
  if (document.getElementById("tags-css")) return;
  const s = document.createElement("style");
  s.id = "tags-css";
  s.textContent = `
/* Layout principal */
.tags { max-width: 720px; margin: 0 auto; padding: var(--space-2); }

/* Cabeçalho */
.tags-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  margin-bottom: var(--space-6);
  flex-wrap: wrap;
}
.tags-header__lead { display: flex; flex-direction: column; gap: var(--space-1); }
.tags-header__title {
  font-size: var(--fs-h2);
  font-weight: var(--fw-bold);
  letter-spacing: var(--ls-tight);
  margin: 0;
  color: var(--text-strong);
}
.tags-header__sub {
  color: var(--text-muted);
  font-size: var(--fs-sm);
  margin: 0;
  line-height: var(--lh-normal);
  max-width: 460px;
}
.tags-header__controls {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-shrink: 0;
}

/* Painel de nova tag */
.tags-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-6);
  margin-bottom: var(--space-6);
  background: var(--surface);
  border: var(--bw-hair) solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-1);
}
.tags-form__row {
  display: flex;
  gap: var(--space-3);
  flex-wrap: wrap;
}
.tags-form__input {
  flex: 1;
  min-width: 160px;
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-sm);
  border: var(--bw-hair) solid var(--border-input);
  background: var(--surface-2);
  color: var(--text);
  font-size: var(--fs-body);
  font-family: var(--font-sans);
  outline: none;
}
.tags-form__input:focus {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--primary-quiet);
}
.tags-form__input--emoji {
  flex: 0 0 80px;
  min-width: 0;
}
.tags-form__palette-label {
  font-size: var(--fs-micro);
  font-weight: var(--fw-medium);
  color: var(--text-faint);
  letter-spacing: var(--ls-label);
  text-transform: uppercase;
  margin-bottom: var(--space-2);
}
.tags-form__palette {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}
.tags-form__swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  cursor: pointer;
  border: 2px solid transparent;
  flex-shrink: 0;
  transition: var(--t-hover), transform var(--dur-fast) var(--ease-entrance);
}
.tags-form__swatch:focus-visible {
  outline: 2px solid var(--border-focus);
  outline-offset: 2px;
}
.tags-form__swatch--selected {
  border-color: var(--text);
  transform: scale(1.15);
}
@media (prefers-reduced-motion: reduce) {
  .tags-form__swatch { transition: none; transform: none !important; }
}
.tags-form__hint {
  font-size: var(--fs-micro);
  color: var(--text-faint);
  margin: 0;
  line-height: var(--lh-normal);
}
.tags-form__error {
  font-size: var(--fs-sm);
  color: var(--danger-400);
  margin: 0;
}

/* Lista de tags */
.tags-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.tags-list__item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-3);
  border-bottom: var(--bw-hair) solid var(--border);
  transition: background var(--dur-fast) var(--ease-standard);
}
.tags-list__item:hover {
  background: var(--surface-hover);
}
@media (prefers-reduced-motion: reduce) {
  .tags-list__item { transition: none; }
}
.tags-list__chip {
  width: 14px;
  height: 22px;
  border-radius: 3px 6px 6px 3px;
  flex-shrink: 0;
}
.tags-list__emoji {
  font-size: var(--fs-body);
  line-height: 1;
  flex-shrink: 0;
}
.tags-list__name {
  flex: 1;
  font-size: var(--fs-sm);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tags-list__name--special {
  font-weight: var(--fw-bold);
  color: var(--text);
}
.tags-list__name--normal {
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.tags-list__name--excluded {
  color: var(--text-muted);
}
.tags-list__total {
  flex-shrink: 0;
}
.tags-list__toggle {
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  border: var(--bw-hair) solid var(--border);
  font-size: var(--fs-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  flex-shrink: 0;
  transition: var(--t-hover);
}
.tags-list__toggle--included {
  background: transparent;
  color: var(--text);
}
.tags-list__toggle--excluded {
  background: var(--surface-2);
  color: var(--text-muted);
}
.tags-list__toggle:focus-visible {
  outline: 2px solid var(--border-focus);
  outline-offset: 2px;
}

/* Rodapé sumário */
.tags-summary {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-5);
  margin-top: var(--space-6);
  padding-top: var(--space-4);
  border-top: var(--bw-hair) solid var(--border-strong);
  flex-wrap: wrap;
}
.tags-summary__label {
  font-size: var(--fs-sm);
  color: var(--text-muted);
}
.tags-summary__total {
  font-family: var(--font-money);
  font-variant-numeric: tabular-nums;
  font-size: var(--fs-money-md);
  font-weight: var(--fw-semibold);
  color: var(--text);
}
.tags-summary__excluded-note {
  font-size: var(--fs-micro);
  color: var(--text-faint);
  margin: 0;
  margin-top: var(--space-1);
}
`;
  document.head.appendChild(s);
})();

/* ---- dados de demo representativos ---- */
const PALETTE = [
  { value: "var(--cat-jade)", name: "Verde", hex: "#3fbf8f" },
  { value: "var(--cat-sky)", name: "Azul", hex: "#5fa8dc" },
  { value: "var(--cat-orchid)", name: "Orquídea", hex: "#c98bd4" },
  { value: "var(--cat-violet)", name: "Violeta", hex: "#8c8ae6" },
  { value: "var(--cat-teal)", name: "Turquesa", hex: "#5fc9c0" },
  { value: "var(--cat-amber)", name: "Âmbar", hex: "#ddb061" },
  { value: "var(--cat-coral)", name: "Coral", hex: "#e68a84" },
];

const DEMO_TAGS = [
  {
    id: "1",
    name: "! Pagar",
    emoji: "",
    color: "var(--cat-coral)",
    is_special: true,
    exclude_from_totals: false,
    total_cents: -284500,
  },
  {
    id: "2",
    name: "! Fatura cartão",
    emoji: "",
    color: "var(--cat-violet)",
    is_special: true,
    exclude_from_totals: false,
    total_cents: -142000,
  },
  {
    id: "3",
    name: "Mercado",
    emoji: "🛒",
    color: "var(--cat-jade)",
    is_special: false,
    exclude_from_totals: false,
    total_cents: -73200,
  },
  {
    id: "4",
    name: "Alimentação fora",
    emoji: "🍽",
    color: "var(--cat-amber)",
    is_special: false,
    exclude_from_totals: false,
    total_cents: -38900,
  },
  {
    id: "5",
    name: "Transporte",
    emoji: "🚌",
    color: "var(--cat-sky)",
    is_special: false,
    exclude_from_totals: false,
    total_cents: -24100,
  },
  {
    id: "6",
    name: "Assinaturas",
    emoji: "📦",
    color: "var(--cat-teal)",
    is_special: false,
    exclude_from_totals: false,
    total_cents: -18700,
  },
  {
    id: "7",
    name: "Saúde",
    emoji: "🩺",
    color: "var(--cat-orchid)",
    is_special: false,
    exclude_from_totals: false,
    total_cents: -9800,
  },
  {
    id: "8",
    name: "Reembolso empresa",
    emoji: "",
    color: "var(--cat-jade)",
    is_special: false,
    exclude_from_totals: true,
    total_cents: -45000,
  },
];

function fmtBRL(cents) {
  const abs = Math.abs(cents);
  const n = (abs / 100).toLocaleString("pt-BR", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return "R$ " + n;
}

function shiftYm(ym, delta) {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(Date.UTC(y, m - 1 + delta, 1));
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

const MONTH_NAMES = [
  "Janeiro",
  "Fevereiro",
  "Março",
  "Abril",
  "Maio",
  "Junho",
  "Julho",
  "Agosto",
  "Setembro",
  "Outubro",
  "Novembro",
  "Dezembro",
];
function monthLabel(ym) {
  const [y, m] = ym.split("-").map(Number);
  return `${MONTH_NAMES[m - 1]} de ${y}`;
}

/* ---- Painel de nova tag ---- */
function NewTagForm({ onCancel }) {
  const [name, setName] = React.useState("");
  const [emoji, setEmoji] = React.useState("");
  const [color, setColor] = React.useState(PALETTE[0].value);
  const [saving, setSaving] = React.useState(false);

  const swatchRefs = React.useRef([]);

  function handleSwatchKey(e, i) {
    const last = PALETTE.length - 1;
    let next = null;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = i === last ? 0 : i + 1;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp")
      next = i === 0 ? last : i - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    if (next === null) return;
    e.preventDefault();
    setColor(PALETTE[next].value);
    swatchRefs.current[next]?.focus();
  }

  function handleSubmit() {
    if (!name.trim() || saving) return;
    setSaving(true);
    // Simula criação (demo estático)
    setTimeout(() => {
      setSaving(false);
      onCancel();
    }, 600);
  }

  return (
    <div className="tags-form" role="region" aria-label="Nova tag">
      <div className="tags-form__row">
        <input
          aria-label="Nome da tag"
          placeholder="Nome (ex.: Mercado, ! Pagar)"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="tags-form__input"
          autoFocus
        />
        <input
          aria-label="Emoji da tag"
          placeholder="Emoji"
          value={emoji}
          onChange={(e) => setEmoji(e.target.value)}
          className="tags-form__input tags-form__input--emoji"
          maxLength={4}
        />
      </div>

      <div>
        <p className="tags-form__palette-label" id="palette-label">
          Cor da tag
        </p>
        <div
          className="tags-form__palette"
          role="radiogroup"
          aria-labelledby="palette-label"
        >
          {PALETTE.map((c, i) => (
            <button
              key={c.value}
              ref={(el) => {
                swatchRefs.current[i] = el;
              }}
              type="button"
              role="radio"
              aria-checked={color === c.value}
              aria-label={c.name}
              tabIndex={color === c.value ? 0 : -1}
              onClick={() => setColor(c.value)}
              onKeyDown={(e) => handleSwatchKey(e, i)}
              className={`tags-form__swatch${color === c.value ? " tags-form__swatch--selected" : ""}`}
              style={{ background: c.value }}
            />
          ))}
        </div>
      </div>

      <p className="tags-form__hint">
        Tags que começam com "!" ficam no topo e são marcadas como especiais. Use a tag
        "Reembolso empresa" como ignorada nos cálculos.
      </p>

      <div style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
        <Button
          onClick={handleSubmit}
          disabled={!name.trim() || saving}
          variant="primary"
        >
          {saving ? "Criando…" : "Criar tag"}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

/* ---- Item de tag ---- */
function TagItem({ tag }) {
  const [excluded, setExcluded] = React.useState(tag.exclude_from_totals);

  return (
    <li className="tags-list__item">
      <span
        aria-hidden="true"
        className="tags-list__chip"
        style={{ background: tag.color }}
      />
      {tag.emoji ? (
        <span aria-hidden="true" className="tags-list__emoji">
          {tag.emoji}
        </span>
      ) : null}
      <span
        className={[
          "tags-list__name",
          tag.is_special ? "tags-list__name--special" : "tags-list__name--normal",
          excluded ? "tags-list__name--excluded" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {tag.name}
      </span>
      <span className="tags-list__total">
        <Money cents={tag.total_cents} size="sm" />
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={excluded}
        aria-label={
          excluded
            ? `Incluir "${tag.name}" nos cálculos`
            : `Ignorar "${tag.name}" nos cálculos`
        }
        onClick={() => setExcluded((v) => !v)}
        className={`tags-list__toggle${excluded ? " tags-list__toggle--excluded" : " tags-list__toggle--included"}`}
      >
        {excluded ? "ignorado" : "incluído"}
      </button>
    </li>
  );
}

/* ---- Tela completa ---- */
function TagsScreen(props) {
  const todayYm = "2026-06";
  const [ym, setYm] = React.useState(todayYm);
  const [formOpen, setFormOpen] = React.useState(false);

  const totalCents = DEMO_TAGS.filter((t) => !t.exclude_from_totals).reduce(
    (s, t) => s + t.total_cents,
    0,
  );
  const excludedCount = DEMO_TAGS.filter((t) => t.exclude_from_totals).length;

  return (
    <div className="tags">
      {/* Cabeçalho */}
      <header className="tags-header">
        <div className="tags-header__lead">
          <h1 className="tags-header__title">Tags</h1>
          <p className="tags-header__sub">
            Totais de {monthLabel(ym)}. Tags são diagnóstico — para onde foi o dinheiro,
            não orçamento; "! Pagar" e similares ficam no topo.
          </p>
        </div>
        <div className="tags-header__controls">
          <MonthNav
            label={monthLabel(ym)}
            onPrev={() => setYm((v) => shiftYm(v, -1))}
            onNext={() => setYm((v) => shiftYm(v, 1))}
            onToday={() => setYm(todayYm)}
            atToday={ym === todayYm}
            prevLabel="Mês anterior"
            nextLabel="Próximo mês"
          />
          <Button
            onClick={() => setFormOpen((v) => !v)}
            variant={formOpen ? "ghost" : "primary"}
          >
            {formOpen ? "Cancelar" : "Nova tag"}
          </Button>
        </div>
      </header>

      {/* Painel de criação */}
      {formOpen ? <NewTagForm onCancel={() => setFormOpen(false)} /> : null}

      {/* Lista de tags */}
      <ul className="tags-list" aria-label="Tags do mês">
        {DEMO_TAGS.map((tag) => (
          <TagItem key={tag.id} tag={tag} />
        ))}
      </ul>

      {/* Sumário */}
      <footer className="tags-summary">
        <div>
          <p className="tags-summary__label">Total incluído nos cálculos</p>
          {excludedCount > 0 ? (
            <p className="tags-summary__excluded-note">
              {excludedCount} {excludedCount === 1 ? "tag ignorada" : "tags ignoradas"}{" "}
              não entram neste total.
            </p>
          ) : null}
        </div>
        <span
          className="tags-summary__total"
          aria-label={`Total: ${fmtBRL(totalCents)}`}
          style={{ color: totalCents < 0 ? "var(--money-neg)" : "var(--money-pos)" }}
        >
          {totalCents < 0 ? "−" : ""}
          {fmtBRL(Math.abs(totalCents))}
        </span>
      </footer>
    </div>
  );
}

window.TagsScreen = TagsScreen;
