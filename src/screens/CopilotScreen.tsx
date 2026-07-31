import "./mia.css";
import { useEffect, useId, useRef, useState } from "react";
import { ArrowUp, Check, ChevronDown, Square } from "lucide-react";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { Money } from "../design-system/components/Money";
import { SR_ONLY } from "../design-system/srOnly";
import {
  getFlagSetting,
  MIA_SHOW_RECEIPT,
  getDashboardSummary,
  getForecast,
  getMiaConsent,
  isTauri,
  listTags,
} from "../lib/api";
import { centsToBRLInput, parseBRLToCents } from "../lib/format";
import { motionEnabled } from "../lib/motion";
import { TYPE_META, type TypeMeta } from "../lib/nkFormat";
import { useCommand } from "../lib/useCommand";
import { useNekoApp } from "../shell/appContext";
import { greetingForHour, localTodayIso } from "./hojeView";
import {
  canApproveProposal,
  displayProposalStatus,
  proposalExpiryLabel,
  type MiaProposalKind,
  type MiaProposalPayload,
  type ProposalCardState,
} from "./miaRuntime";
import {
  approveSessionProposal,
  askInSession,
  askInSessionRuntime,
  cancelRunningRound,
  clearSession,
  editSessionProposal,
  hydrateSession,
  rejectSessionProposal,
  sessionLog,
  sessionProposals,
} from "./miaSession";
import {
  buildTimeline,
  contextFacts,
  timeLabel,
  SUGGESTIONS,
  type AnswerCta,
  type ContextFact,
  type MiaAnswer,
  type MiaFacts,
  type MiaMessage,
  type ReceiptLine,
  type Span,
  type Tone,
} from "./miaView";

// A tela da conversa. Toda derivação — roteamento da pergunta, resposta, recibo, recusa —
// vive em `miaView`; aqui é superfície: a thread, o painel dos números e o composer
// ancorado. O recibo é a assinatura da tela: a conta que o motor fez, impressa.

const TONE_CLASS: Record<Tone, string> = {
  ok: "mia--ok",
  warn: "mia--warn",
  bad: "mia--bad",
};

/** Símbolo impresso e a palavra que o leitor de tela ouve no lugar dele. */
const OP_META: Record<string, { glyph: string; spoken: string }> = {
  min: { glyph: "mín", spoken: "O menor dos dois — " },
  minus: { glyph: "−", spoken: "Menos " },
  div: { glyph: "÷", spoken: "Dividido por " },
  eq: { glyph: "=", spoken: "Resultado — " },
};

/* ------------------------------------------------------------------ */
/* Trechos de texto: prosa, ênfase e dinheiro (tabular, nunca anima)   */
/* ------------------------------------------------------------------ */

function Prose({ spans }: { spans: Span[] }) {
  // A identidade de um trecho é o conteúdo dele; repetições ganham um contador. Índice de
  // array seria frágil se a frase mudasse de forma, e aqui não custa nada ser exato.
  const seen = new Map<string, number>();
  const keyed = spans.map((span) => {
    const base = span.t === "money" ? `money:${span.cents}` : `${span.t}:${span.s}`;
    const nth = (seen.get(base) ?? 0) + 1;
    seen.set(base, nth);
    return { span, key: `${base}#${nth}` };
  });
  return (
    <>
      {keyed.map(({ span, key }) =>
        span.t === "text" ? (
          <span key={key}>{span.s}</span>
        ) : (
          // O selo epistêmico anda colado ao número que qualifica — é o que o mantém legível
          // quando a conta está recolhida e o que impede a dúvida sobre a qual valor ele se
          // refere numa frase com mais de um.
          <span key={key}>
            {span.t === "money" ? (
              <Money cents={span.cents} size="inherit" />
            ) : (
              <b>{span.s}</b>
            )}
            {span.mark ? <EstimateMark term={span.mark.term} /> : null}
          </span>
        ),
      )}
    </>
  );
}

/* ------------------------------------------------------------------ */
/* Recibo — a conta à mostra                                           */
/* ------------------------------------------------------------------ */

