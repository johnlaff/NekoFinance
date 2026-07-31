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
