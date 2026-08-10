import { describe, expect, it, vi } from "vitest";
import {
  createUpdaterMachine,
  downloadFraction,
  downloadLabel,
  formatBytes,
  missingSpaceLabel,
  updateStatusCopy,
  type DownloadProgress,
  type UpdaterAdapter,
} from "./updaterView";

/** Adapter fake — nunca toca `@tauri-apps/plugin-updater`/`plugin-process` de verdade. */
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
        download: vi.fn(),
        install: vi.fn(),
        close: vi.fn(),
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
        download: vi.fn(),
        install: vi.fn(),
        close: vi.fn(),
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

  it("update disponível mas sem espaço: vira blocked-space e nunca chega a baixar", async () => {
    const download = vi.fn();
    const close = vi.fn().mockResolvedValue(undefined);
    const checkSpace = vi.fn().mockResolvedValue({
      ok: false,
      required_bytes: 500 * 1024 * 1024,
      free_bytes: 100 * 1024 * 1024,
      missing_bytes: 400 * 1024 * 1024,
    });
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download,
        install: vi.fn(),
        close,
      }),
      checkSpace,
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();

    expect(machine.getState()).toEqual({
      status: "blocked-space",
      version: "1.2.0",
      missingBytes: 400 * 1024 * 1024,
      requiredBytes: 500 * 1024 * 1024,
    });
    expect(close).toHaveBeenCalledTimes(1);
    expect(download).not.toHaveBeenCalled();
  });

  it("checkSpace falha na medição ao confirmar o update: degrada para o convite normal", async () => {
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download: vi.fn(),
        install: vi.fn(),
        close: vi.fn(),
      }),
      checkSpace: vi.fn().mockRejectedValue(new Error("comando indisponível")),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();

    expect(machine.getState()).toEqual({
      status: "available",
      version: "1.2.0",
      currentVersion: "1.1.0",
      notes: null,
    });
  });

  it("progresso de download: disponível → baixando (com progresso) → pronto para reiniciar", async () => {
    const download = pendingDownload();
    let emitProgress!: (progress: DownloadProgress) => void;
    const downloadFn = vi.fn((onProgress: (p: DownloadProgress) => void) => {
      emitProgress = onProgress;
      return download.promise;
    });
    const install = vi.fn().mockResolvedValue(undefined);
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download: downloadFn,
        install,
        close: vi.fn(),
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
    expect(install).toHaveBeenCalledTimes(1);
  });

  it("espaço falta só depois do download: a re-checagem barra o install e descarta o handle", async () => {
    const download = vi.fn().mockResolvedValue(undefined);
    const install = vi.fn();
    const close = vi.fn().mockResolvedValue(undefined);
    const checkSpace = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        required_bytes: 0,
        free_bytes: 0,
        missing_bytes: 0,
      })
      .mockResolvedValueOnce({
        ok: false,
        required_bytes: 500 * 1024 * 1024,
        free_bytes: 50 * 1024 * 1024,
        missing_bytes: 450 * 1024 * 1024,
      });
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download,
        install,
        close,
      }),
      checkSpace,
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    await machine.downloadAndInstall();

    expect(machine.getState()).toEqual({
      status: "blocked-space",
      version: "1.2.0",
      missingBytes: 450 * 1024 * 1024,
      requiredBytes: 500 * 1024 * 1024,
    });
    expect(install).not.toHaveBeenCalled();
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("re-checagem falha na medição: prossegue para o install (degradação, nunca trava o fim do download)", async () => {
    const install = vi.fn().mockResolvedValue(undefined);
    const checkSpace = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        required_bytes: 0,
        free_bytes: 0,
        missing_bytes: 0,
      })
      .mockRejectedValueOnce(new Error("comando indisponível"));
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download: vi.fn().mockResolvedValue(undefined),
        install,
        close: vi.fn(),
      }),
      checkSpace,
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();
    await machine.downloadAndInstall();

    expect(machine.getState()).toEqual({ status: "ready", version: "1.2.0" });
    expect(install).toHaveBeenCalledTimes(1);
  });

  it("erro de download: baixando → erro, com o handle descartado", async () => {
    const download = pendingDownload();
    const downloadFn = vi.fn().mockReturnValue(download.promise);
    const close = vi.fn().mockResolvedValue(undefined);
    const adapter = fakeAdapter({
      check: vi.fn().mockResolvedValue({
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
        download: downloadFn,
        install: vi.fn(),
        close,
      }),
    });
    const machine = createUpdaterMachine(adapter);
    await machine.checkForUpdate();

    const installing = machine.downloadAndInstall();
    download.reject(new Error("disk full"));
    await installing;

    expect(machine.getState()).toEqual({
      status: "error",
      message: "Não foi possível baixar a atualização.",
    });
    expect(close).toHaveBeenCalledTimes(1);
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
        download: vi.fn().mockReturnValue(download.promise),
        install: vi.fn().mockResolvedValue(undefined),
        close: vi.fn(),
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
          download: vi.fn().mockReturnValue(download.promise),
          install: vi.fn(),
          close: vi.fn(),
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

describe("missingSpaceLabel — arredonda para cima em múltiplos de 10 MiB", () => {
  it("1 byte já sobe para o próximo múltiplo (10 MB)", () => {
    expect(missingSpaceLabel(1)).toBe("10,0 MB");
  });

  it("exatamente 10 MiB fica em 10 MB — não sobe sem faltar nada", () => {
    expect(missingSpaceLabel(10 * 1024 * 1024)).toBe("10,0 MB");
  });

  it("10 MiB + 1 byte sobe para o múltiplo seguinte (20 MB)", () => {
    expect(missingSpaceLabel(10 * 1024 * 1024 + 1)).toBe("20,0 MB");
  });

  it("104 MiB sobe para 110 MB", () => {
    expect(missingSpaceLabel(104 * 1024 * 1024)).toBe("110,0 MB");
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

describe("updateStatusCopy — leitura textual de cada estado (bloco de Configurações)", () => {
  it("ocioso", () => {
    expect(updateStatusCopy({ status: "idle" })).toEqual({
      headline: "Nenhuma atualização pendente",
      detail: null,
    });
  });

  it("checando", () => {
    expect(updateStatusCopy({ status: "checking" })).toEqual({
      headline: "Checando atualização…",
      detail: null,
    });
  });

  it("disponível traz a versão no detail", () => {
    expect(
      updateStatusCopy({
        status: "available",
        version: "1.2.0",
        currentVersion: "1.1.0",
        notes: null,
      }),
    ).toEqual({
      headline: "Atualização disponível",
      detail: "v1.2.0 pronta para baixar.",
    });
  });

  it("baixando traz a legenda de progresso no detail", () => {
    expect(
      updateStatusCopy({
        status: "downloading",
        version: "1.2.0",
        progress: { downloadedBytes: 512, totalBytes: 2048 },
      }),
    ).toEqual({
      headline: "Baixando atualização…",
      detail: "512 B de 2,0 KB",
    });
  });

  it("pronto para reiniciar traz a versão no detail", () => {
    expect(updateStatusCopy({ status: "ready", version: "1.2.0" })).toEqual({
      headline: "Pronto para reiniciar",
      detail: "v1.2.0 instalada — reinicie para aplicar.",
    });
  });

  it("bloqueado por espaço traz quanto liberar e a versão parada", () => {
    expect(
      updateStatusCopy({
        status: "blocked-space",
        version: "1.2.0",
        missingBytes: 400 * 1024 * 1024,
        requiredBytes: 500 * 1024 * 1024,
      }),
    ).toEqual({
      headline: "Sem espaço em disco para atualizar",
      detail: "Libere ~400,0 MB para instalar a v1.2.0.",
    });
  });

  it("erro repassa a mensagem no detail", () => {
    expect(updateStatusCopy({ status: "error", message: "Disco cheio." })).toEqual({
      headline: "Não foi possível instalar a atualização",
      detail: "Disco cheio.",
    });
  });
});
