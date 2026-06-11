import { useState } from "react";
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
import { invalidateCommands } from "../../lib/useCommand";

export function GoogleSheetsPanel({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const [spreadsheets, setSpreadsheets] = useState<UserSpreadsheet[]>([]);
  const [selectedSpreadsheet, setSelectedSpreadsheet] = useState<string>("");
  const [sheets, setSheets] = useState<SheetInfo[]>([]);
  const [selectedSheet, setSelectedSheet] = useState<string>("");
  const [preview, setPreview] = useState<SheetPreview | null>(null);
  const [mappings, setMappings] = useState<SheetMappingEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importResult, setImportResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState<"connect" | "pick" | "preview" | "mapping">(
    "connect",
  );

  const handleConnect = async () => {
    if (!GOOGLE_CLIENT_ID) {
      setError(
        "GOOGLE_CLIENT_ID não configurado. Defina VITE_GOOGLE_CLIENT_ID no .env",
      );
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await startOAuthFlow(GOOGLE_CLIENT_ID);
      setTimeout(async () => {
        const status = await checkAuthStatus();
        onAuthChange(status);
        if (status === "connected") {
          setStep("pick");
          await loadSpreadsheets();
        }
      }, 3000);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnectGoogle();
      onAuthChange("disconnected");
      setStep("connect");
      setSpreadsheets([]);
      setSheets([]);
      setPreview(null);
      setMappings([]);
    } catch (e) {
      setError(String(e));
    }
  };

  const loadSpreadsheets = async () => {
    setLoading(true);
    try {
      const list = await listUserSpreadsheets(GOOGLE_CLIENT_ID);
      setSpreadsheets(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleSpreadsheetSelect = async (id: string) => {
    setSelectedSpreadsheet(id);
    setSelectedSheet("");
    setPreview(null);
    setMappings([]);
    setLoading(true);
    try {
      const list = await listSheetNames(id, GOOGLE_CLIENT_ID);
      setSheets(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleSheetSelect = async (name: string) => {
    setSelectedSheet(name);
    setLoading(true);
    try {
      const [prev, maps] = await Promise.all([
        fetchSheetPreview(selectedSpreadsheet, name, GOOGLE_CLIENT_ID),
        getSheetMappings(name).catch(() => [] as SheetMappingEntry[]),
      ]);
      setPreview(prev);
      setMappings(maps);
      setStep("preview");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleDetectLayout = async () => {
    setLoading(true);
    try {
      await detectSheetLayout(selectedSpreadsheet, selectedSheet, GOOGLE_CLIENT_ID);
      const maps = await getSheetMappings(selectedSheet);
      setMappings(maps);
      setStep("mapping");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleToggleMapping = async (mapping: SheetMappingEntry) => {
    const newActive = mapping.is_active !== 1;
    try {
      await saveSheetMapping(mapping.id, mapping.block_offset, newActive);
      setMappings((prev) =>
        prev.map((m) =>
          m.id === mapping.id ? { ...m, is_active: newActive ? 1 : 0 } : m,
        ),
      );
    } catch (e) {
      setError(String(e));
    }
  };

  const handleImport = async () => {
    setImporting(true);
    setImportResult(null);
    setError(null);
    try {
      const profileId = crypto.randomUUID();
      const count = await importSheetData(
        selectedSpreadsheet,
        selectedSheet,
        profileId,
        GOOGLE_CLIENT_ID,
      );
      invalidateCommands(); // finance numbers changed — drop every cached screen
      setImportResult(
        count === 0
          ? "Dados já importados anteriormente (dedup)."
          : `${count} transações importadas.`,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  if (authStatus === "connected" && step !== "connect") {
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
            onClick={() => void handleDisconnect()}
            title="Desconectar Google"
          >
            <Unlink size={14} strokeWidth={1.75} />
          </button>
        </div>

        {step === "pick" && (
          <div className="gs-step">
            <label className="gs-label" htmlFor="gs-spreadsheet">
              Planilha
            </label>
            <div className="gs-select-wrap">
              <select
                id="gs-spreadsheet"
                className="gs-select"
                value={selectedSpreadsheet}
                onChange={(e) => void handleSpreadsheetSelect(e.target.value)}
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

            {selectedSpreadsheet && sheets.length > 0 && (
              <>
                <span className="gs-label" style={{ marginTop: "var(--space-4)" }}>
                  Aba
                </span>
                <div className="gs-sheets">
                  {sheets.map((s) => (
                    <button
                      key={s.sheet_id}
                      type="button"
                      className={`gs-sheet-btn ${selectedSheet === s.title ? "gs-sheet-btn--active" : ""}`}
                      onClick={() => void handleSheetSelect(s.title)}
                      disabled={loading}
                    >
                      <FileSpreadsheet size={14} strokeWidth={1.75} />
                      {s.title}
                    </button>
                  ))}
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
        )}

        {step === "preview" && preview && (
          <div className="gs-step">
            <div className="gs-preview-head">
              <span className="gs-preview-title">
                {selectedSheet} — {preview.total_rows} linhas
              </span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void handleDetectLayout()}
              >
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
            <Button
              variant="primary"
              onClick={() => void handleImport()}
              disabled={importing}
            >
              {importing ? (
                <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
              ) : (
                <Download size={14} strokeWidth={1.75} />
              )}
              {importing ? "Importando…" : "Importar dados"}
            </Button>
            {importResult && (
              <div className="gs-result gs-result--ok">
                <CheckCircle2 size={14} strokeWidth={1.75} />
                {importResult}
              </div>
            )}
          </div>
        )}

        {step === "mapping" && mappings.length > 0 && (
          <div className="gs-step">
            <div className="gs-mapping-head">
              <span className="gs-mapping-title">Mapeamento de colunas</span>
              <span className="gs-mapping-sub">
                Ajuste quais colunas importam dados
              </span>
            </div>
            <div className="gs-mapping-list">
              {mappings.map((m) => (
                <div key={m.id} className="gs-mapping-row">
                  <div className="gs-mapping-info">
                    <span className="gs-mapping-field">
                      {m.column_header ?? m.target_field}
                    </span>
                    <span className="gs-mapping-meta">
                      {m.target_table}.{m.target_field} — offset {m.block_offset}
                    </span>
                  </div>
                  <button
                    type="button"
                    className={`gs-toggle ${m.is_active === 1 ? "gs-toggle--on" : "gs-toggle--off"}`}
                    onClick={() => void handleToggleMapping(m)}
                    aria-pressed={m.is_active === 1}
                    aria-label={`Coluna ${m.column_header ?? m.target_field}`}
                  >
                    <span className="gs-toggle__knob" />
                  </button>
                </div>
              ))}
            </div>
            <Button
              variant="primary"
              onClick={() => void handleImport()}
              disabled={importing}
            >
              {importing ? (
                <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
              ) : (
                <Download size={14} strokeWidth={1.75} />
              )}
              {importing ? "Importando…" : "Importar com mapeamento"}
            </Button>
            {importResult && (
              <div className="gs-result gs-result--ok">
                <CheckCircle2 size={14} strokeWidth={1.75} />
                {importResult}
              </div>
            )}
          </div>
        )}

        {error && (
          <div className="gs-result gs-result--err">
            <AlertCircle size={14} strokeWidth={1.75} />
            {error}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="gs-panel">
      <div className="gs-connect">
        <div className="gs-connect__icon">
          <Link2 size={20} strokeWidth={1.75} />
        </div>
        <p className="gs-connect__text">
          Conecte sua conta Google para importar dados da planilha em tempo real.
        </p>
        <Button
          variant="primary"
          onClick={() => void handleConnect()}
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
            Configure VITE_GOOGLE_CLIENT_ID no arquivo .env
          </p>
        )}
        {error && (
          <div className="gs-result gs-result--err">
            <AlertCircle size={14} strokeWidth={1.75} />
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
