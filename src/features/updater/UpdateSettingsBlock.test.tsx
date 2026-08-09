import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UpdateSettingsBlock } from "./UpdateSettingsBlock";
import { createUpdaterMachine, type UpdaterAdapter } from "./updaterView";

function fakeAdapter(overrides: Partial<UpdaterAdapter> = {}): UpdaterAdapter {
  return {
    check: vi.fn().mockResolvedValue(null),
    checkSpace: vi.fn().mockResolvedValue({
      ok: true,
      required_bytes: 0,
      free_bytes: 0,
      missing_bytes: 0,
    }),
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

describe("UpdateSettingsBlock — estados", () => {
  it("ocioso: estado neutro e ação de checagem manual habilitada", () => {
    const machine = createUpdaterMachine(fakeAdapter());
    render(<UpdateSettingsBlock machine={machine} />);

    expect(screen.getByText("Nenhuma atualização pendente")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Verificar agora" })).toBeEnabled();
  });

  it("verificar agora dispara a checagem e reflete o update encontrado", async () => {
    const user = userEvent.setup();
    const check = vi.fn().mockResolvedValue({
      version: "1.2.0",
      currentVersion: "1.1.0",
      notes: null,
      download: vi.fn(),
      install: vi.fn(),
      close: vi.fn(),
    });
    const machine = createUpdaterMachine(fakeAdapter({ check }));
    render(<UpdateSettingsBlock machine={machine} />);

    await user.click(screen.getByRole("button", { name: "Verificar agora" }));

    expect(check).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/Atualização disponível/)).toBeInTheDocument();
    expect(screen.getByText(/v1\.2\.0/)).toBeInTheDocument();
  });

  it("verificar agora sem update: reflete o estado ocioso de novo", async () => {
    const user = userEvent.setup();
    const check = vi.fn().mockResolvedValue(null);
    const machine = createUpdaterMachine(fakeAdapter({ check }));
    render(<UpdateSettingsBlock machine={machine} />);

    await user.click(screen.getByRole("button", { name: "Verificar agora" }));

    expect(check).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Nenhuma atualização pendente")).toBeInTheDocument();
  });

  it("disponível: a ação vira baixar e instalar, mesma frase do convite calmo", async () => {
    const user = userEvent.setup();
    const download = vi.fn().mockResolvedValue(undefined);
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          download,
          install: vi.fn().mockResolvedValue(undefined),
          close: vi.fn(),
        }),
      }),
    );
    await machine.checkForUpdate();
    render(<UpdateSettingsBlock machine={machine} />);

    const action = screen.getByRole("button", { name: "Baixar e instalar" });
    expect(action).toBeEnabled();
    await user.click(action);

    expect(download).toHaveBeenCalledTimes(1);
  });

  it("baixando: mostra o progresso e desabilita a ação enquanto ocorre", async () => {
    const download = pendingDownload();
    let emitProgress!: (progress: {
      downloadedBytes: number;
      totalBytes: number | null;
    }) => void;
    const downloadFn = vi.fn(
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
          download: downloadFn,
          install: vi.fn().mockResolvedValue(undefined),
          close: vi.fn(),
        }),
      }),
    );
    await machine.checkForUpdate();
    render(<UpdateSettingsBlock machine={machine} />);

    void machine.downloadAndInstall();
    emitProgress({ downloadedBytes: 512, totalBytes: 2048 });

    expect(await screen.findByRole("img", { name: "25% baixado" })).toBeInTheDocument();
    expect(screen.getByText(/512 B de 2,0 KB/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Baixando…" })).toBeDisabled();
  });

  it("pronto para reiniciar: a ação vira reiniciar agora", async () => {
    const user = userEvent.setup();
    const download = pendingDownload();
    const relaunch = vi.fn().mockResolvedValue(undefined);
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          download: vi.fn().mockReturnValue(download.promise),
          install: vi.fn().mockResolvedValue(undefined),
          close: vi.fn(),
        }),
        relaunch,
      }),
    );
    await machine.checkForUpdate();
    const installing = machine.downloadAndInstall();
    download.resolve();
    await installing;
    render(<UpdateSettingsBlock machine={machine} />);

    expect(screen.getByText(/Pronto para reiniciar/)).toBeInTheDocument();
    const action = screen.getByRole("button", { name: "Reiniciar agora" });
    expect(action).toBeEnabled();
    await user.click(action);

    expect(relaunch).toHaveBeenCalledTimes(1);
  });

  it("sem espaço em disco: copy exata na linha, didática e ação de re-checagem", async () => {
    const user = userEvent.setup();
    const MIB = 1024 * 1024;
    const check = vi.fn().mockResolvedValue({
      version: "1.2.0",
      currentVersion: "1.1.0",
      notes: null,
      download: vi.fn(),
      install: vi.fn(),
      close: vi.fn().mockResolvedValue(undefined),
    });
    const checkSpace = vi.fn().mockResolvedValue({
      ok: false,
      required_bytes: 112 * MIB,
      free_bytes: 3 * MIB,
      missing_bytes: 109 * MIB,
    });
    const machine = createUpdaterMachine(fakeAdapter({ check, checkSpace }));
    await machine.checkForUpdate();
    render(<UpdateSettingsBlock machine={machine} />);

    expect(
      screen.getByText(
        "Sem espaço em disco para atualizar · Libere ~110,0 MB para instalar a v1.2.0.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Como funciona?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("erro de instalação: mostra a mensagem de falha e permite verificar de novo", async () => {
    const download = pendingDownload();
    const machine = createUpdaterMachine(
      fakeAdapter({
        check: vi.fn().mockResolvedValue({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          download: vi.fn().mockReturnValue(download.promise),
          install: vi.fn(),
          close: vi.fn(),
        }),
      }),
    );
    await machine.checkForUpdate();
    const installing = machine.downloadAndInstall();
    download.reject(new Error("disk full"));
    await installing;
    render(<UpdateSettingsBlock machine={machine} />);

    expect(
      screen.getByText(/Não foi possível instalar a atualização/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Verificar agora" })).toBeEnabled();
  });
});
