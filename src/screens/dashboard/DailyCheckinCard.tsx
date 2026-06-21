import {
  useEffect,
  useEffectEvent,
  useReducer,
  useRef,
  type CSSProperties,
} from "react";
import { CalendarCheck } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import { Money } from "../../design-system/components/Money";
import { MovBadge, type MovKind } from "../../design-system/components/MovBadge";
import {
  createTransaction,
  updateTransactionItems,
  type DashboardSummary,
  type LineItemDraft,
} from "../../lib/api";
import { safeErrorMessage } from "../../lib/errors";
import { formatBRL, parseBRLToCents, todayISO } from "../../lib/format";
import { invalidateCommands } from "../../lib/useCommand";
import { FORM_KINDS, kindToFields } from "../../lib/movement";
import { SR_ONLY } from "../../design-system/srOnly";
import { LineItemEditor } from "../../design-system/components/LineItemEditor";

// Chips do check-in rápido — a mesma ordem canônica do form completo. Economia exige uma
// conta-destino (transfer só vale com reserva/ilíquida), que pede um seletor — fora do caminho
// rápido. Por isso o chip Economia aparece desabilitado e direciona ao form de Lançamentos.
const QUICK_KINDS: MovKind[] = FORM_KINDS;

// Trilho da barra de teto diário (estático). A barra usa `<progress>` sr-only para a semântica e
// um trilho visual aria-hidden — assim mantém a animação scaleX (GPU) idêntica nos dois WebViews
// do Tauri, sem mapear para um `<progress>` visível (estilização inconsistente entre WebViews).
const DAILY_BAR_TRACK: CSSProperties = {
  height: 6,
  borderRadius: "var(--radius-pill)",
  background: "var(--bg-subtle)",
  overflow: "hidden",
  marginBottom: "var(--space-4)",
};

const DAILY_INPUT_STYLE: CSSProperties = {
  flex: 1,
  height: "var(--hit-min)",
  padding: "0 var(--space-3)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  fontFamily: "var(--font-money)",
  fontSize: "var(--fs-body)",
};

// A descrição usa fonte sans (texto livre), não a money-mono do valor.
const DAILY_DESC_STYLE: CSSProperties = {
  ...DAILY_INPUT_STYLE,
  fontFamily: "var(--font-sans)",
};

// Linha de chips do seletor de tipo. Estática (hoisted p/ o React Compiler).
const QUICK_KIND_ROW: CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  flexWrap: "wrap",
  marginBottom: "var(--space-2)",
};

// Base estática dos chips de tipo; fundo/borda do estado ativo entram por merge no render.
const QUICK_KIND_BTN_BASE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "var(--space-2)",
  height: "var(--hit-min)",
  padding: "0 var(--space-2)",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  color: "var(--text)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "transparent",
};

const QUICK_HINT_STYLE: CSSProperties = {
  margin: "var(--space-2) 0 0",
  fontSize: "var(--fs-micro)",
  color: "var(--text-faint)",
};

// Link discreto sob os chips: leva ao form completo de Lançamentos p/ registrar uma Economia
// (que exige seletor de conta-destino). Só aparece quando o pai fornece `onGoToLancamentos`.
const ECONOMIA_LINK_STYLE: CSSProperties = {
  marginTop: "var(--space-1)",
  padding: 0,
  border: 0,
  background: "transparent",
  color: "var(--primary)",
  cursor: "pointer",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-micro)",
  textDecoration: "underline",
};

// Coluna título + subtítulo do cabeçalho (mantém o head como 2 filhos flex: grupo | status).
const CARD_TITLE_GROUP: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
  minWidth: 0,
};

// Subtítulo discreto: enquadra o card como "qualquer gasto de hoje", não só o ritual do Diário —
// importante para quem registra tudo no crédito (Diário=0) e nunca usaria o Diário.
const CARD_SUBTITLE: CSSProperties = {
  fontSize: "var(--fs-micro)",
  color: "var(--text-faint)",
  fontWeight: "var(--fw-regular)",
};

