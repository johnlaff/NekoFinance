import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useWriteBackPending } from "./useWriteBackPending";
import { mockInvoke } from "../test/commands";
import { invalidateCommands } from "../lib/useCommand";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// O `listenEvent` importa este módulo dinamicamente; devolvemos um `unlisten` no-op.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

const PREVIEW_3_PENDING = {
  cells: [
    {
      a1: "B5",
      row: 5,
      col: 2,
      date: "2026-06-01",
      kind: "saida",
      current: "R$ 100,00",
      proposed: "R$ 120,00",
      value_cents: 12000,
      changed: true,
    },
    {
      a1: "B6",
      row: 6,
      col: 2,
      date: "2026-06-02",
      kind: "saida",
      current: "R$ 50,00",
      proposed: "R$ 80,00",
      value_cents: 8000,
      changed: true,
    },
    {
      a1: "B7",
      row: 7,
      col: 2,
      date: "2026-06-03",
      kind: "saida",
      current: "R$ 30,00",
      proposed: "R$ 45,00",
      value_cents: 4500,
      changed: true,
    },
  ],
  preview_revision: "rev-abc",
  conflicts_pending: false,
  multi_card_warning: false,
};

const PREVIEW_NONE = {
  cells: [],
  preview_revision: "rev-empty",
  conflicts_pending: false,
  multi_card_warning: false,
};

const CONFLICTS_2 = [
  {
    id: "c1",
    transaction_id: "t1",
    field: "amount",
    base_value: "10000",
    local_value: "15000",
    sheet_value: "20000",
  },
  {
    id: "c2",
    transaction_id: "t2",
    field: "description",
    base_value: "a",
    local_value: "b",
    sheet_value: "c",
  },
];

const MAPPING_JSON = JSON.stringify({ spreadsheetId: "ss-1", label: "minha planilha" });

/**
 * Roteia `invoke` por comando E, para `get_app_setting`, pela chave — o helper `mockCommands`
 * roteia só por nome do comando, mas o hook lê três chaves de preferência distintas.
 */
function route(opts: {
  appSetting?: Record<string, string | null>;
  preview?: unknown;
  conflicts?: unknown;
  writeBackEnabled?: unknown;
}) {
  invalidateCommands();
  mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "get_app_setting") {
      const key = String(args?.["key"]);
      const value = opts.appSetting?.[key] ?? null;
      return Promise.resolve(value);
    }
    if (cmd === "preview_write_back_status") {
      return opts.preview instanceof Error
        ? Promise.reject(opts.preview)
        : Promise.resolve(opts.preview ?? PREVIEW_NONE);
    }
    if (cmd === "get_import_conflicts") return Promise.resolve(opts.conflicts ?? []);
    if (cmd === "write_back_enabled") {
      return Promise.resolve(opts.writeBackEnabled ?? true);
    }
    return Promise.reject(new Error(`unmocked command: ${cmd}`));
  });
}

describe("useWriteBackPending", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("retorna zeros e nenhum erro quando não há planilha mapeada", async () => {
    route({ appSetting: { sheets_last_import: null } });
    const { result } = renderHook(() => useWriteBackPending());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.pendingCount).toBe(0);
    expect(result.current.conflictCount).toBe(0);
    expect(result.current.enabled).toBe(false);
    expect(result.current.error).toBeNull();
    // Sem mapeamento, a prévia (que toca a rede) nunca é consultada.
    expect(
      mockInvoke.mock.calls.some((c) => c[0] === "preview_write_back_status"),
    ).toBe(false);
  });

  it("conta 3 células divergentes com a flag ligada", async () => {
    route({
      appSetting: {
        sheets_last_import: MAPPING_JSON,
        sheets_last_sheet: "2026",
        sheets_client_id: "cid",
      },
      preview: PREVIEW_3_PENDING,
      conflicts: [],
      writeBackEnabled: true,
    });
    const { result } = renderHook(() => useWriteBackPending());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.pendingCount).toBe(3);
    expect(result.current.conflictCount).toBe(0);
    expect(result.current.enabled).toBe(true);
    expect(result.current.error).toBeNull();
    expect(result.current.spreadsheetId).toBe("ss-1");
    expect(result.current.sheetName).toBe("2026");
    expect(result.current.clientId).toBe("cid");
  });

  it("surfaca 2 conflitos quando não há células divergentes", async () => {
    route({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_NONE,
      conflicts: CONFLICTS_2,
      writeBackEnabled: true,
    });
    const { result } = renderHook(() => useWriteBackPending());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.pendingCount).toBe(0);
    expect(result.current.conflictCount).toBe(2);
  });

  it("mantém enabled=false quando a flag está desligada, mesmo com pendências", async () => {
    route({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: PREVIEW_3_PENDING,
      conflicts: [],
      writeBackEnabled: false,
    });
    const { result } = renderHook(() => useWriteBackPending());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.pendingCount).toBe(3);
    expect(result.current.enabled).toBe(false);
  });

  it("define error e zera o pendingCount quando a prévia falha", async () => {
    route({
      appSetting: { sheets_last_import: MAPPING_JSON, sheets_last_sheet: "2026" },
      preview: new Error("rede indisponível"),
      conflicts: [],
      writeBackEnabled: true,
    });
    const { result } = renderHook(() => useWriteBackPending());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).not.toBeNull();
    expect(result.current.pendingCount).toBe(0);
  });
});
