import type { MiaErrorCode, MiaScreenEvent } from "../lib/api";
import type { MiaAnswer, Span } from "./miaView";

// View-model puro da RODADA remota — o redutor que traduz o fio de eventos do runtime
// (`MiaScreenEvent`) num `MiaAnswer` da mesma forma que a tela já sabe desenhar. Nenhuma
// chamada de rede mora aqui: a tela alimenta o redutor a cada evento, e o composer some pelo
// `status` que ele produz. Resposta é sempre ATÔMICA — nunca existe um estado de "texto
// parcial", só o marco de progresso (ferramenta em curso) até `answer_ready` publicar a
// bolha inteira.

const t = (s: string): Span => ({ t: "text", s });

/** Rótulo humano da ferramenta em execução — o nome técnico da fachada nunca aparece cru
 *  na tela; uma ferramenta futura sem entrada aqui cai no rótulo genérico de progresso. */
const TOOL_LABELS: Record<string, string> = {
  get_financial_snapshot: "Consultando seus números",
  get_month_analysis: "Consultando o mês",
  get_year_analysis: "Consultando o ano",
  get_cashflow_calendar: "Consultando o calendário",
  search_transactions: "Buscando lançamentos",
  get_tags: "Consultando as tags",
  get_commitments: "Consultando compromissos",
  get_forecast: "Consultando a projeção",
  simulate_scenario: "Simulando o cenário",
  get_accounts_and_net_worth: "Consultando contas e patrimônio",
  get_budget_settings: "Consultando o orçamento",
  get_data_status: "Consultando o status dos dados",
  get_method_guidance: "Consultando o método",
  propose_transaction: "Montando a proposta",
};

export function toolLabel(tool: string): string {
  return TOOL_LABELS[tool] ?? "Consultando seus números";
}

/** Mensagem NOSSA por classe de erro — complementa `message`/`fix` do evento, que já vêm em
 *  pt-BR do backend; o texto daqui é só o cabeçalho que dá nome à porta que fechou. */
const ERROR_HEADLINE: Record<MiaErrorCode, string> = {
  consent_missing: "A conversa não está ligada.",
  provider_unavailable: "O provedor da conversa não respondeu.",
  rate_limited: "O provedor pediu para esperar.",
  provider_refused: "O provedor recusou a rodada.",
  protocol_violation: "A rodada saiu do contrato esperado.",
  turn_cap: "A rodada chegou ao teto de turnos.",
  tool_call_cap: "A rodada chegou ao teto de consultas.",
  cost_cap: "A rodada chegou ao teto de custo.",
  time_cap: "A rodada chegou ao teto de tempo.",
  cancelled: "Rodada cancelada.",
  ungrounded: "A resposta não tinha número de origem confiável.",
  context_cap: "A conversa chegou ao teto da janela.",
};

/** "US$ 0,0026" · "custo não declarado pelo provedor" — nulo é lacuna, nunca zero. */
function costLabel(costMicroUsd: number | null): string {
  if (costMicroUsd === null) return "custo não declarado pelo provedor";
  const usd = costMicroUsd / 1_000_000;
  return `US$ ${usd.toFixed(4).replace(".", ",")}`;
}

/** A linha de transparência por rodada: provedor efetivo, modelo, custo — a prova de que a
 *  resposta saiu de fora, exigida pela spec para toda rodada do runtime. */
export function transparencyLine(usage: {
  endpoint: string;
  model: string;
  cost_micro_usd: number | null;
}): string {
  return `Provedor: ${usage.endpoint} · Modelo: ${usage.model} · Custo: ${costLabel(usage.cost_micro_usd)}`;
}

export interface RuntimeProposal {
  id: string;
  proposal: unknown;
}

export interface RuntimeRoundState {
  status: "running" | "done";
  runId: string | null;
  /** Rótulo da ferramenta em curso, ou `null` entre chamadas / antes da primeira. */
  toolLabel: string | null;
  /** Propostas registradas nesta rodada. Cartão de aprovação é fatia futura — aqui é só o
   *  marco discreto de que uma proposta chegou, nunca aprovação inventada. */
  proposals: RuntimeProposal[];
  answer: MiaAnswer | null;
}

