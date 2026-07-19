/**
 * Helpers puros do ciclo do cartão usados pela tela Cartões: aritmética da
 * identidade mensal "YYYY-MM" e validação da fronteira do formulário
 * (fechamento 1–28 para nunca pular fevereiro; vencimento 1–31).
 */
export function shiftCycleMonth(cycle: string, delta: number): string {
  const match = /^(\d{4})-(\d{2})$/.exec(cycle);
  if (!match) return cycle;

  const year = Number(match[1]);
  const monthIndex = Number(match[2]) - 1 + delta;
  const shiftedYear = year + Math.floor(monthIndex / 12);
  const shiftedMonth = (((monthIndex % 12) + 12) % 12) + 1;
  return `${shiftedYear}-${String(shiftedMonth).padStart(2, "0")}`;
}

export function validateCardCycle(closing: string, due: string): string | null {
  const closingDay = Number(closing);
  const dueDay = Number(due);
  if (!Number.isInteger(closingDay) || closingDay < 1 || closingDay > 28) {
    return "Fechamento deve ser entre 1 e 28.";
  }
  if (!Number.isInteger(dueDay) || dueDay < 1 || dueDay > 31) {
    return "Vencimento deve ser entre 1 e 31.";
  }
  return null;
}
