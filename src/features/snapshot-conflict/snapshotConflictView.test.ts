import { describe, expect, it } from "vitest";
import {
  AFTER_POOL_CLOSED_SUFFIX,
  CHECKIN_REFUSED_STALE_CONFLICT,
  conflictGestureDatedLabel,
  conflictGestureTypeLabel,
  conflictRemoteDeviceLabel,
  isAfterPoolClosedError,
  resolveConflictErrorMessage,
} from "./snapshotConflictView";
import type { DriveConflictGesture } from "../../lib/api";

function gesture(overrides: Partial<DriveConflictGesture> = {}): DriveConflictGesture {
  return {
    at: "2026-08-12 09:00:00",
    event_type: "import",
    entity_type: "transaction",
    source_sheet: null,
    ...overrides,
  };
}

describe("conflictGestureTypeLabel", () => {
  it("traduz os event_type conhecidos", () => {
    expect(conflictGestureTypeLabel(gesture({ event_type: "import" }))).toBe(
      "Importação da planilha",
    );
    expect(conflictGestureTypeLabel(gesture({ event_type: "write_back" }))).toBe(
      "Escrita de volta na planilha",
    );
  });

  it("adiciona a aba quando source_sheet está presente", () => {
    expect(
      conflictGestureTypeLabel(
        gesture({ event_type: "import", source_sheet: "Diário" }),
      ),
    ).toBe("Importação da planilha (aba Diário)");
  });

  it("cai num rótulo genérico para event_type desconhecido, nunca trava a tela", () => {
    expect(conflictGestureTypeLabel(gesture({ event_type: "split" }))).toBe(
      "Gesto (split)",
    );
  });
});

describe("conflictGestureDatedLabel", () => {
  it("usa a recência quando o timestamp é reconhecível", () => {
    const now = new Date("2026-08-12T09:18:00Z").getTime();
    const label = conflictGestureDatedLabel(
      gesture({ at: "2026-08-12 09:00:00", event_type: "import" }),
      now,
    );
    expect(label).toBe("Importação da planilha — há 18 min");
  });

  it("cai na data crua quando o timestamp não é de um formato reconhecido", () => {
    const label = conflictGestureDatedLabel(
      gesture({ at: "não é uma data", event_type: "write_back" }),
    );
    expect(label).toBe("Escrita de volta na planilha — não é uma data");
  });
});

describe("conflictRemoteDeviceLabel", () => {
  it("identifica o outro aparelho pelos 8 primeiros caracteres do id", () => {
    const label = conflictRemoteDeviceLabel(
      {
        device_id: "abcdef12-3456-7890-abcd-ef1234567890",
        sequence: 5,
        created_at: "2026-08-12T09:00:00Z",
        app_version: "0.2.1",
        schema_version: 1,
      },
      "aparelho-deste-dono",
    );
    expect(label).toBe("de outro aparelho (abcdef12)");
  });

  it("reconhece o PRÓPRIO id em vez de cravar 'outro aparelho' (issue #446 item 11b)", () => {
    // Cenário do check-in morto: a resolução de conflito de uma sessão anterior publicou, mas a
    // gravação local morreu antes de terminar — o manifest que esta tela busca a seguir pode ser
    // a NOSSA PRÓPRIA publicação, nunca de fato "outro aparelho".
    const label = conflictRemoteDeviceLabel(
      {
        device_id: "aparelho-deste-dono",
        sequence: 6,
        created_at: "2026-08-12T09:00:00Z",
        app_version: "0.2.1",
        schema_version: 1,
      },
      "aparelho-deste-dono",
    );
    expect(label).toBe("deste aparelho");
  });
});

describe("resolveConflictErrorMessage", () => {
  it("mostra verbatim um erro atrás do prefixo de consentimento obsoleto", () => {
    expect(resolveConflictErrorMessage(new Error(CHECKIN_REFUSED_STALE_CONFLICT))).toBe(
      CHECKIN_REFUSED_STALE_CONFLICT,
    );
  });

  it("mostra verbatim um erro atrás do prefixo de restauração recusada", () => {
    const message =
      "Restauração recusada: o snapshot do outro aparelho foi publicado por uma versão " +
      "mais nova do Neko Finance (schema 9 > 7) — atualize o app antes de continuar.";
    expect(resolveConflictErrorMessage(new Error(message))).toBe(message);
  });

  it("cai no fallback calmo para um erro sem prefixo de contrato conhecido", () => {
    const message = resolveConflictErrorMessage(
      new Error("error returned from database: (code: 5) database is locked"),
    );
    expect(message).not.toContain("database is locked");
    expect(message).toContain("banco local está ocupado");
  });

  it("erro vazio cai no fallback dedicado desta tela, nunca no genérico de outra", () => {
    expect(resolveConflictErrorMessage(new Error(""))).toBe(
      "Não foi possível concluir a resolução do conflito.",
    );
  });
});

describe("isAfterPoolClosedError", () => {
  it("reconhece o sufixo compartilhado de todo erro pós-fechamento do pool", () => {
    expect(
      isAfterPoolClosedError(
        new Error(`trocar pelo snapshot baixado: IO${AFTER_POOL_CLOSED_SUFFIX}`),
      ),
    ).toBe(true);
    expect(
      isAfterPoolClosedError(
        new Error(
          `adotar estado pós-restauração: banco travado${AFTER_POOL_CLOSED_SUFFIX}`,
        ),
      ),
    ).toBe(true);
  });

  it("nunca reconhece um erro que não passou do ponto de não-retorno", () => {
    expect(isAfterPoolClosedError(new Error(CHECKIN_REFUSED_STALE_CONFLICT))).toBe(
      false,
    );
    expect(
      isAfterPoolClosedError(
        new Error(
          "Restauração recusada: o snapshot do outro aparelho foi publicado por uma versão " +
            "mais nova do Neko Finance (schema 9 > 7) — atualize o app antes de continuar.",
        ),
      ),
    ).toBe(false);
  });
});
