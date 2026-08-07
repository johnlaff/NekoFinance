import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

// O setup global finge __TAURI_INTERNALS__ (isTauri: true) para todo teste. Aqui isTauri
// precisa ser false: os estados deste arquivo são pré-montados na máquina ANTES de renderizar,
// e a checagem de fundo do mount (se ligada) reabriria "checando" por cima e apagaria o estado
// pré-montado antes das asserções rodarem. A checagem em si tem arquivo próprio
// (UpdateInvitation.launchCheck.test.tsx), com isTauri: true.
vi.mock("../../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: false,
}));

import { UpdateInvitation } from "./UpdateInvitation";
import { createUpdaterMachine, type UpdaterAdapter } from "./updaterView";

function fakeAdapter(overrides: Partial<UpdaterAdapter> = {}): UpdaterAdapter {
  return {
    check: vi.fn().mockResolvedValue(null),
    relaunch: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function pendingDownload() {
  let resolve!: () => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("UpdateInvitation — estados", () => {
  it("não renderiza nada ocioso", () => {
    const machine = createUpdaterMachine(fakeAdapter());
    const { container } = render(<UpdateInvitation machine={machine} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("não checa fora do Tauri (preview web)", () => {
    const check = vi.fn().mockResolvedValue(null);
    const machine = createUpdaterMachine(fakeAdapter({ check }));
    render(<UpdateInvitation machine={machine} />);
    expect(check).not.toHaveBeenCalled();
  });

  it("convite disponível: título, versão e as duas ações", async () => {
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall: vi.fn(),
        }),
      }),
    );
    await machine.checkForUpdate();
    render(<UpdateInvitation machine={machine} />);

    expect(screen.getByRole("status")).toHaveTextContent("Atualização disponível");
    expect(screen.getByText(/v1\.2\.0/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Baixar e instalar" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Agora não" })).toBeInTheDocument();
  });

  it("recusar o convite dispensa o estado disponível — sem insistir na sessão", async () => {
    const user = userEvent.setup();
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall: vi.fn(),
        }),
      }),
    );
    await machine.checkForUpdate();
    render(<UpdateInvitation machine={machine} />);

    await user.click(screen.getByRole("button", { name: "Agora não" }));

    expect(screen.queryByText("Atualização disponível")).not.toBeInTheDocument();
  });

  it("recusar a v1.2.0 não silencia uma v1.3.0 encontrada depois na mesma sessão", async () => {
    const user = userEvent.setup();
    const check = vi
      .fn()
      .mockResolvedValueOnce({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall: vi.fn(),
      })
      .mockResolvedValueOnce({
        version: "1.3.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall: vi.fn(),
      });
    const machine = createUpdaterMachine(fakeAdapter({ check }));
    await machine.checkForUpdate();
    render(<UpdateInvitation machine={machine} />);
    await user.click(screen.getByRole("button", { name: "Agora não" }));
    expect(screen.queryByText(/v1\.2\.0/)).not.toBeInTheDocument();

    // Uma checagem manual mais tarde (ex.: o gesto de #383 nas Configurações, mesma
    // instância de máquina) encontra uma versão NOVA — é uma oferta diferente.
    await machine.checkForUpdate();

    expect(screen.getByText(/v1\.3\.0/)).toBeInTheDocument();
  });

  it("aceitar dispara o download e mostra o progresso", async () => {
    const user = userEvent.setup();
    const download = pendingDownload();
    let emitProgress!: (progress: {
      downloadedBytes: number;
      totalBytes: number | null;
    }) => void;
    const downloadAndInstall = vi.fn(
      (
        onProgress: (p: { downloadedBytes: number; totalBytes: number | null }) => void,
      ) => {
        emitProgress = onProgress;
        return download.promise;
      },
    );
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall,
        }),
      }),
    );
    await machine.checkForUpdate();
    render(<UpdateInvitation machine={machine} />);

    await user.click(screen.getByRole("button", { name: "Baixar e instalar" }));

    expect(screen.getByRole("status")).toHaveTextContent("Baixando atualização");
    emitProgress({ downloadedBytes: 512, totalBytes: 2048 });
    expect(await screen.findByText("512 B de 2,0 KB")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "25% baixado" })).toBeInTheDocument();

    download.resolve();
    await screen.findByText("Pronto para reiniciar");
  });

  it("pronto para reiniciar: avisa antes de fechar e chama relaunch ao aceitar", async () => {
    const user = userEvent.setup();
    const relaunch = vi.fn().mockResolvedValue(undefined);
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall: vi.fn().mockResolvedValue(undefined),
        }),
        relaunch,
      }),
    );
    await machine.checkForUpdate();
    await machine.downloadAndInstall();
    render(<UpdateInvitation machine={machine} />);

    expect(screen.getByText(/vai fechar e abrir de novo/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reiniciar agora" }));

    expect(relaunch).toHaveBeenCalledTimes(1);
  });

  it("erro de instalação: mensagem visível e dispensável", async () => {
    const user = userEvent.setup();
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall: vi.fn().mockRejectedValue(new Error("disk full")),
        }),
      }),
    );
    await machine.checkForUpdate();
    await machine.downloadAndInstall();
    render(<UpdateInvitation machine={machine} />);

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Não foi possível instalar a atualização",
    );
    await user.click(screen.getByRole("button", { name: "Fechar" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
