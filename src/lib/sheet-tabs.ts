/**
 * Classificação de abas da planilha do método.
 *
 * Abas-ano (2025, 2026, …) têm o layout de blocos mensais Data|Entrada|Saída|Diário|Saldo
 * e são as únicas importáveis como transações. Abas de métricas (Economia, Totais) têm
 * layout próprio (mês|Entradas|Economia|%) — importá-las como transações produziria lixo;
 * elas terão um importador de métricas dedicado (spec 010, "Aba Economia como métricas").
 */

const METRIC_TAB_NAMES = new Set(["economia", "totais", "total"]);

function normalize(title: string): string {
  return title
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}

export function isMetricTab(title: string): boolean {
  return METRIC_TAB_NAMES.has(normalize(title));
}

export function isTransactionTab(title: string): boolean {
  return !isMetricTab(title);
}