function ReceiptRow({ line }: { line: ReceiptLine }) {
  const op = line.op ? OP_META[line.op] : null;
  return (
    <div className={"mia__rl" + (line.result ? " mia__rl--result" : "")}>
      <dt className="mia__rl-label">
        {op ? (
          <>
            <span className="mia__op" aria-hidden="true">
              {op.glyph}
            </span>
            <span style={SR_ONLY}>{op.spoken}</span>
          </>
        ) : null}
        {line.label}
      </dt>
      <dd className={"mia__rl-val " + (line.tone ? TONE_CLASS[line.tone] : "")}>
        {line.cents === undefined ? (
          <span className="mia__rl-text">{line.text}</span>
        ) : (
          <Money cents={line.cents} size="inherit" />
        )}
        {line.mark ? <EstimateMark term={line.mark.term} /> : null}
      </dd>
    </div>
  );
}

/**
 * Rótulo repetido acontece — duas faturas do mesmo cartão, duas séries de mesmo nome — e um
 * `key` só de rótulo faria o React descartar uma linha: a conta impressa deixaria de fechar.
 * O contador desempata pelo conteúdo, sem depender da posição no array.
 */
function keyed(lines: ReceiptLine[]): { line: ReceiptLine; key: string }[] {
  const seen = new Map<string, number>();
  return lines.map((line) => {
    const base = `${line.label}:${line.cents ?? line.text ?? ""}`;
    const nth = (seen.get(base) ?? 0) + 1;
    seen.set(base, nth);
    return { line, key: `${base}#${nth}` };
  });
}

function ReceiptLines({
  lines,
  className,
}: {
  lines: ReceiptLine[];
  className: string;
}) {
  return (
    <dl className={className}>
      {keyed(lines).map(({ line, key }) => (
        <ReceiptRow key={key} line={line} />
      ))}
    </dl>
  );
}

function Receipt({ lines }: { lines: ReceiptLine[] }) {
  return <ReceiptLines lines={lines} className="mia__receipt" />;
}

/**
 * Recibo com a aritmética recolhida: a preferência de exibição esconde os operandos, nunca
 * o resultado — a linha `result` fica sempre à mostra, e o botão abre o resto da conta ali
 * mesmo, sem navegar.
 */
function CollapsedReceipt({ lines }: { lines: ReceiptLine[] }) {
  const [open, setOpen] = useState(false);
  const foldId = useId();
  const resultLines = lines.filter((line) => line.result);
  const restLines = lines.filter((line) => !line.result);

  if (restLines.length === 0) return <Receipt lines={lines} />;

  return (
    <>
      {/* Os operandos vêm antes do resultado mesmo recolhidos: aberta, a conta se lê na
          ordem em que foi feita, sem o resultado saltar para o topo. A moldura tracejada é o
          sinal de "aqui tem conta" — fechada, ela não promete o que não mostra. */}
      <div className="mia__receipt" data-open={open}>
        <div id={foldId} data-open={open} inert={!open} className="mia__receipt-fold">
          <ReceiptLines lines={restLines} className="mia__receipt-lines" />
        </div>
        <ReceiptLines lines={resultLines} className="mia__receipt-lines" />
        <button
          type="button"
          className="mia__receipt-toggle"
          aria-expanded={open}
          aria-controls={foldId}
          onClick={() => setOpen((current) => !current)}
        >
          <ChevronDown size={14} aria-hidden="true" />
          {open ? "Ocultar a conta" : "Ver a conta"}
        </button>
      </div>
    </>
  );
}

/* ------------------------------------------------------------------ */
/* Cartão de proposta — registrar por proposta                        */
/* ------------------------------------------------------------------ */

const PROPOSAL_KIND_META: Record<MiaProposalKind, TypeMeta> = {
  income: TYPE_META.entrada,
  expense: TYPE_META.saida,
};

const PROPOSAL_INFO = {
  title: "Como funciona?",
  body: "A Mia monta o lançamento a partir do que você descreveu. Nada entra no seu histórico até você tocar em Aprovar aqui — e editar qualquer campo pede a aprovação de novo.",
};

