import { describe, expect, it } from "vitest";
import {
  addMonths,
  agendaTransactions,
  buildCalendarMonth,
  cellLabel,
  cellMoney,
  cellSigned,
  dayComponents,
  gridBand,
  monthHeadline,
  monthMarks,
  railSeries,
  shiftIso,
  shortDate,
  type DayRow,
} from "./calendarioView";
import type { TransactionRow } from "../lib/api";

const TODAY = "2026-07-15";

function row(overrides: Partial<DayRow> & { date: string }): DayRow {
  return {
    income_cents: 0,
    fixed_out_cents: 0,
    daily_out_cents: 0,
    balance_cents: null,
    ...overrides,
  };
}

/** Corrente realizada de julho/2026 até a véspera de hoje (dias 1–14). */
function julyRealized(): DayRow[] {
  const balances: Record<number, number> = {
    1: 1_520_296,
    2: 1_520_299,
    3: 1_517_699,
    4: 1_517_699,
    5: 1_517_699,
    6: 1_516_700,
    7: 1_523_400,
    8: 1_623_799,
    9: 1_623_799,
    10: 1_395_299,
    11: 1_367_123,
    12: 556_965,
    13: 556_965,
    14: 556_965,
  };
  return Object.entries(balances).map(([d, b]) =>
    row({
      date: `2026-07-${String(d).padStart(2, "0")}`,
      balance_cents: b,
      income_cents: d === "1" || d === "7" || d === "8" ? 100 : 0,
    }),
  );
}

/** Projeção de hoje em diante (dias 15–31). */
function julyForecast(): DayRow[] {
  const out: DayRow[] = [];
  for (let d = 15; d <= 31; d++) {
    out.push(
      row({
        date: `2026-07-${String(d).padStart(2, "0")}`,
        balance_cents: d >= 20 ? 756_830 : 556_965,
        income_cents: d === 20 ? 199_865 : 0,
        economia_cents: 0,
      }),
    );
  }
  return out;
}

function build(overrides?: Partial<Parameters<typeof buildCalendarMonth>[0]>) {
  return buildCalendarMonth({
    year: 2026,
    month0: 6,
    today: TODAY,
    realized: [
      ...julyRealized(),
      row({ date: "2026-06-30", balance_cents: 1_330_026 }),
    ],
    forecast: julyForecast(),
    ...overrides,
  });
}

function dayCell(month: ReturnType<typeof buildCalendarMonth>, day: number) {
  for (const week of month.weeks) {
    for (const cell of week) {
      if (cell?.day === day) return cell;
    }
  }
  throw new Error(`day ${day} not found`);
}

describe("buildCalendarMonth — matriz Seg-first", () => {
  it("abre julho/2026 (quarta) com duas células vazias e fecha semanas de 7", () => {
    const m = build();
    expect(m.weeks[0]?.slice(0, 2)).toEqual([null, null]);
    expect(m.weeks[0]?.[2]?.day).toBe(1);
    for (const week of m.weeks) expect(week).toHaveLength(7);
    // 2 vazias + 31 dias = 33 → 5 semanas com 2 vazias no fim.
    expect(m.weeks).toHaveLength(5);
    expect(m.weeks[4]?.[6]).toBeNull();
  });
});

describe("buildCalendarMonth — meses de 6 semanas", () => {
  it("agosto/2026 (sábado) abre com 5 vazias e fecha em 6 semanas", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 7,
      today: TODAY,
      realized: [],
      forecast: [],
    });
    expect(m.weeks).toHaveLength(6);
    for (const week of m.weeks) expect(week).toHaveLength(7);
    expect(m.weeks[0]?.slice(0, 5)).toEqual([null, null, null, null, null]);
    expect(m.weeks[0]?.[5]?.day).toBe(1);
    expect(m.weeks[5]?.[0]?.day).toBe(31);
  });
});

describe("buildCalendarMonth — costura realizado × previsto", () => {
  it("passado lê a corrente realizada; hoje em diante lê a projeção", () => {
    const m = build();
    expect(dayCell(m, 12).balanceCents).toBe(556_965);
    expect(dayCell(m, 15).balanceCents).toBe(556_965);
    expect(dayCell(m, 20).balanceCents).toBe(756_830);
  });

  it("marca hoje e futuro", () => {
    const m = build();
    expect(dayCell(m, 15).isToday).toBe(true);
    expect(dayCell(m, 15).isFuture).toBe(false);
    expect(dayCell(m, 16).isFuture).toBe(true);
    expect(dayCell(m, 14).isFuture).toBe(false);
  });
});

