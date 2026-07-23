import { describe, expect, it } from "vitest";
import type {
  TagRulerEffects,
  TagRulerFlags,
  TagsScreenDto,
  TagsScreenTag,
  TagsScreenThirdParty,
} from "../lib/api";
import { formatBRL } from "../lib/format";
import {
  RULER_ORDER,
  exceptionSummary,
  isException,
  labelFraction,
  maxLabelTotal,
  monthLabelLower,
  offRulerCount,
  personInitials,
  personRow,
  pluralLancamentos,
  pluralPessoas,
  pluralRotulos,
  resolveHeadline,
  rulerEffect,
  rulerMeasures,
  rulerSwitchLabel,
  splitExceptionsAndLabels,
  verdictLabel,
} from "./tagsView";

const ALL_ON: TagRulerFlags = {
  performance: true,
  cost_of_living: true,
  savings: true,
  daily_avg: true,
};

const ZERO_EFFECTS: TagRulerEffects = {
  performance_delta_cents: 0,
  cost_delta_cents: 0,
  savings_base_delta_cents: 0,
  savings_amount_delta_cents: 0,
  daily_avg_delta_cents: 0,
};

function tag(overrides: Partial<TagsScreenTag> & { id: string }): TagsScreenTag {
  return {
    name: "Tag",
    color: "var(--cat-jade)",
    emoji: null,
    is_special: false,
    counts_in: ALL_ON,
    month_total_cents: 0,
    txn_count: 0,
    effects: ZERO_EFFECTS,
    ...overrides,
  };
}

function person(
  overrides: Partial<TagsScreenThirdParty> & {
    person_id: string;
    state: TagsScreenThirdParty["state"];
  },
): TagsScreenThirdParty {
  return {
    name: "Pessoa",
    out_cents: 0,
    back_cents: 0,
    expected_cents: 0,
    open_since_days: null,
    series_done: null,
    series_total: null,
    settled_date: null,
    ...overrides,
  };
}

function dto(overrides: Partial<TagsScreenDto> = {}): TagsScreenDto {
  return {
    month: "2026-07",
    verdict: {
      cost_current_cents: 0,
      cost_all_on_cents: 0,
      third_party_avg_cents: null,
      third_party_people: 0,
      has_exceptions: false,
    },
    third_parties: [],
    tags: [],
    last_sync_at: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Manchete — máquina de 6 estados (E/carregando é plumbing genérico da tela).
// ---------------------------------------------------------------------------

describe("resolveHeadline", () => {
  it("D — zero tags vence qualquer outro sinal", () => {
    const d = dto({
      tags: [],
      verdict: {
        cost_current_cents: 100,
        cost_all_on_cents: 100,
        third_party_avg_cents: 500,
        third_party_people: 2,
        has_exceptions: true,
      },
      last_sync_at: "2026-07-20 10:00:00",
    });
    expect(resolveHeadline(d)).toEqual({ kind: "empty-tags" });
  });

  it("F — leitura falhou (com cache) vence exceções e detecção de terceiros", () => {
    const d = dto({
      tags: [tag({ id: "t1" })],
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 1211288,
        third_party_avg_cents: 282300,
        third_party_people: 5,
        has_exceptions: true,
      },
      last_sync_at: "2026-07-20 22:14:00",
    });
    expect(resolveHeadline(d, true)).toEqual({
      kind: "stale",
      costCents: 702873,
      staleAt: "2026-07-20 22:14:00",
    });
  });

  it("F — last_sync_at preenchido SEM falha de leitura nunca vira stale (todo DTO o carrega)", () => {
    const d = dto({
      tags: [tag({ id: "t1" })],
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 1211288,
        third_party_avg_cents: null,
        third_party_people: 0,
        has_exceptions: true,
      },
      last_sync_at: "2026-07-20 22:14:00",
    });
    expect(resolveHeadline(d).kind).toBe("exceptions");
  });

  it("A — has_exceptions calcula o excluído como a diferença all-on menos current", () => {
    const d = dto({
      tags: [tag({ id: "t1" })],
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 1211288,
        third_party_avg_cents: null,
        third_party_people: 0,
        has_exceptions: true,
      },
    });
    expect(resolveHeadline(d)).toEqual({
      kind: "exceptions",
      costCents: 702873,
      excludedCents: 1211288 - 702873,
      allOnCents: 1211288,
    });
  });

  it("B — sem exceção, com terceiros detectados", () => {
    const d = dto({
      tags: [tag({ id: "t1" })],
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 702873,
        third_party_avg_cents: 282300,
        third_party_people: 5,
        has_exceptions: false,
      },
    });
    expect(resolveHeadline(d)).toEqual({
      kind: "third-party",
      avgCents: 282300,
      peopleCount: 5,
    });
  });

  it("C — sem exceção e sem detecção: número seco", () => {
    const d = dto({
      tags: [tag({ id: "t1" })],
      verdict: {
        cost_current_cents: 702873,
        cost_all_on_cents: 702873,
        third_party_avg_cents: null,
        third_party_people: 0,
        has_exceptions: false,
      },
    });
    expect(resolveHeadline(d)).toEqual({ kind: "clean", costCents: 702873 });
  });
});

