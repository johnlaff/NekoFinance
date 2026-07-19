import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { WriteBackPreview } from "./WriteBackPreview";
import { isSafeForFastPath } from "./writeBack";
import type { CellWrite, WriteBackPreviewResult } from "../../lib/api";
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

function previewResult(
  over: Partial<WriteBackPreviewResult> = {},
): WriteBackPreviewResult {
  return {
    cells: CELLS,
    preview_revision: "2026-01-01T00:00:00.000Z",
    conflicts_pending: false,
    ...over,
  };
}

/** Comandos padrão de um cenário com a flag LIGADA e uma célula divergente. */
function baseHandlers(over: Record<string, unknown> = {}) {
  return {
    write_back_enabled: true,
    preview_write_back_status: previewResult(),
    preview_economia_write_back_status: previewResult({ cells: [] }),
    get_import_conflicts: [],
    apply_write_back: { written: 1, note_warning: null },
    apply_economia_write_back: 0,
    ...over,
  };
}

async function generatePreview(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Gerar prévia do diff" }));
  await waitFor(() =>
    expect(screen.getByText(/1 célula\(s\) divergente\(s\)/)).toBeInTheDocument(),
  );
}

describe("WriteBackPreview", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra a flag desligada e gera o diff apenas das células divergentes", async () => {
    const user = userEvent.setup();
    mockCommands(
      baseHandlers({
        write_back_enabled: false,
      }),
    );
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);

    await waitFor(() => expect(screen.getByText("desligado")).toBeInTheDocument());
    await generatePreview(user);

    // Só a célula divergente (E3) vira um card; a inalterada (C3) não.
    expect(screen.getByText(/E3 · 2026-01-01/)).toBeInTheDocument();
    expect(screen.queryByText(/C3 · 2026-01-01/)).not.toBeInTheDocument();

    // O botão de envio está desabilitado (flag off).
    const sendBtn = screen.getByRole("button", { name: "Envio desligado" });
    expect(sendBtn).toBeDisabled();
  });

  it("informa quando não há divergências", async () => {
    const user = userEvent.setup();
    mockCommands(
      baseHandlers({
        preview_write_back_status: previewResult({ cells: [CELLS[1]!] }), // só a inalterada
      }),
    );
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await user.click(screen.getByRole("button", { name: "Gerar prévia do diff" }));
    await waitFor(() => expect(screen.getByText(/Nada a enviar/)).toBeInTheDocument());
  });

  it("exige a 2ª confirmação antes de escrever: cancelar não envia, confirmar envia", async () => {
    const user = userEvent.setup();
    mockCommands(baseHandlers());
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await waitFor(() => expect(screen.getByText("habilitado")).toBeInTheDocument());
    await generatePreview(user);

    // Clicar em Aprovar abre o diálogo — ainda NÃO chamou apply_write_back.
    await user.click(screen.getByRole("button", { name: /Aprovar e enviar/ }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());

    // Cancelar fecha o diálogo sem escrever.
    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mockInvoke).not.toHaveBeenCalledWith("apply_write_back", expect.anything());

    // Reabrir e confirmar dispara a escrita (com o preview_revision da prévia).
    await user.click(screen.getByRole("button", { name: /Aprovar e enviar/ }));
    await user.click(screen.getByRole("button", { name: "Confirmar envio" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "apply_write_back",
        expect.objectContaining({
          previewRevision: "2026-01-01T00:00:00.000Z",
        }),
      ),
    );
    await waitFor(() => expect(screen.getByText(/Enviado: 1/)).toBeInTheDocument());
  });

  it("desabilita o Aprovar enquanto um envio está em andamento (sem duplo-clique)", async () => {
    const user = userEvent.setup();
    // apply pendente que só resolve quando dispararmos: prova o estado "Enviando…".
    let resolveApply!: (r: { written: number; note_warning: string | null }) => void;
    const pending = new Promise<{ written: number; note_warning: string | null }>(
      (res) => {
        resolveApply = res;
      },
    );
    mockCommands(baseHandlers({ apply_write_back: pending }));
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await waitFor(() => expect(screen.getByText("habilitado")).toBeInTheDocument());
    await generatePreview(user);

    await user.click(screen.getByRole("button", { name: /Aprovar e enviar/ }));
    await user.click(screen.getByRole("button", { name: "Confirmar envio" }));

    // Durante o envio o botão vira "Enviando…" e fica desabilitado (guard anti-duplo-clique).
    const sending = await screen.findByRole("button", { name: "Enviando…" });
    expect(sending).toBeDisabled();

    resolveApply({ written: 1, note_warning: null });
    await waitFor(() => expect(screen.getByText(/Enviado: 1/)).toBeInTheDocument());
  });

  it("bloqueia o Aprovar quando há conflitos de importação pendentes", async () => {
    const user = userEvent.setup();
    mockCommands(
      baseHandlers({
        preview_write_back_status: previewResult({ conflicts_pending: true }),
      }),
    );
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await waitFor(() => expect(screen.getByText("habilitado")).toBeInTheDocument());
    await generatePreview(user);

    await waitFor(() =>
      expect(
        screen.getByText(/conflitos de importação pendentes/i),
      ).toBeInTheDocument(),
    );
    const sendBtn = screen.getByRole("button", {
      name: "Resolva os conflitos primeiro",
    });
    expect(sendBtn).toBeDisabled();
  });

  it("trata o erro de planilha mudada: avisa e refaz a prévia", async () => {
    const user = userEvent.setup();
    mockCommands(
      baseHandlers({
        apply_write_back: new Error(
          "A planilha mudou desde a prévia — gere o preview de novo e revise antes de enviar.",
        ),
      }),
    );
    render(<WriteBackPreview spreadsheetId="ss" sheetName="2026" clientId="cid" />);
    await waitFor(() => expect(screen.getByText("habilitado")).toBeInTheDocument());
    await generatePreview(user);

    const previewCallsBefore = mockInvoke.mock.calls.filter(
      (c) => c[0] === "preview_write_back_status",
    ).length;

    await user.click(screen.getByRole("button", { name: /Aprovar e enviar/ }));
    await user.click(screen.getByRole("button", { name: "Confirmar envio" }));

    // Mensagem clara de re-revisão + re-prévia automática (uma nova chamada ao *_status).
    await waitFor(() =>
      expect(screen.getByText(/A planilha mudou/)).toBeInTheDocument(),
    );
    await waitFor(() => {
      const after = mockInvoke.mock.calls.filter(
        (c) => c[0] === "preview_write_back_status",
      ).length;
      expect(after).toBeGreaterThan(previewCallsBefore);
    });
  });
});

