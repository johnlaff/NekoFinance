/**
 * Classificação de abas da planilha do método.
 *
 * Abas-ano (2025, 2026, …) têm o layout de blocos mensais Data|Entrada|Saída|Diário|Saldo
 * e são as únicas importáveis como transações. Abas de métricas (Economia, Totais) têm
 * layout próprio (mês|Entradas|Economia|%) — importá-las como transações produziria lixo;
 * elas terão um importador de métricas dedicado (spec 010, "Aba Economia como métricas").
 */

const METRIC_TAB_NAMES = new Set(["economia", "totais", "total", "metricas"]);

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

/**
 * A aba "Economia" \u00e9 m\u00e9trica (n\u00e3o tem o layout de blocos mensais), MAS tem importador dedicado
 * (`importEconomiaSheet` \u2192 poupan\u00e7a por m\u00eas). Por isso \u00e9 tratada \u00e0 parte das demais m\u00e9tricas
 * (Totais/m\u00e9tricas), que ainda n\u00e3o t\u00eam importador.
 */
export function isEconomiaTab(title: string): boolean {
  return normalize(title) === "economia";
}

export function isTransactionTab(title: string): boolean {
  return !isMetricTab(title);
}
