export function safeErrorMessage(
  error: unknown,
  fallback = "Não foi possível concluir a ação. Tente novamente.",
): string {
  const raw = errorText(error).trim();
  if (!raw) return fallback;

  if (/database is locked|db locked|locked/i.test(raw)) {
    return "O banco local está ocupado. Tente novamente em alguns segundos.";
  }
  if (/auth|oauth|token|expired|unauthorized|forbidden/i.test(raw)) {
    return "A conexão precisa ser revisada em Configurações e privacidade.";
  }
  if (/network|fetch|request|timeout|offline/i.test(raw)) {
    return "A conexão falhou. Verifique a internet e tente novamente.";
  }

  return fallback;
}

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (typeof error === "number" || typeof error === "boolean") return String(error);
  return "";
}
