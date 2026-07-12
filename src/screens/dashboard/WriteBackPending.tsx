import { useRef, useState, type CSSProperties } from "react";
import { UploadCloud } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import {
  ConfirmDialog,
  WriteBackPreview,
} from "../../features/sheets/WriteBackPreview";
import { KIND_LABEL, isSafeForFastPath } from "../../features/sheets/writeBack";
import type { WriteBackPendingState } from "../../hooks/useWriteBackPending";
import { applyWriteBack, previewWriteBackStatus, type CellWrite } from "../../lib/api";
import { invalidateCommands } from "../../lib/useCommand";
import { safeErrorMessage } from "../../lib/errors";
import { fmtDayMonth } from "../../lib/format";

// Estilos estáticos hoisted (regra do React Compiler — nunca inline no JSX).
const WB_PENDING_DISABLED: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  width: "100%",
  textAlign: "left",
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--bg-subtle)",
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
};
const WB_ICON: CSSProperties = { flexShrink: 0 };
const WB_HINT: CSSProperties = { color: "var(--text-muted)", marginLeft: "auto" };

// Bloco do caminho rápido "Sincronizar": selo de pendência + ações lado a lado.
const WB_FAST_WRAP: CSSProperties = {
  display: "grid",
  gap: "var(--space-2)",
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--bg-subtle)",
};
const WB_FAST_HEAD: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-2)",
  color: "var(--brass-400)",
  fontSize: "var(--fs-sm)",
};
const WB_FAST_ACTIONS: CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  flexWrap: "wrap",
};
// Resumo inline do diff antes da confirmação (compacto; ≤ 5 células viram uma linha).
const WB_FAST_SUMMARY: CSSProperties = {
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  margin: 0,
};
const WB_FAST_ERR: CSSProperties = {
  color: "var(--danger-400)",
  fontSize: "var(--fs-sm)",
  margin: 0,
};

/** Resumo de uma linha das células que serão escritas (ex.: "Diário 01/06, Saída 15/06"). */
function summarizeChanged(changed: CellWrite[]): string {
  return changed
    .map((c) => `${KIND_LABEL[c.kind] ?? c.kind} ${fmtDayMonth(c.date)}`)
    .join(", ");
}

/**
 * Selo do write-back pendente + os dois caminhos para o MESMO apply do painel de
 * Configurações, sem reimplementar o diff/apply.
 *
 * - "Sincronizar" (caminho rápido): para mudanças de só-valor sem conflito/risco de
 *   fórmula/corrida de frescura, colapsa os cliques (prévia silenciosa → resumo inline → 1 confirmação).
 *   As salvaguardas do backend (frescura por modifiedTime, gate de conflito, blocklist de fórmula)
 *   seguem rodando — só os cliques manuais somem. Quando o diff NÃO é seguro, cai no fluxo completo.
 * - "Revisar e enviar" (fluxo completo): expande o painel multi-etapas, sempre disponível.
 *
 * Com a flag-mestre desligada, vira um aviso não-clicável — o envio mora em Configurações.
 */
function WriteBackStatusBanner({
  pendingCount,
  enabled,
  expanded,
  syncing,
  onSincronizar,
  onToggle,
}: {
  pendingCount: number;
  enabled: boolean;
  expanded: boolean;
  syncing: boolean;
  onSincronizar: () => void;
  onToggle: () => void;
}) {
  const label = `${pendingCount} célula(s) local → planilha pendente(s)`;
  if (!enabled) {
    // `<output>` tem role implícito "status" (live region polite) — preferido a role explícito.
    return (
      <output style={WB_PENDING_DISABLED}>
        <UploadCloud size={15} strokeWidth={1.75} style={WB_ICON} aria-hidden />
        <span>{label}</span>
        <span style={WB_HINT}>Envio desativado nas Configurações.</span>
      </output>
    );
  }
  return (
    <div style={WB_FAST_WRAP}>
      <div style={WB_FAST_HEAD}>
        <UploadCloud size={15} strokeWidth={1.75} style={WB_ICON} aria-hidden />
        <span aria-live="polite">{label}</span>
      </div>
      <div style={WB_FAST_ACTIONS}>
        <Button variant="primary" size="sm" disabled={syncing} onClick={onSincronizar}>
          {syncing ? "Sincronizando…" : "Sincronizar"}
        </Button>
        <Button variant="ghost" size="sm" onClick={onToggle} aria-expanded={expanded}>
          {expanded ? "Fechar" : "Revisar e enviar"}
        </Button>
      </div>
    </div>
  );
}

/**
 * Painel de write-back pendente do dashboard: o selo + o caminho rápido "Sincronizar" +
 * o fallback para o painel multi-etapas completo. Some quando não há nada a enviar /
 * fora de um sheet mapeado / durante o carregamento. Owna todo o estado do envio rápido para não
 * inflar o `DashboardScreen`.
 */
