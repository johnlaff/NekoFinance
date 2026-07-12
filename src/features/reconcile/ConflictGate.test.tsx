import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ConflictGate } from "./ConflictGate";
import type { ImportConflict } from "../../lib/api";
import { mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Captura o handler do `neko://sync-done` e devolve um `unlisten` espionável. O
// `listenEvent` da api importa este módulo dinamicamente.
const unlistenSpy = vi.fn();
let syncDoneHandler: ((e: { payload: { conflict_count: number } }) => void) | undefined;
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (_event: string, cb: (e: { payload: { conflict_count: number } }) => void) => {
      syncDoneHandler = cb;
      return Promise.resolve(unlistenSpy);
    },
  ),
}));

const CONFLICTS: ImportConflict[] = [
  {
    id: "c1",
    transaction_id: "t1",
    field: "amount",
    base_value: "10000",
    local_value: "15000",
    sheet_value: "20000",
  },
];

describe("ConflictGate", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    unlistenSpy.mockClear();
    syncDoneHandler = undefined;
  });

  it("não renderiza nada quando não há conflitos", async () => {
    mockCommands({ get_import_conflicts: [] });
    const { container } = render(<ConflictGate />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("mostra o conflito com os dois valores e resolve mantendo o local", async () => {
    const user = userEvent.setup();
    mockCommands({ get_import_conflicts: CONFLICTS, resolve_import_conflict: null });
    const onResolved = vi.fn();
    render(<ConflictGate onResolved={onResolved} />);

    await waitFor(() =>
      expect(screen.getByText(/1 conflito de importação/)).toBeInTheDocument(),
    );
    // Valores formatados como moeda (15000c → R$ 150,00 local; 20000c → R$ 200,00 planilha).
    expect(screen.getByText("R$ 150,00")).toBeInTheDocument();
    expect(screen.getByText("R$ 200,00")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Manter o meu" }));

    await waitFor(() => expect(onResolved).toHaveBeenCalledTimes(1));
    const call = mockInvoke.mock.calls.find((c) => c[0] === "resolve_import_conflict");
    expect(call?.[1]).toMatchObject({ id: "c1", choice: "local" });
    // O card some após resolver.
    expect(screen.queryByText(/1 conflito de importação/)).not.toBeInTheDocument();
  });

  it("resolve usando a planilha", async () => {
    const user = userEvent.setup();
    mockCommands({ get_import_conflicts: CONFLICTS, resolve_import_conflict: null });
    render(<ConflictGate />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Usar da planilha" }),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Usar da planilha" }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c) => c[0] === "resolve_import_conflict",
      );
      expect(call?.[1]).toMatchObject({ id: "c1", choice: "sheet" });
    });
  });

  it("mostra erro e mantém o conflito quando a resolução falha", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_import_conflicts: CONFLICTS,
      resolve_import_conflict: new Error("db locked"),
    });
    render(<ConflictGate />);

    await waitFor(() =>
      expect(screen.getByText(/1 conflito de importação/)).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Manter o meu" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível resolver o conflito",
    );
    expect(screen.getByText(/1 conflito de importação/)).toBeInTheDocument();
  });

  // Ao receber `neko://sync-done`, o gate re-busca os conflitos (badge sem ação do
  // usuário). Aqui começa vazio e, após o evento, o backend passa a devolver um conflito.
  it("re-busca os conflitos quando chega o evento sync-done", async () => {
    mockCommands({ get_import_conflicts: [] });
    render(<ConflictGate />);
    await waitFor(() => expect(syncDoneHandler).toBeDefined());

    const before = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_import_conflicts",
    ).length;

    // Próxima leitura traz um conflito; dispara o evento manualmente.
    mockCommands({ get_import_conflicts: CONFLICTS });
    syncDoneHandler?.({ payload: { conflict_count: 1 } });

    await waitFor(() =>
      expect(screen.getByText(/1 conflito de importação/)).toBeInTheDocument(),
    );
    const after = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_import_conflicts",
    ).length;
    expect(after).toBeGreaterThan(before);
  });

  // A assinatura do evento é cancelada no unmount (sem vazar listener no HMR).
  it("cancela a assinatura do evento ao desmontar", async () => {
    mockCommands({ get_import_conflicts: [] });
    const { unmount } = render(<ConflictGate />);
    await waitFor(() => expect(syncDoneHandler).toBeDefined());

    unmount();

    await waitFor(() => expect(unlistenSpy).toHaveBeenCalled());
  });
});
