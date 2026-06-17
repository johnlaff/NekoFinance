import { useEffect, useReducer, useRef } from "react";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  Download,
  FileSpreadsheet,
  Link2,
  Loader2,
  RefreshCw,
  Unlink,
} from "lucide-react";
import { Button } from "../../design-system/components/Button";
import {
  GOOGLE_CLIENT_ID,
  checkAuthStatus,
  detectSheetLayout,
  disconnectGoogle,
  fetchSheetPreview,
  getSheetMappings,
  importSheetData,
  importEconomiaSheet,
  listSheetNames,
  listUserSpreadsheets,
  saveSheetMapping,
  startOAuthFlow,
  type AuthStatus,
  type SheetInfo,
  type SheetMappingEntry,
  type SheetPreview,
  type UserSpreadsheet,
} from "../../lib/api";
import { extractSpreadsheetId } from "../../lib/spreadsheet-url";
import { safeErrorMessage } from "../../lib/errors";
import { isMetricTab } from "../../lib/sheet-tabs";
import { invalidateCommands } from "../../lib/useCommand";
import { withLoading } from "../../lib/withLoading";
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
  error: string | null;
  step: Step;
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
  error: null,
  step: "connect",
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

  // App aberto já conectado (token persistido): o passo efetivo é a escolha de planilha — sem isso o
  // painel fica preso em "Conectar Google" para sempre, porque o step só avançava dentro do
  // handleConnect (achado do dogfooding).
  const effectiveStep: Step =
    authStatus === "connected" && state.step === "connect" ? "pick" : state.step;

  const loadSpreadsheets = async () => {
    await withLoading(setLoading, async () => {
      try {
        const list = await listUserSpreadsheets(GOOGLE_CLIENT_ID);
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
    set({ error: null });
    await withLoading(setLoading, async () => {
      try {
        await startOAuthFlow(GOOGLE_CLIENT_ID);
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
          const status = await checkAuthStatus();
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
        set({ error: safeErrorMessage(e, "Não foi possível conectar ao Google.") });
      }
    });
  };

  const handleDisconnect = async () => {
    try {
      await disconnectGoogle();
      onAuthChange("disconnected");
      set({
        step: "connect",
        spreadsheets: [],
        sheets: [],
        preview: null,
        mappings: [],
      });
    } catch (e) {
      set({ error: safeErrorMessage(e, "Não foi possível desconectar o Google.") });
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
        const list = await listSheetNames(id, GOOGLE_CLIENT_ID);
        set({ sheets: list });
      } catch (e) {
        set({ error: safeErrorMessage(e, "Não foi possível listar as abas.") });
      }
    });
  };

  const handleSheetSelect = async (name: string) => {
    set({ selectedSheet: name });
    await withLoading(setLoading, async () => {
      try {
        const [prev, maps] = await Promise.all([
          fetchSheetPreview(state.selectedSpreadsheet, name, GOOGLE_CLIENT_ID),
          getSheetMappings(name).catch(() => [] as SheetMappingEntry[]),
        ]);
        set({ preview: prev, mappings: maps, step: "preview" });
      } catch (e) {
        set({
          error: safeErrorMessage(e, "Não foi possível carregar a prévia da aba."),
        });
      }
    });
  };

  const handleDetectLayout = async () => {
    await withLoading(setLoading, async () => {
      try {
        await detectSheetLayout(
          state.selectedSpreadsheet,
          state.selectedSheet,
          GOOGLE_CLIENT_ID,
        );
        const maps = await getSheetMappings(state.selectedSheet);
        set({ mappings: maps, step: "mapping" });
      } catch (e) {
        set({
          error: safeErrorMessage(e, "Não foi possível detectar o layout da aba."),
        });
      }
    });
  };

  const handleToggleMapping = async (mapping: SheetMappingEntry) => {
    const newActive = mapping.is_active !== 1;
    try {
      await saveSheetMapping(mapping.id, mapping.block_offset, newActive);
      dispatch({ type: "toggleMappingActive", id: mapping.id, active: newActive });
    } catch (e) {
      set({ error: safeErrorMessage(e, "Não foi possível salvar o mapeamento.") });
    }
  };

  const handleImport = async () => {
    set({ importResult: null, error: null });
    await withLoading(setImporting, async () => {
      try {
        const profileId = crypto.randomUUID();
        const count = await importSheetData(
          state.selectedSpreadsheet,
          state.selectedSheet,
          profileId,
          GOOGLE_CLIENT_ID,
        );
        invalidateCommands(); // finance numbers changed — drop every cached screen
        set({
          importResult:
            count === 0
              ? "Dados já importados antes (linhas repetidas são ignoradas)."
              : `${count} transações importadas.`,
        });
      } catch (e) {
        set({
          error: safeErrorMessage(e, "Não foi possível importar a aba selecionada."),
        });
      }
    });
  };

  const handleImportEconomia = async () => {
    set({ importResult: null, error: null });
    await withLoading(setImporting, async () => {
      try {
        const count = await importEconomiaSheet(
          state.selectedSpreadsheet,
          GOOGLE_CLIENT_ID,
        );
        invalidateCommands();
        set({
          importResult:
            count === 0
              ? "Nenhuma Economia encontrada na aba Economia."
              : `Economia importada: ${count} mês(es) (poupança → reserva).`,
        });
      } catch (e) {
        set({
          error: safeErrorMessage(e, "Não foi possível importar a aba Economia."),
        });
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
  };
}

function PickStep({
  state,
  onSpreadsheetSelect,
  onPastedUrl,
  onSheetSelect,
}: {
  state: SheetState;
  onSpreadsheetSelect: (id: string) => void;
  onPastedUrl: (value: string) => void;
  onSheetSelect: (name: string) => void;
}) {
  const {
    spreadsheets,
    selectedSpreadsheet,
    pastedUrl,
    sheets,
    selectedSheet,
    loading,
  } = state;
  return (
    <div className="gs-step">
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
              // Abas de métricas (Economia/Totais) têm layout próprio, não o de blocos mensais —
              // importá-las como transações geraria lixo. Terão importador dedicado (spec 010).
              const metric = isMetricTab(s.title);
              return (
                <button
                  key={s.sheet_id}
                  type="button"
                  className={`gs-sheet-btn ${selectedSheet === s.title ? "gs-sheet-btn--active" : ""}`}
                  onClick={() => onSheetSelect(s.title)}
                  disabled={loading || metric}
                  title={
                    metric
                      ? "Aba de métricas do método: import dedicado em breve"
                      : undefined
                  }
                >
                  <FileSpreadsheet size={14} strokeWidth={1.75} />
                  {s.title}
                  {metric && <span className="gs-sheet-btn__tag">métricas</span>}
                </button>
              );
            })}
          </div>
        </>
      )}

      {loading && (
        <div className="gs-loading">
          <Loader2 size={16} className="gs-spin" strokeWidth={1.75} />
          <span>Carregando…</span>
        </div>
      )}
    </div>
  );
}

