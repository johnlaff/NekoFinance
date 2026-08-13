import { describe, expect, it } from "vitest";
import {
  conflictGestureDatedLabel,
  conflictGestureTypeLabel,
  conflictRemoteDeviceLabel,
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
    const label = conflictRemoteDeviceLabel({
      device_id: "abcdef12-3456-7890-abcd-ef1234567890",
      sequence: 5,
      created_at: "2026-08-12T09:00:00Z",
      app_version: "0.2.1",
      schema_version: 1,
    });
    expect(label).toBe("outro aparelho (abcdef12)");
  });
});
