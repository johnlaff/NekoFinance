import { describe, expect, it } from "vitest";
import type { LineItem, TransactionRow } from "../lib/api";
import {
  applySearch,
  buildDayGroups,
  cellHeadLabel,
  countRows,
  daymarkLabel,
  emptyListCopy,
  monthTitle,
  normalizeQuery,
  sectionLabel,
  splitAroundToday,
  sumSigned,
  toMovementType,
} from "./lancamentosView";

const TODAY = "2026-07-15";

function txn(overrides: Partial<TransactionRow> & { id: string }): TransactionRow {
  return {
    type: "expense",
    amount: -1000,
    description: "Lançamento",
    date: "2026-07-10",
    payment_method: "",
    is_projection: false,
    is_fixed: false,
    owners: [],
    tags: [],
    provenance: "importado",
    line_items: [],
    due_date: null,
    installment_index: null,
    installment_total: null,
    has_refund_link: false,
    ...overrides,
  };
}

function item(
  overrides: Partial<LineItem> & { id: string; amount_cents: number },
): LineItem {
  return {
    transaction_id: "t1",
    description: "Item",
    position: 0,
    kind: "cartao",
    section: null,
    ...overrides,
  };
}

describe("toMovementType", () => {
  it("mapeia os 5 tipos do método", () => {
    expect(toMovementType(txn({ id: "a", type: "income" }))).toBe("entrada");
    expect(toMovementType(txn({ id: "b", type: "transfer" }))).toBe("economia");
    expect(toMovementType(txn({ id: "c", is_fixed: true }))).toBe("saida");
    expect(toMovementType(txn({ id: "d", payment_method: "credit" }))).toBe("cartao");
    expect(toMovementType(txn({ id: "e" }))).toBe("diario");
  });
});

describe("rótulos", () => {
  it("monthTitle capitaliza e inclui o ano", () => {
    expect(monthTitle("2026-07")).toBe("Julho de 2026");
  });

  it("daymarkLabel escreve o dia por extenso", () => {
    expect(daymarkLabel("2026-07-12")).toBe("Domingo, 12 de julho");
  });

  it("sectionLabel limpa a pontuação da gramática da nota", () => {
    expect(sectionLabel("CONTAS |")).toBe("Contas");
    expect(sectionLabel("FATURAS:")).toBe("Faturas");
    expect(sectionLabel(null)).toBeNull();
    expect(sectionLabel("  |")).toBeNull();
  });

  it("cellHeadLabel só alega célula para colunas da planilha", () => {
    expect(cellHeadLabel("saida")).toBe("Saída — Total da célula");
    expect(cellHeadLabel("entrada")).toBe("Entrada — Total da célula");
    expect(cellHeadLabel("cartao")).toBe("Cartão — Soma na fatura");
    expect(cellHeadLabel("economia")).toBe("Economia — Total do dia");
  });
});

describe("buildDayGroups — célula×nota", () => {
  // O caso real da planilha: célula do dia 12 = 8.101,58; a nota soma 8.101,28.
  const day12 = txn({
    id: "cel-12",
    amount: -810158,
    description: "Saída",
    date: "2026-07-12",
    is_fixed: true,
    line_items: [
      item({ id: "li-1", amount_cents: 400066, description: "Bradesco João", section: "CARTÕES |", position: 0 }),
      item({ id: "li-2", amount_cents: 407764, description: "Bradesco Gio", section: null, position: 1 }),
      item({ id: "li-3", amount_cents: 2298, description: "Inter", section: null, position: 2 }),
    ],
  });

  it("explode a nota em linhas e preserva a célula como autoridade", () => {
    const days = buildDayGroups([day12], TODAY);
    expect(days).toHaveLength(1);
    const cell = days[0]!.cells[0]!;
    expect(cell.type).toBe("saida");
    expect(cell.totalCents).toBe(810158);
    expect(cell.rows).toHaveLength(3);
    expect(cell.rows[0]!.name).toBe("Bradesco João");
    expect(cell.rows[0]!.context).toBe("Cartões");
    expect(cell.rows[0]!.signedCents).toBe(-400066);
    expect(cell.rows[0]!.kind).toBe("cartao");
  });

  it("acusa a diferença célula×nota (R$ 0,30) sem criar item", () => {
    const days = buildDayGroups([day12], TODAY);
    const cell = days[0]!.cells[0]!;
    expect(cell.diffCents).toBe(30);
    expect(countRows(days)).toBe(3);
  });

  it("tolera 1 centavo de arredondamento", () => {
    const quaseBate = txn({
      id: "q1",
      amount: -1001,
      is_fixed: true,
      line_items: [item({ id: "li-q", amount_cents: 1000, description: "Só" })],
    });
    const days = buildDayGroups([quaseBate], TODAY);
    expect(days[0]!.cells[0]!.diffCents).toBe(0);
  });

  it("separa células por tipo dentro do dia, na ordem canônica", () => {
    const entrada = txn({ id: "e1", type: "income", amount: 24, date: "2026-07-10" });
    const saida = txn({ id: "s1", amount: -228524, is_fixed: true, date: "2026-07-10" });
    const days = buildDayGroups([saida, entrada], TODAY);
    expect(days[0]!.cells.map((c) => c.type)).toEqual(["entrada", "saida"]);
  });

  it("linha simples carrega pílulas do lançamento; item de nota não repete", () => {
    const parcela = txn({
      id: "p1",
      amount: -100651,
      installment_index: 10,
      installment_total: 36,
      has_refund_link: true,
    });
    const days = buildDayGroups([parcela, day12], TODAY);
    const simple = days.find((d) => d.date === "2026-07-10")!.cells[0]!.rows[0]!;
    expect(simple.pills.installment).toBe("10/36");
    expect(simple.pills.refund).toBe(true);
    const exploded = days.find((d) => d.date === "2026-07-12")!.cells[0]!.rows[0]!;
    expect(exploded.pills.installment).toBeNull();
    expect(exploded.pills.tags).toHaveLength(0);
  });

  it("marca previsto por projeção ou data futura", () => {
    const proj = txn({ id: "pr1", is_projection: true });
    const futuro = txn({ id: "fu1", date: "2026-07-30" });
    const days = buildDayGroups([proj, futuro], TODAY);
    const all = days.flatMap((d) => d.cells.flatMap((c) => c.rows));
    expect(all.every((r) => r.pills.previsto)).toBe(true);
  });

  it("contexto de vencimento conjuga pelo calendário", () => {
    const vencida = txn({ id: "v1", due_date: "2026-07-12", is_fixed: true });
    const aVencer = txn({ id: "a1", due_date: "2026-08-12", is_fixed: true });
    const days = buildDayGroups([vencida, aVencer], TODAY);
    const rows = days[0]!.cells[0]!.rows;
    expect(rows.find((r) => r.key === "v1")!.context).toBe("Saída · venceu 12 de julho");
    expect(rows.find((r) => r.key === "a1")!.context).toBe("Saída · vence 12 de agosto");
  });

  it("tags de lançamento itemizado sobem para o cel-head", () => {
    const tag = { id: "tg1", name: "Gio", color: "#fff", emoji: null };
    const comTag = { ...day12, tags: [tag] };
    const days = buildDayGroups([comTag], TODAY);
    expect(days[0]!.cells[0]!.tags).toEqual([tag]);
  });
});