function PreviewStep({
  state,
  onDetectLayout,
  onImport,
  onImportEconomia,
}: {
  state: SheetState;
  onDetectLayout: () => void;
  onImport: () => void;
  onImportEconomia: () => void;
}) {
  const { preview, selectedSheet, selectedSpreadsheet, importing, importResult } =
    state;
  if (!preview) return null;
  return (
    <div className="gs-step">
      <div className="gs-preview-head">
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
        {importing ? "Importando…" : "Importar dados"}
      </Button>
      <Button
        variant="secondary"
        onClick={onImportEconomia}
        disabled={importing || !selectedSpreadsheet}
      >
        Importar aba Economia (poupança por mês)
      </Button>
      {importResult && (
        <output className="gs-result gs-result--ok">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {importResult}
        </output>
      )}
    </div>
  );
}

function MappingStep({
  state,
  onToggle,
  onImport,
}: {
  state: SheetState;
  onToggle: (mapping: SheetMappingEntry) => void;
  onImport: () => void;
}) {
  const { mappings, importing, importResult, selectedSpreadsheet, selectedSheet } =
    state;
  return (
    <div className="gs-step">
      <div className="gs-mapping-head">
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
  onConnect,
}: {
  loading: boolean;
  error: string | null;
  onConnect: () => void;
}) {
  return (
    <div className="gs-panel">
      <div className="gs-connect">
        <div className="gs-connect__icon">
          <Link2 size={20} strokeWidth={1.75} />
        </div>
        <p className="gs-connect__text">
          Conecte sua conta Google para importar os dados da planilha oficial.
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
            {error}
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
        />
      )}

      {effectiveStep === "preview" && (
        <PreviewStep
          state={state}
          onDetectLayout={() => void sheet.handleDetectLayout()}
          onImport={() => void sheet.handleImport()}
          onImportEconomia={() => void sheet.handleImportEconomia()}
        />
      )}

      {effectiveStep === "mapping" && state.mappings.length > 0 && (
        <MappingStep
          state={state}
          onToggle={(m) => void sheet.handleToggleMapping(m)}
          onImport={() => void sheet.handleImport()}
        />
      )}

      {state.error && (
        <div role="alert" className="gs-result gs-result--err">
          <AlertCircle size={14} strokeWidth={1.75} />
          {state.error}
        </div>
      )}
    </div>
  );
}
