import { useEffect, useReducer, useRef } from "react";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  Download,
  FileSpreadsheet,
  Link2,
  Loader2,
  RefreshCw,
  Unlink,
} from "lucide-react";
import { Button } from "../../design-system/components/Button";
import { GOOGLE_CLIENT_ID } from "../../lib/env";
import { extractSpreadsheetId } from "../../lib/spreadsheet-url";
import { safeErrorMessage } from "../../lib/errors";
import { isEconomiaTab, isMetricTab } from "../../lib/sheet-tabs";
import { invalidateCommands, useCommand } from "../../lib/useCommand";
import { withLoading } from "../../lib/withLoading";
import {
  BG_SYNC_KEY,
  CLIENT_ID_KEY,
  LAST_IMPORT_KEY,
  LAST_SHEET_KEY,
  NOTES_DEGRADED_KEY,
  connectGoogleCmd,
  detectSheetLayoutCmd,
  disconnectGoogleCmd,
  fetchGoogleAuthStatus,
  fetchSheetMappings,
  fetchSheetNames,
  fetchSheetPreviewCmd,
  fetchSheetsSetting,
  fetchUserSpreadsheets,
  importEconomiaSheetCmd,
  importSheetDataCmd,
  saveSheetMappingCmd,
  setSheetsSetting,
  type AuthStatus,
  type ImportDiagnostic,
  type SheetInfo,
  type SheetMappingEntry,
  type SheetPreview,
  type UserSpreadsheet,
} from "./sheetsView";
import { WriteBackPreview } from "./WriteBackPreview";

/** Rótulo PT-BR amigável do campo de destino (esconde o jargão de banco do usuário). */
const FIELD_LABELS_PT: Record<string, string> = {
  date: "Data",
  amount_in: "Entrada",
  amount_out: "Saída",
  amount_daily: "Diário",
  daily_budget: "Teto do diário",
  balance: "Saldo",
};
function fieldLabelPt(field: string): string {
  return FIELD_LABELS_PT[field] ?? field;
}

type Step = "connect" | "pick" | "preview" | "mapping";

/** Última PLANILHA importada (persistida em app_setting). Re-sincronizar = re-importa todas as abas. */
interface LastImport {
  spreadsheetId: string;
  label: string;
}

// Estado do fluxo de import (conectar → escolher → prévia → mapear) agrupado num reducer, em vez de
// doze useState relacionados. Toda a lógica vive no hook `useSheetImport`; as views são puras.
interface SheetState {
  spreadsheets: UserSpreadsheet[];
  selectedSpreadsheet: string;
  pastedUrl: string;
  sheets: SheetInfo[];
  selectedSheet: string;
  preview: SheetPreview | null;
  mappings: SheetMappingEntry[];
  loading: boolean;
  importing: boolean;
  importResult: string | null;
  /** Nota que não deu para itemizar ou item↔célula divergente — informativo. */
  importDiagnostics: ImportDiagnostic[];
  error: string | null;
  /** Erro técnico cru (do backend) — mostrado em "Detalhes técnicos" para suporte/diagnóstico. */
  errorDetail: string | null;
  step: Step;
  /** Carregado de app_setting no mount; alimenta o atalho "Re-sincronizar". */
  lastImport: LastImport | null;
  /** Atualização automática em segundo plano. Padrão ligado; separado do re-sync manual. */
  bgSyncEnabled: boolean;
}

/** Texto cru do erro do backend (o `invoke` rejeita com a String de erro do Rust). */
function detailOf(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return String(e);
}

const initialSheetState: SheetState = {
  spreadsheets: [],
  selectedSpreadsheet: "",
  pastedUrl: "",
  sheets: [],
  selectedSheet: "",
  preview: null,
  mappings: [],
  loading: false,
  importing: false,
  importResult: null,
  importDiagnostics: [],
  error: null,
  errorDetail: null,
  step: "connect",
  lastImport: null,
  bgSyncEnabled: true,
};

type SheetAction =
  | { type: "set"; patch: Partial<SheetState> }
  | { type: "toggleMappingActive"; id: string; active: boolean };

function sheetReducer(s: SheetState, a: SheetAction): SheetState {
  switch (a.type) {
    case "set":
      return { ...s, ...a.patch };
    case "toggleMappingActive":
      return {
        ...s,
        mappings: s.mappings.map((m) =>
          m.id === a.id ? { ...m, is_active: a.active ? 1 : 0 } : m,
        ),
      };
  }
}

