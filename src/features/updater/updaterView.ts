//! Porta do domínio Update: quem quiser checar/baixar/instalar uma atualização importa só
//! daqui — nunca `@tauri-apps/plugin-updater`/`@tauri-apps/plugin-process` diretamente. O
//! adapter fica na borda (`realUpdaterAdapter`) para a máquina de estados ser testável com um
//! fake, sem depender do shell Tauri.

import { relaunch as pluginRelaunch } from "@tauri-apps/plugin-process";
import { check as pluginCheck } from "@tauri-apps/plugin-updater";
import { checkUpdateSpace, type UpdateSpaceVerdict } from "../../lib/api";
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
  download: (onProgress: (progress: DownloadProgress) => void) => Promise<void>;
  install: () => Promise<void>;
  /**
   * Descarta o handle sem instalar. O update baixado vive em memória no plugin (~12 MB) —
   * reter o handle depois de desistir (convite recusado, sem espaço) retém aquela memória à
   * toa até o processo fechar.
   */
  close: () => Promise<void>;
}

export interface UpdaterAdapter {
  /** Resolve `null` quando não há update. */
  check: () => Promise<UpdaterCheckResult | null>;
  /**
   * Veredito de espaço em disco para o update pendente. Pode rejeitar (comando indisponível,
   * plataforma sem suporte): a máquina trata a rejeição como falha de MEDIÇÃO, nunca como
   * falta de espaço — o convite segue o fluxo normal em vez de travar por uma checagem que
   * não rodou.
   */
  checkSpace: () => Promise<UpdateSpaceVerdict>;
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
  /**
   * O NSIS aborta no meio do Extract com disco cheio e deixa o exe truncado — pior que nunca
   * ter baixado. A checagem de espaço acontece duas vezes (ao confirmar o update, de novo
   * entre download e install) e este estado é o resultado de QUALQUER uma delas reprovar.
   */
  | {
      status: "blocked-space";
      version: string;
      missingBytes: number;
      requiredBytes: number;
    }
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

const TEN_MEBIBYTES = 10 * 1024 * 1024;

/**
 * Legenda de "quanto liberar" — arredonda PARA CIMA em múltiplos de 10 MiB. O veredito do
 * backend é exato até o byte, mas pedir para liberar exatamente o que falta é um convite a
 * voltar a travar na primeira variação do instalador; o degrau de 10 MiB dá folga sem inflar
 * a mensagem a esmo.
 */
export function missingSpaceLabel(missingBytes: number): string {
  const roundedUp =
    Math.ceil(Math.max(0, missingBytes) / TEN_MEBIBYTES) * TEN_MEBIBYTES;
  return formatBytes(roundedUp);
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

/** Didática do bloqueio por espaço — vive atrás de pergunta (`InfoPopover`), nunca em
 *  parágrafo fixo, e é compartilhada entre o convite e Configurações para nunca divergir.
 *  Explica por que o pedido supera o tamanho do download. */
export const blockedSpaceExplainer = {
  title: "Por que esse tamanho?",
  body: "A conta soma o instalador temporário e os arquivos que ele extrai, com margem de segurança — ficar sem espaço no meio da escrita corromperia o app instalado.",
};

export interface UpdateStatusCopy {
  headline: string;
  /** Complemento opcional (versão, progresso, mensagem de erro) — null quando o
   *  headline já é a frase inteira (ocioso, checando). */
  detail: string | null;
}

/** Leitura textual do estado para superfícies fora do convite (bloco de Configurações,
 *  issue #383) — o usuário sabe onde está sem esperar o convite calmo aparecer. */
export function updateStatusCopy(state: UpdaterState): UpdateStatusCopy {
  switch (state.status) {
    case "idle":
      return { headline: "Nenhuma atualização pendente", detail: null };
    case "checking":
      return { headline: "Checando atualização…", detail: null };
    case "available":
      return {
        headline: "Atualização disponível",
        detail: `v${state.version} pronta para baixar.`,
      };
    case "downloading":
      return {
        headline: "Baixando atualização…",
        detail: downloadLabel(state.progress),
      };
    case "ready":
      return {
        headline: "Pronto para reiniciar",
        detail: `v${state.version} instalada — reinicie para aplicar.`,
      };
    case "blocked-space":
      return {
        headline: "Sem espaço em disco para atualizar",
        detail: `Libere ~${missingSpaceLabel(state.missingBytes)} para instalar a v${state.version}.`,
      };
    case "error":
      return {
        headline: "Não foi possível instalar a atualização",
        detail: state.message,
      };
  }
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

  /** Fecha o handle e engole o erro — um `close()` que falha não deve impedir a transição
   *  para `blocked-space`, o handle já não serve para nada de qualquer forma. */
  async function closeQuietly(handle: UpdaterCheckResult): Promise<void> {
    try {
      await handle.close();
    } catch {
      // silencioso de propósito — ver comentário acima.
    }
  }

  /**
   * Roda o veredito de espaço. Falha de MEDIÇÃO (comando indisponível, plataforma sem
   * suporte) nunca vira bloqueio: `null` sinaliza "não sei" e quem chama trata como se a
   * checagem não tivesse rodado — o convite não pode travar por um diagnóstico que falhou.
   */
  async function tryCheckSpace(): Promise<UpdateSpaceVerdict | null> {
    try {
      return await adapter.checkSpace();
    } catch (cause) {
      console.warn(
        "Medição de espaço em disco falhou — seguindo sem pré-checagem:",
        cause,
      );
      return null;
    }
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

    // Primeira checagem de espaço: antes de OFERECER o update, não depois — evita convidar
    // para um download que o instalador não vai conseguir extrair.
    const verdict = await tryCheckSpace();
    if (verdict && !verdict.ok) {
      await closeQuietly(result);
      pending = null;
      setState({
        status: "blocked-space",
        version: result.version,
        missingBytes: verdict.missing_bytes,
        requiredBytes: verdict.required_bytes,
      });
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
      await handle.download((progress) => {
        if (state.status !== "downloading") return;
        setState({ status: "downloading", version, progress });
      });
    } catch (cause) {
      await closeQuietly(handle);
      pending = null;
      setState({
        status: "error",
        message: safeErrorMessage(cause, "Não foi possível baixar a atualização."),
      });
      return;
    }

    // Segunda checagem: fecha a janela de corrida entre o convite e o instalador (outro
    // programa pode ter enchido o disco nesse meio-tempo). Ela protege o Extract do NSIS,
    // que já baixou o pacote e agora vai gravar no disco de verdade.
    const verdict = await tryCheckSpace();
    if (verdict && !verdict.ok) {
      await closeQuietly(handle);
      pending = null;
      setState({
        status: "blocked-space",
        version,
        missingBytes: verdict.missing_bytes,
        requiredBytes: verdict.required_bytes,
      });
      return;
    }

    try {
      await handle.install();
      pending = null;
      setState({ status: "ready", version });
    } catch (cause) {
      await closeQuietly(handle);
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
      download(onProgress) {
        let totalBytes: number | null = null;
        let downloadedBytes = 0;
        return update.download((event) => {
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
      install: () => update.install(),
      close: () => update.close(),
    };
  },
  checkSpace: () => checkUpdateSpace(),
  relaunch: pluginRelaunch,
};

/** Instância única do app — quem consome (launch/Configurações) compartilha o mesmo estado. */
export const updaterMachine: UpdaterMachine = createUpdaterMachine(realUpdaterAdapter);
