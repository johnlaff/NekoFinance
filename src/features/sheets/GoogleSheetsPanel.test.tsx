import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { GoogleSheetsPanel, ImportDiagnosticsNotice } from "./GoogleSheetsPanel";
import { mockInvoke } from "../../test/commands";
import { invalidateCommands } from "../../lib/useCommand";
import type { ImportDiagnostic } from "../../lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

/**
 * Roteia `get_app_setting`/`set_app_setting` pela CHAVE (o helper `mockCommands` roteia só por nome
 * de comando). `appSettings` simula a tabela KV; `null` = chave ausente.
 */
function mockSettings(appSettings: Record<string, string | null>) {
  invalidateCommands();
  mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "get_app_setting") {
      const key = args?.["key"] as string;
      return Promise.resolve(appSettings[key] ?? null);
    }
    if (cmd === "set_app_setting") {
      const key = args?.["key"] as string;
      appSettings[key] = args?.["value"] as string;
      return Promise.resolve(null);
    }
    if (cmd === "list_user_spreadsheets") return Promise.resolve([]);
    return Promise.reject(new Error(`unmocked command: ${cmd}`));
  });
}

const LAST_IMPORT = JSON.stringify({ spreadsheetId: "ss-1", label: "Minha planilha" });

describe("GoogleSheetsPanel — atualização automática (plano 026)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("persiste sheets_bg_sync_enabled como false ao desmarcar", async () => {
    const user = userEvent.setup();
    const settings: Record<string, string | null> = {
      sheets_last_import: LAST_IMPORT,
      sheets_bg_sync_enabled: "true",
    };
    mockSettings(settings);

    render(<GoogleSheetsPanel authStatus="connected" onAuthChange={() => undefined} />);

    const checkbox = await screen.findByRole("checkbox", {
      name: /Atualização automática/,
    });
    expect(checkbox).toBeChecked();

    await user.click(checkbox);

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c) => c[0] === "set_app_setting" && c[1]?.["key"] === "sheets_bg_sync_enabled",
      );
      expect(call?.[1]).toMatchObject({
        key: "sheets_bg_sync_enabled",
        value: "false",
      });
    });
    expect(checkbox).not.toBeChecked();
  });

  it("vem marcado por padrão quando a chave está ausente", async () => {
    mockSettings({
      sheets_last_import: LAST_IMPORT,
      sheets_bg_sync_enabled: null, // chave nunca gravada → padrão LIGADO
    });

    render(<GoogleSheetsPanel authStatus="connected" onAuthChange={() => undefined} />);

    const checkbox = await screen.findByRole("checkbox", {
      name: /Atualização automática/,
    });
    expect(checkbox).toBeChecked();
  });
});

// Plano 070: nota que não deu para itemizar ou item↔célula divergente — a mesma superfície é
// reusada pelo import do Sheets e pelo import de .xlsx local (LocalXlsxImport), por isso é
// testada como componente independente aqui.
describe("ImportDiagnosticsNotice (plano 070)", () => {
  const SAMPLE: ImportDiagnostic[] = [
    {
      sheet: "2026",
      cell: "2026-02-11 (itens não somam à célula)",
      kind: "ItemsDoNotSumToCell",
      detail: "célula R$ 100,00 vs. itens R$ 120,00 (diferença -R$ 20,00)",
    },
  ];

  it("mostra a contagem e a lista expandível quando há diagnósticos", async () => {
    const user = userEvent.setup();
    render(<ImportDiagnosticsNotice diagnostics={SAMPLE} />);

    const toggle = screen.getByText(/1 nota precisa de atenção/);
    expect(toggle).toBeInTheDocument();

    await user.click(toggle);
    expect(screen.getByText(/célula R\$ 100,00/)).toBeInTheDocument();
  });

  it("pluraliza quando há mais de um diagnóstico", () => {
    render(<ImportDiagnosticsNotice diagnostics={[...SAMPLE, SAMPLE[0]!]} />);
    expect(screen.getByText(/2 notas precisam de atenção/)).toBeInTheDocument();
  });

  it("não renderiza nada quando não há diagnósticos", () => {
    const { container } = render(<ImportDiagnosticsNotice diagnostics={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
