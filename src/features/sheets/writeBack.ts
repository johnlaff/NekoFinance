//! Lógica pura e constantes do write-back, fora do arquivo de componente para o Fast Refresh
//! tratar `WriteBackPreview.tsx` como módulo só-de-componentes. Sem JSX, sem hooks — testável.

import type { CellWrite } from "../../lib/api";

/** Rótulos pt-BR por tipo de célula do diff (apresentação). */
export const KIND_LABEL: Record<string, string> = {
  entrada: "Entrada",
  saida: "Saída",
  diario: "Diário",
  economia: "Economia",
};

// Tipos de célula que tocam APENAS colunas de valor (sem fórmula): o caminho rápido só os aceita.
// `balance`/`date` são colunas de fórmula (FORMULA_ONLY_FIELDS no backend) e nunca entram aqui.
const SAFE_KINDS = new Set(["entrada", "saida", "diario"]);

/**
 * Caminho rápido "Sincronizar" (plano 039): COLAPSA os cliques do fluxo (banner → expandir →
 * gerar prévia → aprovar → confirmar) num único botão + confirmação — NUNCA colapsa uma checagem
 * de segurança. Esta função decide se o diff pendente é seguro para o atalho. É verdadeira só quando
 * TODAS valem:
 *   1. `enabled` — a flag-mestre do write-back está ligada.
 *   2. `conflictCount === 0` — nenhum conflito de importação bloqueando o envio.
 *   3. `!multiCardWarning` — sem cenário ambíguo de data de fatura.
 *   4. todas as células `changed` têm `kind` em SAFE_KINDS (só valor, sem coluna de fórmula).
 *   5. `previewRevision` não-vazia — uma prévia fresca acabou de ser calculada (token de frescura).
 * Fora disso, a UI cai no fluxo multi-etapas completo. As salvaguardas do backend (re-checagem de
 * frescura via modifiedTime, gate de conflito, blocklist de colunas de fórmula) seguem rodando
 * SEMPRE — este atalho só evita os cliques manuais nos casos rotineiros.
 */
export function isSafeForFastPath(
  enabled: boolean,
  conflictCount: number,
  multiCardWarning: boolean,
  changed: CellWrite[],
  previewRevision: string | null,
): boolean {
  return (
    enabled &&
    conflictCount === 0 &&
    !multiCardWarning &&
    changed.length > 0 &&
    changed.every((c) => SAFE_KINDS.has(c.kind)) &&
    !!previewRevision
  );
}
