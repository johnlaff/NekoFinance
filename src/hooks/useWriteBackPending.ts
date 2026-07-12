import { useEffect, useState } from "react";
import {
  getAppSetting,
  getImportConflicts,
  listenEvent,
  previewWriteBackStatus,
  writeBackEnabled,
  SYNC_DONE_EVENT,
  type SyncDonePayload,
} from "../lib/api";

/**
 * Chaves de preferência local gravadas pelo painel de importação (Configurações → planilha).
 * Lemos diretamente para descobrir a última planilha/aba mapeada — sem acoplar a UI do dashboard
 * ao estado interno daquele painel.
 */
const LAST_IMPORT_KEY = "sheets_last_import";
const LAST_SHEET_KEY = "sheets_last_sheet";
const CLIENT_ID_KEY = "sheets_client_id";

export interface WriteBackPendingState {
  /** Células que divergem (local → planilha). 0 quando desconhecido/sem mapeamento. */
  pendingCount: number;
  /** Conflitos de importação que bloqueiam o write-back (0 quando não há). */
  conflictCount: number;
  /** Flag-mestre do write-back; `false` → botão de envio desabilitado. */
  enabled: boolean;
  loading: boolean;
  error: string | null;
  /** Planilha mapeada (string vazia quando não há mapeamento). Repassada ao painel de aprovação. */
  spreadsheetId: string;
  /** Aba/ano mapeado (string vazia quando ausente). Repassado ao painel de aprovação. */
  sheetName: string;
  /** Client id do OAuth persistido (string vazia quando ausente). */
  clientId: string;
  /** Re-busca o estado sob demanda (ex.: após um lançamento local). Estável o suficiente p/ effects. */
  refresh: () => void;
}

interface Resolved {
  pendingCount: number;
  conflictCount: number;
  enabled: boolean;
  error: string | null;
  spreadsheetId: string;
  sheetName: string;
  clientId: string;
}

const EMPTY: Resolved = {
  pendingCount: 0,
  conflictCount: 0,
  enabled: false,
  error: null,
  spreadsheetId: "",
  sheetName: "",
  clientId: "",
};

/** Extrai um `spreadsheetId` (string) do JSON gravado em `sheets_last_import`; null se inválido. */
function parseSpreadsheetId(raw: string | null): string | null {
  if (!raw) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && "spreadsheetId" in parsed) {
      const id = parsed.spreadsheetId;
      if (typeof id === "string" && id.length > 0) return id;
    }
  } catch {
    // JSON malformado (ou um valor não-objeto, como o "true" do mock e2e) → sem mapeamento.
  }
  return null;
}

/**
 * Busca o estado do write-back pendente a partir dos comandos existentes:
 * lê a última planilha/aba mapeada das preferências locais, mede o diff local → planilha via
 * `previewWriteBackStatus` e conta os conflitos de importação via `getImportConflicts`.
 *
 * Degrada com elegância: sem mapeamento (ou fora do Tauri / mock e2e onde o JSON não bate) retorna
 * zeros e nunca chama a prévia — o banner do dashboard simplesmente não aparece, em vez de erro.
 */
async function fetchPending(): Promise<Resolved> {
  const spreadsheetId = parseSpreadsheetId(await getAppSetting(LAST_IMPORT_KEY));
  if (!spreadsheetId) return EMPTY;

  // Aba/ano: a última gravada; senão, o ano corrente (a aba-ano da planilha do método).
  const sheetName =
    (await getAppSetting(LAST_SHEET_KEY)) ?? String(new Date().getFullYear());
  const clientId = (await getAppSetting(CLIENT_ID_KEY)) ?? "";

  // Flag e conflitos são independentes da prévia; busca-os em paralelo. A flag e os conflitos
  // toleram falha (degradam para o padrão); só a prévia pode marcar `error`.
  const [enabled, conflictCount] = await Promise.all([
    writeBackEnabled().catch(() => false),
    getImportConflicts()
      .then((c) => c.length)
      .catch(() => 0),
  ]);

  try {
    const preview = await previewWriteBackStatus(spreadsheetId, sheetName, clientId);
    const pendingCount = preview.cells.filter((c) => c.changed).length;
    return {
      pendingCount,
      conflictCount,
      enabled,
      error: null,
      spreadsheetId,
      sheetName,
      clientId,
    };
  } catch (e) {
    // A prévia toca a rede/planilha; se falhar, NÃO surgimos um banner de envio (pendingCount=0),
    // mas guardamos o erro e ainda surfamos conflitos (que vêm de uma leitura local barata).
    return {
      pendingCount: 0,
      conflictCount,
      enabled,
      error: e instanceof Error ? e.message : "Falha ao consultar o write-back.",
      spreadsheetId,
      sheetName,
      clientId,
    };
  }
}

/**
 * Estado do "write-back pendente" para o dashboard. Busca no mount, re-busca quando o sync em
 * segundo plano termina (`neko://sync-done`, mesmo padrão do `ConflictGate`) e expõe `refresh()`
 * para re-buscar após um lançamento local. Sem polling em laço — só estes três gatilhos pontuais.
 */
export function useWriteBackPending(): WriteBackPendingState {
  // Estado único (dados + `loading`) atualizado SÓ no callback assíncrono — nada de setState
  // síncrono no corpo do effect (evita renders em cascata). No mount `loading` é true; nos
  // re-fetches (evento/refresh) os dados anteriores permanecem visíveis até a nova busca resolver.
  const [view, setView] = useState<Resolved & { loading: boolean }>({
    ...EMPTY,
    loading: true,
  });
  // Contador-gatilho: incrementá-lo (no mount, no evento, no refresh) re-roda o effect de busca.
  // Evita disparar buscas concorrentes a cada render — só nos três gatilhos pontuais.
  const [tick, setTick] = useState(0);

  // Busca única por `tick`, com guarda de unmount (não chama setState após sair).
  useEffect(() => {
    let alive = true;
    fetchPending()
      .then((r) => {
        if (alive) setView({ ...r, loading: false });
      })
      .catch(() => {
        if (alive) setView({ ...EMPTY, loading: false });
      });
    return () => {
      alive = false;
    };
  }, [tick]);

  // Sync em segundo plano: quando o backend conclui um import automático ele emite
  // `neko://sync-done`. Re-buscamos o estado (mesmo padrão do `ConflictGate`). Cancela a assinatura
  // no unmount (sem vazar o listener no HMR). Fora do Tauri, `listenEvent` devolve um no-op.
  useEffect(() => {
    let alive = true;
    const unlistenPromise = listenEvent<SyncDonePayload>(SYNC_DONE_EVENT, () => {
      if (alive) setTick((t) => t + 1);
    });
    return () => {
      alive = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  return {
    pendingCount: view.pendingCount,
    conflictCount: view.conflictCount,
    enabled: view.enabled,
    loading: view.loading,
    error: view.error,
    spreadsheetId: view.spreadsheetId,
    sheetName: view.sheetName,
    clientId: view.clientId,
    refresh: () => setTick((t) => t + 1),
  };
}