export function WriteBackPending({ writeBack }: { writeBack: WriteBackPendingState }) {
  // `showWriteBack` revela o painel multi-etapas. `syncing` cobre a prévia silenciosa
  // (anti-duplo-clique). `fastPath` guarda o diff seguro + o token de frescura DAQUELA prévia (nunca
  // um `previewRevision` velho do hook). `fastErr` surfa falhas. `applyingFastRef` é guarda
  // anti-duplo-clique do envio (só lido/escrito em handler → ref, não re-renderiza).
  const [showWriteBack, setShowWriteBack] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [fastPath, setFastPath] = useState<{
    changed: CellWrite[];
    previewRevision: string;
  } | null>(null);
  const [fastErr, setFastErr] = useState<string | null>(null);
  const applyingFastRef = useRef(false);

  // Caminho rápido "Sincronizar": busca a prévia EM SILÊNCIO (mesma API do painel completo), avalia
  // a segurança e — se segura — abre só a confirmação com um resumo inline. Se NÃO for segura
  // (conflito, risco de coluna de fórmula, multi-cartão, ou frescura ausente), cai no fluxo completo
  // expandindo o painel. As salvaguardas do backend rodam de qualquer forma; este atalho só decide
  // quais cliques o usuário precisa dar.
  // Sem `finally` de propósito: o React Compiler não otimiza componentes com try/finally.
  async function handleSincronizar() {
    setFastErr(null);
    setSyncing(true);
    try {
      const result = await previewWriteBackStatus(
        writeBack.spreadsheetId,
        writeBack.sheetName,
        writeBack.clientId,
      );
      const changed = result.cells.filter((c) => c.changed);
      const safe = isSafeForFastPath(
        writeBack.enabled,
        writeBack.conflictCount,
        result.multi_card_warning,
        changed,
        result.preview_revision,
      );
      setSyncing(false);
      if (!safe) {
        // Fallback explícito: abre o painel multi-etapas (mesma checagem humana de sempre).
        setFastPath(null);
        setShowWriteBack(true);
        return;
      }
      // Amarra a confirmação ao token de frescura DESTA prévia (não ao do hook, que não o guarda).
      setFastPath({ changed, previewRevision: result.preview_revision });
    } catch (e) {
      setSyncing(false);
      setFastErr(safeErrorMessage(e, "Não foi possível preparar o envio rápido."));
    }
  }

  // Confirmação do caminho rápido: UMA escrita real via o MESMO `apply_write_back` (com o
  // `previewRevision` da prévia → o backend aborta se a planilha mudou). Sem `finally` de propósito.
  async function confirmFastWrite() {
    if (!fastPath || applyingFastRef.current) return;
    applyingFastRef.current = true;
    try {
      await applyWriteBack(
        writeBack.spreadsheetId,
        writeBack.sheetName,
        writeBack.clientId,
        fastPath.previewRevision,
      );
      applyingFastRef.current = false;
      setFastPath(null);
      invalidateCommands(); // os números mudaram — derruba todo cache de tela (igual ao import)
      writeBack.refresh();
    } catch (e) {
      applyingFastRef.current = false;
      setFastPath(null);
      // Inclui o caso "planilha mudou": a salvaguarda de frescura do backend bloqueou a escrita.
      setFastErr(
        safeErrorMessage(
          e,
          "Envio bloqueado. Nada foi escrito — revise e tente de novo.",
        ),
      );
    }
  }

  if (writeBack.loading || writeBack.pendingCount === 0) return null;

  return (
    <>
      <WriteBackStatusBanner
        pendingCount={writeBack.pendingCount}
        enabled={writeBack.enabled}
        expanded={showWriteBack}
        syncing={syncing}
        onSincronizar={() => void handleSincronizar()}
        onToggle={() => setShowWriteBack((v) => !v)}
      />

      {/* Erro do caminho rápido (prévia/escrita): aviso não-bloqueante; o fluxo completo segue à mão. */}
      {fastErr && (
        <output role="alert" style={WB_FAST_ERR}>
          {fastErr}
        </output>
      )}

      {/* Resumo inline do diff seguro + 1 confirmação (caminho rápido). O resumo de uma
          linha cobre os casos rotineiros (≤ 5 células); acima disso mostramos só a contagem e o
          diálogo (que repete a contagem) para não estourar o banner. */}
      {fastPath && (
        <output style={WB_FAST_SUMMARY}>
          {fastPath.changed.length} célula(s):{" "}
          {fastPath.changed.length <= 5
            ? summarizeChanged(fastPath.changed)
            : `${fastPath.changed.length} valores a atualizar`}
        </output>
      )}

      {fastPath && (
        <ConfirmDialog
          count={fastPath.changed.length}
          scope={`Serão atualizadas ${fastPath.changed.length} célula(s) da aba ${writeBack.sheetName}.`}
          onConfirm={() => void confirmFastWrite()}
          onCancel={() => setFastPath(null)}
        />
      )}

      {showWriteBack && writeBack.spreadsheetId && writeBack.sheetName && (
        <WriteBackPreview
          spreadsheetId={writeBack.spreadsheetId}
          sheetName={writeBack.sheetName}
          clientId={writeBack.clientId}
        />
      )}
    </>
  );
}
