import { useState } from "react";
import { AlertCircle, CheckCircle2, FileUp, Loader2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "../../design-system/components/Button";
import { type ImportDiagnostic, importLocalXlsx, isTauri } from "../../lib/api";
import { safeErrorMessage } from "../../lib/errors";
import { invalidateCommands } from "../../lib/useCommand";
import { withLoading } from "../../lib/withLoading";
import { ImportDiagnosticsNotice } from "./GoogleSheetsPanel";

/**
 * Imports a local .xlsx copy of the spreadsheet through a native file dialog.
 * Useful when the Google account is not connected (offline-first path).
 */
export function LocalXlsxImport() {
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<ImportDiagnostic[]>([]);
  const [error, setError] = useState<string | null>(null);

  const handlePick = async () => {
    setError(null);
    setResult(null);
    setDiagnostics([]);
    // Seleção do arquivo primeiro (sem loading); o try/finally do loading mora em withLoading.
    let file: string;
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Planilha", extensions: ["xlsx"] }],
      });
      if (typeof selected !== "string") return; // dialog dismissed
      file = selected;
    } catch (e) {
      setError(safeErrorMessage(e, "Não foi possível selecionar o arquivo."));
      return;
    }
    await withLoading(setImporting, async () => {
      try {
        const profileId = crypto.randomUUID();
        const outcome = await importLocalXlsx(file, profileId);
        invalidateCommands(); // finance numbers changed — drop every cached screen
        setResult(outcome.summary);
        setDiagnostics(outcome.diagnostics);
      } catch (e) {
        setError(safeErrorMessage(e, "Não foi possível importar o arquivo local."));
      }
    });
  };

  return (
    <div className="xlsx-import">
      <Button
        variant="ghost"
        onClick={() => void handlePick()}
        disabled={importing || !isTauri}
      >
        {importing ? (
          <Loader2 size={14} className="gs-spin" strokeWidth={1.75} />
        ) : (
          <FileUp size={14} strokeWidth={1.75} />
        )}
        {importing ? "Importando…" : "Escolher arquivo .xlsx"}
      </Button>
      {result && (
        <div className="gs-result gs-result--ok">
          <CheckCircle2 size={14} strokeWidth={1.75} />
          {result}
        </div>
      )}
      <ImportDiagnosticsNotice diagnostics={diagnostics} />
      {error && (
        <div className="gs-result gs-result--err">
          <AlertCircle size={14} strokeWidth={1.75} />
          {error}
        </div>
      )}
    </div>
  );
}