interface SheetImport {
  state: SheetState;
  effectiveStep: Step;
  handleConnect: () => Promise<void>;
  handleDisconnect: () => Promise<void>;
  handlePastedUrl: (value: string) => Promise<void>;
  handleSpreadsheetSelect: (id: string) => Promise<void>;
  handleSheetSelect: (name: string) => Promise<void>;
  handleDetectLayout: () => Promise<void>;
  handleToggleMapping: (mapping: SheetMappingEntry) => Promise<void>;
  handleImport: () => Promise<void>;
  handleImportEconomia: () => Promise<void>;
  handleImportAll: () => Promise<void>;
  handleResync: () => Promise<void>;
  handleBackToPick: () => void;
  handleToggleBgSync: (enabled: boolean) => Promise<void>;
}

/** Estado + ações do import do Sheets. Hook (não componente) → toda a lógica fica fora da árvore. */
function useSheetImport(
  authStatus: AuthStatus,
  onAuthChange: (status: AuthStatus) => void,
): SheetImport {
  const [state, dispatch] = useReducer(sheetReducer, initialSheetState);
  const set = (patch: Partial<SheetState>) => dispatch({ type: "set", patch });
  const setLoading = (v: boolean) => set({ loading: v });
  const setImporting = (v: boolean) => set({ importing: v });
  // Erro amigável (mapeado) + o detalhe técnico cru, para diagnóstico/suporte.
  const fail = (e: unknown, fallback: string) =>
    set({ error: safeErrorMessage(e, fallback), errorDetail: detailOf(e) });

  // Quando o app abre já conectado, o passo efetivo é a escolha de planilha; `step` só muda dentro
  // de `handleConnect`, que não roda nesse caminho.
  const effectiveStep: Step =
    authStatus === "connected" && state.step === "connect" ? "pick" : state.step;

  const loadSpreadsheets = async () => {
    await withLoading(setLoading, async () => {
      try {
        const list = await fetchUserSpreadsheets(GOOGLE_CLIENT_ID);
        set({ spreadsheets: list });
      } catch {
        // Sem scope do Drive (token antigo) a listagem dá 403 — o campo de URL colada continua
        // funcionando, então a falha do picker não bloqueia o fluxo.
        set({ spreadsheets: [] });
      }
    });
  };

  const handleConnect = async () => {
    if (!GOOGLE_CLIENT_ID) {
      // Detalhe técnico vai para o console, não para o usuário final.
      console.warn(
        "Conexão Google indisponível: VITE_GOOGLE_CLIENT_ID ausente no build.",
      );
      set({
        error:
          "A conexão com o Google não está configurada nesta instalação. Você ainda pode importar sua planilha como arquivo .xlsx.",
      });
      return;
    }
    set({ error: null, errorDetail: null });
    await withLoading(setLoading, async () => {
      try {
        await connectGoogleCmd(GOOGLE_CLIENT_ID);
        // O consentimento no navegador leva o tempo que o usuário levar — sondar até conectar (ou
        // desistir após 2 min). Sondagem é sequencial por natureza (espera → checa → repete), então
        // usamos recursão com setTimeout em vez de await-dentro-de-loop.
        const pollUntilConnected = async (attempt: number): Promise<void> => {
          if (attempt >= 60) {
            set({
              error:
                "Tempo esgotado aguardando o consentimento. Tente conectar de novo.",
            });
            return;
          }
          await new Promise((resolve) => setTimeout(resolve, 2000));
          const status = await fetchGoogleAuthStatus();
          if (status === "connected") {
            onAuthChange(status);
            set({ step: "pick" });
            await loadSpreadsheets();
            return;
          }
          return pollUntilConnected(attempt + 1);
        };
        await pollUntilConnected(0);
      } catch (e) {
        fail(e, "Não foi possível conectar ao Google.");
      }
    });
  };

  const handleDisconnect = async () => {
    try {
      await disconnectGoogleCmd();
      onAuthChange("disconnected");
      set({
        step: "connect",
        spreadsheets: [],
        sheets: [],
        preview: null,
        mappings: [],
      });
    } catch (e) {
      fail(e, "Não foi possível desconectar o Google.");
    }
  };

  const handlePastedUrl = async (value: string) => {
    set({ pastedUrl: value });
    const id = extractSpreadsheetId(value);
    if (id && id !== state.selectedSpreadsheet) {
      await handleSpreadsheetSelect(id);
    }
  };

  const handleSpreadsheetSelect = async (id: string) => {
    set({ selectedSpreadsheet: id, selectedSheet: "", preview: null, mappings: [] });
    await withLoading(setLoading, async () => {
      try {
        const list = await fetchSheetNames(id, GOOGLE_CLIENT_ID);
        set({ sheets: list });
      } catch (e) {
        fail(e, "Não foi possível listar as abas.");
      }
    });
  };

  const handleSheetSelect = async (name: string) => {
    set({ selectedSheet: name });
    await withLoading(setLoading, async () => {
      try {
        const [prev, maps] = await Promise.all([
          fetchSheetPreviewCmd(state.selectedSpreadsheet, name, GOOGLE_CLIENT_ID),
          fetchSheetMappings(name).catch(() => [] as SheetMappingEntry[]),
        ]);
        set({ preview: prev, mappings: maps, step: "preview" });
      } catch (e) {
        fail(e, "Não foi possível carregar a prévia da aba.");
      }
    });
  };

  const handleDetectLayout = async () => {
    await withLoading(setLoading, async () => {
      try {
        await detectSheetLayoutCmd(
          state.selectedSpreadsheet,
          state.selectedSheet,
          GOOGLE_CLIENT_ID,
        );
        const maps = await fetchSheetMappings(state.selectedSheet);
        set({ mappings: maps, step: "mapping" });
      } catch (e) {
        fail(e, "Não foi possível detectar o layout da aba.");
      }
    });
  };

  const handleToggleMapping = async (mapping: SheetMappingEntry) => {
    const newActive = mapping.is_active !== 1;
    try {
      await saveSheetMappingCmd(mapping.id, mapping.block_offset, newActive);
      dispatch({ type: "toggleMappingActive", id: mapping.id, active: newActive });
    } catch (e) {
      fail(e, "Não foi possível salvar o mapeamento.");
    }
  };

  // Lembra a última PLANILHA importada para o "Re-sincronizar" (re-importa todas as abas dela).
  const persistLastImport = async (spreadsheetId: string) => {
    const label =
      state.spreadsheets.find((s) => s.id === spreadsheetId)?.name ??
      (state.lastImport?.spreadsheetId === spreadsheetId
        ? state.lastImport.label
        : "sua planilha");
    const last: LastImport = { spreadsheetId, label };
    set({ lastImport: last });
    try {
      await setSheetsSetting(LAST_IMPORT_KEY, JSON.stringify(last));
      // Aba-ano importada → o indicador de write-back pendente do dashboard a lê
      // direto desta preferência para medir o diff local → planilha da aba mapeada.
      if (state.selectedSheet)
        await setSheetsSetting(LAST_SHEET_KEY, state.selectedSheet);
      // O client id vive no build do frontend; a tarefa de sync em segundo plano (sem estado da UI)
      // precisa dele para renovar o token. Persistimos junto da última importação.
      if (GOOGLE_CLIENT_ID) await setSheetsSetting(CLIENT_ID_KEY, GOOGLE_CLIENT_ID);
    } catch {
      // Best-effort: o atalho some até a próxima importação, sem quebrar o import.
    }
  };

  // Liga/desliga a atualização automática em segundo plano. Persistido em app_setting;
  // independente do "Re-sincronizar" manual, que continua sempre disponível.
  const handleToggleBgSync = async (enabled: boolean) => {
    set({ bgSyncEnabled: enabled });
    try {
      await setSheetsSetting(BG_SYNC_KEY, enabled ? "true" : "false");
    } catch {
      // Best-effort: reverte o visual se a gravação falhar.
      set({ bgSyncEnabled: !enabled });
    }
  };

  const runImport = async (spreadsheetId: string, sheetName: string) => {
    if (!spreadsheetId || !sheetName) return;
    set({ importResult: null, importDiagnostics: [], error: null, errorDetail: null });
    await withLoading(setImporting, async () => {
      try {
        const profileId = crypto.randomUUID();
        const { count, diagnostics } = await importSheetDataCmd(
          spreadsheetId,
          sheetName,
          profileId,
          GOOGLE_CLIENT_ID,
        );
        invalidateCommands(); // finance numbers changed — drop every cached screen
        await persistLastImport(spreadsheetId);
        set({
          importResult:
            count === 0
              ? "Tudo em dia: nenhuma linha nova (as já importadas são ignoradas)."
              : `${count} transações importadas.`,
          importDiagnostics: diagnostics,
        });
      } catch (e) {
        fail(e, "Não foi possível importar a aba selecionada.");
      }
    });
  };

  const handleImport = () => runImport(state.selectedSpreadsheet, state.selectedSheet);

  // Importa TODAS as abas importáveis da planilha (anos → lançamentos; Economia → poupança;
  // métricas sem importador são puladas). Usado pelo "Importar todas as abas" e pelo "Re-sincronizar"
  // (que agora vale para a planilha inteira). Lista as abas antes, se preciso (ex.: re-sync no mount).
  const importAllTabs = async (spreadsheetId: string) => {
    if (!spreadsheetId) return;
    set({ importResult: null, importDiagnostics: [], error: null, errorDetail: null });
    await withLoading(setImporting, async () => {
      try {
        let sheets = state.sheets;
        if (sheets.length === 0 || state.selectedSpreadsheet !== spreadsheetId) {
          sheets = await fetchSheetNames(spreadsheetId, GOOGLE_CLIENT_ID);
          set({ selectedSpreadsheet: spreadsheetId, sheets });
        }
        const profileId = crypto.randomUUID();
        // Import SEQUENCIAL de propósito: uma escrita SQLite por vez (paralelizar daria "database is
        // locked") e ordem estável. Recursão em vez de await-dentro-de-loop.
        interface Acc {
          txns: number;
          econ: number;
          years: string[];
          diagnostics: ImportDiagnostic[];
        }
        const importFrom = async (i: number, acc: Acc): Promise<Acc> => {
          if (i >= sheets.length) return acc;
          const s = sheets[i]!;
          if (isEconomiaTab(s.title)) {
            acc.econ += await importEconomiaSheetCmd(spreadsheetId, GOOGLE_CLIENT_ID);
          } else if (!isMetricTab(s.title)) {
            const outcome = await importSheetDataCmd(
              spreadsheetId,
              s.title,
              profileId,
              GOOGLE_CLIENT_ID,
            );
            acc.txns += outcome.count;
            acc.diagnostics.push(...outcome.diagnostics);
            acc.years.push(s.title);
          }
          return importFrom(i + 1, acc);
        };
        const { txns, econ, years, diagnostics } = await importFrom(0, {
          txns: 0,
          econ: 0,
          years: [],
          diagnostics: [],
        });
        invalidateCommands();
        await persistLastImport(spreadsheetId);
        const parts: string[] = [];
        if (years.length > 0) parts.push(`${years.join(", ")} (${txns} transações)`);
        if (econ > 0) parts.push(`Economia (${econ} mês(es))`);
        set({
          importResult: parts.length
            ? `Importado: ${parts.join(" + ")}.`
            : "Tudo em dia: nenhuma linha nova.",
          importDiagnostics: diagnostics,
        });
      } catch (e) {
        fail(e, "Não foi possível importar as abas.");
      }
    });
  };

  const handleImportAll = () => importAllTabs(state.selectedSpreadsheet);

  // Re-sincroniza a PLANILHA INTEIRA já importada (todas as abas), sem re-buscar nem re-colar a URL.
  const handleResync = async () => {
    if (!state.lastImport) return;
    await importAllTabs(state.lastImport.spreadsheetId);
  };

  // Volta à lista de abas (do preview/mapeamento) para importar outra aba sem refazer a busca.
  const handleBackToPick = () => set({ step: "pick" });

  const handleImportEconomia = async () => {
    set({ importResult: null, importDiagnostics: [], error: null, errorDetail: null });
    await withLoading(setImporting, async () => {
      try {
        const count = await importEconomiaSheetCmd(
          state.selectedSpreadsheet,
          GOOGLE_CLIENT_ID,
        );
        invalidateCommands();
        await persistLastImport(state.selectedSpreadsheet);
        set({
          importResult:
            count === 0
              ? "Nenhuma Economia encontrada na aba Economia."
              : `Economia importada: ${count} mês(es) como métrica mensal.`,
        });
      } catch (e) {
        fail(e, "Não foi possível importar a aba Economia.");
      }
    });
  };

  // Carga inicial das planilhas quando o painel monta JÁ conectado (token persistido): é data-fetch
  // de montagem, não um event handler disfarçado — o caminho por evento (botão Conectar) já chama
  // loadSpreadsheets diretamente. O ref garante uma única tentativa (o Drive pode dar 403).
  const triedAutoLoad = useRef(false);
  useEffect(() => {
    if (
      // react-doctor-disable-next-line react-doctor/no-event-handler -- carga de montagem (o painel pode nascer já conectado por token persistido), não event-faking; o caminho por evento (botão Conectar) já chama loadSpreadsheets diretamente
      authStatus === "connected" &&
      effectiveStep === "pick" &&
      !triedAutoLoad.current
    ) {
      triedAutoLoad.current = true;
      void loadSpreadsheets();
    }
    // Mount-once com guarda por ref: `loadSpreadsheets` muda a cada render mas só roda uma vez (ref),
    // então não entra nas deps (evitaria recriação por render / re-subscribe sem ganho).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [authStatus, effectiveStep]);

  // Carrega a última importação (planilha/aba) persistida para habilitar o "Re-sincronizar". `dispatch`
  // é estável (useReducer), então o effect roda só no mount.
  useEffect(() => {
    let alive = true;
    fetchSheetsSetting(LAST_IMPORT_KEY)
      .then((raw) => {
        if (!alive || !raw) return;
        try {
          const parsed = JSON.parse(raw) as LastImport;
          if (parsed?.spreadsheetId) {
            dispatch({
              type: "set",
              patch: {
                lastImport: {
                  spreadsheetId: parsed.spreadsheetId,
                  label: parsed.label || "sua planilha",
                },
              },
            });
          }
        } catch {
          // app_setting corrompido → ignora o atalho.
        }
      })
      .catch(() => undefined);
    // Estado da atualização automática. Chave ausente = ligado por padrão.
    fetchSheetsSetting(BG_SYNC_KEY)
      .then((raw) => {
        if (!alive) return;
        dispatch({ type: "set", patch: { bgSyncEnabled: raw !== "false" } });
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  return {
    state,
    effectiveStep,
    handleConnect,
    handleDisconnect,
    handlePastedUrl,
    handleSpreadsheetSelect,
    handleSheetSelect,
    handleDetectLayout,
    handleToggleMapping,
    handleImport,
    handleImportEconomia,
    handleImportAll,
    handleResync,
    handleBackToPick,
    handleToggleBgSync,
  };
}

const fetchNotesDegraded = () => fetchSheetsSetting(NOTES_DEGRADED_KEY);

/** Aviso do ciclo degradado: o último sync não conseguiu ler as NOTAS de célula (a
 *  classificação Cartão/Economia/Patrimônio ficou congelada no último ciclo saudável).
 *  O backend grava/limpa a chave a cada ciclo; sem valor → nada a mostrar. */
function NotesDegradedNotice() {
  const q = useCommand(`get_app_setting:${NOTES_DEGRADED_KEY}`, fetchNotesDegraded);
  if (!q.data) return null;
  return (
    <p
      role="status"
      style={{
        margin: 0,
        padding: "6px 10px",
        borderRadius: "var(--radius-md)",
        background: "var(--warning-tint)",
        color: "var(--warning-400)",
        fontSize: 12.5,
      }}
    >
      No último sync as notas de célula de «{q.data}» não puderam ser lidas — a
      classificação por seção segue a do último ciclo saudável.
    </p>
  );
}

/**
 * Torna visível quando uma nota não deu para itemizar OU os itens divergem do total
 * da célula — a célula continua dona do total, isto só reporta (não recalcula nada). Só um
 * componente (reusado nos passos do import do Sheets e no import de .xlsx local); informativo,
 * nunca bloqueante — some quando `diagnostics` está vazio.
 */
export function ImportDiagnosticsNotice({
  diagnostics,
}: {
  diagnostics: ImportDiagnostic[];
}) {
  if (diagnostics.length === 0) return null;
  const n = diagnostics.length;
  return (
    <details className="gs-diagnostics">
      <summary className="gs-diagnostics__summary">
        <AlertCircle size={14} strokeWidth={1.75} />
        {n} {n === 1 ? "nota precisa" : "notas precisam"} de atenção
      </summary>
      <ul className="gs-diagnostics__list">
        {diagnostics.map((d) => (
          <li key={`${d.sheet}-${d.cell}-${d.detail}`} className="gs-diagnostics__item">
            <span className="gs-diagnostics__meta">
              {d.sheet} · {d.cell}
            </span>
            <span className="gs-diagnostics__detail">{d.detail}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

function PickStep({
  state,
  onSpreadsheetSelect,
  onPastedUrl,
  onSheetSelect,
  onResync,
  onImportEconomia,
  onImportAll,
  onToggleBgSync,
}: {
  state: SheetState;
  onSpreadsheetSelect: (id: string) => void;
  onPastedUrl: (value: string) => void;
  onSheetSelect: (name: string) => void;
  onResync: () => void;
  onImportEconomia: () => void;
  onImportAll: () => void;
  onToggleBgSync: (enabled: boolean) => void;
}) {
  const {
    spreadsheets,
    selectedSpreadsheet,
    pastedUrl,
    sheets,
    selectedSheet,
    loading,
    importing,
    importResult,
    importDiagnostics,
    lastImport,
    bgSyncEnabled,
  } = state;
  return (
    <div className="gs-step">
      {lastImport && (
        <div className="gs-resync">
          <div className="gs-resync__info">
            <span className="gs-resync__label">Última planilha</span>
            <span className="gs-resync__name">{lastImport.label}</span>
          </div>
          <Button variant="primary" onClick={onResync} disabled={importing || loading}>
            {importing ? (
              <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
            ) : (
              <RefreshCw size={14} strokeWidth={1.75} />
            )}
            {importing ? "Sincronizando…" : "Re-sincronizar"}
          </Button>
          {/* Atualização automática em segundo plano: controle secundário,
              separado do botão manual acima. Checkbox nativo = acessível sem componente custom. */}
          <label className="gs-bgsync">
            <input
              type="checkbox"
              checked={bgSyncEnabled}
              onChange={(e) => onToggleBgSync(e.target.checked)}
            />
            <span className="gs-bgsync__label">Atualização automática</span>
          </label>
          <NotesDegradedNotice />
          <span className="gs-label" style={{ marginTop: "var(--space-2)" }}>
            Ou importar outra planilha/aba
          </span>
        </div>
      )}
      {spreadsheets.length > 0 && (
        <>
          <label className="gs-label" htmlFor="gs-spreadsheet">
            Planilha
          </label>
          <div className="gs-select-wrap">
            <select
              id="gs-spreadsheet"
              className="gs-select"
              value={selectedSpreadsheet}
              onChange={(e) => onSpreadsheetSelect(e.target.value)}
              disabled={loading}
            >
              <option value="">Selecionar planilha…</option>
              {spreadsheets.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
            <ChevronDown size={14} className="gs-select__arrow" />
          </div>
        </>
      )}

      <label
        className="gs-label"
        htmlFor="gs-spreadsheet-url"
        style={spreadsheets.length > 0 ? { marginTop: "var(--space-4)" } : undefined}
      >
        {spreadsheets.length > 0 ? "Ou cole a URL da planilha" : "URL da planilha"}
      </label>
      <input
        id="gs-spreadsheet-url"
        className="gs-select"
        type="text"
        placeholder="https://docs.google.com/spreadsheets/d/…"
        value={pastedUrl}
        onChange={(e) => onPastedUrl(e.target.value)}
        disabled={loading}
        spellCheck={false}
      />

      {selectedSpreadsheet && sheets.length > 0 && (
        <>
          <span className="gs-label" style={{ marginTop: "var(--space-4)" }}>
            Aba
          </span>
          <div className="gs-sheets">
            {sheets.map((s) => {
              // Economia tem importador próprio (poupança por mês) → clicável, importa direto.
              // As demais métricas (Totais/métricas) têm layout próprio e ainda não têm importador;
              // importá-las como transações geraria lixo → bloqueadas.
              const economia = isEconomiaTab(s.title);
              const metric = !economia && isMetricTab(s.title);
              return (
                <button
                  key={s.sheet_id}
                  type="button"
                  className={`gs-sheet-btn ${selectedSheet === s.title ? "gs-sheet-btn--active" : ""}`}
                  onClick={() =>
                    economia ? onImportEconomia() : onSheetSelect(s.title)
                  }
                  disabled={loading || importing || metric}
                  title={
                    economia
                      ? "Importa a poupança por mês (aba Economia)"
                      : metric
                        ? "Aba de métricas do método: import dedicado em breve"
                        : undefined
                  }
                >
                  <FileSpreadsheet size={14} strokeWidth={1.75} />
                  {s.title}
                  {metric && <span className="gs-sheet-btn__tag">Métricas</span>}
                </button>
              );
            })}
          </div>
          <p className="gs-hint">
            Aba-ano (2025, 2026…) importa lançamentos; <strong>Economia</strong> importa
            a poupança por mês.
          </p>
          <Button
            variant="primary"
            onClick={onImportAll}
            disabled={loading || importing}
          >
            {importing ? (
              <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
            ) : (
              <Download size={14} strokeWidth={1.75} />
            )}
            {importing ? "Importando…" : "Importar todas as abas"}
          </Button>
        </>
      )}

      {(loading || importing) && (
        <div className="gs-loading">
          <Loader2 size={16} className="gs-spin" strokeWidth={1.75} />
          <span>{importing ? "Importando…" : "Carregando…"}</span>
        </div>
      )}
      {importResult && (
        <output className="gs-result gs-result--ok">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {importResult}
        </output>
      )}
      <ImportDiagnosticsNotice diagnostics={importDiagnostics} />
    </div>
  );
}

function PreviewStep({
  state,
  onDetectLayout,
  onImport,
  onBack,
}: {
  state: SheetState;
  onDetectLayout: () => void;
  onImport: () => void;
  onBack: () => void;
}) {
  const { preview, selectedSheet, importing, importResult, importDiagnostics } = state;
  if (!preview) return null;
  return (
    <div className="gs-step">
      <div className="gs-preview-head">
        <Button variant="ghost" size="sm" onClick={onBack} disabled={importing}>
          <ChevronLeft size={14} strokeWidth={1.75} />
          Abas
        </Button>
        <span className="gs-preview-title">
          {selectedSheet} — {preview.total_rows} linhas
        </span>
        <Button variant="ghost" size="sm" onClick={onDetectLayout}>
          <RefreshCw size={14} strokeWidth={1.75} />
          Detectar layout
        </Button>
      </div>
      <div className="gs-preview-table">
        <table>
          <thead>
            <tr>
              {preview.headers.slice(0, 8).map((h, i) => (
                <th key={i}>{h || `Col ${i}`}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.rows.slice(0, 5).map((row, ri) => (
              <tr key={ri}>
                {row.slice(0, 8).map((cell, ci) => (
                  <td key={ci}>{cell}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <Button variant="primary" onClick={onImport} disabled={importing}>
        {importing ? (
          <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
        ) : (
          <Download size={14} strokeWidth={1.75} />
        )}
        {importing ? "Importando…" : `Importar ${selectedSheet}`}
      </Button>
      {importResult && (
        <output className="gs-result gs-result--ok">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {importResult}
        </output>
      )}
      <ImportDiagnosticsNotice diagnostics={importDiagnostics} />
    </div>
  );
}

function MappingStep({
  state,
  onToggle,
  onImport,
  onBack,
}: {
  state: SheetState;
  onToggle: (mapping: SheetMappingEntry) => void;
  onImport: () => void;
  onBack: () => void;
}) {
  const {
    mappings,
    importing,
    importResult,
    importDiagnostics,
    selectedSpreadsheet,
    selectedSheet,
  } = state;
  return (
    <div className="gs-step">
      <div className="gs-mapping-head">
        <Button variant="ghost" size="sm" onClick={onBack} disabled={importing}>
          <ChevronLeft size={14} strokeWidth={1.75} />
          Abas
        </Button>
        <span className="gs-mapping-title">Mapeamento de colunas</span>
        <span className="gs-mapping-sub">Ajuste quais colunas importam dados</span>
      </div>
      <div className="gs-mapping-list">
        {mappings.map((m) => (
          <div key={m.id} className="gs-mapping-row">
            <div className="gs-mapping-info">
              <span className="gs-mapping-field">
                {m.column_header ?? fieldLabelPt(m.target_field)}
              </span>
              <span
                className="gs-mapping-meta"
                title={`${m.target_table}.${m.target_field} · offset ${m.block_offset}`}
              >
                Importa como {fieldLabelPt(m.target_field)}
              </span>
            </div>
            <button
              type="button"
              className={`gs-toggle ${m.is_active === 1 ? "gs-toggle--on" : "gs-toggle--off"}`}
              onClick={() => onToggle(m)}
              aria-pressed={m.is_active === 1}
              aria-label={`Coluna ${m.column_header ?? m.target_field}`}
            >
              <span className="gs-toggle__knob" />
            </button>
          </div>
        ))}
      </div>
      <Button variant="primary" onClick={onImport} disabled={importing}>
        {importing ? (
          <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
        ) : (
          <Download size={14} strokeWidth={1.75} />
        )}
        {importing ? "Importando…" : "Importar com mapeamento"}
      </Button>
      {importResult && (
        <output className="gs-result gs-result--ok">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {importResult}
        </output>
      )}
      <ImportDiagnosticsNotice diagnostics={importDiagnostics} />

      <WriteBackPreview
        spreadsheetId={selectedSpreadsheet}
        sheetName={selectedSheet}
        clientId={GOOGLE_CLIENT_ID}
      />
    </div>
  );
}

function ConnectView({
  loading,
  error,
  errorDetail,
  onConnect,
}: {
  loading: boolean;
  error: string | null;
  errorDetail: string | null;
  onConnect: () => void;
}) {
  return (
    <div className="gs-panel">
      <div className="gs-connect">
        <div className="gs-connect__icon">
          <Link2 size={20} strokeWidth={1.75} />
        </div>
        <p className="gs-connect__text">
          Conecte sua conta Google para importar os dados da sua planilha.
        </p>
        <Button
          variant="primary"
          onClick={onConnect}
          disabled={loading || !GOOGLE_CLIENT_ID}
        >
          {loading ? (
            <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
          ) : (
            <Link2 size={14} strokeWidth={1.75} />
          )}
          {loading ? "Conectando…" : "Conectar Google"}
        </Button>
        {!GOOGLE_CLIENT_ID && (
          <p className="gs-connect__hint">
            Conexão Google indisponível nesta instalação — use a importação de arquivo
            .xlsx abaixo.
          </p>
        )}
        {error && (
          <div role="alert" className="gs-result gs-result--err">
            <AlertCircle size={14} strokeWidth={1.75} />
            <span>
              {error}
              {errorDetail && (
                <details className="gs-error-detail">
                  <summary>Detalhes técnicos</summary>
                  <code>{errorDetail}</code>
                </details>
              )}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

export function GoogleSheetsPanel({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const sheet = useSheetImport(authStatus, onAuthChange);
  const { state, effectiveStep } = sheet;

  if (authStatus !== "connected") {
    return (
      <ConnectView
        loading={state.loading}
        error={state.error}
        errorDetail={state.errorDetail}
        onConnect={() => void sheet.handleConnect()}
      />
    );
  }

  return (
    <div className="gs-panel">
      <div className="gs-header">
        <div className="gs-header__left">
          <div className="gs-status-dot gs-status-dot--ok" />
          <span className="gs-header__text">Google Sheets conectado</span>
        </div>
        <button
          type="button"
          className="gs-disconnect"
          onClick={() => void sheet.handleDisconnect()}
          title="Desconectar Google"
          aria-label="Desconectar Google"
        >
          <Unlink size={14} strokeWidth={1.75} />
        </button>
      </div>

      {effectiveStep === "pick" && (
        <PickStep
          state={state}
          onSpreadsheetSelect={(id) => void sheet.handleSpreadsheetSelect(id)}
          onPastedUrl={(value) => void sheet.handlePastedUrl(value)}
          onSheetSelect={(name) => void sheet.handleSheetSelect(name)}
          onResync={() => void sheet.handleResync()}
          onImportEconomia={() => void sheet.handleImportEconomia()}
          onImportAll={() => void sheet.handleImportAll()}
          onToggleBgSync={(enabled) => void sheet.handleToggleBgSync(enabled)}
        />
      )}

      {effectiveStep === "preview" && (
        <PreviewStep
          state={state}
          onDetectLayout={() => void sheet.handleDetectLayout()}
          onImport={() => void sheet.handleImport()}
          onBack={() => sheet.handleBackToPick()}
        />
      )}

      {effectiveStep === "mapping" && state.mappings.length > 0 && (
        <MappingStep
          state={state}
          onToggle={(m) => void sheet.handleToggleMapping(m)}
          onImport={() => void sheet.handleImport()}
          onBack={() => sheet.handleBackToPick()}
        />
      )}

      {state.error && (
        <div role="alert" className="gs-result gs-result--err">
          <AlertCircle size={14} strokeWidth={1.75} />
          <span>
            {state.error}
            {state.errorDetail && (
              <details className="gs-error-detail">
                <summary>Detalhes técnicos</summary>
                <code>{state.errorDetail}</code>
              </details>
            )}
          </span>
        </div>
      )}
    </div>
  );
}
