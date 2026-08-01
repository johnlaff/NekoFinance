import { describe, expect, it } from "vitest";
import type { MiaScreenEvent } from "../lib/api";
import {
  applyApprovalResult,
  applyMiaScreenEvent,
  canApproveProposal,
  displayProposalStatus,
  editProposalField,
  initialRuntimeRound,
  initProposalCard,
  parseMiaProposal,
  proposalExpiryLabel,
  proposalRejected,
  requestApprovalGeneration,
  runningAnswer,
  toolLabel,
  transparencyLine,
  type MiaProposalEnvelope,
  type RuntimeRoundState,
} from "./miaRuntime";
import { plainText } from "./miaView";

// O redutor da rodada remota é o seam da tela: um evento, um passo, sem rede. Cada teste prova
// uma garantia externa — resposta atômica, transparência honesta, recusa que nunca esconde a
// porta que fechou, cancelamento.

function reduce(events: MiaScreenEvent[]): RuntimeRoundState {
  return events.reduce(applyMiaScreenEvent, initialRuntimeRound());
}

describe("toolLabel", () => {
  it("traduz o nome técnico da ferramenta para um rótulo humano em pt-BR", () => {
    expect(toolLabel("get_financial_snapshot")).toBe("Consultando seus números");
    expect(toolLabel("get_year_analysis")).toBe("Consultando o ano");
  });

  it("cai num rótulo genérico para ferramenta desconhecida — nunca imprime o nome cru", () => {
    const label = toolLabel("uma_ferramenta_que_ainda_nao_existe");
    expect(label).not.toContain("_");
  });
});

describe("progresso da rodada", () => {
  it("run_started registra o id sem publicar resposta — a bolha continua de progresso", () => {
    const state = reduce([
      { kind: "run_started", run_id: "r1", model: "m", endpoint: "e" },
    ]);
    expect(state.runId).toBe("r1");
    expect(state.answer).toBeNull();
    expect(state.status).toBe("running");
  });

  it("tool_started nomeia a ferramenta em curso; tool_finished limpa o rótulo", () => {
    let state = reduce([{ kind: "tool_started", id: "t1", tool: "get_forecast" }]);
    expect(state.toolLabel).toBe("Consultando a projeção");
    state = applyMiaScreenEvent(state, {
      kind: "tool_finished",
      id: "t1",
      tool: "get_forecast",
      ok: true,
    });
    expect(state.toolLabel).toBeNull();
  });

  it("a bolha de progresso mostra a ferramenta corrente — nunca o nome técnico", () => {
    const answer = runningAnswer("Consultando o mês");
    expect(plainText(answer.text)).toBe("Consultando o mês…");
    expect(answer.provenance).toBe("runtime");
  });

  it("sem ferramenta em curso a bolha de progresso é genérica", () => {
    expect(plainText(runningAnswer(null).text)).toBe("Pensando…");
  });
});

describe("answer_ready — resposta atômica", () => {
  it("publica a bolha inteira de uma vez, com proveniência runtime", () => {
    const state = reduce([
      {
        kind: "answer_ready",
        text: "Você pode gastar até R$ 80 hoje.",
        provenance: "calculo",
      },
    ]);
    expect(state.answer?.provenance).toBe("runtime");
    expect(plainText(state.answer!.text)).toBe("Você pode gastar até R$ 80 hoje.");
    // A proveniência do runtime NUNCA alega resposta local — mesmo quando o evento de origem
    // carrega "calculo" (o cálculo é do backend, não do dispositivo offline).
    expect(state.answer?.refusal).toBeUndefined();
  });

  it("answer_ready encerra o rótulo de ferramenta em curso", () => {
    const state = reduce([
      { kind: "tool_started", id: "t1", tool: "get_forecast" },
      { kind: "answer_ready", text: "Pronto.", provenance: "calculo" },
    ]);
    expect(state.toolLabel).toBeNull();
  });
});

describe("transparência por rodada", () => {
  it("formata provedor, modelo e custo declarado", () => {
    const line = transparencyLine({
      endpoint: "openai",
      model: "gpt-5.6-terra",
      cost_micro_usd: 2_600,
    });
    expect(line).toBe("Provedor: openai · Modelo: gpt-5.6-terra · Custo: US$ 0,0026");
  });

  it("custo nulo é lacuna declarada — nunca imprime zero", () => {
    const line = transparencyLine({
      endpoint: "azure",
      model: "gpt-5.6-sol",
      cost_micro_usd: null,
    });
    expect(line).toContain("custo não declarado pelo provedor");
    expect(line).not.toContain("US$ 0,00");
  });

  it("decora a resposta já publicada; sem resposta ainda, o evento usage é descartado", () => {
    const withAnswer = reduce([
      { kind: "answer_ready", text: "Pronto.", provenance: "calculo" },
      {
        kind: "usage",
        model: "gpt-5.6-terra",
        endpoint: "openai",
        prompt_tokens: 100,
        completion_tokens: 40,
        cost_micro_usd: 1_200,
        attempts: 1,
      },
    ]);
    expect(withAnswer.answer?.transparency).toBe(
      "Provedor: openai · Modelo: gpt-5.6-terra · Custo: US$ 0,0012",
    );

    const withoutAnswer = reduce([
      {
        kind: "usage",
        model: "gpt-5.6-terra",
        endpoint: "openai",
        prompt_tokens: 100,
        completion_tokens: 40,
        cost_micro_usd: 1_200,
        attempts: 1,
      },
    ]);
    expect(withoutAnswer.answer).toBeNull();
  });
});

