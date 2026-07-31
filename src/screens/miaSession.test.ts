import { describe, expect, it, vi, beforeEach } from "vitest";
import { mockCommands, mockInvoke } from "../test/commands";
import {
  askInSession,
  clearSession,
  hydrateSession,
  resetSession,
  sessionLog,
} from "./miaSession";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// A sessão é o único lugar que sabe converter a conversa guardada (formato do banco) na forma
// que a tela desenha, e o único que decide quando gravar. Aqui provamos a hidratação
// (conversão + ordem + resiliência a registro malformado), a gravação de cada exchange e o
// apagar de verdade.

const OK_ANSWER = {
  text: [{ t: "text", s: "Tudo certo." }],
  provenance: "calculo" as const,
};

describe("hydrateSession", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    resetSession();
  });

  it("converte as linhas guardadas em pares pergunta/resposta, na ordem", async () => {
    mockCommands({
      load_mia_conversation: [
        {
          author: "voce",
          question: "Quanto posso gastar hoje?",
          answer: null,
          at_iso: "2026-07-30T10:00:00Z",
        },
        {
          author: "mia",
          question: null,
          answer: OK_ANSWER,
          at_iso: "2026-07-30T10:00:01Z",
        },
      ],
    });

    const log = await hydrateSession();

    expect(log).toHaveLength(2);
    expect(log[0]).toMatchObject({
      author: "voce",
      question: "Quanto posso gastar hoje?",
    });
    expect(log[1]).toMatchObject({ author: "mia", answer: OK_ANSWER });
  });

  it("descarta uma resposta malformada em vez de derrubar a hidratação", async () => {
    mockCommands({
      load_mia_conversation: [
        {
          author: "voce",
          question: "oi",
          answer: null,
          at_iso: "2026-07-30T10:00:00Z",
        },
        {
          author: "mia",
          question: null,
          answer: { garbage: true },
          at_iso: "2026-07-30T10:00:01Z",
        },
        {
          author: "voce",
          question: "e agora?",
          answer: null,
          at_iso: "2026-07-30T10:01:00Z",
        },
        {
          author: "mia",
          question: null,
          answer: OK_ANSWER,
          at_iso: "2026-07-30T10:01:01Z",
        },
      ],
    });

    const log = await hydrateSession();

    expect(log).toHaveLength(3);
    expect(log.filter((m) => m.author === "mia")).toHaveLength(1);
  });

  it("nunca sobrescreve mensagens já na sessão nem repete a busca (idempotente)", async () => {
    mockCommands({ load_mia_conversation: [], append_mia_exchange: undefined });
    askInSession("pergunta feita antes do banco responder", null, false);
    expect(sessionLog()).toHaveLength(2);
    mockInvoke.mockClear();

    const log = await hydrateSession();
    const again = await hydrateSession();

    expect(log).toHaveLength(2);
    expect(again).toHaveLength(2);
    expect(mockInvoke).not.toHaveBeenCalledWith("load_mia_conversation");
  });

  it("uma falha ao carregar não derruba a tela — a sessão segue vazia", async () => {
    mockCommands({ load_mia_conversation: new Error("db indisponível") });
    const spy = vi.spyOn(console, "error").mockImplementation(() => undefined);

    const log = await hydrateSession();

    expect(log).toEqual([]);
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe("persistência de cada exchange", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    resetSession();
  });

  it("askInSession grava o par pergunta/resposta no piso offline", () => {
    mockCommands({ append_mia_exchange: undefined });

    askInSession("Como está a economia do ano?", null, false);

    expect(mockInvoke).toHaveBeenCalledWith(
      "append_mia_exchange",
      expect.objectContaining({ question: "Como está a economia do ano?" }),
    );
  });
});

describe("clearSession", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    resetSession();
  });

  it("apaga a conversa no banco e zera o log local", async () => {
    mockCommands({
      append_mia_exchange: undefined,
      delete_mia_conversation: undefined,
    });
    askInSession("pergunta", null, false);
    expect(sessionLog()).toHaveLength(2);

    await clearSession();

    expect(mockInvoke).toHaveBeenCalledWith("delete_mia_conversation");
    expect(sessionLog()).toEqual([]);
  });
});
