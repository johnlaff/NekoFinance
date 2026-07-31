import { describe, expect, it, vi, beforeEach, type Mock } from "vitest";
import { mockCommands, mockInvoke } from "../test/commands";
import type * as Api from "../lib/api";
import { runMiaRound, type MiaScreenEvent } from "../lib/api";
import {
  approveSessionProposal,
  askInSession,
  askInSessionRuntime,
  clearSession,
  editSessionProposal,
  hydrateSession,
  rejectSessionProposal,
  resetSession,
  sessionLog,
  sessionProposals,
} from "./miaSession";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// `runMiaRound` fala com o canal do Tauri, fora do alcance de `mockInvoke` — as demais funções
// de `lib/api` seguem reais (e roteadas por `mockInvoke`, como no resto do arquivo).
vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof Api>();
  return { ...actual, runMiaRound: vi.fn() };
});
const runMiaRoundMock = runMiaRound as Mock<typeof runMiaRound>;

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

const PROPOSAL_ENVELOPE = {
  id: 1,
  schema_version: 1 as const,
  payload: {
    kind: "expense" as const,
    amount_cents: 5_000,
    date: "2026-07-31",
    description: "Mercado",
    is_fixed: false,
    tag_ids: [],
  },
  data_revision: "rev-1",
  issued_at: "2026-07-31T10:00:00.000Z",
  expires_at: "2099-01-01T00:00:00.000Z",
  hash: "hash-abc",
};

/** Roteiriza uma rodada do runtime que chega com uma proposta — a única forma de a sessão
 *  aprender sobre um cartão (`sessionProposals` só é preenchido por `proposal_ready`). */
function runRoundWithProposal(): Promise<void> {
  runMiaRoundMock.mockImplementation(
    (_question: string, onEvent: (e: MiaScreenEvent) => void) => {
      onEvent({ kind: "run_started", run_id: "r1", model: "m", endpoint: "e" });
      onEvent({ kind: "proposal_ready", id: "prop-1", proposal: PROPOSAL_ENVELOPE });
      onEvent({
        kind: "answer_ready",
        text: "Aqui está a proposta.",
        provenance: "calculo",
      });
      onEvent({ kind: "run_finished", stop: "answered" });
      return Promise.resolve("r1");
    },
  );
  return askInSessionRuntime("lança 50 no mercado", () => undefined);
}

describe("cartão de proposta na sessão", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    runMiaRoundMock.mockReset();
    resetSession();
  });

  it("proposal_ready cria o cartão, alcançável por sessionProposals", async () => {
    mockCommands({ append_mia_exchange: undefined });
    await runRoundWithProposal();

    const card = sessionProposals()["prop-1"];
    expect(card?.gesture).toBe("proposta");
    expect(card?.draft.amount_cents).toBe(5_000);
  });

  it("editSessionProposal atualiza o rascunho e move o cartão para 'editando'", async () => {
    mockCommands({ append_mia_exchange: undefined });
    await runRoundWithProposal();

    const updated = editSessionProposal("prop-1", "amount_cents", 9_000);

    expect(updated?.gesture).toBe("editando");
    expect(sessionProposals()["prop-1"]?.draft.amount_cents).toBe(9_000);
  });

  it("approveSessionProposal manda o hash do envelope ORIGINAL e o rascunho corrente", async () => {
    mockCommands({
      append_mia_exchange: undefined,
      approve_mia_proposal: "tx-nova",
    });
    await runRoundWithProposal();
    editSessionProposal("prop-1", "amount_cents", 9_000);

    const updated = await approveSessionProposal("prop-1");

    expect(mockInvoke).toHaveBeenCalledWith(
      "approve_mia_proposal",
      expect.objectContaining({ proposalId: 1, hash: "hash-abc" }),
    );
    const sentPayload = JSON.parse(
      (
        mockInvoke.mock.calls.find((c) => c[0] === "approve_mia_proposal")?.[1] as {
          payloadJson: string;
        }
      ).payloadJson,
    ) as { amount_cents: number };
    expect(sentPayload.amount_cents).toBe(9_000);
    expect(updated?.gesture).toBe("aprovada");
    expect(updated?.approvedTransactionId).toBe("tx-nova");
  });

  it("uma falha do backend vira erro honesto no cartão — nunca alert genérico", async () => {
    mockCommands({
      append_mia_exchange: undefined,
      approve_mia_proposal: new Error("proposta expirada"),
    });
    await runRoundWithProposal();

    const updated = await approveSessionProposal("prop-1");

    expect(updated?.gesture).toBe("proposta");
    expect(updated?.error).toBeTruthy();
  });

  it("rejectSessionProposal recusa no backend e marca o cartão como recusado", async () => {
    mockCommands({
      append_mia_exchange: undefined,
      reject_mia_proposal: undefined,
    });
    await runRoundWithProposal();

    const updated = await rejectSessionProposal("prop-1");

    expect(mockInvoke).toHaveBeenCalledWith("reject_mia_proposal", {
      proposalId: 1,
    });
    expect(updated?.gesture).toBe("recusada");
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