describe("buildCalendarMonth — movimento (delta da corrente)", () => {
  it("o movimento é o delta contra a véspera", () => {
    const m = build();
    expect(dayCell(m, 12).movementCents).toBe(556_965 - 1_367_123);
    expect(dayCell(m, 13).movementCents).toBe(0);
    expect(dayCell(m, 20).movementCents).toBe(756_830 - 556_965);
  });

  it("o dia 1 usa a cauda do mês anterior como véspera", () => {
    const m = build();
    expect(dayCell(m, 1).movementCents).toBe(1_520_296 - 1_330_026);
  });

  it("sem véspera conhecida o movimento é nulo, nunca zero", () => {
    const m = build({
      realized: julyRealized(), // sem 30/06
    });
    expect(dayCell(m, 1).movementCents).toBeNull();
  });

  it("janeiro cruza o ano para buscar a véspera em dezembro", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 0,
      today: "2026-07-15",
      realized: [
        row({ date: "2025-12-31", balance_cents: 100_000 }),
        row({ date: "2026-01-01", balance_cents: 150_000 }),
      ],
      forecast: [],
    });
    expect(dayCell(m, 1).movementCents).toBe(50_000);
  });
});

describe("buildCalendarMonth — eventos do mês", () => {
  it("marca dias de entrada a partir da fonte certa", () => {
    const m = build();
    expect(dayCell(m, 1).hasIncome).toBe(true);
    expect(dayCell(m, 20).hasIncome).toBe(true);
    expect(dayCell(m, 12).hasIncome).toBe(false);
  });

  it("o menor saldo do mês é único e fica no primeiro dia em caso de empate", () => {
    const m = build();
    expect(m.lowestIso).toBe("2026-07-12");
    expect(dayCell(m, 12).isLowest).toBe(true);
    expect(dayCell(m, 13).isLowest).toBe(false);
  });

  it("mês inteiro sem corrente não elege menor saldo", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 2,
      today: TODAY,
      realized: [],
      forecast: [],
    });
    expect(m.lowestIso).toBeNull();
    expect(dayCell(m, 10).balanceCents).toBeNull();
  });
});

describe("dayComponents", () => {
  it("lista só componentes não-zerados, na ordem canônica", () => {
    const comps = dayComponents(
      row({
        date: "2026-07-12",
        income_cents: 0,
        fixed_out_cents: 238_298,
        daily_out_cents: 0,
        economia_cents: 30_639,
      }),
    );
    expect(comps.map((c) => c.label)).toEqual(["Saídas fixas", "Economia"]);
    expect(comps[0]?.cents).toBe(238_298);
  });

  it("dia sem componentes rende lista vazia; linha ausente também", () => {
    expect(dayComponents(row({ date: "2026-07-13" }))).toEqual([]);
    expect(dayComponents(null)).toEqual([]);
  });
});

describe("agendaTransactions", () => {
  it("filtra pela data preservando a ordem", () => {
    const rows = [
      { id: "a", date: "2026-07-12" },
      { id: "b", date: "2026-07-13" },
      { id: "c", date: "2026-07-12" },
    ] as TransactionRow[];
    expect(agendaTransactions(rows, "2026-07-12").map((t) => t.id)).toEqual(["a", "c"]);
  });
});

describe("agendaSignedCents", () => {
  it("despesa é negativa e entrada positiva, como no Livro-razão", async () => {
    const { agendaSignedCents } = await import("./calendarioView");
    const expense = {
      type: "expense",
      amount: 4300,
      is_fixed: false,
      payment_method: "debit",
    } as TransactionRow;
    const income = {
      type: "income",
      amount: 700000,
      is_fixed: false,
      payment_method: "debit",
    } as TransactionRow;
    expect(agendaSignedCents(expense)).toBe(-4300);
    expect(agendaSignedCents(income)).toBe(700000);
  });
});

describe("cellLabel", () => {
  it("carrega data, saldo, movimento e eventos", () => {
    const m = build();
    const label = cellLabel(dayCell(m, 12));
    expect(label).toContain("12 de julho");
    // `formatBRL` cola "R$" ao número com NBSP.
    expect(label).toContain("Saldo R$ 5.569,65");
    expect(label).toContain("Menor saldo do mês");
  });

  it("distingue passado sem dados de projeção indisponível", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 6,
      today: TODAY,
      realized: [],
      forecast: [],
    });
    expect(cellLabel(dayCell(m, 3))).toContain("Sem dados");
    expect(cellLabel(dayCell(m, 20))).toContain("Projeção indisponível");
  });

  it("um dia futuro declara-se previsto", () => {
    const m = build();
    expect(cellLabel(dayCell(m, 20))).toContain("Previsto");
    expect(cellLabel(dayCell(m, 12))).not.toContain("Previsto");
  });
});

