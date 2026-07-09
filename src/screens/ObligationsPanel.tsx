/**
 * Obrigações recorrentes (plano 069) — EXTENSÃO do Neko, não do método/planilha: a planilha
 * não guarda nenhum vínculo entre as ocorrências mensais de um item recorrente ("Aluguel" é só
 * uma linha repetida dentro da célula Saída, mês a mês, sem id nenhum ligando as doze). Aqui o
 * usuário nomeia o item recorrente UMA vez e o Neko resolve quais itens casam — sempre via uma
 * prévia confirmada pelo usuário, nunca por inferência silenciosa.
 *
 * Dois pontos de entrada:
 * - `MarkObligationAction`: botão + painel inline num item de nota, para criar a obrigação.
 * - `ObligationsCard`: lista as obrigações salvas + histórico mensal (média, tendência).
 */
import { useEffect, useRef, useState } from "react";
import {
  ChevronRight,
  Minus,
  Repeat,
  Trash2,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import {
  createObligation,
  deleteObligation,
  listObligations,
  obligationHistory,
  previewObligationMatches,
  type LineItem,
  type Obligation,
} from "../lib/api";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { MES } from "../lib/nkFormat";
import { Money } from "../design-system/components/Money";
import { Button } from "../design-system/components/Button";
import { safeErrorMessage } from "../lib/errors";

/** Rótulo + cor por kind de obrigação (mesmos 6 buckets de `classify_line_item`; nunca
 * "entrada" — o resolver não faz esse caso especial, só o line_item de exibição faz). */
const OBLIGATION_KIND_META: Record<string, { name: string; color: string }> = {
  saida: { name: "Saída", color: "var(--type-saida)" },
  cartao: { name: "Cartão", color: "var(--type-cartao)" },
  diario: { name: "Diário", color: "var(--type-diario)" },
  economia: { name: "Economia", color: "var(--type-economia)" },
  patrimonio: { name: "Patrimônio", color: "var(--text-muted)" },
  ajuste: { name: "Ajuste", color: "var(--warning-400)" },
};

/** "jun/2026" a partir de {year, month} (1-based). */
function monthLabel(year: number, month: number): string {
  return `${MES[month - 1]?.slice(0, 3) ?? month}/${year}`;
}

// ---------------------------------------------------------------------------
// Marcar um item da nota como obrigação recorrente
// ---------------------------------------------------------------------------

/** Botão de ícone num item de nota que abre o painel de confirmação. Autocontido — não afeta
 * o layout do item quando fechado. */
export function MarkObligationAction({ item }: { item: LineItem }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="lc-mark-wrap">
      <button
        type="button"
        className="lc-mark-btn"
        aria-label={`Marcar "${item.description}" como obrigação recorrente`}
        title="Marcar como obrigação recorrente"
        aria-expanded={open}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
      >
        <Repeat size={12} strokeWidth={1.75} />
      </button>
      {open && (
        // `key={item.id}` força remontar se o item por trás mudar (edição/re-import sempre
        // troca o id — ver update_transaction_items_cmd), então o name/matchDesc iniciado
        // do prop nunca fica com uma cópia obsoleta (React Doctor: no-derived-useState).
        <MarkObligationPanel key={item.id} item={item} onDone={() => setOpen(false)} />
      )}
    </span>
  );
}

