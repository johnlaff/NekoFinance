import { useEffect, useState } from "react";
import { Button } from "../../design-system/components/Button";
import {
  ApprovalDiffCard,
  type DiffChange,
} from "../../design-system/components/ApprovalDiffCard";
import {
  getImportConflicts,
  resolveImportConflict,
  type ImportConflict,
} from "../../lib/api";
import { invalidateCommands } from "../../lib/useCommand";
import { fmtBRL } from "../../lib/format";

const FIELD_LABEL: Record<string, string> = {
  amount: "Valor",
  description: "Descrição",
};

function fmtValue(field: string, value: string): string {
  if (field === "amount") {
    const n = Number(value);
    return Number.isFinite(n) ? fmtBRL(Math.abs(n)) : value;
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

  function load() {
    getImportConflicts()
      .then(setConflicts)
      .catch(() => setConflicts([]));
  }
  useEffect(load, []);

  async function resolve(c: ImportConflict, choice: "sheet" | "local") {
    setBusy(c.id);
    try {
      await resolveImportConflict(c.id, choice);
      invalidateCommands();
      setConflicts((cur) => cur.filter((x) => x.id !== c.id));
      onResolved?.();
    } finally {
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
