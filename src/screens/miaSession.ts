import {
  answerFor,
  localStamp,
  routeQuestion,
  type MiaFacts,
  type MiaMessage,
} from "./miaView";

// A conversa da sessão. Ela sobrevive à navegação entre telas (a tela remonta a cada troca)
// e morre com o app: transcript persistido e apagável é contrato do runtime do copiloto, e
// um segundo store aqui nasceria com regra de privacidade própria para conciliar depois.

let log: MiaMessage[] = [];
let seq = 1;

export function sessionLog(): MiaMessage[] {
  return log;
}

/** Registra a pergunta e a resposta como um par, e devolve a conversa nova. */
export function askInSession(question: string, facts: MiaFacts | null): MiaMessage[] {
  const at = localStamp();
  log = [
    ...log,
    { id: seq++, author: "voce", atISO: at, question },
    {
      id: seq++,
      author: "mia",
      atISO: at,
      answer: answerFor(routeQuestion(question), facts),
    },
  ];
  return log;
}

/** Zera a conversa — usado pelos testes para isolar cenários. */
export function resetSession(): void {
  log = [];
  seq = 1;
}