// Toggle "Detalhar" — discreto (link-button), para não pesar o caminho rápido (estático/hoisted).
const DETALHAR_BTN: CSSProperties = {
  marginTop: "var(--space-2)",
  padding: 0,
  border: 0,
  background: "transparent",
  color: "var(--text-muted)",
  cursor: "pointer",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--fs-micro)",
};

// Estado do check-in agrupado num reducer (uma atualização lógica = um render), no mesmo estilo
// do form completo, em vez de useStates relacionados que fariam fan-out de renders.
interface CheckinState {
  kind: MovKind;
  description: string;
  amount: string;
  /** Plano 036: detalhamento em partes (revelado pelo toggle "Detalhar"). */
  showItems: boolean;
  items: LineItemDraft[];
  busy: boolean;
  error: string | null;
}

// Persiste o último tipo usado entre sessões (localStorage), espelhando o padrão do ThemeToggle.
// Quem registra tudo no crédito (Diário=0) reabre o card no Cartão, não num Diário que não usa.
const LAST_KIND_KEY = "neko_last_kind";

function readLastKind(): MovKind {
  if (typeof window === "undefined") return "diario";
  const stored = localStorage.getItem(LAST_KIND_KEY);
  // Valida: precisa ser um dos 5 movimentos canônicos.
  if (
    stored === "entrada" ||
    stored === "saida" ||
    stored === "diario" ||
    stored === "cartao" ||
    stored === "economia"
  ) {
    return stored;
  }
  return "diario";
}

const INITIAL_CHECKIN: CheckinState = {
  kind: "diario", // sobrescrito por initCheckin() com o tipo persistido a cada mount
  description: "",
  amount: "",
  showItems: false,
  items: [],
  busy: false,
  error: null,
};

// Inicializador preguiçoso do useReducer: lê o último tipo do localStorage A CADA mount
// (não só no carregamento do módulo), restaurando a escolha do dono entre sessões.
function initCheckin(): CheckinState {
  return { ...INITIAL_CHECKIN, kind: readLastKind() };
}

type CheckinAction =
  | { type: "set"; patch: Partial<CheckinState> }
  | { type: "toggleItems" }
  | { type: "setItems"; items: LineItemDraft[] }
  | { type: "submitStart" }
  | { type: "submitSuccess" }
  | { type: "fail"; error: string };

function checkinReducer(s: CheckinState, a: CheckinAction): CheckinState {
  switch (a.type) {
    case "set":
      return { ...s, ...a.patch };
    case "toggleItems":
      return { ...s, showItems: !s.showItems };
    case "setItems":
      return { ...s, items: a.items };
    case "submitStart":
      return { ...s, busy: true, error: null };
    case "submitSuccess":
      // Reset dos campos voláteis; mantém o tipo (e data=hoje) p/ lançamentos em sequência.
      return {
        ...s,
        amount: "",
        description: "",
        showItems: false,
        items: [],
        busy: false,
      };
    case "fail":
      return { ...s, busy: false, error: a.error };
  }
}

/**
 * Seletor de tipo — 5 movimentos do método. role=radiogroup; cada chip é um radio.
 * Extraído do card (componente próprio) para manter o pai enxuto e cada peça focada.
 * Economia exige conta-destino (seletor) → fora do caminho rápido: chip desabilitado.
 */
