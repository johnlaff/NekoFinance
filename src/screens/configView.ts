import type { AuthStatus } from "../lib/api";

export type GreetTone = "ok" | "warn" | "muted";

/** Pílula de estado do veredito de Configurações. */
export interface GreetState {
  tone: GreetTone;
  /** Parte forte ("Conectado" / "Desconectado" / "Sessão expirada"). */
  headline: string;
  /** Complemento após o separador " · " (null → só a headline). */
  detail: string | null;
}

function pendingPhrase(n: number): string {
  return n === 1 ? "1 mudança aguardando" : `${n} mudanças aguardando`;
}

function conflictPhrase(n: number): string {
  return n === 1
    ? "Conflito de importação a resolver"
    : `${n} conflitos de importação a resolver`;
}

/**
 * Deriva a pílula de estado do greet a partir da conexão e das pendências.
 * O veredito diz a má notícia com o mesmo peso da boa: o pior fato disponível
 * ganha o complemento (conflito > mudanças aguardando > recência do sync), e o
 * ponto só fica verde quando não há nada bloqueando.
 */
export function greetState(
  auth: AuthStatus,
  pendingCount: number,
  conflictCount: number,
  syncLabel: string | null,
): GreetState {
  if (auth === "loading") {
    return { tone: "muted", headline: "Verificando conexão…", detail: null };
  }

  const headline =
    auth === "connected"
      ? "Conectado"
      : auth === "expired"
        ? "Sessão expirada"
        : "Desconectado";

  if (conflictCount > 0) {
    return { tone: "warn", headline, detail: conflictPhrase(conflictCount) };
  }
  if (pendingCount > 0) {
    return {
      tone: auth === "connected" ? "ok" : "warn",
      headline,
      detail: pendingPhrase(pendingCount),
    };
  }
  if (auth !== "connected") {
    return { tone: "warn", headline, detail: null };
  }
  return {
    tone: "ok",
    headline,
    detail: syncLabel ? `Sincronizado ${syncLabel}` : null,
  };
}