describe("applySearch", () => {
  const days = buildDayGroups(
    [
      txn({
        id: "cel-12",
        amount: -810158,
        date: "2026-07-12",
        is_fixed: true,
        line_items: [
          item({ id: "li-1", amount_cents: 400066, description: "Bradesco João" }),
          item({ id: "li-2", amount_cents: 407764, description: "Bradesco Gio" }),
          item({ id: "li-3", amount_cents: 2298, description: "Inter" }),
        ],
      }),
      txn({ id: "alug", amount: -148474, description: "Aluguel", date: "2026-07-10", is_fixed: true }),
    ],
    TODAY,
  );

  it("filtra por item, ignora caixa e acento", () => {
    const found = applySearch(days, "JOAO");
    expect(countRows(found)).toBe(1);
    expect(found[0]!.cells[0]!.rows[0]!.name).toBe("Bradesco João");
  });

  it("esconde a reconciliação com busca ativa (subconjunto não compara com a célula)", () => {
    const found = applySearch(days, "Bradesco");
    expect(found.find((d) => d.date === "2026-07-12")!.cells[0]!.diffCents).toBe(0);
  });

  it("derruba dias sem sobreviventes", () => {
    const found = applySearch(days, "Aluguel");
    expect(found).toHaveLength(1);
    expect(found[0]!.date).toBe("2026-07-10");
  });

  it("normalizeQuery remove diacríticos", () => {
    expect(normalizeQuery("João Áçãô")).toBe("joao acao");
  });
});

describe("splitAroundToday", () => {
  it("passado desce do mais recente; futuro sobe do mais próximo", () => {
    const days = buildDayGroups(
      [
        txn({ id: "d10", date: "2026-07-10" }),
        txn({ id: "d12", date: "2026-07-12" }),
        txn({ id: "d15", date: "2026-07-15" }),
        txn({ id: "d26", date: "2026-07-26" }),
        txn({ id: "d30", date: "2026-07-30" }),
      ],
      TODAY,
    );
    const { future, past } = splitAroundToday(days, TODAY);
    expect(past.map((d) => d.date)).toEqual(["2026-07-15", "2026-07-12", "2026-07-10"]);
    expect(future.map((d) => d.date)).toEqual(["2026-07-26", "2026-07-30"]);
  });

  it("sumSigned soma com sinal (entrada positiva)", () => {
    const days = buildDayGroups(
      [
        txn({ id: "in", type: "income", amount: 5000 }),
        txn({ id: "out", amount: -2000 }),
      ],
      TODAY,
    );
    expect(sumSigned(days)).toBe(3000);
  });
});

describe("emptyListCopy", () => {
  it("prioriza a busca, cita o termo", () => {
    expect(
      emptyListCopy({ query: "pix", filterName: "Diário", monthName: "julho", cardMode: true }),
    ).toBe('Nada em julho para "pix". Limpe a busca ou troque o filtro.');
  });

  it("filtro de Diário no modo cartão ensina onde o variável vive", () => {
    expect(
      emptyListCopy({ query: "", filterName: "Diário", monthName: "julho", cardMode: true }),
    ).toBe(
      "Nenhum lançamento de diário em julho. No modo cartão, o variável vive nas faturas.",
    );
  });

  it("sem filtro nem busca, frase neutra", () => {
    expect(
      emptyListCopy({ query: "", filterName: null, monthName: "julho", cardMode: false }),
    ).toBe("Nenhum lançamento em julho.");
  });
});
