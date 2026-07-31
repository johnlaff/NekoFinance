import { cancelMiaRound, runMiaRound, type MiaScreenEvent } from "../lib/api";
import { applyMiaScreenEvent, initialRuntimeRound, runningAnswer } from "./miaRuntime";
import {
  answerFor,
  localStamp,
  routeQuestion,
  type MiaFacts,
  type MiaMessage,
} from "./miaView";

// A conversa da sessão. Ela sobrevive à navegação entre telas (a tela remonta a cada troca)
// e morre com o app: transcript persistido e apagável é contrato do runtime do copiloto, e
// um segundo store aqui nasceria com regra de privacidade própria para conciliar depois.

let log: MiaMessage[] = [];
let seq = 1;

/** O id da rodada em curso — o único jeito de o gesto de cancelar (disparado de outra
 *  chamada, na mesma tela) alcançar a rodada que já está falando com o provedor. */
let runningRoundId: string | null = null;

export function sessionLog(): MiaMessage[] {
  return log;
}

/** Registra a pergunta e a resposta como um par, e devolve a conversa nova. Piso offline: sem
 *  a conversa ligada, as seis contas locais respondem sem rede. */
export function askInSession(
  question: string,
  facts: MiaFacts | null,
  linked = false,
): MiaMessage[] {
  const at = localStamp();
  log = [
    ...log,
    { id: seq++, author: "voce", atISO: at, question },
    {
      id: seq++,
      author: "mia",
      atISO: at,
      answer: answerFor(routeQuestion(question), facts, linked),
    },
  ];
  return log;
}

/**
 * Registra a pergunta e roteia para a rodada do runtime — com a conversa ligada, TODA
 * pergunta vai por aqui, inclusive as seis que o piso offline resolve local. A promessa
 * resolve quando a rodada FECHA (evento `error` ou `run_finished`), não quando o `run_id`
 * chega; `onUpdate` publica a cada evento, para a tela acompanhar o progresso ao vivo.
 */
export function askInSessionRuntime(
  question: string,
  onUpdate: (log: MiaMessage[]) => void,
): Promise<void> {
  const at = localStamp();
  const miaId = seq + 1;
  log = [
    ...log,
    { id: seq++, author: "voce", atISO: at, question },
    { id: seq++, author: "mia", atISO: at, answer: runningAnswer(null) },
  ];
  onUpdate(log);

  let round = initialRuntimeRound();

  return new Promise((resolve) => {
    let settled = false;
    let lastPublished = "";
    function publish() {
      const answer = round.answer ?? runningAnswer(round.toolLabel);
      // A thread é região viva de leitor de tela: republicar a mesma bolha a cada evento
      // (ex.: `tool_finished` seguido do próximo `tool_started`) viraria rajada de anúncios
      // idênticos. Só o que muda de verdade chega à tela.
      const rendered = JSON.stringify(answer);
      if (rendered === lastPublished && round.status !== "done") return;
      lastPublished = rendered;
      log = log.map((message) =>
        message.id === miaId ? { ...message, answer } : message,
      );
      onUpdate(log);
      if (round.status === "done" && !settled) {
        settled = true;
        runningRoundId = null;
        resolve();
      }
    }

    function onEvent(event: MiaScreenEvent) {
      round = applyMiaScreenEvent(round, event);
      if (event.kind === "run_started") runningRoundId = event.run_id;
      publish();
    }

    runMiaRound(question, onEvent).catch((error: unknown) => {
      round = applyMiaScreenEvent(round, {
        kind: "error",
        code: "provider_unavailable",
        message: "Não consegui abrir a rodada com o provedor da conversa.",
        fix: error instanceof Error ? error.message : "Tente novamente em instantes.",
      });
      publish();
    });
  });
}

/** Cancela a rodada do runtime em curso, se houver uma. Gesto sem efeito quando já fechou. */
export function cancelRunningRound(): Promise<void> {
  if (!runningRoundId) return Promise.resolve();
  return cancelMiaRound(runningRoundId).catch(() => undefined);
}

/** Zera a conversa — usado pelos testes para isolar cenários. */
export function resetSession(): void {
  log = [];
  seq = 1;
  runningRoundId = null;
}
