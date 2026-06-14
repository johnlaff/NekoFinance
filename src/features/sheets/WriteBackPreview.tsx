import { useEffect, useState } from "react";
import { GitCompareArrows, Lock } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import {
  ApprovalDiffCard,
  type DiffChange,
} from "../../design-system/components/ApprovalDiffCard";
import {
  applyWriteBack,
  previewWriteBack,
  writeBackEnabled,
  type CellWrite,
} from "../../lib/api";
import { withLoading } from "../../lib/withLoading";

const KIND_LABEL: Record<string, string> = {
  entrada: "Entrada",
  saida: "Saída",
  diario: "Diário",
};

/**
 * Write-back (spec 018) — pré-visualiza o caminho inverso (transação → célula da planilha) como um
 * diff para aprovação humana. O ENVIO real fica atrás de uma flag desligada: o botão "Aprovar e
 * enviar" só age quando `write_back_enabled` for true; até lá, mostra o estado bloqueado. A prévia
 * é read-only e segura.
 */
export function WriteBackPreview({
  spreadsheetId,
  sheetName,
  clientId,
}: {
  spreadsheetId: string;
  sheetName: string;
  clientId: string;
}) {
  const [enabled, setEnabled] = useState(false);
  const [cells, setCells] = useState<CellWrite[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applyMsg, setApplyMsg] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    writeBackEnabled()
      .then((v) => alive && setEnabled(v))
      .catch(() => alive && setEnabled(false));
    return () => {
      alive = false;
    };
  }, []);

  async function preview() {
    setError(null);
    setApplyMsg(null);
    await withLoading(setLoading, async () => {
      try {
        const result = await previewWriteBack(spreadsheetId, sheetName, clientId);
        setCells(result);
      } catch (e) {
        setError(String(e));
      }
    });
  }

  async function approve() {
    setApplyMsg(null);
    try {
      await applyWriteBack();
      setApplyMsg("Enviado.");
    } catch (e) {
      // Flag desligada → mensagem clara, nada foi escrito.
      setApplyMsg(String(e));
    }
  }

  const changed = (cells ?? []).filter((c) => c.changed);

  return (
    <section
      style={{
        marginTop: "var(--space-5)",
        paddingTop: "var(--space-4)",
        borderTop: "var(--bw-hair) solid var(--border)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-3)",
          marginBottom: "var(--space-2)",
        }}
      >
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 7,
            fontWeight: "var(--fw-semibold)",
            color: "var(--text)",
            fontSize: "var(--fs-body)",
          }}
        >
          <GitCompareArrows size={15} strokeWidth={1.75} aria-hidden="true" />{" "}
          Write-back para a planilha
        </span>
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 5,
            fontSize: "var(--fs-micro)",
            fontWeight: "var(--fw-bold)",
            textTransform: "uppercase",
            letterSpacing: "0.05em",
            padding: "3px 8px",
            borderRadius: "var(--radius-pill)",
            background: enabled ? "var(--success-tint)" : "var(--bg-subtle)",
            color: enabled ? "var(--success-400)" : "var(--text-muted)",
          }}
        >
          <Lock size={11} strokeWidth={2} aria-hidden="true" />
          {enabled ? "habilitado" : "desligado"}
        </span>
      </div>

      <p
        style={{
          color: "var(--text-muted)",
          fontSize: "var(--fs-sm)",
          margin: "0 0 12px",
        }}
      >
        Pré-visualize o que o app escreveria de volta na planilha (transação → célula).
        O envio real está atrás de uma flag desligada — nada é gravado sem você ligar e
        aprovar.
      </p>

      <Button variant="secondary" disabled={loading} onClick={() => void preview()}>
        {loading ? "Gerando prévia…" : "Gerar prévia do diff"}
      </Button>

      {error && (
        <p
          role="alert"
          style={{
            color: "var(--danger-400)",
            fontSize: "var(--fs-sm)",
            marginTop: 10,
          }}
        >
          {error}
        </p>
      )}

      {cells != null && (
        <div style={{ marginTop: 14 }}>
          {changed.length === 0 ? (
            <p style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
              Nada a enviar — a planilha já reflete suas transações ({cells.length}{" "}
              célula(s) conferida(s)).
            </p>
          ) : (
            <div style={{ display: "grid", gap: 12 }}>
              <p
                style={{
                  color: "var(--text-muted)",
                  fontSize: "var(--fs-sm)",
                  margin: 0,
                }}
              >
                {changed.length} célula(s) divergente(s):
              </p>
              {changed.map((c) => {
                const changes: DiffChange[] = [
                  {
                    field: KIND_LABEL[c.kind] ?? c.kind,
                    before: c.current,
                    after: c.proposed,
                  },
                ];
                return (
                  <ApprovalDiffCard
                    key={c.a1}
                    sheet={sheetName}
                    range={`${c.a1} · ${c.date}`}
                    changes={changes}
                    status="pending"
                  />
                );
              })}
              {/* Uma única aprovação para TODO o lote — `applyWriteBack()` envia tudo de uma vez;
                  um botão por célula passaria a impressão errada de aprovação célula a célula. */}
              <Button
                variant="primary"
                disabled={!enabled}
                onClick={() => void approve()}
              >
                {enabled
                  ? `Aprovar e enviar (${changed.length} célula(s))`
                  : "Envio desligado"}
              </Button>
            </div>
          )}
          {applyMsg && (
            <p
              style={{
                color: "var(--text-muted)",
                fontSize: "var(--fs-sm)",
                marginTop: 10,
              }}
            >
              {applyMsg}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