export function initialRuntimeRound(): RuntimeRoundState {
  return {
    status: "running",
    runId: null,
    toolLabel: null,
    proposals: [],
    answer: null,
  };
}

/** A bolha de progresso enquanto a rodada corre — nunca confundível com uma resposta final:
 *  sem recibo, e o rodapé declara o andamento, nunca uma resposta que ainda não chegou. */
export function runningAnswer(toolLabel: string | null): MiaAnswer {
  return {
    text: [t(toolLabel ? `${toolLabel}…` : "Pensando…")],
    provenance: "runtime",
    transparency: "Rodada em andamento",
  };
}

function errorAnswer(code: MiaErrorCode, message: string, fix: string): MiaAnswer {
  const headline = ERROR_HEADLINE[code];
  const detail = [message, fix].filter(Boolean).join(" ");
  return {
    text: detail ? [t(headline), t(" " + detail)] : [t(headline)],
    provenance: "runtime",
    // O rodapé nunca alega resposta quando a rodada não fechou com uma.
    transparency: "Rodada não concluída",
    refusal: "execucao",
    ...(code === "consent_missing"
      ? { cta: { label: "Autorizar a conversa", target: "config" as const } }
      : {}),
  };
}

function cancelledAnswer(): MiaAnswer {
  return {
    text: [t(ERROR_HEADLINE.cancelled)],
    provenance: "runtime",
    transparency: "Rodada não concluída",
  };
}

/**
 * Um evento, um passo. Nunca lança: um evento em ordem inesperada (ex.: `usage` sem
 * `answer_ready` prévio) é ignorado com segurança — a rodada real do backend sempre fecha
 * `run_finished` por último, então o pior caso é uma linha de transparência que não chegou
 * a tempo de decorar a resposta.
 */
