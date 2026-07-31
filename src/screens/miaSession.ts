import {
  appendMiaExchange,
  approveMiaProposal,
  cancelMiaRound,
  deleteMiaConversation,
  loadMiaConversation,
  rejectMiaProposal,
  runMiaRound,
  type MiaScreenEvent,
  type StoredMiaMessage,
} from "../lib/api";
import { safeErrorMessage } from "../lib/errors";
import {
  applyApprovalResult,
  applyMiaScreenEvent,
  editProposalField as editProposalFieldPure,
  initialRuntimeRound,
  initProposalCard,
  parseMiaProposal,
  proposalRejected,
  requestApprovalGeneration,
  runningAnswer,
  type MiaProposalPayload,
  type ProposalCardState,
} from "./miaRuntime";
import {
  answerFor,
  localStamp,
  routeQuestion,
  type MiaAnswer,
  type MiaFacts,
  type MiaMessage,
  type Span,
} from "./miaView";

// A conversa da sessão. Ela sobrevive à navegação entre telas (a tela remonta a cada troca) e à
// reabertura do app: o log em memória é hidratado a partir da conversa guardada no banco na
// montagem da tela (`hydrateSession`), e cada rodada concluída — piso offline e runtime —
// persiste o par pergunta/resposta assim que fecha. `clearSession` apaga as duas cópias juntas.

let log: MiaMessage[] = [];
let seq = 1;

/** A hidratação acontece uma vez por vida do módulo — chamadas seguintes são gesto sem
 *  efeito, tela remontando sobre a mesma sessão. */
let hydrated = false;

/** O id da rodada em curso — o único jeito de o gesto de cancelar (disparado de outra
 *  chamada, na mesma tela) alcançar a rodada que já está falando com o provedor. */
let runningRoundId: string | null = null;

/** Cartões de proposta da sessão corrente, por id do evento `proposal_ready`. A proposta é
 *  do rastro técnico da rodada, não da conversa persistida — não sobrevive a fechar o app
 *  (a mesma decisão de escopo do runId em curso). */
let proposals: Record<string, ProposalCardState> = {};

export function sessionLog(): MiaMessage[] {
  return log;
}

export function sessionProposals(): Record<string, ProposalCardState> {
  return proposals;
}

function setProposal(id: string, card: ProposalCardState): void {
  proposals = { ...proposals, [id]: card };
}

function isSpan(value: unknown): value is Span {
  if (typeof value !== "object" || value === null) return false;
  const span = value as Record<string, unknown>;
  if (span["t"] === "money") return typeof span["cents"] === "number";
  if (span["t"] === "text" || span["t"] === "strong")
    return typeof span["s"] === "string";
  return false;
}

const VALID_PROVENANCE = new Set(["calculo", "metodo", "runtime"]);

/** Valida no boundary a forma mínima de uma resposta guardada — o backend trata o JSON como
 *  opaco, então um registro de uma versão anterior do formato, ou corrompido, não pode
 *  derrubar a tela. Malformado vira `null`: a linha correspondente é descartada na hidratação. */
function parseStoredAnswer(raw: unknown): MiaAnswer | null {
  if (typeof raw !== "object" || raw === null) return null;
  const value = raw as Record<string, unknown>;
  if (!Array.isArray(value["text"]) || !value["text"].every(isSpan)) return null;
  const provenance = value["provenance"];
  if (typeof provenance !== "string" || !VALID_PROVENANCE.has(provenance)) return null;
  return raw as MiaAnswer;
}

/** Registra a pergunta+resposta no banco. Fogo-e-esquece por decisão: a tela já mostrou a
 *  resposta, e uma falha de gravação não deve travar a conversa — só fica no console, para o
 *  próximo diagnóstico. */
function persistExchange(question: string, answer: MiaAnswer): void {
  appendMiaExchange(question, JSON.stringify(answer)).catch((error: unknown) => {
    console.error("Falha ao guardar a mensagem da conversa:", error);
  });
}

/**
 * Hidrata o log em memória a partir da conversa guardada. Idempotente: chamada de novo depois
 * da primeira, ou com mensagens já na sessão (uma pergunta feita antes do banco responder), não
 * faz nada — nunca sobrescreve uma conversa que já está acontecendo na tela.
 */
