import { describe, expect, it, vi } from "vitest";
import {
  createUpdaterMachine,
  downloadFraction,
  downloadLabel,
  formatBytes,
  type DownloadProgress,
  type UpdaterAdapter,
} from "./updaterView";

/** Adapter fake — nunca toca `@tauri-apps/plugin-updater`/`plugin-process` de verdade. */
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

describe("updaterView — máquina de estados do auto-update", () => {
  it("começa ocioso", () => {
    const machine = createUpdaterMachine(fakeAdapter());
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("update disponível: ocioso → checando → disponível", async () => {
    const listener = vi.fn();
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: "Correções de sincronização.",
        downloadAndInstall: vi.fn(),
      }),
    });
    const machine = createUpdaterMachine(adapter);
    machine.subscribe(listener);

    const checking = machine.checkForUpdate();
    expect(machine.getState()).toEqual({ status: "checking" });
    await checking;

    expect(machine.getState()).toEqual({
      status: "available",
      version: "1.2.0",
      currentVersion: "1.1.0",
      notes: "Correções de sincronização.",
    });
    expect(listener).toHaveBeenCalled();
  });

  it("sem update: check() resolve null → volta a ocioso", async () => {
    const machine = createUpdaterMachine(
      fakeAdapter({ check: vi.fn().mockResolvedValue(null) }),
    );
    await machine.checkForUpdate();
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("check() enganoso — objeto não-nulo com a mesma versão instalada não vira convite", async () => {
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.1.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall: vi.fn(),
      }),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("falha de rede resolve para ocioso, em silêncio (sem estado de erro)", async () => {
    const adapter = fakeAdapter({
      check: vi.fn().mockRejectedValue(new Error("network error")),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("checkForUpdate ignora chamada concorrente enquanto já checa", async () => {
    let resolveCheck!: (value: null) => void;
    const adapter = fakeAdapter({
      check: vi.fn().mockReturnValue(
        new Promise<null>((resolve) => {
          resolveCheck = resolve;
        }),
      ),
    });
    const machine = createUpdaterMachine(adapter);

    const first = machine.checkForUpdate();
    const second = machine.checkForUpdate();
    resolveCheck(null);
    await Promise.all([first, second]);

    expect(adapter.check).toHaveBeenCalledTimes(1);
  });

  it("progresso de download: disponível → baixando (com progresso) → pronto para reiniciar", async () => {
    const download = pendingDownload();
    let emitProgress!: (progress: DownloadProgress) => void;
    const downloadAndInstall = vi.fn((onProgress: (p: DownloadProgress) => void) => {
      emitProgress = onProgress;
      return download.promise;
    });
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall,
      }),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();

    const installing = machine.downloadAndInstall();
    expect(machine.getState()).toEqual({
      status: "downloading",
      version: "1.2.0",
      progress: { downloadedBytes: 0, totalBytes: null },
    });

    emitProgress({ downloadedBytes: 512, totalBytes: 2048 });
    expect(machine.getState()).toEqual({
      status: "downloading",
      version: "1.2.0",
      progress: { downloadedBytes: 512, totalBytes: 2048 },
    });

    download.resolve();
    await installing;
    expect(machine.getState()).toEqual({ status: "ready", version: "1.2.0" });
  });

  it("erro de instalação: baixando → erro", async () => {
    const download = pendingDownload();
    const downloadAndInstall = vi.fn().mockReturnValue(download.promise);
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall,
      }),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();

    const installing = machine.downloadAndInstall();
    download.reject(new Error("disk full"));
    await installing;

    expect(machine.getState()).toEqual({
      status: "error",
      message: "Não foi possível instalar a atualização.",
    });
  });

  it("downloadAndInstall é no-op fora do estado disponível", async () => {
    const adapter = fakeAdapter();
    const machine = createUpdaterMachine(adapter);
    await machine.downloadAndInstall();
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("relaunch só age no estado pronto para reiniciar", async () => {
    const adapter = fakeAdapter();
    const machine = createUpdaterMachine(adapter);
    await machine.relaunch();
    expect(adapter.relaunch).not.toHaveBeenCalled();
  });

  it("relaunch chama o adapter quando pronto para reiniciar", async () => {
    const download = pendingDownload();
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        downloadAndInstall: vi.fn().mockReturnValue(download.promise),
      }),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    const installing = machine.downloadAndInstall();
    download.resolve();
    await installing;

    await machine.relaunch();
    expect(adapter.relaunch).toHaveBeenCalledTimes(1);
  });

  it("checar de novo depois de um erro sai do estado de erro", async () => {
    const download = pendingDownload();
    const adapter = fakeAdapter({
      check: vi
        .fn()
        .mockResolvedValueOnce({
          version: "1.2.0",
          currentVersion: "1.1.0",
          notes: null,
          downloadAndInstall: vi.fn().mockReturnValue(download.promise),
        })
        .mockResolvedValueOnce(null),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    const installing = machine.downloadAndInstall();
    download.reject(new Error("disk full"));
    await installing;
    expect(machine.getState().status).toBe("error");

    await machine.checkForUpdate();
    expect(machine.getState()).toEqual({ status: "idle" });
  });

  it("subscribe devolve uma função que para de notificar", async () => {
    const listener = vi.fn();
    const adapter = fakeAdapter();
    const machine = createUpdaterMachine(adapter);
    const unsubscribe = machine.subscribe(listener);
    unsubscribe();
    await machine.checkForUpdate();
    expect(listener).not.toHaveBeenCalled();
  });
});

describe("formatBytes — legenda compacta em pt-BR", () => {
  it("bytes crus abaixo de 1 KB", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("KB com uma casa decimal e vírgula", () => {
    expect(formatBytes(1536)).toBe("1,5 KB");
  });

  it("MB", () => {
    expect(formatBytes(5 * 1024 * 1024)).toBe("5,0 MB");
  });

  it("GB — teto de unidade, não passa disso", () => {
    expect(formatBytes(2.25 * 1024 * 1024 * 1024)).toBe("2,3 GB");
  });
});

describe("downloadFraction — 0–1 ou indeterminado", () => {
  it("null quando o servidor não informa o tamanho total", () => {
    expect(downloadFraction({ downloadedBytes: 100, totalBytes: null })).toBeNull();
  });

  it("null quando o total é zero (divisão indefinida)", () => {
    expect(downloadFraction({ downloadedBytes: 0, totalBytes: 0 })).toBeNull();
  });

  it("fração calculada quando o total é conhecido", () => {
    expect(downloadFraction({ downloadedBytes: 512, totalBytes: 2048 })).toBe(0.25);
  });
});

describe("downloadLabel — a verdade completa nunca satura (regra 26)", () => {
  it("só o baixado quando o total é desconhecido", () => {
    const progress: DownloadProgress = { downloadedBytes: 1536, totalBytes: null };
    expect(downloadLabel(progress)).toBe("1,5 KB baixados");
  });

  it("baixado de total quando ambos são conhecidos", () => {
    const progress: DownloadProgress = {
      downloadedBytes: 1024 * 1024,
      totalBytes: 4 * 1024 * 1024,
    };
    expect(downloadLabel(progress)).toBe("1,0 MB de 4,0 MB");
  });
});
