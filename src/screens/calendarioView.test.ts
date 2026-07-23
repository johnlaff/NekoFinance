import { describe, expect, it } from "vitest";
import {
  addMonths,
  agendaTransactions,
  buildCalendarMonth,
  cellLabel,
  cellMoney,
  cellSigned,
  dayComponents,
  shiftIso,
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