export async function hydrateSession(): Promise<MiaMessage[]> {
  if (hydrated || log.length > 0) return log;
  hydrated = true;
  let stored: StoredMiaMessage[];
  try {
    stored = await loadMiaConversation();
  } catch (error) {
    console.error("Falha ao carregar a conversa guardada:", error);
    return log;
  }
  const restored: MiaMessage[] = [];
  for (const row of stored) {
    const atISO = localStamp(new Date(row.at_iso));
    if (row.author === "voce") {
      if (typeof row.question !== "string") continue;
      restored.push({ id: seq++, author: "voce", atISO, question: row.question });
      continue;
    }
    const answer = parseStoredAnswer(row.answer);
    if (!answer) continue;
    restored.push({ id: seq++, author: "mia", atISO, answer });
  }
  if (log.length === 0) log = restored;
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
  const answer = answerFor(routeQuestion(question), facts, linked);
  log = [
    ...log,
    { id: seq++, author: "voce", atISO: at, question },
    { id: seq++, author: "mia", atISO: at, answer },
  ];
  persistExchange(question, answer);
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
        persistExchange(question, answer);
        resolve();
      }
    }

    function onEvent(event: MiaScreenEvent) {
      round = applyMiaScreenEvent(round, event);
      if (event.kind === "run_started") runningRoundId = event.run_id;
      if (event.kind === "proposal_ready" && !proposals[event.id]) {
        const envelope = parseMiaProposal(event.proposal);
        if (envelope) setProposal(event.id, initProposalCard(envelope));
        // O cartão novo não muda a bolha de progresso (mesmo texto, mesmo rótulo de
        // ferramenta): `publish()` sozinho não republicaria. Uma referência nova de `log`
        // força a tela a reler `sessionProposals()` no próximo render.
        onUpdate([...log]);
      }
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

/** Edita um campo do rascunho do cartão. Gesto sem efeito para um id sem cartão (a tela nunca
 *  deveria chamar isto fora do ciclo de vida do cartão, mas o módulo não confia no chamador). */
export function editSessionProposal<K extends keyof MiaProposalPayload>(
  id: string,
  field: K,
  value: MiaProposalPayload[K],
): ProposalCardState | null {
  const card = proposals[id];
  if (!card) return null;
  const updated = editProposalFieldPure(card, field, value);
  setProposal(id, updated);
  return updated;
}

/** Aprova o cartão: revalida no backend com o hash do envelope ORIGINAL (a prova de que a
 *  proposta não mudou embaixo da pessoa) mais o rascunho corrente. Uma resposta que chega
 *  depois de uma edição nova é descartada pelo view-model puro (`applyApprovalResult`) — o
 *  cartão nunca aprova um valor que a tela não está mais mostrando. */
export async function approveSessionProposal(
  id: string,
): Promise<ProposalCardState | null> {
  const card = proposals[id];
  if (!card) return null;
  const generation = requestApprovalGeneration(card);
  try {
    const transactionId = await approveMiaProposal(
      card.envelope.id,
      JSON.stringify(card.draft),
      card.envelope.hash,
    );
    const updated = applyApprovalResult(card, {
      generation,
      outcome: { ok: true, transactionId },
    });
    setProposal(id, updated);
    return updated;
  } catch (error) {
    const updated = applyApprovalResult(card, {
      generation,
      outcome: {
        ok: false,
        message: safeErrorMessage(
          error,
          "Não foi possível aprovar a proposta. Tente novamente.",
        ),
      },
    });
    setProposal(id, updated);
    return updated;
  }
}

/** Recusa explícita — nunca inferida de texto no chat. Recusar é gesto local e definitivo; o
 *  backend só precisa saber para não segurar o ledger de propostas pendente para sempre. */
export async function rejectSessionProposal(
  id: string,
): Promise<ProposalCardState | null> {
  const card = proposals[id];
  if (!card) return null;
  try {
    await rejectMiaProposal(card.envelope.id);
  } catch (error) {
    console.error("Falha ao recusar a proposta:", error);
  }
  const updated = proposalRejected(card);
  setProposal(id, updated);
  return updated;
}

/** Zera a conversa — usado pelos testes para isolar cenários. Não toca o banco: quem apaga de
 *  verdade é `clearSession`. */
export function resetSession(): void {
  log = [];
  seq = 1;
  hydrated = false;
  runningRoundId = null;
  proposals = {};
}

/** Apaga a conversa de verdade — o que a pessoa leu e o rastro técnico das rodadas somem
 *  juntos, no banco e na sessão. A promessa só resolve depois da gravação sumir: a tela volta
 *  ao vazio quando a exclusão é fato, não antes. */
export async function clearSession(): Promise<void> {
  await deleteMiaConversation();
  log = [];
  seq = 1;
  hydrated = true;
  proposals = {};
}