describe("navegação de mês e dia", () => {
  it("addMonths cruza anos nos dois sentidos", () => {
    expect(addMonths("2026-07", 1)).toBe("2026-08");
    expect(addMonths("2026-01", -1)).toBe("2025-12");
    expect(addMonths("2025-12", 1)).toBe("2026-01");
  });

  it("shiftIso absorve fronteiras de mês e ano", () => {
    expect(shiftIso("2026-07-15", 7)).toBe("2026-07-22");
    expect(shiftIso("2026-07-01", -1)).toBe("2026-06-30");
    expect(shiftIso("2025-12-31", 1)).toBe("2026-01-01");
  });
});

describe("shortDate — a data curta do olho", () => {
  it("imprime dia/mês com dois dígitos", () => {
    expect(shortDate("2026-06-10")).toBe("10/06");
    expect(shortDate("2026-12-01")).toBe("01/12");
  });
});

describe("formatadores de célula", () => {
  it("cellMoney arredonda a reais inteiros com milhar", () => {
    expect(cellMoney(1_520_296)).toBe("15.203");
    expect(cellMoney(-810_158)).toBe("−8.102");
  });

  it("cellSigned carrega o sinal e cala abaixo de meio real", () => {
    expect(cellSigned(190_270)).toBe("+1.903");
    expect(cellSigned(-2_600)).toBe("−26");
    expect(cellSigned(3)).toBe("");
    expect(cellSigned(0)).toBe("");
  });
});

// ---------------------------------------------------------------------------
// A leitura do mês: a manchete, os dias que o marcam, a faixa que a grade
// pinta e a série do trilho. Tudo derivado do mês já montado.
// ---------------------------------------------------------------------------

describe("gridBand — a cor só aparece quando o dia aperta", () => {
  it("cala nas faixas boas", () => {
    expect(gridBand(1_000_000)).toBeNull();
    expect(gridBand(200_001)).toBeNull(); // folga
    expect(gridBand(200_000)).toBeNull(); // ok — R$ 2.000 cai em "ok"
    expect(gridBand(100_001)).toBeNull();
  });

  it("acende de Apertado para baixo, nos limiares da planilha", () => {
    expect(gridBand(100_000)).toBe("tight"); // R$ 1.000 cai em "apertado"
    expect(gridBand(0)).toBe("tight");
    expect(gridBand(-1)).toBe("negative");
    expect(gridBand(-49_999)).toBe("negative");
    expect(gridBand(-50_001)).toBe("critical");
  });

  it("dado ausente não pinta nada", () => {
    expect(gridBand(null)).toBeNull();
  });
});

describe("monthMarks — os dias que decidem o mês", () => {
  it("abre pelo dia do vale e segue com as entradas", () => {
    const marks = monthMarks(build());
    expect(marks.map((m) => m.kind)).toEqual([
      "lowest-out",
      "income",
      "income",
      "income",
      "income",
    ]);
    expect(marks[0]?.iso).toBe("2026-07-12");
    expect(marks[0]?.cents).toBe(556_965);
  });

  it("as entradas saem em ordem cronológica, com o valor que entrou", () => {
    const incomes = monthMarks(build()).filter((m) => m.kind === "income");
    expect(incomes.map((m) => m.iso)).toEqual([
      "2026-07-01",
      "2026-07-07",
      "2026-07-08",
      "2026-07-20", // a projetada conta como qualquer outra
    ]);
    expect(incomes[2]?.cents).toBe(100);
    expect(incomes[3]?.cents).toBe(199_865);
  });

  it("separa os papéis quando o vale e a maior saída caem em dias diferentes", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 5,
      today: "2026-06-28",
      realized: [
        row({ date: "2026-06-01", balance_cents: 900_000 }),
        row({ date: "2026-06-02", balance_cents: 300_000 }), // queda de 600k
        row({ date: "2026-06-20", balance_cents: 250_000 }), // vale, queda de 50k
      ],
      forecast: [],
    });
    const marks = monthMarks(m);
    expect(marks.map((k) => k.kind)).toEqual(["lowest", "out"]);
    expect(marks[0]?.iso).toBe("2026-06-20");
    expect(marks[1]?.iso).toBe("2026-06-02");
    expect(marks[1]?.cents).toBe(-600_000);
  });

  it("quando menor saldo e maior saída caem no mesmo dia, a data aparece uma vez", () => {
    // Julho da fixture: o dia 12 é o vale E a maior queda do mês.
    const marks = monthMarks(build());
    const dozes = marks.filter((m) => m.iso === "2026-07-12");
    expect(dozes).toHaveLength(1);
    expect(dozes[0]?.kind).toBe("lowest-out");
    expect(dozes[0]?.label).toBe("Menor saldo e maior saída");
    expect(dozes[0]?.cents).toBe(556_965);
    expect(dozes[0]?.extraCents).toBe(556_965 - 1_367_123);
  });

  it("mês sem corrente não tem o que marcar", () => {
    expect(monthMarks(build({ realized: [], forecast: [] }))).toEqual([]);
  });
});

