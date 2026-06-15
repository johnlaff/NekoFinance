import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { WriteBackPreview } from "./WriteBackPreview";
import type { CellWrite } from "../../lib/api";
import { mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const CELLS: CellWrite[] = [
  {
    a1: "E3",
    row: 2,
    col: 4,
    date: "2026-01-01",
    kind: "diario",
    current: "50,00",
    proposed: "75,00",
    value_cents: 7500,
    changed: true,
  },
  {
    a1: "C3",
    row: 2,
    col: 2,
    date: "2026-01-01",
    kind: "entrada",
    current: "1000,00",
    proposed: "1000,00",
    value_cents: 100000,
    changed: false,
  },
];

describe("WriteBackPreview", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra a flag desligada e gera o diff apenas das células divergentes", async () => {
    const user = userEvent.setup();
    mockCommands({
      write_back_enabled: false,
      preview_write_back: CELLS,
      preview_economia_write_back: [],
      apply_write_back: new Error(
        "Write-back desligado: o envio ao Sheets está atrás de uma flag desabilitada.",
      ),
    });
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);

    await waitFor(() => expect(screen.getByText("desligado")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Gerar prévia do diff" }));

    // Só a célula divergente (E3) vira um card; a inalterada (C3) não.
    await waitFor(() =>
      expect(screen.getByText(/1 célula\(s\) divergente\(s\)/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/E3 · 2026-01-01/)).toBeInTheDocument();
    expect(screen.queryByText(/C3 · 2026-01-01/)).not.toBeInTheDocument();

    // O botão de envio está desabilitado (flag off).
    const sendBtn = screen.getByRole("button", { name: "Envio desligado" });
    expect(sendBtn).toBeDisabled();
  });

  it("informa quando não há divergências", async () => {
    const user = userEvent.setup();
    mockCommands({
      write_back_enabled: false,
      preview_write_back: [CELLS[1]], // só a inalterada
    });
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await user.click(screen.getByRole("button", { name: "Gerar prévia do diff" }));
    await waitFor(() => expect(screen.getByText(/Nada a enviar/)).toBeInTheDocument());
  });
});
