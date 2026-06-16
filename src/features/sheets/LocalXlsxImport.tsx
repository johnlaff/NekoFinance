import { useState } from "react";
import { AlertCircle, CheckCircle2, FileUp, Loader2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "../../design-system/components/Button";
import { importLocalXlsx, isTauri } from "../../lib/api";
import { invalidateCommands } from "../../lib/useCommand";
import { withLoading } from "../../lib/withLoading";

/**
 * Imports a local .xlsx copy of the spreadsheet through a native file dialog.
 * Useful when the Google account is not connected (offline-first path).
 */
export function LocalXlsxImport() {
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handlePick = async () => {
    setError(null);
    setResult(null);
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
      setError(String(e));
      return;
    }
    await withLoading(setImporting, async () => {
      try {
        const profileId = crypto.randomUUID();
        const summary = await importLocalXlsx(file, profileId);
        invalidateCommands(); // finance numbers changed — drop every cached screen
        setResult(summary);
      } catch (e) {
        setError(String(e));
      }
    });
  };

  return (
    <div className="xlsx-import">
      <Button
        variant="secondary"
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
      {error && (
        <div className="gs-result gs-result--err">
          <AlertCircle size={14} strokeWidth={1.75} />
          {error}
        </div>
      )}
    </div>
  );
}
