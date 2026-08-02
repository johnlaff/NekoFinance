/**
 * Helpers puros do ciclo do cartão usados pela tela Cartões: aritmética da
 * identidade mensal "YYYY-MM" e validação da fronteira do formulário.
 *
 * Fechamento e vencimento aceitam 1–31: um dia que não cabe no mês encurta
 * para o último dia dele, na derivação da data. A validação espelha a do
 * backend (`validate_cycle`) para o formulário recusar antes de submeter.
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
  if (!Number.isInteger(closingDay) || closingDay < 1 || closingDay > 31) {
    return "Fechamento deve ser entre 1 e 31.";
  }
  if (!Number.isInteger(dueDay) || dueDay < 1 || dueDay > 31) {
    return "Vencimento deve ser entre 1 e 31.";
  }
  // Fechamento e vencimento no MESMO mês precisam sobreviver a fevereiro: acima do dia 28 os
  // dois encurtam para o último dia e colidem. Fechar depois do vencimento não tem esse
  // problema — o fechamento passa a ser do mês anterior.
  if (closingDay < dueDay && closingDay >= 28) {
    return `Com fechamento no dia ${closingDay}, o vencimento precisa vir antes dele (do mês seguinte) ou até o dia 27 — em fevereiro os dois cairiam no mesmo dia.`;
  }
  return null;
}
