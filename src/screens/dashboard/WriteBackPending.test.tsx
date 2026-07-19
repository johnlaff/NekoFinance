import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { WriteBackPending } from "./WriteBackPending";
import type { CellWrite, WriteBackPreviewResult } from "../../lib/api";
import type { WriteBackPendingState } from "../../hooks/useWriteBackPending";
import { mockCommands, mockInvoke } from "../../test/commands";
import type * as UseCommandModule from "../../lib/useCommand";
import { invalidateCommands } from "../../lib/useCommand";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
// Espiona `invalidateCommands` mantendo o resto do módulo (o helper `mockCommands` também o chama).
vi.mock("../../lib/useCommand", async (importOriginal) => {
  const mod = await importOriginal<typeof UseCommandModule>();
  return { ...mod, invalidateCommands: vi.fn() };
});

const invalidateSpy = invalidateCommands as unknown as ReturnType<typeof vi.fn>;

// Diff seguro p/ o caminho rápido: uma célula de só-valor (kind="diario"), sem conflito/multi-cartão.
const SAFE_CELLS: CellWrite[] = [
  {
    a1: "E3",
    row: 2,
    col: 4,
    date: "2026-06-15",
    kind: "diario",
    current: "50,00",
    proposed: "75,00",
    value_cents: 7500,
    changed: true,
  },
];

function previewResult(
  over: Partial<WriteBackPreviewResult> = {},
): WriteBackPreviewResult {
  return {
    cells: SAFE_CELLS,
    preview_revision: "2026-06-15T00:00:00.000Z",
    conflicts_pending: false,
    ...over,
  };
}

/** Stub mínimo do estado do hook: flag ligada, 1 pendência, sem conflitos. */
function wbState(over: Partial<WriteBackPendingState> = {}): WriteBackPendingState {
  return {
    loading: false,
    pendingCount: 1,
    enabled: true,
    error: null,
    spreadsheetId: "ss-1",
    sheetName: "2026",
    clientId: "cid-1",
    conflictCount: 0,
    refresh: vi.fn(),
    ...over,
  };
}

/** Dispara o caminho rápido até o diálogo de confirmação aparecer. */
async function openConfirm(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Sincronizar" }));
  return screen.findByRole("button", { name: "Confirmar envio" });
}

describe("WriteBackPending — caminho rápido (plano 042)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    invalidateSpy.mockClear();
  });

  it("confirmFastWrite calls invalidateCommands after a successful fast-path apply", async () => {
    const user = userEvent.setup();
    mockCommands({
      preview_write_back_status: previewResult(),
      apply_write_back: { written: 1, note_warning: null },
    });
    const wb = wbState();
    render(<WriteBackPending writeBack={wb} />);

    const confirm = await openConfirm(user);
    // `mockCommands` chamou `invalidateCommands` (cache frio); zera p/ medir só o efeito do apply.
    invalidateSpy.mockClear();
    await user.click(confirm);

    await waitFor(() => expect(invalidateSpy).toHaveBeenCalled());
    expect(wb.refresh).toHaveBeenCalled();
  });

  it("confirmFastWrite does NOT call invalidateCommands on error", async () => {
    const user = userEvent.setup();
    mockCommands({
      preview_write_back_status: previewResult(),
      apply_write_back: new Error("Envio bloqueado — a planilha mudou desde a prévia."),
    });
    const wb = wbState();
    render(<WriteBackPending writeBack={wb} />);

    const confirm = await openConfirm(user);
    invalidateSpy.mockClear();
    await user.click(confirm);

    // O erro do apply surfa como alerta não-bloqueante; nada de invalidação nem refresh.
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/bloqueado/i),
    );
    expect(invalidateSpy).not.toHaveBeenCalled();
    expect(wb.refresh).not.toHaveBeenCalled();
  });
});
