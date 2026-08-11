import { useState } from "react";
import { AlertCircle, CheckCircle2, FileUp, Loader2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import { appCacheDir, join } from "@tauri-apps/api/path";
import { Button } from "../../design-system/components/Button";
import { isTauri } from "../../lib/env";
import { safeErrorMessage } from "../../lib/errors";
import { invalidateCommands } from "../../lib/useCommand";
import { withLoading } from "../../lib/withLoading";
import { ImportDiagnosticsNotice } from "./GoogleSheetsPanel";
import { importLocalXlsxCmd, type ImportDiagnostic } from "./sheetsView";

/**
 * O picker do `dialog` devolve um `content://` no Android — sem caminho de filesystem real,
 * o que o comando Rust (que lê o arquivo direto com `std::fs`) não consegue abrir. Materializar
 * os bytes num arquivo próprio do app antes de invocar resolve as duas plataformas pelo mesmo
 * caminho: em desktop o picker já devolve um caminho real, então a cópia é redundante mas
 * inofensiva (planilhas são pequenas).
 */
async function materializeLocalPath(pickedPath: string): Promise<string> {
  const bytes = await readFile(pickedPath);
  const dest = await join(await appCacheDir(), "neko-local-import.xlsx");
  await writeFile(dest, bytes);
  return dest;
}

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
      file = await materializeLocalPath(selected);
    } catch (e) {
      setError(safeErrorMessage(e, "Não foi possível selecionar o arquivo."));
      return;
    }
    await withLoading(setImporting, async () => {
      try {
        const profileId = crypto.randomUUID();
        const outcome = await importLocalXlsxCmd(file, profileId);
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
