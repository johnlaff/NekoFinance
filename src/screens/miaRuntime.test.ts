import { describe, expect, it } from "vitest";
import type { MiaScreenEvent } from "../lib/api";
import {
  applyMiaScreenEvent,
  initialRuntimeRound,
  runningAnswer,
  toolLabel,
  transparencyLine,
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
