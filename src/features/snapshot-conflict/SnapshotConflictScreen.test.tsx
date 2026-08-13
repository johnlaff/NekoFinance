import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockCommands, mockInvoke } from "../../test/commands";
import type { DriveConflictDetails } from "./snapshotConflictView";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { SnapshotConflictScreen } from "./SnapshotConflictScreen";
import {
  closeSnapshotConflict,
  openSnapshotConflict,
  snapshotConflictOpenSnapshot,
} from "./snapshotConflictStore";

const DETAILS: DriveConflictDetails = {
  remote_manifest: {
    device_id: "abcdef12-3456-7890-abcd-ef1234567890",
    sequence: 5,
    created_at: "2026-08-12T08:00:00Z",
    app_version: "0.2.1",
    schema_version: 1,
  },
  local_gestures: [
    {
      at: "2026-08-11 10:00:00",
      event_type: "import",
      entity_type: "transaction",
      source_sheet: "Diário",
    },
    {
      at: "2026-08-12 07:00:00",
      event_type: "write_back",
      entity_type: "transaction",
      source_sheet: "Saídas",
    },
  ],
  remote_gestures: [
    {
      at: "2026-08-12 06:00:00",
      event_type: "import",
      entity_type: "transaction",
      source_sheet: "Cartão",
    },
  ],
};

describe("SnapshotConflictScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    closeSnapshotConflict();
  });

  it("carrega e mostra os gestos de CADA lado antes de qualquer escolha", async () => {
    mockCommands({ drive_conflict_details: DETAILS });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText(/Importação da planilha \(aba Diário\)/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Escrita de volta na planilha \(aba Saídas\)/),
    ).toBeInTheDocument();
    // O gesto do OUTRO aparelho aparece numa lista própria — o que se perde se o dono escolher
    // manter este aparelho, nunca misturado com os gestos locais.
    expect(
      screen.getByText(/Importação da planilha \(aba Cartão\)/),
    ).toBeInTheDocument();
    expect(screen.getAllByText(/outro aparelho \(abcdef12\)/).length).toBeGreaterThan(
      0,
    );
    expect(
      screen.getByRole("button", { name: "Manter este aparelho" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Usar o outro aparelho" }),
    ).toBeInTheDocument();
  });

  it("mostra o estado honesto quando um dos lados não tem gesto nenhum registrado", async () => {
    mockCommands({
      drive_conflict_details: { ...DETAILS, local_gestures: [], remote_gestures: [] },
    });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText(/Nenhum gesto registrado neste aparelho/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Nenhum gesto registrado no outro aparelho/),
    ).toBeInTheDocument();
  });

  it("mostra erro de carregamento com saída para fechar, sem travar a tela", async () => {
    const user = userEvent.setup();
    mockCommands({ drive_conflict_details: new Error("rede fora do ar") });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText("Não foi possível carregar o conflito"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Fechar" }));
    expect(snapshotConflictOpenSnapshot()).toBe(false);
  });

  it("manter este aparelho: publica e fecha a tela sem exigir reinício", async () => {
    const user = userEvent.setup();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: {
        choice: "keep_local",
        requires_restart: false,
        sequence: 6,
      },
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    );

    await waitFor(() => expect(snapshotConflictOpenSnapshot()).toBe(false));
    const call = mockInvoke.mock.calls.find((c) => c[0] === "resolve_drive_conflict");
    expect(call?.[1]).toMatchObject({ choice: "keep_local" });
  });

  it("usar o outro aparelho: exige reinício e nunca fecha a tela sozinha", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: {
        choice: "use_remote",
        requires_restart: true,
        sequence: 5,
      },
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Usar o outro aparelho" }),
    );

    expect(
      await screen.findByText("Feche e abra o Neko Finance de novo"),
    ).toBeInTheDocument();
    // Trocar o arquivo ativo debaixo do pool em uso exige reiniciar — a tela nunca finge que o
    // app segue operável fechando sozinha.
    expect(snapshotConflictOpenSnapshot()).toBe(true);
    const call = mockInvoke.mock.calls.find((c) => c[0] === "resolve_drive_conflict");
    expect(call?.[1]).toMatchObject({ choice: "use_remote" });
  });

  it("mostra o erro e mantém a escolha disponível quando a resolução falha", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: new Error(
        "Check-in recusado: os dois lados mudaram de novo.",
      ),
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Check-in recusado: os dois lados mudaram de novo.",
    );
    expect(snapshotConflictOpenSnapshot()).toBe(true);
    expect(
      screen.getByRole("button", { name: "Manter este aparelho" }),
    ).not.toBeDisabled();
  });
});