// Pré-condição do caminho rápido "Sincronizar": só colapsa os cliques quando o diff é
// de só-valor, sem conflito, sem multi-cartão e com um token de frescura presente. Qualquer falha
// cai no fluxo completo — as salvaguardas do backend rodam de qualquer forma.
describe("isSafeForFastPath", () => {
  const SAFE_CHANGED: CellWrite[] = [
    {
      a1: "E3",
      row: 2,
      col: 4,
      date: "2026-06-01",
      kind: "diario",
      current: "50,00",
      proposed: "75,00",
      value_cents: 7500,
      changed: true,
    },
  ];

  it("returns true for a clean amount-only diff", () => {
    expect(isSafeForFastPath(true, 0, SAFE_CHANGED, "rev-1")).toBe(true);
  });

  it("returns true for entrada/saida/diario kinds", () => {
    const kinds: CellWrite[] = [
      { ...SAFE_CHANGED[0]!, kind: "entrada", a1: "C3" },
      { ...SAFE_CHANGED[0]!, kind: "saida", a1: "D3" },
      { ...SAFE_CHANGED[0]!, kind: "diario", a1: "E3" },
    ];
    expect(isSafeForFastPath(true, 0, kinds, "rev-1")).toBe(true);
  });

  it("returns false when disabled", () => {
    expect(isSafeForFastPath(false, 0, SAFE_CHANGED, "rev-1")).toBe(false);
  });

  it("returns false when conflict count > 0", () => {
    expect(isSafeForFastPath(true, 1, SAFE_CHANGED, "rev-1")).toBe(false);
  });

  it("returns false when changed list is empty", () => {
    expect(isSafeForFastPath(true, 0, [], "rev-1")).toBe(false);
  });

  it("returns false when previewRevision is null or empty", () => {
    expect(isSafeForFastPath(true, 0, SAFE_CHANGED, null)).toBe(false);
    expect(isSafeForFastPath(true, 0, SAFE_CHANGED, "")).toBe(false);
  });

  it("returns false when any changed cell has a formula-column kind (balance/date)", () => {
    const withBalance: CellWrite[] = [
      ...SAFE_CHANGED,
      { ...SAFE_CHANGED[0]!, kind: "balance", a1: "F3", col: 5 },
    ];
    expect(isSafeForFastPath(true, 0, withBalance, "rev-1")).toBe(false);

    const withDate: CellWrite[] = [
      ...SAFE_CHANGED,
      { ...SAFE_CHANGED[0]!, kind: "date", a1: "A3", col: 0 },
    ];
    expect(isSafeForFastPath(true, 0, withDate, "rev-1")).toBe(false);
  });
});