function ProposalCard({ id }: { id: string }) {
  const [card, setCard] = useState<ProposalCardState | null>(
    () => sessionProposals()[id] ?? null,
  );
  const [amountInput, setAmountInput] = useState(() =>
    centsToBRLInput(card?.draft.amount_cents ?? 0),
  );
  const [busy, setBusy] = useState(false);
  const tagsQ = useCommand("list_tags:lc", listTags);

  if (!card) return null;

  const status = displayProposalStatus(card, new Date().toISOString());
  const editable = status === "proposta" || status === "editando";

  function commitField<K extends keyof MiaProposalPayload>(
    field: K,
    value: MiaProposalPayload[K],
  ) {
    const updated = editSessionProposal(id, field, value);
    if (updated) setCard(updated);
  }

  function toggleTag(tagId: string) {
    const ids = card!.draft.tag_ids;
    commitField(
      "tag_ids",
      ids.includes(tagId) ? ids.filter((t) => t !== tagId) : [...ids, tagId],
    );
  }

  function approve() {
    setBusy(true);
    void approveSessionProposal(id)
      .then((updated) => {
        if (updated) setCard(updated);
      })
      .finally(() => setBusy(false));
  }

  function reject() {
    setBusy(true);
    void rejectSessionProposal(id)
      .then((updated) => {
        if (updated) setCard(updated);
      })
      .finally(() => setBusy(false));
  }

  if (status === "aprovada") {
    return (
      <div className="mia__proposal mia__proposal--done">
        <p className="mia__proposal-status mia__proposal-status--ok">
          <Check size={14} strokeWidth={2} aria-hidden="true" />
          Lançamento registrado —{" "}
          <Money cents={card.draft.amount_cents} size="inherit" />
        </p>
      </div>
    );
  }
  if (status === "recusada") {
    return (
      <div className="mia__proposal mia__proposal--done">
        <p className="mia__proposal-status">Proposta recusada.</p>
      </div>
    );
  }

  const meta = PROPOSAL_KIND_META[card.draft.kind];
  // Set em vez de `includes` no map de tags: busca O(1) por tag em vez de varrer o array
  // inteiro a cada botão renderizado.
  const selectedTagIds = new Set(card.draft.tag_ids);
  // O fundo de acento só existe quando o gesto está disponível: uma única autoridade sobre a
  // cor do botão, sem CSS e inline style disputando a mesma propriedade.
  const approveDisabled = busy || !canApproveProposal(card, new Date().toISOString());

  return (
    <div className="mia__proposal">
      <div className="mia__proposal-head">
        <span>Proposta de lançamento</span>
        <InfoPopover term={PROPOSAL_INFO}>Como funciona?</InfoPopover>
      </div>

      <div className="mia__proposal-types">
        {(["expense", "income"] as const).map((k) => {
          const km = PROPOSAL_KIND_META[k];
          const on = card.draft.kind === k;
          return (
            <button
              key={k}
              type="button"
              className={"cmp-type" + (on ? " is-on" : "")}
              disabled={!editable}
              onClick={() => commitField("kind", k)}
              style={
                on
                  ? {
                      background: `color-mix(in srgb, ${km.color} 18%, transparent)`,
                      color: "var(--text-strong)",
                    }
                  : undefined
              }
            >
              <span className="cmp-type__dot" style={{ background: km.color }} />
              {km.name}
            </button>
          );
        })}
      </div>

      <div className="mia__proposal-row">
        <div>
          <span className="cmp-label">Valor</span>
          <input
            className="cmp-field cmp-field--money"
            inputMode="decimal"
            value={amountInput}
            disabled={!editable}
            onChange={(e) => {
              setAmountInput(e.target.value);
              const cents = parseBRLToCents(e.target.value);
              if (cents !== null) commitField("amount_cents", cents);
            }}
            aria-label="Valor da proposta"
          />
        </div>
        <div>
          <span className="cmp-label">Data</span>
          <input
            type="date"
            className="cmp-field"
            value={card.draft.date}
            disabled={!editable}
            onChange={(e) => commitField("date", e.target.value)}
            aria-label="Data da proposta"
          />
        </div>
      </div>

      <div>
        <span className="cmp-label">Descrição</span>
        <input
          className="cmp-field"
          placeholder="Do que se trata?"
          value={card.draft.description ?? ""}
          disabled={!editable}
          onChange={(e) => commitField("description", e.target.value)}
          aria-label="Descrição da proposta"
        />
      </div>

      <div>
        <span className="cmp-label">Forma de pagamento</span>
        <input
          className="cmp-field"
          placeholder="Ex.: Débito, Cartão…"
          value={card.draft.payment_method ?? ""}
          disabled={!editable}
          onChange={(e) => commitField("payment_method", e.target.value)}
          aria-label="Forma de pagamento"
        />
      </div>

      <label className="mia__proposal-fixed">
        <input
          type="checkbox"
          checked={card.draft.is_fixed}
          disabled={!editable}
          onChange={(e) => commitField("is_fixed", e.target.checked)}
        />
        Lançamento fixo
      </label>

      {(tagsQ.data ?? []).length > 0 ? (
        <div className="mia__proposal-tags">
          {(tagsQ.data ?? []).map((tag) => {
            const on = selectedTagIds.has(tag.id);
            return (
              <button
                key={tag.id}
                type="button"
                className={"mia__tagchip" + (on ? " is-on" : "")}
                disabled={!editable}
                aria-pressed={on}
                onClick={() => toggleTag(tag.id)}
                style={
                  on
                    ? {
                        background: `color-mix(in srgb, ${tag.color} 20%, transparent)`,
                        borderColor: tag.color,
                        color: tag.color,
                      }
                    : undefined
                }
              >
                {tag.emoji ? `${tag.emoji} ` : ""}
                {tag.name}
              </button>
            );
          })}
        </div>
      ) : null}

      <p className="mia__proposal-validity">
        {status === "expirada"
          ? "Esta proposta expirou — os dados podem ter mudado debaixo dela. Peça de novo para gerar outra."
          : `Válida até ${proposalExpiryLabel(card.envelope)}.`}
      </p>

      {card.error ? (
        <p role="alert" className="mia__proposal-error">
          {card.error}
        </p>
      ) : null}

      <div className="mia__proposal-actions">
        <button
          type="button"
          className="mia__proposal-approve"
          disabled={approveDisabled}
          style={approveDisabled ? undefined : { background: meta.color }}
          onClick={approve}
        >
          Aprovar
        </button>
        <button
          type="button"
          className="mia__proposal-reject"
          disabled={busy}
          onClick={reject}
        >
          Recusar
        </button>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Uma resposta da Mia                                                 */
/* ------------------------------------------------------------------ */

function Answer({
  answer,
  at,
  onAsk,
  onCta,
  showReceipt,
}: {
  answer: MiaAnswer;
  at: string;
  onAsk: (question: string) => void;
  onCta: (cta: AnswerCta) => void;
  showReceipt: boolean;
}) {
  return (
    <div className="mia__said">
      <p className="mia__say">
        <Prose spans={answer.text} />
      </p>
      {answer.receipt ? (
        showReceipt ? (
          <Receipt lines={answer.receipt} />
        ) : (
          <CollapsedReceipt lines={answer.receipt} />
        )
      ) : null}
      {answer.proposalIds?.map((id) => (
        <ProposalCard key={id} id={id} />
      ))}
      {answer.note ? (
        <p className="mia__note">
          <Prose spans={answer.note} />
        </p>
      ) : null}
      {answer.options ? (
        <div className="mia__opts">
          {answer.options.map((option) => (
            <button
              key={option}
              type="button"
              className="mia__chip"
              onClick={() => onAsk(option)}
            >
              {option}
            </button>
          ))}
        </div>
      ) : null}
      {answer.cta ? (
        <button type="button" className="mia__cta" onClick={() => onCta(answer.cta!)}>
          {answer.cta.label}
        </button>
      ) : null}
      <p className="mia__prov">
        <span>
          {answer.provenance === "calculo"
            ? "Cálculo determinístico · Lê sua planilha · Responde local"
            : answer.provenance === "runtime"
              ? (answer.explanation ? "Explicação do método · " : "") +
                (answer.transparency ?? "Resposta da conversa ligada")
              : "Explicação do método"}
        </span>
        <time>{timeLabel(at)}</time>
      </p>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Painel: os números por trás (a densidade que o mouse ganha)         */
/* ------------------------------------------------------------------ */

function ContextPanel({
  facts,
  loading,
  onAsk,
}: {
  facts: ContextFact[];
  loading: boolean;
  onAsk: (question: string) => void;
}) {
  if (facts.length === 0 && !loading) return null;
  return (
    <aside className="mia__ctx" aria-labelledby="mia-ctx-t">
      <h2 id="mia-ctx-t">Os números por trás</h2>
      <p className="mia__ctx-note">
        A Mia responde com estes fatos — toque em um para ela explicar.
      </p>
      {facts.length === 0 ? <EmptyState variant="skeleton" skeletonRows={4} /> : null}
      {facts.map((fact) => (
        <button
          key={fact.key}
          type="button"
          className="mia__mc"
          onClick={() => onAsk(fact.question)}
        >
          <span className="mia__mc-label">{fact.label}</span>
          <span className={"mia__mc-val " + (fact.tone ? TONE_CLASS[fact.tone] : "")}>
            {fact.cents === undefined ? (
              fact.text
            ) : (
              <Money cents={fact.cents} size="inherit" />
            )}
          </span>
        </button>
      ))}
      <p className="mia__ctx-foot">Cálculos determinísticos, nunca inventados.</p>
    </aside>
  );
}

/** Viewport de teclado físico (o breakpoint em que o painel aparece). Defensivo: o
 *  ambiente de teste não implementa `matchMedia`. */
function pointerViewport(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(min-width: 701px)").matches
  );
}

/** O contêiner rolável mais próximo — a tela não sabe (nem precisa saber) qual é o do shell. */
function scrollerOf(el: HTMLElement | null): HTMLElement | null {
  for (let node = el?.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (
      (overflow === "auto" || overflow === "scroll") &&
      node.scrollHeight > node.clientHeight
    ) {
      return node;
    }
  }
  return null;
}

/** Módulo, não estado local — o gesto de cancelar só repassa ao runId em curso na sessão. */
function cancelRound(): void {
  void cancelRunningRound();
}

/** O texto do confirm declara ANTES o que some e o que sobrevive — apagar é irreversível, e a
 *  proveniência de um lançamento aprovado não é rastro da conversa, é histórico financeiro. */
const CLEAR_CONFIRM =
  "Apagar a conversa?\n\n" +
  "Apaga as mensagens e o rastro técnico das rodadas.\n" +
  "A origem dos lançamentos que você aprovou fica no seu histórico financeiro.";

/* ------------------------------------------------------------------ */
/* CopilotScreen                                                       */
/* ------------------------------------------------------------------ */

export function CopilotScreen() {
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const consentQ = useCommand("get_mia_consent", getMiaConsent);
  const { navigate, openCompose } = useNekoApp();

  const [log, setLog] = useState<MiaMessage[]>(sessionLog);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  // A chave esconde aritmética, nunca estado do dado: default ligado quando a preferência
  // nunca foi gravada (ou a leitura falha) — o recibo some só quando a pessoa pediu.
  const [showReceipt, setShowReceipt] = useState(true);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const stopRef = useRef<HTMLButtonElement | null>(null);

  const summary = summaryQ.data;
  const forecast = forecastQ.data;
  const facts: MiaFacts | null =
    summary && forecast ? { summary, forecast, today: forecast.today } : null;
  const loading = summaryQ.loading === true || forecastQ.loading === true;
  const fetchError = summaryQ.error ?? forecastQ.error;
  const linked = consentQ.data?.linked === true;

  // Quem abre a tela da conversa veio conversar: o campo recebe o foco de largada em
  // teclado físico. No polegar não — o autofoco abriria o teclado virtual por cima da
  // saudação, sem ninguém ter pedido.
  useEffect(() => {
    if (pointerViewport()) inputRef.current?.focus();
  }, []);

  // A conversa guardada carrega antes de a tela decidir entre a saudação e a thread — sem
  // isso, quem reabre o app veria o vazio piscar antes das próprias mensagens voltarem.
  useEffect(() => {
    void hydrateSession().then(setLog);
  }, []);

  useEffect(() => {
    getFlagSetting(MIA_SHOW_RECEIPT, true)
      .then(setShowReceipt)
      .catch(() => setShowReceipt(true));
  }, []);

  // Desabilitar o campo focado solta o foco do documento — sem realocação, o Tab seguinte
  // recomeça do topo. Durante a rodada o foco vai ao cancelar; ao fechar, volta ao campo
  // (só em teclado físico — no polegar reabriria o teclado virtual sem pedido).
  const wasBusy = useRef(false);
  useEffect(() => {
    if (busy) stopRef.current?.focus();
    else if (wasBusy.current && pointerViewport()) inputRef.current?.focus();
    wasBusy.current = busy;
  }, [busy]);

  // A conversa acompanha a última resposta rolando o SCROLLER até o fim — `scrollIntoView`
  // alinharia o fim da tela ao fim do scrollport e pararia antes, deixando o dock ancorado
  // (que flutua enquanto sobra rolagem) por cima da resposta nova.
  useEffect(() => {
    const scroller = scrollerOf(rootRef.current);
    scroller?.scrollTo({
      top: scroller.scrollHeight,
      behavior: motionEnabled() ? "smooth" : "auto",
    });
  }, [log.length]);

  function ask(question: string) {
    const trimmed = question.trim();
    if (!trimmed || busy) return;
    setInput("");
    // Com a conversa ligada, TODA pergunta vai ao runtime — inclusive as seis que o piso
    // offline resolve local: a recusa "ainda não está ligada" só é a resposta honesta
    // quando `linked` é falso de verdade.
    if (linked) {
      setBusy(true);
      askInSessionRuntime(trimmed, setLog)
        .catch(() => undefined)
        .finally(() => setBusy(false));
      return;
    }
    setLog(askInSession(trimmed, facts, linked));
  }

  function runCta(cta: AnswerCta) {
    if (cta.target === "compose") openCompose({ mode: "new" });
    else navigate(cta.target);
  }

  function clearConversation() {
    if (!window.confirm(CLEAR_CONFIRM)) return;
    clearSession()
      .then(() => setLog([]))
      .catch((error: unknown) => {
        console.error("Falha ao apagar a conversa:", error);
      });
  }

  const timeline = buildTimeline(log, localTodayIso());

  return (
    <div ref={rootRef} className={"mia" + (log.length === 0 ? " mia--empty" : "")}>
      <div
        className="mia__thread"
        role="log"
        aria-live="polite"
        aria-label="Conversa com a Mia"
      >
        {fetchError ? (
          <p role="status" className="mia__stale">
            Não foi possível atualizar agora — respondo com os últimos dados carregados.
          </p>
        ) : null}
        {log.length === 0 ? (
          <div className="mia__greet">
            <span className="mia__greet-cat" aria-hidden="true">
              <MiaAvatar width={68} height={68} />
            </span>
            <h1 data-large-title>{greetingForHour(new Date().getHours())}</h1>
            <p className="mia__greet-say">
              Sou a Mia. Pergunte sobre os seus números — eu respondo com a conta à
              mostra, ensino o método por trás dela e digo quando não sei.
            </p>
            {!isTauri ? (
              <p className="mia__greet-web">
                Preview web — abra o app desktop para conversar sobre os seus dados.
              </p>
            ) : null}
          </div>
        ) : null}

        {log.length > 0 ? (
          // Com conversa a saudação sai de cena, e a tela ficaria sem título: o leitor de
          // tela perderia o nível 1 da hierarquia (o painel abre em h2).
          <h1 style={SR_ONLY}>Conversa com a Mia</h1>
        ) : null}

        {timeline.map((item) =>
          item.kind === "daymark" ? (
            <p key={item.key} className="mia__daymark">
              <span>{item.label}</span>
            </p>
          ) : item.message.author === "voce" ? (
            <div key={item.key} className="mia__msg mia__msg--you">
              <p className="mia__bubble">
                <span style={SR_ONLY}>Você: </span>
                {item.message.question}
                <time>{timeLabel(item.message.atISO)}</time>
              </p>
            </div>
          ) : (
            <div key={item.key} className="mia__msg">
              <span className="mia__av" aria-hidden="true">
                <MiaAvatar width={22} height={22} />
              </span>
              <span style={SR_ONLY}>Mia: </span>
              <Answer
                answer={item.message.answer!}
                at={item.message.atISO}
                onAsk={ask}
                onCta={runCta}
                showReceipt={showReceipt}
              />
            </div>
          ),
        )}
      </div>

      <ContextPanel facts={contextFacts(facts)} loading={loading} onAsk={ask} />

      <div className="mia__dock">
        <div className="mia__sugg">
          {SUGGESTIONS.map((suggestion) => (
            <button
              key={suggestion}
              type="button"
              className="mia__chip"
              onClick={() => ask(suggestion)}
            >
              {suggestion}
            </button>
          ))}
        </div>
        <form
          className="mia__composer"
          onSubmit={(e) => {
            e.preventDefault();
            ask(input);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape" && busy) cancelRound();
          }}
        >
          <input
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={busy ? "A Mia está respondendo…" : "Converse com a Mia"}
            aria-label="Mensagem para a Mia"
            disabled={busy}
          />
          {busy ? (
            <button
              ref={stopRef}
              type="button"
              className="mia__send mia__send--stop"
              aria-label="Cancelar a rodada"
              onClick={cancelRound}
            >
              <Square size={15} strokeWidth={2} fill="currentColor" />
            </button>
          ) : (
            <button
              type="submit"
              className="mia__send"
              aria-label="Enviar"
              disabled={!input.trim()}
            >
              <ArrowUp size={19} strokeWidth={2} />
            </button>
          )}
        </form>
        <p className="mia__honesty">
          {linked
            ? "Conversa ligada · Provedor externo · Cada rodada mostra provedor, modelo e custo"
            : "Lê sua planilha · Responde local · A conversa fica no seu computador"}
        </p>
        {log.length > 0 ? (
          <button type="button" className="mia__clear" onClick={clearConversation}>
            Apagar conversa
          </button>
        ) : null}
      </div>
    </div>
  );
}
