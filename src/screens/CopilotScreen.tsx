import "./mia.css";
import { useEffect, useRef, useState } from "react";
import { ArrowUp } from "lucide-react";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { Money } from "../design-system/components/Money";
import { SR_ONLY } from "../design-system/srOnly";
import { getDashboardSummary, getForecast, isTauri } from "../lib/api";
import { motionEnabled } from "../lib/motion";
import { useCommand } from "../lib/useCommand";
import { useNekoApp } from "../shell/appContext";
import { greetingForHour, localTodayIso } from "./hojeView";
import { askInSession, sessionLog } from "./miaSession";
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
        span.t === "money" ? (
          <Money key={key} cents={span.cents} size="inherit" />
        ) : span.t === "strong" ? (
          <b key={key}>{span.s}</b>
        ) : (
          <span key={key}>{span.s}</span>
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

function Receipt({ lines }: { lines: ReceiptLine[] }) {
  return (
    <dl className="mia__receipt">
      {lines.map((line) => (
        <ReceiptRow key={line.label} line={line} />
      ))}
    </dl>
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
}: {
  answer: MiaAnswer;
  at: string;
  onAsk: (question: string) => void;
  onCta: (cta: AnswerCta) => void;
}) {
  return (
    <div className="mia__said">
      <p className="mia__say">
        <Prose spans={answer.text} />
      </p>
      {answer.receipt ? <Receipt lines={answer.receipt} /> : null}
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

/* ------------------------------------------------------------------ */
/* CopilotScreen                                                       */
/* ------------------------------------------------------------------ */

export function CopilotScreen() {
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const { navigate, openCompose } = useNekoApp();

  const [log, setLog] = useState<MiaMessage[]>(sessionLog);
  const [input, setInput] = useState("");
  const rootRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const summary = summaryQ.data;
  const forecast = forecastQ.data;
  const facts: MiaFacts | null =
    summary && forecast ? { summary, forecast, today: forecast.today } : null;
  const loading = summaryQ.loading === true || forecastQ.loading === true;
  const fetchError = summaryQ.error ?? forecastQ.error;

  // Quem abre a tela da conversa veio conversar: o campo recebe o foco de largada em
  // teclado físico. No polegar não — o autofoco abriria o teclado virtual por cima da
  // saudação, sem ninguém ter pedido.
  useEffect(() => {
    if (pointerViewport()) inputRef.current?.focus();
  }, []);

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
    if (!trimmed) return;
    setLog(askInSession(trimmed, facts));
    setInput("");
  }

  function runCta(cta: AnswerCta) {
    if (cta.target === "compose") openCompose({ mode: "new" });
    else navigate(cta.target);
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
        >
          <input
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Converse com a Mia"
            aria-label="Mensagem para a Mia"
          />
          <button
            type="submit"
            className="mia__send"
            aria-label="Enviar"
            disabled={!input.trim()}
          >
            <ArrowUp size={19} strokeWidth={2} />
          </button>
        </form>
        <p className="mia__honesty">
          Lê sua planilha · Responde local · A conversa fica só nesta sessão
        </p>
      </div>
    </div>
  );
}
