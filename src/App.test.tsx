import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";
import { FORECAST, SUMMARY, TXNS, mockCommands, mockInvoke } from "./test/commands";
import type { DriveConflictDetails } from "./features/snapshot-conflict/snapshotConflictView";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Captura o handler do `neko://snapshot-sync-done` e devolve um `unlisten` espionável — o mesmo
// padrão de `ConflictGate.test.tsx`. `listenSnapshotSyncDone` importa este módulo dinamicamente.
const unlistenSpy = vi.fn();
let snapshotSyncDoneHandler:
  ((e: { payload: { conflict_pending: boolean } }) => void) | undefined;
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (_event: string, cb: (e: { payload: { conflict_pending: boolean } }) => void) => {
      snapshotSyncDoneHandler = cb;
      return Promise.resolve(unlistenSpy);
    },
  ),
}));

const CONFLICT_DETAILS: DriveConflictDetails = {
  remote_manifest: {
    device_id: "outro-aparelho-11111111",
    sequence: 5,
    created_at: "2026-08-14T08:00:00Z",
    app_version: "0.2.1",
    schema_version: 1,
  },
  local_gestures: [],
  remote_gestures: [],
  this_device_id: "este-aparelho-99999999",
};

const BASE_COMMANDS = {
  check_auth_status: "disconnected",
  get_app_setting: "true",
  get_dashboard_summary: SUMMARY,
  get_forecast: FORECAST,
  get_recent_transactions: TXNS,
  get_upcoming_bills_cmd: [],
};

describe("App (redesign)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    unlistenSpy.mockClear();
    snapshotSyncDoneHandler = undefined;
  });

  it("renders the Hoje screen with loaded data", async () => {
    mockCommands({
      ...BASE_COMMANDS,
      last_drive_checkin: { conflict_pending: false },
    });
    render(<App />);
    expect(await screen.findByText(/Pode gastar hoje/)).toBeInTheDocument();
    expect(screen.getByText("Gasto variável de hoje")).toBeInTheDocument();
  });

  // Regressão do item 10b (issue #446): a costura "tela de conflito abre sozinha" vive em DOIS
  // useEffect (o listener do evento automático de sync, e o estado persistido conferido no
  // mount) sem cobertura nenhuma antes deste teste.
  it("abre a tela de conflito sozinha quando o gatilho automático de sync descobre uma disputa", async () => {
    mockCommands({
      ...BASE_COMMANDS,
      last_drive_checkin: { conflict_pending: false },
      drive_conflict_details: CONFLICT_DETAILS,
    });
    render(<App />);
    await screen.findByText(/Pode gastar hoje/);
    expect(
      screen.queryByRole("dialog", {
        name: "Conflito de sincronização entre aparelhos",
      }),
    ).toBeNull();

    await waitFor(() => expect(snapshotSyncDoneHandler).toBeDefined());
    snapshotSyncDoneHandler?.({ payload: { conflict_pending: true } });

    expect(
      await screen.findByRole("dialog", {
        name: "Conflito de sincronização entre aparelhos",
      }),
    ).toBeInTheDocument();
  });

  it("abre a tela de conflito sozinha ao montar quando o estado persistido já tem uma disputa pendente", async () => {
    // Uma disputa descoberta numa sessão ANTERIOR (o dono fechou o app sem resolver) — o
    // listener acima só reage a tentativas DESTA sessão, então o mount confere
    // `last_drive_checkin` direto (o segundo `useEffect` de `App.tsx`).
    mockCommands({
      ...BASE_COMMANDS,
      last_drive_checkin: { conflict_pending: true },
      drive_conflict_details: CONFLICT_DETAILS,
    });
    render(<App />);
    expect(
      await screen.findByRole("dialog", {
        name: "Conflito de sincronização entre aparelhos",
      }),
    ).toBeInTheDocument();
  });
});