// ---------------------------------------------------------------------------
// Frases por régua: metade fixa sempre + efeito só quando desligada.
// ---------------------------------------------------------------------------

describe("rulerMeasures", () => {
  it("é a mesma frase ligada ou desligada (metade fixa)", () => {
    expect(rulerMeasures("performance")).toBe("Quanto sobrou no mês.");
    expect(rulerMeasures("cost_of_living")).toBe(
      "Quanto você gasta para viver por mês.",
    );
    expect(rulerMeasures("savings")).toBe("Quanto você guarda da sua renda.");
    expect(rulerMeasures("daily_avg")).toBe("Quanto você pode gastar por dia.");
  });
});

describe("rulerEffect", () => {
  it("régua ligada nunca mostra efeito", () => {
    expect(
      rulerEffect("performance", true, {
        ...ZERO_EFFECTS,
        performance_delta_cents: -1,
      }),
    ).toBeNull();
  });

  it("performance: contribuição positiva (Gio real: +900,00, devolve mais do que gasta) — fora, o resultado mostra a menos", () => {
    expect(
      rulerEffect("performance", false, {
        ...ZERO_EFFECTS,
        performance_delta_cents: 90000,
      }),
    ).toEqual({
      kind: "money",
      cents: 90000,
      suffix: " a menos no resultado do mês — entra mais do que sai.",
    });
  });

  it("performance: líquido zero — entrou e saiu, resultado não muda", () => {
    expect(rulerEffect("performance", false, ZERO_EFFECTS)).toEqual({
      kind: "text",
      text: "entrou e saiu: o resultado não muda.",
    });
  });

  it("performance: contribuição negativa (sai mais do que entra) — fora, o resultado infla a mais", () => {
    expect(
      rulerEffect("performance", false, {
        ...ZERO_EFFECTS,
        performance_delta_cents: -50000,
      }),
    ).toEqual({
      kind: "money",
      cents: 50000,
      suffix: " a mais no resultado do mês — sai mais do que entra.",
    });
  });

  it("custo de vida: contribuição positiva (Gio real: 4.077,64) — fora, o custo mostra a menos", () => {
    expect(
      rulerEffect("cost_of_living", false, {
        ...ZERO_EFFECTS,
        cost_delta_cents: 407764,
      }),
    ).toEqual({
      kind: "money",
      cents: 407764,
      suffix: " a menos no seu custo de vida.",
    });
  });

  it("custo de vida: contribuição negativa (resíduo com sinal) — fora, o custo infla a mais", () => {
    expect(
      rulerEffect("cost_of_living", false, {
        ...ZERO_EFFECTS,
        cost_delta_cents: -3000,
      }),
    ).toEqual({
      kind: "money",
      cents: 3000,
      suffix: " a mais no seu custo de vida.",
    });
  });

  it("custo de vida: zero — não pesa no mês", () => {
    expect(rulerEffect("cost_of_living", false, ZERO_EFFECTS)).toEqual({
      kind: "text",
      text: "não pesa no seu custo de vida.",
    });
  });

  it("economia: efeito R$ 0 nunca vira 'R$ 0,00 a menos' — texto conceitual", () => {
    expect(rulerEffect("savings", false, ZERO_EFFECTS)).toEqual({
      kind: "text",
      text: "não entra na sua renda.",
    });
  });

  it("economia: base domina quando |base| >= |amount| (Reembolso real: 167,00 na base)", () => {
    expect(
      rulerEffect("savings", false, {
        ...ZERO_EFFECTS,
        savings_base_delta_cents: 16700,
        savings_amount_delta_cents: 0,
      }),
    ).toEqual({ kind: "money", cents: 16700, suffix: " a menos na base da economia." });
  });

  it("economia: economia registrada domina quando |amount| > |base|", () => {
    expect(
      rulerEffect("savings", false, {
        ...ZERO_EFFECTS,
        savings_base_delta_cents: 100,
        savings_amount_delta_cents: 5000,
      }),
    ).toEqual({
      kind: "money",
      cents: 5000,
      suffix: " a menos na economia registrada.",
    });
  });

  it("diário médio nunca mostra número, mesmo com delta não-zero", () => {
    expect(
      rulerEffect("daily_avg", false, {
        ...ZERO_EFFECTS,
        daily_avg_delta_cents: -99999,
      }),
    ).toEqual({ kind: "text", text: "não pesa no seu teto do dia a dia." });
  });
});