function KindSelector({
  kind,
  onSelect,
  onGoToLancamentos,
}: {
  kind: MovKind;
  onSelect: (k: MovKind) => void;
  /**
   * Plano 043: quando fornecido, mostra um link "Registrar Economia → Lançamentos" sob os chips,
   * tornando o chip Economia desabilitado em um caminho navegável em vez de um beco sem saída.
   * Opcional porque a navegação por aba mora no shell (App.tsx), fora do escopo deste plano; o pai
   * (DashboardScreen) deve threadear esse callback quando a navegação por aba estiver disponível.
   */
  onGoToLancamentos?: (() => void) | undefined;
}) {
  return (
    <div>
      <div role="radiogroup" aria-label="Tipo de movimento" style={QUICK_KIND_ROW}>
        {QUICK_KINDS.map((k) => {
          const active = k === kind;
          const economiaDisabled = k === "economia";
          const btnStyle: CSSProperties = active
            ? {
                ...QUICK_KIND_BTN_BASE,
                background: "var(--surface-selected)",
                borderColor: "var(--primary)",
              }
            : economiaDisabled
              ? { ...QUICK_KIND_BTN_BASE, cursor: "not-allowed", opacity: 0.5 }
              : QUICK_KIND_BTN_BASE;
          return (
            <button
              key={k}
              type="button"
              role="radio"
              aria-checked={active}
              disabled={economiaDisabled}
              title={
                economiaDisabled
                  ? "Economia precisa de uma conta-destino — abra a aba Lançamentos e use o form completo."
                  : undefined
              }
              onClick={() => onSelect(k)}
              style={btnStyle}
            >
              <MovBadge kind={k} showLabel size={14} />
            </button>
          );
        })}
      </div>
      {onGoToLancamentos && (
        <button type="button" onClick={onGoToLancamentos} style={ECONOMIA_LINK_STYLE}>
          Registrar Economia → Lançamentos
        </button>
      )}
    </div>
  );
}

/**
 * Check-in diário — o ritual do método: a cada dia o dono registra o gasto e vê o quanto já gastou
 * contra o teto do dia. Registro rápido sem sair da tela: tipo (5 movimentos) + descrição opcional
 * + valor. O caminho rápido continua rápido — Diário/hoje por padrão; o tipo persiste entre
 * lançamentos em sequência. Tags, Repetir e a conta-destino da Economia ficam no form completo
 * (Lançamentos). Economia aqui exigiria um seletor de conta-destino, então o chip direciona ao form.
 */
