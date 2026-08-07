//! Porta do domínio Update: quem quiser checar/baixar/instalar uma atualização importa só
//! daqui — nunca `@tauri-apps/plugin-updater`/`@tauri-apps/plugin-process` diretamente. O
//! adapter fica na borda (`realUpdaterAdapter`) para a máquina de estados ser testável com um
//! fake, sem depender do shell Tauri.

import { relaunch as pluginRelaunch } from "@tauri-apps/plugin-process";
import { check as pluginCheck } from "@tauri-apps/plugin-updater";
import { safeErrorMessage } from "../../lib/errors";

export interface DownloadProgress {
  downloadedBytes: number;
  /** `null` quando o servidor não informa o tamanho total. */
  totalBytes: number | null;
}

export interface UpdaterCheckResult {
  version: string;
  currentVersion: string;
  notes: string | null;
  downloadAndInstall: (
    onProgress: (progress: DownloadProgress) => void,
  ) => Promise<void>;
}

export interface UpdaterAdapter {
  /** Resolve `null` quando não há update. */
  check: () => Promise<UpdaterCheckResult | null>;
  relaunch: () => Promise<void>;
}

export type UpdaterState =
  | { status: "idle" }
  | { status: "checking" }
  | {
      status: "available";
      version: string;
      currentVersion: string;
      notes: string | null;
    }
  | { status: "downloading"; version: string; progress: DownloadProgress }
  | { status: "ready"; version: string }
  | { status: "error"; message: string };

/** Legenda compacta em pt-BR (vírgula decimal), teto em GB — o convite nunca precisa de mais. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${Math.max(0, Math.round(bytes))} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(1).replace(".", ",")} ${units[unitIndex]}`;
}

/** Fração 0–1 do progresso, ou `null` quando o total é desconhecido/zero (barra indeterminada). */
export function downloadFraction(progress: DownloadProgress): number | null {
  if (progress.totalBytes == null || progress.totalBytes <= 0) return null;
  return progress.downloadedBytes / progress.totalBytes;
}

/** Legenda textual do progresso — a régua satura, o texto nunca (regra 26 do ui-standards). */
export function downloadLabel(progress: DownloadProgress): string {
  const downloaded = formatBytes(progress.downloadedBytes);
  return progress.totalBytes == null
    ? `${downloaded} baixados`
    : `${downloaded} de ${formatBytes(progress.totalBytes)}`;
}

export interface UpdaterMachine {
  getState(): UpdaterState;
  subscribe(listener: () => void): () => void;
  checkForUpdate(): Promise<void>;
  downloadAndInstall(): Promise<void>;
  relaunch(): Promise<void>;
}

/**
 * Máquina ocioso → checando → disponível → baixando → pronto para reiniciar → erro.
 * O adapter é injetado (nunca importado direto pela máquina de produção) para o teste
 * exercitar cada transição com um fake, sem tocar o plugin real.
 */
export function createUpdaterMachine(adapter: UpdaterAdapter): UpdaterMachine {
  let state: UpdaterState = { status: "idle" };
  // Handle do update aceito no `check()`, guardado entre "disponível" e "baixando" — a
  // máquina não reconsulta o plugin para instalar o que já anunciou.
  let pending: UpdaterCheckResult | null = null;
  const listeners = new Set<() => void>();

  function setState(next: UpdaterState) {
    state = next;
    for (const listener of listeners) listener();
  }

  async function checkForUpdate(): Promise<void> {
    if (state.status === "checking" || state.status === "downloading") return;
    setState({ status: "checking" });

    let result: UpdaterCheckResult | null;
    try {
      result = await adapter.check();
    } catch {
      // App local-first não reclama de rede: qualquer falha (offline, timeout) volta a
      // ocioso em silêncio, sem estado de erro.
      pending = null;
      setState({ status: "idle" });
      return;
    }

    // Defesa contra o retorno enganoso do plugin: um objeto não-nulo cuja versão anunciada
    // é igual à instalada não é um update real.
    if (!result || result.version === result.currentVersion) {
      pending = null;
      setState({ status: "idle" });
      return;
    }

    pending = result;
    setState({
      status: "available",
      version: result.version,
      currentVersion: result.currentVersion,
      notes: result.notes,
    });
  }

  async function downloadAndInstall(): Promise<void> {
    if (state.status !== "available" || !pending) return;
    const handle = pending;
    const version = handle.version;
    setState({
      status: "downloading",
      version,
      progress: { downloadedBytes: 0, totalBytes: null },
    });

    try {
      await handle.downloadAndInstall((progress) => {
        if (state.status !== "downloading") return;
        setState({ status: "downloading", version, progress });
      });
      pending = null;
      setState({ status: "ready", version });
    } catch (cause) {
      pending = null;
      setState({
        status: "error",
        message: safeErrorMessage(cause, "Não foi possível instalar a atualização."),
      });
    }
  }

  async function relaunch(): Promise<void> {
    if (state.status !== "ready") return;
    await adapter.relaunch();
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    checkForUpdate,
    downloadAndInstall,
    relaunch,
  };
}

const realUpdaterAdapter: UpdaterAdapter = {
  async check() {
    const update = await pluginCheck();
    if (!update) return null;
    return {
      version: update.version,
      currentVersion: update.currentVersion,
      notes: update.body ?? null,
      downloadAndInstall(onProgress) {
        let totalBytes: number | null = null;
        let downloadedBytes = 0;
        return update.downloadAndInstall((event) => {
          if (event.event === "Started") {
            totalBytes = event.data.contentLength ?? null;
          } else if (event.event === "Progress") {
            downloadedBytes += event.data.chunkLength;
          } else {
            return;
          }
          onProgress({ downloadedBytes, totalBytes });
        });
      },
    };
  },
  relaunch: pluginRelaunch,
};

/** Instância única do app — quem consome (launch/Configurações) compartilha o mesmo estado. */
export const updaterMachine: UpdaterMachine = createUpdaterMachine(realUpdaterAdapter);