describe("rulerSwitchLabel", () => {
  it("é estável — carrega régua + tag, nunca o estado (aria-checked anuncia isso)", () => {
    expect(rulerSwitchLabel("cost_of_living", "Gio")).toBe("Custo de vida · tag Gio");
  });
});

// ---------------------------------------------------------------------------
// Resumo de réguas + agrupamento exceção × rótulo.
// ---------------------------------------------------------------------------

describe("offRulerCount / exceptionSummary / isException", () => {
  it("todas ligadas: resumo é 'Conta em todas as réguas', não é exceção", () => {
    expect(offRulerCount(ALL_ON)).toBe(0);
    expect(exceptionSummary(ALL_ON)).toBe("Conta em todas as réguas");
    expect(isException({ counts_in: ALL_ON })).toBe(false);
  });

  it("1 de 4 desligada: 'Fora de 1 de 4 réguas', é exceção", () => {
    const counts: TagRulerFlags = { ...ALL_ON, savings: false };
    expect(offRulerCount(counts)).toBe(1);
    expect(exceptionSummary(counts)).toBe("Fora de 1 de 4 réguas");
    expect(isException({ counts_in: counts })).toBe(true);
  });

  it("4 de 4 desligadas: 'Fora de 4 de 4 réguas'", () => {
    const counts: TagRulerFlags = {
      performance: false,
      cost_of_living: false,
      savings: false,
      daily_avg: false,
    };
    expect(offRulerCount(counts)).toBe(4);
    expect(exceptionSummary(counts)).toBe("Fora de 4 de 4 réguas");
  });
});

describe("splitExceptionsAndLabels", () => {
  it("separa por counts_in e ordena cada lista por movimento decrescente", () => {
    const gio = tag({
      id: "gio",
      name: "Gio",
      counts_in: {
        performance: false,
        cost_of_living: false,
        savings: false,
        daily_avg: false,
      },
      month_total_cents: 407764,
    });
    const transito = tag({
      id: "transito",
      name: "Trânsito",
      counts_in: {
        performance: false,
        cost_of_living: false,
        savings: false,
        daily_avg: false,
      },
      month_total_cents: 100651,
    });
    const moradia = tag({ id: "moradia", name: "Moradia", month_total_cents: 176656 });
    const educacao = tag({
      id: "educacao",
      name: "Educação",
      month_total_cents: 54412,
    });

    const { exceptions, labels } = splitExceptionsAndLabels([
      educacao,
      transito,
      moradia,
      gio,
    ]);

    expect(exceptions.map((t) => t.id)).toEqual(["gio", "transito"]);
    expect(labels.map((t) => t.id)).toEqual(["moradia", "educacao"]);
  });
});

