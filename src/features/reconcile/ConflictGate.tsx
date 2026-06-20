import { useEffect, useState } from "react";
import { Button } from "../../design-system/components/Button";
import {
  ApprovalDiffCard,
  type DiffChange,
} from "../../design-system/components/ApprovalDiffCard";
import {
  getImportConflicts,
  listenEvent,
  resolveImportConflict,
  SYNC_DONE_EVENT,
  type ImportConflict,
  type SyncDonePayload,
} from "../../lib/api";
import { invalidateCommands } from "../../lib/useCommand";
import { formatBRL } from "../../lib/format";

const FIELD_LABEL: Record<string, string> = {
  amount: "Valor",
  description: "Descrição",
};

function fmtValue(field: string, value: string): string {
  if (field === "amount") {
    const n = Number(value);
    return Number.isFinite(n) ? formatBRL(Math.abs(n)) : value;
  }
  return value || "—";
}

/**
 * Gate de conflito de import (spec 013): quando o re-import detecta que VOCÊ editou um valor E a
 * planilha também mudou (de forma diferente), o app não escolhe sozinho. Aqui você decide, lançamento
 * a lançamento, qual versão vale. Some quando não há conflitos.
 */
export function ConflictGate({ onResolved }: { onResolved?: () => void }) {
  const [conflicts, setConflicts] = useState<ImportConflict[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Guarda de unmount: não chama setState se o componente saiu antes do fetch resolver.
  useEffect(() => {
    let alive = true;
    getImportConflicts()
      .then((c) => alive && setConflicts(c))
      .catch(() => alive && setConflicts([]));
    return () => {
      alive = false;
    };
  }, []);

  // Sync em segundo plano (plano 026): quando o backend conclui um import automático ele emite
  // `neko://sync-done`. Re-derruba o cache de finanças (dashboard/grade/totais) e re-busca os
  // conflitos para o badge aparecer sem ação do usuário. Cancela a assinatura no unmount (evita
  // vazar o listener no HMR).
  useEffect(() => {
    let alive = true;
    const unlistenPromise = listenEvent<SyncDonePayload>(SYNC_DONE_EVENT, () => {
      invalidateCommands();
      getImportConflicts()
        .then((c) => alive && setConflicts(c))
        .catch(() => undefined);
    });
    return () => {
      alive = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  async function resolve(c: ImportConflict, choice: "sheet" | "local") {
    setBusy(c.id);
    setError(null);
    try {
      await resolveImportConflict(c.id, choice);
      invalidateCommands();
      setConflicts((cur) => cur.filter((x) => x.id !== c.id));
      onResolved?.();
      setBusy(null);
    } catch {
      setError("Não foi possível resolver o conflito. Tente novamente.");
      setBusy(null);
    }
  }

  if (conflicts.length === 0) return null;

  return (
    <section
      aria-label="Conflitos de importação"
      style={{
        marginBottom: "var(--space-4)",
        display: "grid",
        gap: "var(--space-3)",
      }}
    >
      <header
        style={{ display: "flex", alignItems: "baseline", gap: "var(--space-2)" }}
      >
        <h2
          style={{
            fontSize: "var(--fs-label)",
            fontWeight: "var(--fw-semibold)",
            letterSpacing: "var(--ls-label)",
            textTransform: "uppercase",
            color: "var(--warning-400)",
            margin: 0,
          }}
        >
          {conflicts.length} conflito{conflicts.length > 1 ? "s" : ""} de importação
        </h2>
        <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>
          Você editou e a planilha também mudou. Escolha qual vale.
        </span>
      </header>
      {error && (
        <p role="alert" style={{ margin: 0, color: "var(--danger-400)" }}>
          {error}
        </p>
      )}

      {conflicts.map((c) => {
        const label = FIELD_LABEL[c.field] ?? c.field;
        const changes: DiffChange[] = [
          {
            field: label,
            before: fmtValue(c.field, c.local_value),
            after: fmtValue(c.field, c.sheet_value),
          },
        ];
        return (
          <ApprovalDiffCard
            key={c.id}
            title="Conflito de importação"
            sheet="Seu valor → Planilha"
            changes={changes}
            status="pending"
            note={`O ${label.toLowerCase()} divergiu desde o último import. Mantenha sua edição ou use o que está na planilha.`}
            actions={
              <>
                <Button
                  variant="primary"
                  disabled={busy === c.id}
                  onClick={() => void resolve(c, "local")}
                >
                  Manter o meu
                </Button>
                <Button
                  variant="ghost"
                  disabled={busy === c.id}
                  onClick={() => void resolve(c, "sheet")}
                >
                  Usar da planilha
                </Button>
              </>
            }
          />
        );
      })}
    </section>
  );
}