export function DailyCheckinCard({
  summary,
  monthAvgCents = 0,
  onLogged,
  onAmountRef,
  onGoToTransactions,
}: {
  summary: DashboardSummary;
  /** Diário médio do mês corrente (Σ realizado ÷ dias decorridos) — referência de ritmo. */
  monthAvgCents?: number;
  onLogged: () => void;
  /** Chamado uma vez após o mount com o ref do `<input>` de valor; deixa o AppShell focá-lo (tecla N). */
  onAmountRef?: ((ref: HTMLInputElement | null) => void) | undefined;
  /**
   * Plano 043: navega para a aba Lançamentos (form completo) — usado pelo link da Economia, que
   * exige um seletor de conta-destino fora do caminho rápido. Opcional: a navegação por aba mora
   * no shell; o pai a fornece quando disponível.
   */
  onGoToTransactions?: (() => void) | undefined;
}) {
  const [state, dispatch] = useReducer(checkinReducer, undefined, initCheckin);
  const { kind, description, amount, showItems, items, busy, error } = state;
  const amountRef = useRef<HTMLInputElement>(null);

  // useEffectEvent: o efeito de mount roda uma vez, mas sempre lê o onAmountRef mais recente sem
  // virar dependência (evita o re-subscribe que o React Doctor sinalizaria numa dep mutável).
  const registerAmountRef = useEffectEvent(() => onAmountRef?.(amountRef.current));
  useEffect(() => {
    registerAmountRef();
  }, []);

  const spent = summary.daily_spend_today;
  const ceiling = summary.daily_budget;
  const remaining = ceiling - spent;
  const overspent = ceiling > 0 && remaining < 0;
  const pct = ceiling > 0 ? Math.min(100, Math.round((spent / ceiling) * 100)) : 0;

  // Plano 036: com partes detalhadas, o valor = Σ partes (campo somente-leitura); senão, o digitado.
  const itemsActive = showItems && items.length > 0;
  const itemsTotal = items.reduce((sum, it) => sum + it.amount_cents, 0);
  const typedCents = parseBRLToCents(amount);
  const cents = itemsActive ? itemsTotal : typedCents;
  const canSubmit = cents != null && cents > 0 && !busy;

  async function logSpend() {
    if (cents == null || cents <= 0) {
      dispatch({ type: "fail", error: "Informe um valor válido." });
      return;
    }
    dispatch({ type: "submitStart" });
    // Tipo, fixo e método derivam do mesmo mapeamento do form completo (não hardcoded).
    const fields = kindToFields(kind);
    try {
      const newId = await createTransaction({
        txnType: fields.txnType,
        amountCents: cents,
        description: description.trim() || null,
        date: todayISO(),
        paymentMethod: fields.paymentMethod,
        isFixed: fields.isFixed,
        tagIds: [],
        recurrence: null,
      });
      // Plano 036: persiste as partes (segundo round-trip, reaplica o total). STOP documentado: um
      // crash entre os dois deixaria a transação sem partes (recuperável; total já correto).
      if (itemsActive) {
        await updateTransactionItems(newId, items);
      }
      invalidateCommands();
      // Reset dos campos voláteis; mantém o tipo (e data=hoje) para lançamentos em sequência.
      dispatch({ type: "submitSuccess" });
      onLogged();
    } catch (e) {
      dispatch({
        type: "fail",
        error: safeErrorMessage(
          e,
          "Não foi possível registrar o lançamento. Tente novamente.",
        ),
      });
    }
  }

  return (
    <section aria-labelledby="dash-checkin-title" className="dash-card">
      <div className="dash-card__head">
        <span style={CARD_TITLE_GROUP}>
          <span className="dash-card__title" id="dash-checkin-title">
            <CalendarCheck
              size={16}
              strokeWidth={1.75}
              className="dash-card__ic"
              aria-hidden="true"
            />
            Diário de hoje
          </span>
          <span
            style={CARD_SUBTITLE}
            aria-label="Registre aqui qualquer gasto do dia — Diário, compra no cartão ou saída fixa"
          >
            Diário, cartão ou saída — registre o que aconteceu hoje
          </span>
        </span>
        <span
          style={{
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-semibold)",
            color: overspent ? "var(--danger-400)" : "var(--text-muted)",
          }}
        >
          {ceiling > 0
            ? overspent
              ? `${formatBRL(-remaining)} acima do teto`
              : `${formatBRL(remaining)} disponível`
            : "Teto do dia aparece ao lançar entradas do mês"}
        </span>
      </div>
      <div className="dash-card__body">
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "baseline",
            marginBottom: "var(--space-2)",
          }}
        >
          <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
            Diário registrado hoje
          </span>
          <span style={{ fontWeight: "var(--fw-bold)" }}>
            <Money cents={spent} size="md" />
            {ceiling > 0 && (
              <span
                style={{ color: "var(--text-faint)", fontWeight: "var(--fw-regular)" }}
              >
                {" / "}
                <Money cents={ceiling} size="md" />
              </span>
            )}
          </span>
        </div>

        {ceiling > 0 && (
          <>
            {/* Semântica via `<progress>` nativo (sr-only); o trilho visual abaixo é decorativo. */}
            <progress
              value={pct}
              max={100}
              aria-label={`${pct}% do teto diário usado${overspent ? " — teto estourado" : ""}`}
              style={SR_ONLY}
            />
            <div aria-hidden="true" style={DAILY_BAR_TRACK}>
              <div
                style={{
                  width: "100%",
                  height: "100%",
                  transformOrigin: "left",
                  transform: `scaleX(${pct / 100})`,
                  background: overspent ? "var(--danger-400)" : "var(--type-diario)",
                  // Anima transform (GPU), não width — evita layout thrash (impeccable). `--t-hover`
                  // só lista background/border/color (não transform), então NÃO animaria o transform;
                  // por isso declaramos a transição explícita com dur+ease.
                  transition: "transform var(--dur-slow) var(--ease-entrance)",
                }}
              />
            </div>
          </>
        )}

        {monthAvgCents > 0 && (
          <p
            style={{
              margin: "0 0 var(--space-3)",
              fontSize: "var(--fs-micro)",
              color: "var(--text-faint)",
            }}
          >
            Média do mês: {formatBRL(monthAvgCents)}/dia
          </p>
        )}

        <KindSelector
          kind={kind}
          onSelect={(k) => {
            dispatch({ type: "set", patch: { kind: k } });
            // Persiste o tipo escolhido p/ restaurar no próximo mount (localStorage).
            localStorage.setItem(LAST_KIND_KEY, k);
          }}
          onGoToLancamentos={onGoToTransactions}
        />

        <input
          id="qac-desc"
          aria-label="Descrição (opcional)"
          placeholder="Descrição (opcional) — ex.: mercado, aluguel…"
          value={description}
          onChange={(e) =>
            dispatch({ type: "set", patch: { description: e.target.value } })
          }
          onKeyDown={(e) => {
            // Enter na descrição pula para o valor (atalho de tab-order para velocidade).
            if (e.key === "Enter") amountRef.current?.focus();
          }}
          style={{ ...DAILY_DESC_STYLE, marginBottom: "var(--space-2)" }}
        />

        <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
          <input
            ref={amountRef}
            aria-label="Valor do lançamento (R$)"
            inputMode="decimal"
            placeholder="Valor de hoje (R$) — débito, PIX, dinheiro ou crédito"
            // Plano 036: com partes detalhadas, o valor = Σ partes (somente-leitura).
            value={itemsActive ? formatBRL(itemsTotal) : amount}
            readOnly={itemsActive}
            onChange={(e) =>
              dispatch({ type: "set", patch: { amount: e.target.value } })
            }
            onKeyDown={(e) => {
              if (e.key === "Enter" && canSubmit) void logSpend();
            }}
            style={
              itemsActive
                ? {
                    ...DAILY_INPUT_STYLE,
                    background: "var(--surface-2)",
                    color: "var(--text-muted)",
                  }
                : DAILY_INPUT_STYLE
            }
          />
          <Button
            variant="primary"
            disabled={!canSubmit}
            onClick={() => void logSpend()}
          >
            {busy ? "…" : "Registrar"}
          </Button>
        </div>

        {/* Detalhar em partes (plano 036): colapsado por padrão — o caminho rápido continua rápido. */}
        <button
          type="button"
          aria-expanded={showItems}
          onClick={() => dispatch({ type: "toggleItems" })}
          style={DETALHAR_BTN}
        >
          {showItems ? "Ocultar detalhamento ▴" : "Detalhar em partes ▾"}
        </button>
        {showItems && (
          <div style={{ marginTop: "var(--space-2)" }}>
            <LineItemEditor
              items={items}
              onChange={(next) => dispatch({ type: "setItems", items: next })}
              disabled={busy}
            />
          </div>
        )}
        {/* Dica do tipo selecionado (não-Diário): orienta sem poluir o caminho rápido. */}
        {kind === "saida" && (
          <p style={QUICK_HINT_STYLE}>
            Saída = despesa fixa do mês — contas, fatura no vencimento.
          </p>
        )}
        {kind === "cartao" && (
          <p style={QUICK_HINT_STYLE}>Cartão = compra no crédito (entra na fatura).</p>
        )}
        {kind === "entrada" && (
          <p style={QUICK_HINT_STYLE}>Entrada = renda recebida no mês.</p>
        )}
        {error && (
          <p
            role="alert"
            style={{
              color: "var(--danger-400)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-2) 0 0",
            }}
          >
            {error}
          </p>
        )}
      </div>
    </section>
  );
}