// Campos EDITÁVEIS (semeados uma vez do item, depois digitados livremente): "computar inline"
// não se aplica. Staleness num `item` trocado é responsabilidade do CALL SITE — `key={item.id}`
// em `MarkObligationAction` força remontar, e line_item ids nunca são estáveis entre
// edições/re-import (`update_transaction_items_cmd` faz clear+reinsert com uuid novo), então uma
// descrição diferente sempre chega com um id diferente.
function MarkObligationPanel({ item, onDone }: { item: LineItem; onDone: () => void }) {
  // react-doctor-disable-next-line react-doctor/no-derived-useState -- editable field seeded once; staleness handled by key={item.id} remount at the call site (see banner above)
  const [name, setName] = useState(item.description);
  // react-doctor-disable-next-line react-doctor/no-derived-useState -- same as above
  const [matchDesc, setMatchDesc] = useState(item.description);
  const [restrictSection, setRestrictSection] = useState(item.section != null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);

  // Foca o campo Nome ao abrir (react-doctor no-autofocus: effect+ref em vez de `autoFocus`;
  // painel acionado pelo usuário, mover foco ao 1º campo é o comportamento correto).
  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  const section = restrictSection ? item.section : null;

  // Prévia OBRIGATÓRIA (plano 069: nunca silencioso) — a chave inclui todo input que muda o
  // casamento, então cada edição refaz a busca e mostra exatamente o que será agrupado.
  const previewKey = `preview_obligation:${matchDesc}|${section ?? ""}`;
  const previewQ = useCommand(previewKey, () =>
    previewObligationMatches(matchDesc, section),
  );
  const previewCount = previewQ.data?.length ?? 0;

  function handleConfirm(e?: React.MouseEvent) {
    e?.stopPropagation();
    void confirm();
  }

  // Sem try/finally (React Compiler não compila TryStatement com finalizer, ver React
  // Doctor). No sucesso `onDone()` desmonta o painel, então `setSaving(false)` só é
  // necessário no caminho de erro.
  async function confirm() {
    const trimmedName = name.trim();
    const trimmedMatch = matchDesc.trim();
    if (!trimmedName || !trimmedMatch || saving) return;
    setSaving(true);
    setError(null);
    try {
      await createObligation(trimmedName, trimmedMatch, section);
      invalidateCommands();
      onDone();
    } catch (err) {
      setSaving(false);
      setError(
        safeErrorMessage(err, "Não foi possível criar a obrigação. Tente novamente."),
      );
    }
  }

  return (
    <div className="lc-obligation-panel" onClick={(e) => e.stopPropagation()}>
      <p className="lc-obligation-panel__hint">
        Nomeie o item recorrente. O Neko acompanha toda ocorrência que casar — a prévia
        abaixo mostra exatamente quantas antes de você confirmar.
      </p>
      <div className="lc-obligation-panel__row">
        <label className="lc-obligation-panel__field">
          Nome
          <input
            ref={nameRef}
            aria-label="Nome da obrigação"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label className="lc-obligation-panel__field">
          Texto para casar
          <input
            aria-label="Texto para casar com as ocorrências"
            value={matchDesc}
            onChange={(e) => setMatchDesc(e.target.value)}
          />
        </label>
      </div>
      {item.section != null && (
        <label className="lc-obligation-panel__checkbox">
          <input
            type="checkbox"
            checked={restrictSection}
            onChange={(e) => setRestrictSection(e.target.checked)}
          />
          Restringir à seção desta linha ({item.section})
        </label>
      )}
      <p className="lc-obligation-panel__preview" aria-live="polite">
        {previewQ.loading
          ? "Calculando…"
          : previewQ.error != null
            ? "Não foi possível calcular a prévia."
            : `Isto vai agrupar ${previewCount} ${previewCount === 1 ? "lançamento" : "lançamentos"}.`}
      </p>
      {error && (
        <p role="alert" className="lc-obligation-panel__error">
          {error}
        </p>
      )}
      <div className="lc-obligation-panel__actions">
        <Button size="sm" variant="ghost" onClick={onDone} disabled={saving}>
          Cancelar
        </Button>
        <Button
          size="sm"
          variant="primary"
          onClick={handleConfirm}
          disabled={
            saving ||
            !name.trim() ||
            !matchDesc.trim() ||
            previewQ.loading ||
            previewQ.error != null
          }
        >
          {saving ? "Salvando…" : "Confirmar"}
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Lista + histórico das obrigações salvas
// ---------------------------------------------------------------------------

export function ObligationsCard() {
  const listQ = useCommand("obligations", listObligations);
  const [openId, setOpenId] = useState<string | null>(null);
  const obligations = listQ.data ?? [];

  async function remove(id: string) {
    try {
      await deleteObligation(id);
      invalidateCommands();
      setOpenId((v) => (v === id ? null : v));
    } catch {
      // Best-effort: o próximo refetch reflete o estado real.
    }
  }

  return (
    <section className="card lc-obligations-card">
      <div className="card__head">
        <span className="card__title">
          <Repeat size={16} strokeWidth={1.75} className="ic" />
          Obrigações recorrentes
        </span>
        <span style={{ fontSize: 12, color: "var(--text-faint)" }}>
          Extensão do Neko — a planilha não guarda essa identidade
        </span>
      </div>
      <div className="card__body">
        {listQ.loading ? (
          <p style={{ color: "var(--text-faint)", fontSize: 13 }}>Carregando…</p>
        ) : listQ.error != null ? (
          <p role="alert" style={{ color: "var(--text-faint)", fontSize: 13 }}>
            Não foi possível carregar as obrigações.
          </p>
        ) : obligations.length === 0 ? (
          <p style={{ color: "var(--text-faint)", fontSize: 13 }}>
            Nenhuma obrigação marcada ainda. Abra um lançamento itemizado e use o ícone
            de repetição num item (ex.: "Aluguel") para começar a acompanhar sua série.
          </p>
        ) : (
          obligations.map((ob) => (
            <ObligationRow
              key={ob.id}
              obligation={ob}
              open={openId === ob.id}
              onToggle={() => setOpenId((v) => (v === ob.id ? null : ob.id))}
              onDelete={() => void remove(ob.id)}
            />
          ))
        )}
      </div>
    </section>
  );
}

function ObligationRow({
  obligation,
  open,
  onToggle,
  onDelete,
}: {
  obligation: Obligation;
  open: boolean;
  onToggle: () => void;
  onDelete: () => void;
}) {
  const historyQ = useCommand(`obligation_history:${obligation.id}`, () =>
    obligationHistory(obligation.id),
  );
  const history = (historyQ.data ?? [])
    .slice()
    .sort((a, b) => a.year - b.year || a.month - b.month);
  const hasHistory = !historyQ.error && history.length > 0;
  const avgCents = hasHistory
    ? Math.round(history.reduce((s, h) => s + h.total_cents, 0) / history.length)
    : 0;
  const last = history[history.length - 1];
  const prev = history[history.length - 2];
  const trendCents =
    hasHistory && last && prev ? last.total_cents - prev.total_cents : 0;
  const TrendIcon = trendCents > 0 ? TrendingUp : trendCents < 0 ? TrendingDown : Minus;
  const kindMeta = OBLIGATION_KIND_META[obligation.kind] ?? {
    name: obligation.kind,
    color: "var(--text-muted)",
  };

  return (
    <div className="lc-obligation-row">
      <div className="lc-obligation-row__bar">
        <button
          type="button"
          className="lc-obligation-row__head"
          onClick={onToggle}
          aria-expanded={open}
          aria-label={`Ver histórico de "${obligation.name}"`}
        >
          <ChevronRight
            size={13}
            strokeWidth={1.75}
            className={"lc-chev" + (open ? " is-open" : "")}
          />
          <span className="lc-obligation-row__name">{obligation.name}</span>
          <span
            className="lc-kind"
            style={{
              color: kindMeta.color,
              borderColor: `color-mix(in srgb, ${kindMeta.color} 34%, transparent)`,
              background: `color-mix(in srgb, ${kindMeta.color} 10%, transparent)`,
            }}
          >
            <span className="lc-kind__dot" style={{ background: kindMeta.color }} />
            {kindMeta.name}
          </span>
          {historyQ.error != null ? (
            <span className="lc-obligation-row__avg">Não foi possível carregar.</span>
          ) : hasHistory ? (
            <span className="lc-obligation-row__avg">
              Média <Money cents={avgCents} size="sm" />
            </span>
          ) : (
            <span className="lc-obligation-row__avg">Sem ocorrências ainda</span>
          )}
          {hasHistory && history.length >= 2 && (
            <span
              className={
                "lc-obligation-row__trend" +
                (trendCents > 0 ? " is-up" : trendCents < 0 ? " is-down" : "")
              }
              aria-label={
                trendCents > 0
                  ? "em alta no último mês"
                  : trendCents < 0
                    ? "em queda no último mês"
                    : "estável"
              }
            >
              <TrendIcon size={13} strokeWidth={1.75} />
            </span>
          )}
        </button>
        <button
          type="button"
          className="lc-obligation-row__delete"
          aria-label={`Apagar obrigação "${obligation.name}"`}
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
        >
          <Trash2 size={13} strokeWidth={1.75} />
        </button>
      </div>
      {open && (
        <div className="lc-obligation-history">
          {historyQ.loading ? (
            <p style={{ color: "var(--text-faint)", fontSize: 12 }}>Carregando…</p>
          ) : historyQ.error != null ? (
            <p role="alert" style={{ color: "var(--text-faint)", fontSize: 12 }}>
              Não foi possível carregar.
            </p>
          ) : history.length === 0 ? (
            <p style={{ color: "var(--text-faint)", fontSize: 12 }}>
              Nenhuma ocorrência casada ainda.
            </p>
          ) : (
            history.map((h) => (
              <div className="lc-obligation-history__row" key={`${h.year}-${h.month}`}>
                <span className="lc-obligation-history__month">
                  {monthLabel(h.year, h.month)}
                </span>
                <span className="lc-obligation-history__count">
                  {h.count} {h.count === 1 ? "ocorrência" : "ocorrências"}
                </span>
                <Money cents={h.total_cents} size="sm" />
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