describe("monthHeadline — a forma do mês em palavras", () => {
  it("respira na maior entrada DEPOIS do vale, não na primeira do mês", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 5,
      today: "2026-06-10",
      realized: [
        row({ date: "2026-06-01", balance_cents: 910_000, income_cents: 700_000 }),
        row({ date: "2026-06-09", balance_cents: 845_800 }),
      ],
      forecast: [
        row({ date: "2026-06-20", balance_cents: 575_200 }),
        row({ date: "2026-06-25", balance_cents: 1_275_200, income_cents: 700_000 }),
      ],
    });
    expect(monthHeadline(m, "Junho")).toBe("Junho afunda no dia 20 e respira no 25.");
  });

  it("sem entrada no mês, fala só do vale", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 5,
      today: "2026-06-10",
      realized: [row({ date: "2026-06-03", balance_cents: 400_000 })],
      forecast: [row({ date: "2026-06-18", balance_cents: 120_000 })],
    });
    expect(monthHeadline(m, "Junho")).toBe("Junho afunda no dia 18.");
  });

  it("quando a entrada vem antes do vale, a ordem cronológica manda", () => {
    const m = buildCalendarMonth({
      year: 2026,
      month0: 5,
      today: "2026-06-28",
      realized: [
        row({ date: "2026-06-05", balance_cents: 900_000, income_cents: 500_000 }),
        row({ date: "2026-06-22", balance_cents: 100_000 }),
      ],
      forecast: [],
    });
    expect(monthHeadline(m, "Junho")).toBe("Junho respira no dia 5 e afunda no 22.");
  });

  it("mês sem corrente não tem manchete", () => {
    expect(monthHeadline(build({ realized: [], forecast: [] }), "Julho")).toBeNull();
  });
});

describe("railSeries — o trilho do saldo", () => {
  it("normaliza x pelo dia e v pelo valor, com o vale no piso", () => {
    const s = railSeries(build());
    expect(s).not.toBeNull();
    const pts = s!.points;
    expect(pts[0]?.x).toBe(0);
    expect(pts[pts.length - 1]?.x).toBe(1);
    const vale = pts.find((p) => p.iso === "2026-07-12");
    expect(vale?.v).toBe(0);
    const topo = pts.find((p) => p.iso === "2026-07-08");
    expect(topo?.v).toBe(1);
  });

  it("cada ponto carrega o evento de entrada — a tela não reanda a matriz", () => {
    const s = railSeries(build());
    expect(s!.points.find((p) => p.iso === "2026-07-08")?.hasIncome).toBe(true);
    expect(s!.points.find((p) => p.iso === "2026-07-09")?.hasIncome).toBe(false);
  });

  it("marca onde a corrente vira projeção", () => {
    const s = railSeries(build());
    expect(s!.points.find((p) => p.iso === "2026-07-14")?.isFuture).toBe(false);
    expect(s!.points.find((p) => p.iso === "2026-07-16")?.isFuture).toBe(true);
    expect(s!.todayIndex).toBe(s!.points.findIndex((p) => p.iso === "2026-07-15"));
  });

  it("dia sem corrente não entra na série", () => {
    const s = railSeries(
      buildCalendarMonth({
        year: 2026,
        month0: 5,
        today: "2026-06-10",
        realized: [row({ date: "2026-06-02", balance_cents: 100_000 })],
        forecast: [row({ date: "2026-06-20", balance_cents: 300_000 })],
      }),
    );
    expect(s!.points.map((p) => p.iso)).toEqual(["2026-06-02", "2026-06-20"]);
  });

  it("mês inteiro no mesmo saldo não colapsa o traço na borda", () => {
    const flat = buildCalendarMonth({
      year: 2026,
      month0: 5,
      today: "2026-06-10",
      realized: [
        row({ date: "2026-06-01", balance_cents: 300_000 }),
        row({ date: "2026-06-02", balance_cents: 300_000 }),
      ],
      forecast: [row({ date: "2026-06-20", balance_cents: 300_000 })],
    });
    const s = railSeries(flat);
    expect(s!.points).toHaveLength(3);
    expect(s!.points.map((p) => p.v)).toEqual([0.5, 0.5, 0.5]);
  });

  it("um único ponto fica no meio da faixa, sem divisão por zero", () => {
    const s = railSeries(
      buildCalendarMonth({
        year: 2026,
        month0: 5,
        today: "2026-06-10",
        realized: [row({ date: "2026-06-02", balance_cents: 100_000 })],
        forecast: [],
      }),
    );
    expect(s!.points).toHaveLength(1);
    expect(s!.points[0]?.v).toBe(0.5);
    expect(s!.points[0]?.x).toBe(0);
  });

  it("mês sem corrente não tem trilho", () => {
    expect(railSeries(build({ realized: [], forecast: [] }))).toBeNull();
  });
});