describe("erro — recusa honesta", () => {
  it("carrega a mensagem e a saída do evento, e nunca deixa a tela sem resposta", () => {
    const state = reduce([
      {
        kind: "error",
        code: "provider_refused",
        message: "O provedor recusou a rodada por conteúdo fora do escopo.",
        fix: "Reformule a pergunta sobre os seus números.",
      },
    ]);
    expect(state.status).toBe("done");
    expect(state.answer?.provenance).toBe("runtime");
    expect(state.answer?.refusal).toBe("execucao");
    const text = plainText(state.answer!.text);
    expect(text).toContain("O provedor recusou a rodada por conteúdo fora do escopo.");
    expect(text).toContain("Reformule a pergunta sobre os seus números.");
  });

  it("consent_missing oferece o caminho de ligar, como o piso offline", () => {
    const state = reduce([
      {
        kind: "error",
        code: "consent_missing",
        message: "A conversa ainda não está ligada nesta máquina.",
        fix: "Abra Configurações › Conversa e registre o consentimento.",
      },
    ]);
    expect(state.answer?.cta).toEqual({
      label: "Autorizar a conversa",
      target: "config",
    });
  });

  it("outros códigos de erro não inventam um CTA de ligar a conversa", () => {
    const state = reduce([
      {
        kind: "error",
        code: "cost_cap",
        message: "Teto de custo atingido.",
        fix: "Tente de novo mais tarde.",
      },
    ]);
    expect(state.answer?.cta).toBeUndefined();
  });

  it("context_cap nomeia o teto da janela — o corpo vem do backend", () => {
    const state = reduce([
      {
        kind: "error",
        code: "context_cap",
        message: "Esta conversa chegou ao teto da janela do modelo.",
        fix: "Apague a conversa para começar outra — o que você já leu some junto.",
      },
    ]);
    const text = plainText(state.answer!.text);
    expect(text).toContain("A conversa chegou ao teto da janela.");
    expect(text).toContain("Esta conversa chegou ao teto da janela do modelo.");
    expect(text).toContain("Apague a conversa para começar outra");
    expect(state.answer?.cta).toBeUndefined();
  });
});

describe("cancelamento", () => {
  it("run_finished(cancelled) sem resposta prévia vira o estado neutro 'Rodada cancelada'", () => {
    const state = reduce([
      { kind: "tool_started", id: "t1", tool: "get_forecast" },
      { kind: "run_finished", stop: "cancelled" },
    ]);
    expect(state.status).toBe("done");
    expect(state.toolLabel).toBeNull();
    expect(plainText(state.answer!.text)).toBe("Rodada cancelada.");
    expect(state.answer?.refusal).toBeUndefined();
  });

  it("cancelar depois da resposta pronta preserva a resposta — não a substitui pelo neutro", () => {
    const state = reduce([
      { kind: "answer_ready", text: "Pronto.", provenance: "calculo" },
      { kind: "run_finished", stop: "cancelled" },
    ]);
    expect(plainText(state.answer!.text)).toBe("Pronto.");
  });
});

describe("run_finished — fecha a rodada", () => {
  it("stop=answered fecha o status sem alterar a resposta publicada", () => {
    const state = reduce([
      { kind: "answer_ready", text: "Pronto.", provenance: "metodo" },
      { kind: "run_finished", stop: "answered" },
    ]);
    expect(state.status).toBe("done");
    expect(plainText(state.answer!.text)).toBe("Pronto.");
  });
});

describe("proposal_ready", () => {
  it("registra a proposta no estado sem inventar aprovação nem UI de cartão", () => {
    const state = reduce([
      { kind: "proposal_ready", id: "p1", proposal: { amount_cents: 5_000 } },
    ]);
    expect(state.proposals).toEqual([{ id: "p1", proposal: { amount_cents: 5_000 } }]);
    expect(state.status).toBe("running");
  });

  it("a resposta seguinte carrega os ids das propostas vistas na rodada", () => {
    const state = reduce([
      { kind: "proposal_ready", id: "p1", proposal: { amount_cents: 5_000 } },
      { kind: "answer_ready", text: "Aqui está a proposta.", provenance: "calculo" },
    ]);
    expect(state.answer?.proposalIds).toEqual(["p1"]);
  });

  it("sem proposta na rodada, a resposta não carrega o campo", () => {
    const state = reduce([
      {
        kind: "answer_ready",
        text: "Você pode gastar até R$ 80 hoje.",
        provenance: "calculo",
      },
    ]);
    expect(state.answer?.proposalIds).toBeUndefined();
  });
});

