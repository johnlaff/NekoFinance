import { describe, it, expect } from "vitest";
import { syncRecencyLabel } from "./syncRecency";

// Timestamps do sync_log são `datetime('now')` do SQLite: UTC sem sufixo de fuso.
const NOW = Date.parse("2026-07-23T18:00:00Z");

describe("syncRecencyLabel", () => {
  it("null/undefined → null", () => {
    expect(syncRecencyLabel(null, NOW)).toBeNull();
    expect(syncRecencyLabel(undefined, NOW)).toBeNull();
  });

  it("timestamp inválido → null", () => {
    expect(syncRecencyLabel("não-é-data", NOW)).toBeNull();
  });

  it("menos de 1 min → 'agora mesmo'", () => {
    expect(syncRecencyLabel("2026-07-23 17:59:40", NOW)).toBe("agora mesmo");
  });

  it("minutos", () => {
    expect(syncRecencyLabel("2026-07-23 17:42:00", NOW)).toBe("há 18 min");
  });

  it("horas", () => {
    expect(syncRecencyLabel("2026-07-23 15:30:00", NOW)).toBe("há 2 h");
  });

  it("um dia (singular)", () => {
    expect(syncRecencyLabel("2026-07-22 10:00:00", NOW)).toBe("há 1 dia");
  });

  it("dias (plural)", () => {
    expect(syncRecencyLabel("2026-07-20 18:00:00", NOW)).toBe("há 3 dias");
  });

  it("timestamp futuro (clock skew) não vira negativo", () => {
    expect(syncRecencyLabel("2026-07-23 18:00:30", NOW)).toBe("agora mesmo");
  });

  it("RFC3339 com 'T' e offset explícito (snapshot_cmds.rs) é aceito", () => {
    expect(syncRecencyLabel("2026-07-23T17:42:00+00:00", NOW)).toBe("há 18 min");
  });

  it("RFC3339 com 'T' e sufixo Z é aceito", () => {
    expect(syncRecencyLabel("2026-07-23T17:42:00Z", NOW)).toBe("há 18 min");
  });

  it(
    "'T' SEM offset/Z é rejeitado — nenhum produtor gera essa forma, e aceitá-la leria a hora " +
      "como LOCAL do navegador (recência errada-mas-plausível)",
    () => {
      expect(syncRecencyLabel("2026-07-23T17:42:00", NOW)).toBeNull();
    },
  );
});