export function applyMiaScreenEvent(
  state: RuntimeRoundState,
  event: MiaScreenEvent,
): RuntimeRoundState {
  switch (event.kind) {
    case "run_started":
      return { ...state, runId: event.run_id };

    case "tool_started":
      return { ...state, toolLabel: toolLabel(event.tool) };

    case "tool_finished":
      return { ...state, toolLabel: null };

    case "proposal_ready":
      return {
        ...state,
        proposals: [...state.proposals, { id: event.id, proposal: event.proposal }],
      };

    case "answer_ready":
      return {
        ...state,
        toolLabel: null,
        answer: {
          text: [t(event.text)],
          provenance: "runtime",
          // O evento `usage` sobrescreve com provedor, modelo e custo; até lá o rodapé só
          // afirma o que já é verdade.
          transparency: "Resposta da conversa ligada",
          // A natureza epistêmica viaja com a resposta: explicação do método nunca se
          // apresenta como cálculo sobre os números da pessoa.
          ...(event.provenance === "metodo" ? { explanation: true } : {}),
          // As propostas que a rodada já viu até aqui viajam com a bolha — é assim que a
          // tela sabe sob qual resposta desenhar o cartão de aprovação.
          ...(state.proposals.length > 0
            ? { proposalIds: state.proposals.map((p) => p.id) }
            : {}),
        },
      };

    case "usage":
      // A linha de transparência decora a resposta já publicada; sem resposta ainda (rodada
      // que erra antes de responder), o evento não tem onde pousar e é descartado.
      if (!state.answer) return state;
      return {
        ...state,
        answer: { ...state.answer, transparency: transparencyLine(event) },
      };

    case "error":
      return {
        ...state,
        status: "done",
        toolLabel: null,
        answer: errorAnswer(event.code, event.message, event.fix),
      };

    case "run_finished":
      // `cancelled` sem resposta nem erro prévios é o caso comum (cancelamento no meio da
      // consulta) — o neutro substitui o marco de progresso que ficaria pendurado na tela.
      if (event.stop === "cancelled" && !state.answer) {
        return { ...state, status: "done", toolLabel: null, answer: cancelledAnswer() };
      }
      return { ...state, status: "done", toolLabel: null };

    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Cartão de proposta — view-model puro do gesto de registrar por proposta.
//
// Contrato com o backend: o `proposal` do evento
// `proposal_ready` é o envelope abaixo. `approve_mia_proposal`/`reject_mia_proposal` vivem em
// `lib/api.ts` e a chamada de rede mora na tela — este módulo só decide o que a tela pode
// mostrar e pode disparar, nunca fala com o backend.
//
// Estados: proposta → editando → aprovada / recusada, com "expirada" como leitura DERIVADA
// (nunca gravada no estado — um cartão aprovado antes de expirar continua aprovado para
// sempre, mesmo relido depois de `expires_at`).
// ---------------------------------------------------------------------------

export type MiaProposalKind = "income" | "expense";

export interface MiaProposalPayload {
  kind: MiaProposalKind;
  amount_cents: number;
  date: string;
  description?: string;
  payment_method?: string;
  is_fixed: boolean;
  tag_ids: string[];
}

export interface MiaProposalEnvelope {
  /** Chave da linha do ledger de propostas no backend — numérica, não é o id do tool call. */
  id: number;
  schema_version: 1;
  payload: MiaProposalPayload;
  data_revision: string;
  issued_at: string;
  expires_at: string;
  hash: string;
}

/** Gesto do cartão. "expirada" NUNCA é gravada aqui — é sempre derivada de `expires_at` no
 *  momento da leitura, por `displayProposalStatus`. */
export type ProposalGesture = "proposta" | "editando" | "aprovada" | "recusada";

/** Estado do cartão exibido na tela — inclui a leitura derivada "expirada". */
export type ProposalDisplayStatus = ProposalGesture | "expirada";

export interface ProposalCardState {
  envelope: MiaProposalEnvelope;
  /** Cópia editável do payload — nasce igual a `envelope.payload`. */
  draft: MiaProposalPayload;
  gesture: ProposalGesture;
  /** Bump a cada edição — a "geração" do draft no momento de um pedido de aprovação. Uma
   *  resposta de aprovação cuja geração não bate com a atual é de um draft que já mudou
   *  debaixo dela, e é descartada: é assim que editar invalida o gesto de aprovar anterior
   *  sem o cartão precisar saber se há uma chamada de rede em voo. */
  generation: number;
  approvedTransactionId: string | null;
  /** Mensagem honesta do backend (proposta expirada, hash divergente etc.) — nunca um alerta
   *  genérico. `null` enquanto não houve tentativa de aprovar, ou depois que uma aprovação
   *  fresca sucede. */
  error: string | null;
}

const PROPOSAL_KINDS = new Set<MiaProposalKind>(["income", "expense"]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function parseMiaProposalPayload(raw: unknown): MiaProposalPayload | null {
  if (!isRecord(raw)) return null;
  const kind = raw["kind"];
  const amount_cents = raw["amount_cents"];
  const date = raw["date"];
  const is_fixed = raw["is_fixed"];
  const tag_ids = raw["tag_ids"];
  if (
    typeof kind !== "string" ||
    !PROPOSAL_KINDS.has(kind as MiaProposalKind) ||
    typeof amount_cents !== "number" ||
    typeof date !== "string" ||
    typeof is_fixed !== "boolean" ||
    !Array.isArray(tag_ids) ||
    !tag_ids.every((id) => typeof id === "string")
  ) {
    return null;
  }
  const description = raw["description"];
  const payment_method = raw["payment_method"];
  return {
    kind: kind as MiaProposalKind,
    amount_cents,
    date,
    is_fixed,
    tag_ids,
    ...(typeof description === "string" ? { description } : {}),
    ...(typeof payment_method === "string" ? { payment_method } : {}),
  };
}

/** Valida a forma mínima do envelope de proposta que o evento `proposal_ready` carrega —
 *  boundary do backend, tratado como não confiável até aqui. Malformado vira `null`: a tela
 *  nunca finge um cartão a partir de lixo. */
export function parseMiaProposal(raw: unknown): MiaProposalEnvelope | null {
  if (!isRecord(raw)) return null;
  // O fio carrega o envelope inteiro da ferramenta; a proposta vive em `data.proposal`. A forma
  // já desembrulhada também é aceita — é a que os view-models trocam entre si.
  const data = raw["data"];
  if (isRecord(data) && isRecord(data["proposal"])) {
    return parseMiaProposal(data["proposal"]);
  }
  const { id, schema_version, data_revision, issued_at, expires_at, hash } = raw;
  const payload = parseMiaProposalPayload(raw["payload"]);
  if (
    typeof id !== "number" ||
    schema_version !== 1 ||
    !payload ||
    typeof data_revision !== "string" ||
    typeof issued_at !== "string" ||
    typeof expires_at !== "string" ||
    typeof hash !== "string"
  ) {
    return null;
  }
  return { id, schema_version, payload, data_revision, issued_at, expires_at, hash };
}

export function initProposalCard(envelope: MiaProposalEnvelope): ProposalCardState {
  return {
    envelope,
    draft: envelope.payload,
    gesture: "proposta",
    generation: 0,
    approvedTransactionId: null,
    error: null,
  };
}

const TERMINAL_GESTURES = new Set<ProposalGesture>(["aprovada", "recusada"]);

/** Edita um campo do rascunho. Gesto sem efeito num estado terminal (aprovada/recusada) — o
 *  lançamento já foi decidido, e reabrir edição reescreveria uma decisão fechada. */
export function editProposalField<K extends keyof MiaProposalPayload>(
  state: ProposalCardState,
  field: K,
  value: MiaProposalPayload[K],
): ProposalCardState {
  if (TERMINAL_GESTURES.has(state.gesture)) return state;
  return {
    ...state,
    draft: { ...state.draft, [field]: value },
    gesture: "editando",
    generation: state.generation + 1,
    error: null,
  };
}

/** A geração do draft no instante em que a tela dispara o pedido de aprovação — carregue este
 *  valor até a resposta do backend e devolva-o a `applyApprovalResult`. */
export function requestApprovalGeneration(state: ProposalCardState): number {
  return state.generation;
}

export type ApprovalOutcome =
  { ok: true; transactionId: string } | { ok: false; message: string };

/** Aplica a resposta de uma tentativa de aprovação. Uma resposta cuja `generation` não bate
 *  com a atual é de um draft que uma edição já substituiu — descartada em silêncio, porque a
 *  tela já mostra o draft novo e aprovar o antigo seria a mentira que a invariante 23 proíbe. */
export function applyApprovalResult(
  state: ProposalCardState,
  result: { generation: number; outcome: ApprovalOutcome },
): ProposalCardState {
  if (result.generation !== state.generation) return state;
  if (TERMINAL_GESTURES.has(state.gesture)) return state;
  if (result.outcome.ok) {
    return {
      ...state,
      gesture: "aprovada",
      approvedTransactionId: result.outcome.transactionId,
      error: null,
    };
  }
  return { ...state, error: result.outcome.message };
}

/** Recusa explícita — gesto sem efeito num estado já terminal. */
export function proposalRejected(state: ProposalCardState): ProposalCardState {
  if (TERMINAL_GESTURES.has(state.gesture)) return state;
  return { ...state, gesture: "recusada", error: null };
}

function isProposalExpired(envelope: MiaProposalEnvelope, nowISO: string): boolean {
  return nowISO >= envelope.expires_at;
}

/** O estado que a tela mostra: os estados terminais vencem sempre — um cartão aprovado ou
 *  recusado nunca "expira" depois, porque a decisão já aconteceu. Fora disso, expiração é
 *  leitura pura de `expires_at` contra o relógio, nunca um campo gravado. */
export function displayProposalStatus(
  state: ProposalCardState,
  nowISO: string,
): ProposalDisplayStatus {
  if (TERMINAL_GESTURES.has(state.gesture)) return state.gesture;
  if (isProposalExpired(state.envelope, nowISO)) return "expirada";
  return state.gesture;
}

/** O gesto de aprovar só dispara em "proposta"/"editando" dentro da validade. */
export function canApproveProposal(state: ProposalCardState, nowISO: string): boolean {
  const status = displayProposalStatus(state, nowISO);
  return status === "proposta" || status === "editando";
}

/** "22h41" — o horário local em que a proposta perde validade, no mesmo formato do carimbo
 *  de hora do resto da conversa (`timeLabel` em `miaView`). */
export function proposalExpiryLabel(envelope: MiaProposalEnvelope): string {
  const d = new Date(envelope.expires_at);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}h${mm}`;
}