describe("rodapé por estado — nunca alega resposta que não chegou", () => {
  it("a bolha de progresso declara 'Rodada em andamento'", () => {
    expect(runningAnswer("Consultando o mês").transparency).toBe("Rodada em andamento");
    expect(runningAnswer(null).transparency).toBe("Rodada em andamento");
  });

  it("erro e cancelamento declaram 'Rodada não concluída'", () => {
    const errored = reduce([
      { kind: "error", code: "rate_limited", message: "m.", fix: "f." },
    ]);
    expect(errored.answer?.transparency).toBe("Rodada não concluída");

    const cancelled = reduce([{ kind: "run_finished", stop: "cancelled" }]);
    expect(cancelled.answer?.transparency).toBe("Rodada não concluída");
  });

  it("resposta pronta antes do usage declara só o que já é verdade", () => {
    const state = reduce([
      { kind: "answer_ready", text: "Pronto.", provenance: "calculo" },
    ]);
    expect(state.answer?.transparency).toBe("Resposta da conversa ligada");
  });
});

describe("natureza epistêmica — explicação nunca se disfarça de cálculo", () => {
  it("answer_ready com proveniência de método marca a resposta como explicação", () => {
    const state = reduce([
      { kind: "answer_ready", text: "O colchão é a fundação.", provenance: "metodo" },
    ]);
    expect(state.answer?.explanation).toBe(true);

    const calc = reduce([
      { kind: "answer_ready", text: "R$ 80.", provenance: "calculo" },
    ]);
    expect(calc.answer?.explanation).toBeUndefined();
  });

  it("a marca de explicação sobrevive à chegada da linha de transparência", () => {
    const state = reduce([
      { kind: "answer_ready", text: "O colchão é a fundação.", provenance: "metodo" },
      {
        kind: "usage",
        model: "gpt-5.6-terra",
        endpoint: "openai",
        prompt_tokens: 100,
        completion_tokens: 40,
        cost_micro_usd: 1_200,
        attempts: 1,
      },
    ]);
    expect(state.answer?.explanation).toBe(true);
    expect(state.answer?.transparency).toContain("Provedor: openai");
  });
});

// ---------------------------------------------------------------------
// Cartão de proposta — o view-model puro do estado proposta → editando →
// aprovada / recusada / expirada. TDD: RED antes do comportamento (issue #243).
// ---------------------------------------------------------------------

function makeEnvelope(
  overrides: Partial<MiaProposalEnvelope> = {},
): MiaProposalEnvelope {
  return {
    id: 1,
    schema_version: 1,
    payload: {
      kind: "expense",
      amount_cents: 5_000,
      date: "2026-07-31",
      description: "Mercado",
      payment_method: "debito",
      is_fixed: false,
      tag_ids: [],
    },
    data_revision: "rev-1",
    issued_at: "2026-07-31T10:00:00.000Z",
    expires_at: "2026-07-31T10:10:00.000Z",
    hash: "hash-abc",
    ...overrides,
  };
}

describe("parseMiaProposal — contrato do envelope", () => {
  it("aceita o envelope na forma pinada com o backend", () => {
    const envelope = parseMiaProposal(makeEnvelope());
    expect(envelope).not.toBeNull();
    expect(envelope?.payload.kind).toBe("expense");
  });

  it("desembrulha a forma do fio — o envelope inteiro da ferramenta", () => {
    const wire = {
      tool: "propose_transaction",
      ok: true,
      meta: { currency: "BRL" },
      data: { proposal: makeEnvelope() },
    };
    const envelope = parseMiaProposal(wire);
    expect(envelope).not.toBeNull();
    expect(envelope?.id).toBe(1);
    expect(envelope?.payload.amount_cents).toBe(5_000);
  });

  it("rejeita payload malformado — nunca finge um cartão a partir de lixo", () => {
    expect(parseMiaProposal({ id: "x" })).toBeNull();
    expect(parseMiaProposal(null)).toBeNull();
    expect(
      parseMiaProposal({ ...makeEnvelope(), payload: { kind: "transfer" } }),
    ).toBeNull();
  });
});

