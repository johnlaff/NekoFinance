import { useEffect, useReducer, type CSSProperties } from "react";
import { GitCompareArrows, Lock } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import {
  ApprovalDiffCard,
  type DiffChange,
} from "../../design-system/components/ApprovalDiffCard";
import {
  applyWriteBack,
  previewWriteBack,
  applyEconomiaWriteBack,
  previewEconomiaWriteBack,
  writeBackEnabled,
  type CellWrite,
} from "../../lib/api";
import { safeErrorMessage } from "../../lib/errors";
import { withLoading } from "../../lib/withLoading";

const KIND_LABEL: Record<string, string> = {
  entrada: "Entrada",
  saida: "Saída",
  diario: "Diário",
  economia: "Economia",
};

// Estado do fluxo de prévia/envio agrupado num reducer (uma atualização lógica = um render).
interface WBState {
  enabled: boolean;
  cells: CellWrite[] | null;
  econCells: CellWrite[] | null;
  loading: boolean;
  error: string | null;
  applyMsg: string | null;
  econApplyMsg: string | null;
}

const initialWB: WBState = {
  enabled: false,
  cells: null,
  econCells: null,
  loading: false,
  error: null,
  applyMsg: null,
  econApplyMsg: null,
};

type WBAction =
  | { type: "enabled"; value: boolean }
  | { type: "loading"; value: boolean }
  | { type: "previewReset" }
  | { type: "cells"; value: CellWrite[] | null }
  | { type: "econCells"; value: CellWrite[] | null }
  | { type: "error"; value: string }
  | { type: "applyMsg"; value: string }
  | { type: "econApplyMsg"; value: string };

function wbReducer(s: WBState, a: WBAction): WBState {
  switch (a.type) {
    case "enabled":
      return { ...s, enabled: a.value };
    case "loading":
      return { ...s, loading: a.value };
    case "previewReset":
      return { ...s, error: null, applyMsg: null, econApplyMsg: null };
    case "cells":
      return { ...s, cells: a.value };
    case "econCells":
      return { ...s, econCells: a.value };
    case "error":
      return { ...s, error: a.value };
    case "applyMsg":
      return { ...s, applyMsg: a.value };
    case "econApplyMsg":
      return { ...s, econApplyMsg: a.value };
  }
}

// Base estática do selo de status (não recria por render); o tom (habilitado/desligado) entra por merge.
const STATUS_PILL_BASE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 5,
  fontSize: "var(--fs-micro)",
  fontWeight: "var(--fw-bold)",
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  padding: "3px 8px",
  borderRadius: "var(--radius-pill)",
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
  const [state, dispatch] = useReducer(wbReducer, initialWB);
  const { enabled, cells, econCells, loading, error, applyMsg, econApplyMsg } = state;
  const setLoading = (v: boolean) => dispatch({ type: "loading", value: v });

  // A aba selecionada é uma aba-ano ("2026"). A Economia é uma aba à parte, escrita por ano.
  const year = Number.parseInt(sheetName, 10);
  const yearValid = Number.isInteger(year) && year > 2000;

  useEffect(() => {
    let alive = true;
    writeBackEnabled()
      .then((v) => alive && dispatch({ type: "enabled", value: v }))
      .catch(() => alive && dispatch({ type: "enabled", value: false }));
    return () => {
      alive = false;
    };
  }, []);

  async function preview() {
    dispatch({ type: "previewReset" });
    await withLoading(setLoading, async () => {
      try {
        const result = await previewWriteBack(spreadsheetId, sheetName, clientId);
        dispatch({ type: "cells", value: result });
        // Aba-ano também tem Economia (aba à parte): pré-visualiza o bloco do ano. É OPCIONAL —
        // se não houver aba Economia/dados, não falha a prévia principal da grade diária.
        if (yearValid) {
          try {
            const econ = await previewEconomiaWriteBack(spreadsheetId, year, clientId);
            dispatch({ type: "econCells", value: econ });
          } catch {
            dispatch({ type: "econCells", value: null });
          }
        }
      } catch (e) {
        dispatch({
          type: "error",
          value: safeErrorMessage(e, "Não foi possível pré-visualizar o write-back."),
        });
      }
    });
  }

  async function approve() {
    try {
      const n = await applyWriteBack(spreadsheetId, sheetName, clientId);
      dispatch({ type: "applyMsg", value: `Enviado: ${n} célula(s) atualizada(s).` });
    } catch (e) {
      // Flag desligada → mensagem clara, nada foi escrito.
      dispatch({
        type: "applyMsg",
        value: safeErrorMessage(e, "Write-back bloqueado. Nada foi escrito."),
      });
    }
  }

  async function approveEcon() {
    try {
      const n = await applyEconomiaWriteBack(spreadsheetId, year, clientId);
      dispatch({
        type: "econApplyMsg",
        value: `Enviado: ${n} célula(s) da aba Economia.`,
      });
    } catch (e) {
      dispatch({
        type: "econApplyMsg",
        value: safeErrorMessage(
          e,
          "Write-back da Economia bloqueado. Nada foi escrito.",
        ),
      });
    }
  }

  const changed = (cells ?? []).filter((c) => c.changed);
  const econChanged = (econCells ?? []).filter((c) => c.changed);

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
            ...STATUS_PILL_BASE,
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
        O envio real está atrás de uma flag desligada: nada é gravado sem você ligar e
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
              Nada a enviar: a planilha já reflete suas transações ({cells.length}{" "}
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

      {econCells != null && econChanged.length > 0 && (
        <div
          style={{
            marginTop: 16,
            paddingTop: 12,
            borderTop: "var(--bw-hair) solid var(--border)",
            display: "grid",
            gap: 12,
          }}
        >
          <p
            style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)", margin: 0 }}
          >
            Aba <strong>Economia</strong> ({year}): {econChanged.length} mês(es) a
            atualizar.
          </p>
          {econChanged.map((c) => (
            <ApprovalDiffCard
              key={c.a1}
              sheet="Economia"
              range={`${c.a1} · ${c.date}`}
              changes={[{ field: "Economia", before: c.current, after: c.proposed }]}
              status="pending"
            />
          ))}
          <Button
            variant="primary"
            disabled={!enabled}
            onClick={() => void approveEcon()}
          >
            {enabled
              ? `Aprovar Economia (${econChanged.length} mês(es))`
              : "Envio desligado"}
          </Button>
          {econApplyMsg && (
            <p
              style={{
                color: "var(--text-muted)",
                fontSize: "var(--fs-sm)",
                margin: 0,
              }}
            >
              {econApplyMsg}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