describe("maxLabelTotal / labelFraction", () => {
  it("fração é relativa ao maior rótulo, nunca ao total (partes não somam o todo)", () => {
    const moradia = tag({ id: "moradia", month_total_cents: 176656 });
    const cancelar = tag({ id: "cancelar", month_total_cents: 8490 });
    const max = maxLabelTotal([moradia, cancelar]);
    expect(max).toBe(176656);
    expect(labelFraction(moradia, max)).toBe(1);
    expect(labelFraction(cancelar, max)).toBeCloseTo(8490 / 176656);
  });

  it("lista vazia: máximo 0, fração 0 (sem divisão por zero)", () => {
    expect(maxLabelTotal([])).toBe(0);
    expect(labelFraction(tag({ id: "x", month_total_cents: 100 }), 0)).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// Rótulos de texto (plurais, mês).
// ---------------------------------------------------------------------------

describe("plurais e mês", () => {
  it("singular vs plural nas 3 contagens", () => {
    expect(pluralLancamentos(1)).toBe("1 lançamento");
    expect(pluralLancamentos(2)).toBe("2 lançamentos");
    expect(pluralPessoas(1)).toBe("1 pessoa");
    expect(pluralPessoas(5)).toBe("5 pessoas");
    expect(pluralRotulos(1)).toBe("1 rótulo");
    expect(pluralRotulos(4)).toBe("4 rótulos");
  });

  it("monthLabelLower e verdictLabel usam o mês por extenso em minúsculas", () => {
    expect(monthLabelLower("2026-07")).toBe("julho");
    expect(verdictLabel("2026-07")).toBe("Custo de vida · julho");
  });
});

// ---------------------------------------------------------------------------
// Dinheiro de terceiros: os 5 estados epistêmicos.
// ---------------------------------------------------------------------------

describe("personInitials", () => {
  it("uma palavra: 1 letra; duas ou mais: 2 letras", () => {
    expect(personInitials("Gio")).toBe("G");
    expect(personInitials("Ana Paula")).toBe("AP");
    expect(personInitials("  Bruna  ")).toBe("B");
  });
});

describe("personRow", () => {
  it("favor: valor é o líquido (voltou − saiu), tail 'a seu favor'", () => {
    const row = personRow(
      person({
        person_id: "gio",
        state: "favor",
        name: "Gio",
        out_cents: 407764,
        back_cents: 497764,
      }),
      "julho",
    );
    expect(row.detail).toBe(`Saiu ${formatBRL(407764)} · voltou ${formatBRL(497764)}`);
    expect(row.value).toEqual({ kind: "money", cents: 90000 });
    expect(row.tail).toBe("A seu favor");
  });

  it("open sem retorno: valor é o que saiu, tail com a idade em dias", () => {
    const row = personRow(
      person({
        person_id: "e",
        state: "open",
        name: "Edvaldo",
        out_cents: 5000,
        open_since_days: 13,
      }),
      "julho",
    );
    expect(row.detail).toBe(`Saiu ${formatBRL(5000)} · sem retorno`);
    expect(row.value).toEqual({ kind: "money", cents: 5000 });
    expect(row.tail).toBe("Em aberto há 13 dias");
  });

  it("open com retorno parcial: desconta o que já voltou", () => {
    const row = personRow(
      person({
        person_id: "e2",
        state: "open",
        name: "Edvaldo",
        out_cents: 10000,
        back_cents: 4000,
        open_since_days: 1,
      }),
      "julho",
    );
    expect(row.detail).toBe(
      `Saiu ${formatBRL(10000)} · voltou ${formatBRL(4000)} até agora`,
    );
    expect(row.value).toEqual({ kind: "money", cents: 6000 });
    expect(row.tail).toBe("Em aberto há 1 dia");
  });

  it("series: parcela k de N no detalhe, parcelas restantes na tail", () => {
    const row = personRow(
      person({
        person_id: "pai",
        state: "series",
        name: "Pai",
        back_cents: 11700,
        series_done: 2,
        series_total: 3,
      }),
      "julho",
    );
    expect(row.detail).toBe(`Voltou ${formatBRL(11700)} · parcela 2 de 3`);
    expect(row.value).toEqual({ kind: "money", cents: 11700 });
    expect(row.tail).toBe("Falta 1 parcela");
  });

  it("series concluída: sem parcela restante", () => {
    const row = personRow(
      person({
        person_id: "pai2",
        state: "series",
        name: "Pai",
        back_cents: 100,
        series_done: 3,
        series_total: 3,
      }),
      "julho",
    );
    expect(row.tail).toBe("Série concluída");
  });

  it("settled: valor é texto 'Quitado', nunca fabrica um número", () => {
    const row = personRow(
      person({
        person_id: "pablo",
        state: "settled",
        name: "Pablo",
        out_cents: 2200,
        back_cents: 2200,
        settled_date: "2026-07-04",
      }),
      "julho",
    );
    expect(row.detail).toBe(`Saiu ${formatBRL(2200)} · voltou ${formatBRL(2200)}`);
    expect(row.value).toEqual({ kind: "text", text: "Quitado" });
    expect(row.tail).toBe("Em 4 de julho");
  });

  it("none: sem lançamento no mês, valor é travessão", () => {
    const row = personRow(
      person({ person_id: "bruna", state: "none", name: "Bruna" }),
      "julho",
    );
    expect(row.detail).toBe("Nenhum lançamento em julho.");
    expect(row.value).toEqual({ kind: "text", text: "—" });
    expect(row.tail).toBe("Sem registro");
  });
});

describe("RULER_ORDER", () => {
  it("é a ordem canônica do DTO e do protótipo", () => {
    expect(RULER_ORDER).toEqual([
      "performance",
      "cost_of_living",
      "savings",
      "daily_avg",
    ]);
  });
});