describe("cartão de proposta — estados", () => {
  it("nasce no estado 'proposta', com o draft igual ao payload recebido", () => {
    const card = initProposalCard(makeEnvelope());
    expect(card.gesture).toBe("proposta");
    expect(card.draft).toEqual(card.envelope.payload);
  });

  it("editar qualquer campo move o cartão para 'editando'", () => {
    const card = editProposalField(
      initProposalCard(makeEnvelope()),
      "amount_cents",
      7_000,
    );
    expect(card.gesture).toBe("editando");
    expect(card.draft.amount_cents).toBe(7_000);
    // O restante do payload não muda por editar um campo isolado.
    expect(card.draft.description).toBe("Mercado");
  });

  it("editar depois de aprovado ou recusado é gesto sem efeito — estado terminal", () => {
    const approved = applyApprovalResult(initProposalCard(makeEnvelope()), {
      generation: 0,
      outcome: { ok: true, transactionId: "tx-1" },
    });
    const stillApproved = editProposalField(approved, "amount_cents", 1);
    expect(stillApproved.gesture).toBe("aprovada");
    expect(stillApproved.draft.amount_cents).toBe(5_000);

    const rejected = proposalRejected(initProposalCard(makeEnvelope()));
    const stillRejected = editProposalField(rejected, "amount_cents", 1);
    expect(stillRejected.gesture).toBe("recusada");
  });

  it("(a) editar invalida o gesto de aprovação anterior — uma aprovação em voo some quando o campo muda antes dela chegar", () => {
    let card = initProposalCard(makeEnvelope());
    const generation = requestApprovalGeneration(card); // captura o draft no momento do clique
    card = editProposalField(card, "description", "Mercado — trocado"); // edição chega antes da resposta do backend
    // A resposta da aprovação antiga (do draft pré-edição) chega depois — é descartada.
    card = applyApprovalResult(card, {
      generation,
      outcome: { ok: true, transactionId: "tx-stale" },
    });
    expect(card.gesture).toBe("editando");
    expect(card.approvedTransactionId).toBeNull();

    // Uma aprovação pedida DEPOIS da edição (geração corrente) resolve normalmente.
    const freshGeneration = requestApprovalGeneration(card);
    card = applyApprovalResult(card, {
      generation: freshGeneration,
      outcome: { ok: true, transactionId: "tx-fresh" },
    });
    expect(card.gesture).toBe("aprovada");
    expect(card.approvedTransactionId).toBe("tx-fresh");
  });

  it("(b) nenhum evento de texto/mensagem do chat aprova a proposta", () => {
    const card = initProposalCard(makeEnvelope());
    // A recepção de outros eventos da rodada (resposta, uso, erro) não conhece o cartão de
    // proposta — nada no redutor de eventos da rodada aprova ou recusa um cartão.
    reduce([
      { kind: "answer_ready", text: "Sim, aprovo a proposta.", provenance: "calculo" },
      {
        kind: "usage",
        model: "gpt-5.6-terra",
        endpoint: "openai",
        prompt_tokens: 1,
        completion_tokens: 1,
        cost_micro_usd: 1,
        attempts: 1,
      },
    ]);
    expect(card.gesture).toBe("proposta");
  });

  it("(c) expirada: `expires_at` no passado mostra o cartão expirado e bloqueia o gesto de aprovar", () => {
    const card = initProposalCard(
      makeEnvelope({ expires_at: "2020-01-01T00:00:00.000Z" }),
    );
    const now = "2026-07-31T10:00:00.000Z";
    expect(displayProposalStatus(card, now)).toBe("expirada");
    expect(canApproveProposal(card, now)).toBe(false);
  });

  it("expira também um cartão em edição, não só o recém-chegado", () => {
    const card = editProposalField(
      initProposalCard(makeEnvelope({ expires_at: "2020-01-01T00:00:00.000Z" })),
      "amount_cents",
      1,
    );
    expect(displayProposalStatus(card, "2026-07-31T10:00:00.000Z")).toBe("expirada");
  });

  it("um cartão dentro da validade permite o gesto de aprovar", () => {
    const card = initProposalCard(makeEnvelope());
    expect(canApproveProposal(card, "2026-07-31T10:05:00.000Z")).toBe(true);
  });

  it("um cartão já aprovado ou recusado nunca volta a 'expirada' — o estado terminal vence", () => {
    const approved = applyApprovalResult(
      initProposalCard(makeEnvelope({ expires_at: "2020-01-01T00:00:00.000Z" })),
      { generation: 0, outcome: { ok: true, transactionId: "tx-1" } },
    );
    expect(displayProposalStatus(approved, "2026-07-31T10:00:00.000Z")).toBe(
      "aprovada",
    );
  });
});

describe("proposalExpiryLabel", () => {
  it("formata a validade no padrão de hora local do resto da conversa (HHhMM)", () => {
    const label = proposalExpiryLabel(makeEnvelope());
    expect(label).toMatch(/^\d{2}h\d{2}$/);
  });
});
