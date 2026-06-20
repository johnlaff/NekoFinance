import { useEffect, useReducer, useRef, type CSSProperties } from "react";
import { GitCompareArrows, Lock, AlertTriangle } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import { ApprovalDiffCard } from "../../design-system/components/ApprovalDiffCard";
import {
  applyWriteBack,
  previewWriteBackStatus,
  applyEconomiaWriteBack,
  previewEconomiaWriteBackStatus,
  writeBackEnabled,
  getImportConflicts,
  type CellWrite,
} from "../../lib/api";
import { safeErrorMessage } from "../../lib/errors";
import { formatBRL } from "../../lib/format";
import { withLoading } from "../../lib/withLoading";

const KIND_LABEL: Record<string, string> = {
  entrada: "Entrada",
  saida: "Saída",
  diario: "Diário",
  economia: "Economia",
};

// Qual seção tem o diálogo de 2ª confirmação aberto (uma de cada vez). `null` = nenhum.
type ConfirmTarget = null | "grid" | "econ";

// O backend devolve o erro de re-revisão (Step 4) como uma string PT-BR. Casamos por trecho estável
// ("planilha mudou") em vez do literal inteiro, para não acoplar à pontuação exata da mensagem.
const SHEET_CHANGED_RE = /planilha mudou/i;
const SHEET_CHANGED_MSG =
  "A planilha mudou — gere o preview de novo e revise antes de enviar.";

// Estado do fluxo de prévia/envio agrupado num reducer (uma atualização lógica = um render).
interface WBState {
  enabled: boolean;
  cells: CellWrite[] | null;
  econCells: CellWrite[] | null;
  /** `modifiedTime` do Drive na prévia (token de frescura levado ao apply). */
  previewRevision: string | null;
  /** Carimbo de QUANDO a prévia foi gerada (mostrado ao usuário; reforça "isto pode envelhecer"). */
  previewedAt: number | null;
  /** Há conflitos de import pendentes? Desabilita o envio (espelha o gate do backend, Step 3). */
  conflictsPending: boolean;
  /** Mais de um cartão com ciclo (ou cartão sem ciclo): a data da fatura pode divergir (Step 8). */
  multiCardWarning: boolean;
  /** Prévia em andamento (read-only). */
  loading: boolean;
  /** Envio em andamento (guard anti-duplo-clique no Aprovar). */
  applying: boolean;
  /** Diálogo de 2ª confirmação aberto para qual seção. */
  confirm: ConfirmTarget;
  error: string | null;
  applyMsg: string | null;
  econApplyMsg: string | null;
}

const initialWB: WBState = {
  enabled: false,
  cells: null,
  econCells: null,
  previewRevision: null,
  previewedAt: null,
  conflictsPending: false,
  multiCardWarning: false,
  loading: false,
  applying: false,
  confirm: null,
  error: null,
  applyMsg: null,
  econApplyMsg: null,
};

type WBAction =
  | { type: "enabled"; value: boolean }
  | { type: "loading"; value: boolean }
  | { type: "applying"; value: boolean }
  | { type: "previewReset" }
  | {
      type: "previewOk";
      cells: CellWrite[];
      previewRevision: string;
      conflictsPending: boolean;
      multiCardWarning: boolean;
    }
  | { type: "econCells"; value: CellWrite[] | null }
  | { type: "confirm"; value: ConfirmTarget }
  | { type: "error"; value: string }
  | { type: "applyMsg"; value: string }
  | { type: "econApplyMsg"; value: string };

function wbReducer(s: WBState, a: WBAction): WBState {
  switch (a.type) {
    case "enabled":
      return { ...s, enabled: a.value };
    case "loading":
      return { ...s, loading: a.value };
    case "applying":
      return { ...s, applying: a.value };
    case "previewReset":
      return { ...s, error: null, applyMsg: null, econApplyMsg: null, confirm: null };
    case "previewOk":
      return {
        ...s,
        cells: a.cells,
        previewRevision: a.previewRevision,
        previewedAt: Date.now(),
        conflictsPending: a.conflictsPending,
        multiCardWarning: a.multiCardWarning,
      };
    case "econCells":
      return { ...s, econCells: a.value };
    case "confirm":
      return { ...s, confirm: a.value };
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

// Estilos estáticos hoisted (React Compiler: nunca recriar objeto de estilo por render).
const HINT_TEXT: CSSProperties = {
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  marginTop: 10,
};
const WARN_BANNER: CSSProperties = {
  display: "flex",
  alignItems: "flex-start",
  gap: 7,
  color: "var(--brass-400)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border)",
  borderRadius: "var(--radius-sm)",
  padding: "8px 10px",
  fontSize: "var(--fs-sm)",
  marginTop: 12,
};
// `<dialog>` nativo: focus-trap, Escape-para-fechar e backdrop de graça (a11y do modal de confirmação
// de uma ação que ESCREVE na planilha — vale o cuidado). Estilo na própria caixa, não num overlay.
const CONFIRM_CARD: CSSProperties = {
  background: "var(--bg-elevated, var(--bg))",
  color: "var(--text)",
  border: "var(--bw-hair) solid var(--border)",
  borderRadius: "var(--radius-md)",
  padding: "var(--space-4)",
  maxWidth: 380,
  width: "100%",
  margin: "auto",
  display: "grid",
  gap: "var(--space-3)",
  boxShadow: "var(--shadow-lg, 0 10px 40px rgba(0,0,0,0.4))",
};
const CONFIRM_ACTIONS: CSSProperties = {
  display: "flex",
  gap: "var(--space-2)",
  justifyContent: "flex-end",
};

/**
 * Diálogo de 2ª confirmação antes da escrita real: um clique acidental no Aprovar não escreve.
 * Usa o `<dialog>` nativo via `showModal()` — focus-trap, `Escape` (evento `cancel` → `onCancel`) e
 * backdrop sem código próprio. Re-renderiza/fecha conforme o `count` (a seção alvo já decide quando
 * montar este componente).
 */
function ConfirmDialog({
  count,
  scope,
  onConfirm,
  onCancel,
}: {
  count: number;
  scope: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || el.open) return;
    // `showModal()` dá focus-trap + Escape + backdrop. Em ambientes sem ele (jsdom dos testes), cai
    // para o atributo `open` — o diálogo continua visível e acessível, só sem o focus-trap nativo.
    if (typeof el.showModal === "function") el.showModal();
    else el.setAttribute("open", "");
  }, []);
  return (
    <dialog
      ref={ref}
      style={CONFIRM_CARD}
      aria-labelledby="wb-confirm-title"
      onCancel={(e) => {
        // `Escape` dispara `cancel`; tratamos como Cancelar (sem escrever) e fechamos via estado.
        e.preventDefault();
        onCancel();
      }}
    >
      <strong id="wb-confirm-title" style={{ color: "var(--text)" }}>
        Enviar {count} célula(s) para a planilha?
      </strong>
      <p style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)", margin: 0 }}>
        {scope} O envio grava direto na sua planilha. Confira o diff antes de confirmar.
      </p>
      <div style={CONFIRM_ACTIONS}>
        <Button variant="ghost" onClick={onCancel}>
          Cancelar
        </Button>
        <Button variant="primary" onClick={onConfirm}>
          Confirmar envio
        </Button>
      </div>
    </dialog>
  );
}

const ECON_SECTION: CSSProperties = {
  marginTop: 16,
  paddingTop: 12,
  borderTop: "var(--bw-hair) solid var(--border)",
  display: "grid",
  gap: 12,
};
const MUTED_SM: CSSProperties = {
  color: "var(--text-muted)",
  fontSize: "var(--fs-sm)",
  margin: 0,
};

/** Seção do diff da grade diária (aba-ano): banners de pré-condição, cards e o botão de aprovação. */
function GridDiffSection({
  cells,
  changed,
  sheetName,
  conflictsPending,
  multiCardWarning,
  previewedAt,
  applyMsg,
  sendBlocked,
  sendLabel,
  onApprove,
}: {
  cells: CellWrite[];
  changed: CellWrite[];
  sheetName: string;
  conflictsPending: boolean;
  multiCardWarning: boolean;
  previewedAt: number | null;
  applyMsg: string | null;
  sendBlocked: boolean;
  sendLabel: string | null;
  onApprove: () => void;
}) {
  return (
    <div style={{ marginTop: 14 }}>
      {conflictsPending && (
        <div role="alert" style={WARN_BANNER}>
          <AlertTriangle size={14} strokeWidth={2} aria-hidden="true" />
          <span>
            Há conflitos de importação pendentes. Resolva-os em Conciliação antes de
            enviar — o app não escreve por cima de um valor em conferência.
          </span>
        </div>
      )}
      {multiCardWarning && (
        <output style={WARN_BANNER}>
          <AlertTriangle size={14} strokeWidth={2} aria-hidden="true" />
          <span>
            Mais de um cartão com ciclo (ou cartão sem ciclo): confira a data da fatura
            antes de enviar.
          </span>
        </output>
      )}
      {changed.length === 0 ? (
        <p style={MUTED_SM}>
          Nada a enviar: a planilha já reflete suas transações ({cells.length} célula(s)
          conferida(s)).
        </p>
      ) : (
        <div style={{ display: "grid", gap: 12 }}>
          <p style={MUTED_SM}>{changed.length} célula(s) divergente(s):</p>
          {changed.map((c) => (
            <ApprovalDiffCard
              key={c.a1}
              sheet={sheetName}
              range={`${c.a1} · ${c.date}`}
              changes={[
                {
                  field: KIND_LABEL[c.kind] ?? c.kind,
                  before: c.current,
                  // "O que será escrito": valor exato em R$ (dos centavos), sem ambiguidade.
                  after: formatBRL(c.value_cents),
                },
              ]}
              status="pending"
            />
          ))}
          {/* Uma única aprovação para TODO o lote — `applyWriteBack()` envia tudo de uma vez;
              um botão por célula passaria a impressão errada de aprovação célula a célula. */}
          <Button variant="primary" disabled={sendBlocked} onClick={onApprove}>
            {sendLabel ?? `Aprovar e enviar (${changed.length} célula(s))`}
          </Button>
        </div>
      )}
      {previewedAt != null && changed.length > 0 && (
        <p style={HINT_TEXT}>
          Prévia gerada {new Date(previewedAt).toLocaleTimeString("pt-BR")}. Se a
          planilha mudar, o envio é bloqueado e a prévia é refeita.
        </p>
      )}
      {applyMsg && (
        <output aria-live="polite" style={HINT_TEXT}>
          {applyMsg}
        </output>
      )}
    </div>
  );
}

/** Seção do diff da aba Economia (poupança por mês): cards do ano + botão de aprovação. */
function EconDiffSection({
  year,
  econChanged,
  econApplyMsg,
  sendBlocked,
  sendLabel,
  onApprove,
}: {
  year: number;
  econChanged: CellWrite[];
  econApplyMsg: string | null;
  sendBlocked: boolean;
  sendLabel: string | null;
  onApprove: () => void;
}) {
  return (
    <div style={ECON_SECTION}>
      <p style={MUTED_SM}>
        Aba <strong>Economia</strong> ({year}): {econChanged.length} mês(es) a
        atualizar.
      </p>
      {econChanged.map((c) => (
        <ApprovalDiffCard
          key={c.a1}
          sheet="Economia"
          range={`${c.a1} · ${c.date}`}
          changes={[
            { field: "Economia", before: c.current, after: formatBRL(c.value_cents) },
          ]}
          status="pending"
        />
      ))}
      <Button variant="primary" disabled={sendBlocked} onClick={onApprove}>
        {sendLabel ?? `Aprovar Economia (${econChanged.length} mês(es))`}
      </Button>
      {econApplyMsg && (
        <output aria-live="polite" style={{ ...HINT_TEXT, marginTop: 0 }}>
          {econApplyMsg}
        </output>
      )}
    </div>
  );
}

/**
 * Write-back (spec 018) — pré-visualiza o caminho inverso (transação → célula da planilha) como um
 * diff para aprovação humana, e (com a flag ligada) envia após uma 2ª confirmação. Salvaguardas:
 * a aprovação fica BLOQUEADA enquanto há conflitos de import pendentes; o envio carrega o
 * `preview_revision` da prévia e o backend ABORTA se a planilha mudou (re-revisão); um diálogo de
 * confirmação evita escrita por clique acidental; após o envio a prévia é refeita (não dá para
 * reenviar o mesmo lote). A prévia em si é read-only e segura.
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
  const {
    enabled,
    cells,
    econCells,
    previewRevision,
    previewedAt,
    conflictsPending,
    multiCardWarning,
    loading,
    applying,
    confirm,
    error,
    applyMsg,
    econApplyMsg,
  } = state;
  const setLoading = (v: boolean) => dispatch({ type: "loading", value: v });
  const setApplying = (v: boolean) => dispatch({ type: "applying", value: v });

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

  // `keepMessages` preserva a mensagem de envio/re-revisão quando a prévia é REFEITA logo após um
  // apply (pós-envio / planilha-mudou): o usuário precisa ver o resultado, não vê-lo ser apagado.
  async function preview(keepMessages = false) {
    if (!keepMessages) dispatch({ type: "previewReset" });
    await withLoading(setLoading, async () => {
      try {
        const result = await previewWriteBackStatus(spreadsheetId, sheetName, clientId);
        // Precondição de conflito (Step 5): a prévia já traz o flag do backend, mas re-checamos a
        // fila ao vivo — um conflito pode ter surgido entre prévias. `||` é defensivo.
        let conflictsPending = result.conflicts_pending;
        if (!conflictsPending) {
          try {
            conflictsPending = (await getImportConflicts()).length > 0;
          } catch {
            // Falha ao ler a fila não deve mascarar a prévia; o gate do backend ainda protege o envio.
          }
        }
        dispatch({
          type: "previewOk",
          cells: result.cells,
          previewRevision: result.preview_revision,
          conflictsPending,
          multiCardWarning: result.multi_card_warning,
        });
        // Aba-ano também tem Economia (aba à parte): pré-visualiza o bloco do ano. É OPCIONAL —
        // se não houver aba Economia/dados, não falha a prévia principal da grade diária.
        if (yearValid) {
          try {
            const econ = await previewEconomiaWriteBackStatus(
              spreadsheetId,
              year,
              clientId,
            );
            dispatch({ type: "econCells", value: econ.cells });
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

  // Trata o erro de re-revisão (Step 4): mensagem clara + re-prévia automática (a UI passa a refletir
  // o novo estado da planilha). Retorna `true` se ERA esse erro (já tratado).
  function handleSheetChanged(e: unknown): boolean {
    const raw = e instanceof Error ? e.message : typeof e === "string" ? e : "";
    if (SHEET_CHANGED_RE.test(raw)) {
      dispatch({ type: "applyMsg", value: SHEET_CHANGED_MSG });
      void preview(true);
      return true;
    }
    return false;
  }

  async function approve() {
    dispatch({ type: "confirm", value: null });
    await withLoading(setApplying, async () => {
      try {
        const n = await applyWriteBack(
          spreadsheetId,
          sheetName,
          clientId,
          previewRevision,
        );
        dispatch({ type: "applyMsg", value: `Enviado: ${n} célula(s) atualizada(s).` });
        // Pós-envio: refaz a prévia para refletir o novo estado da planilha — um 2º envio idêntico
        // não dispara (o diff fica vazio). Não reusa o `previewRevision` antigo. `keepMessages` para
        // a mensagem de sucesso sobreviver à re-prévia.
        void preview(true);
      } catch (e) {
        if (handleSheetChanged(e)) return;
        dispatch({
          type: "applyMsg",
          value: safeErrorMessage(e, "Write-back bloqueado. Nada foi escrito."),
        });
      }
    });
  }

  async function approveEcon() {
    dispatch({ type: "confirm", value: null });
    await withLoading(setApplying, async () => {
      try {
        const n = await applyEconomiaWriteBack(
          spreadsheetId,
          year,
          clientId,
          previewRevision,
        );
        dispatch({
          type: "econApplyMsg",
          value: `Enviado: ${n} célula(s) da aba Economia.`,
        });
        void preview(true);
      } catch (e) {
        if (handleSheetChanged(e)) {
          dispatch({ type: "econApplyMsg", value: SHEET_CHANGED_MSG });
          return;
        }
        dispatch({
          type: "econApplyMsg",
          value: safeErrorMessage(
            e,
            "Write-back da Economia bloqueado. Nada foi escrito.",
          ),
        });
      }
    });
  }

  const changed = (cells ?? []).filter((c) => c.changed);
  const econChanged = (econCells ?? []).filter((c) => c.changed);
  // Aprovar fica BLOQUEADO se: flag desligada, conflito pendente, ou um envio já em andamento.
  const sendBlocked = !enabled || conflictsPending || applying;
  const sendLabel = applying
    ? "Enviando…"
    : !enabled
      ? "Envio desligado"
      : conflictsPending
        ? "Resolva os conflitos primeiro"
        : null;

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
        Nada é gravado sem você aprovar e confirmar.
      </p>

      <Button
        variant="secondary"
        disabled={loading || applying}
        onClick={() => void preview()}
      >
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
        <GridDiffSection
          cells={cells}
          changed={changed}
          sheetName={sheetName}
          conflictsPending={conflictsPending}
          multiCardWarning={multiCardWarning}
          previewedAt={previewedAt}
          applyMsg={applyMsg}
          sendBlocked={sendBlocked}
          sendLabel={sendLabel}
          onApprove={() => dispatch({ type: "confirm", value: "grid" })}
        />
      )}

      {econCells != null && econChanged.length > 0 && (
        <EconDiffSection
          year={year}
          econChanged={econChanged}
          econApplyMsg={econApplyMsg}
          sendBlocked={sendBlocked}
          sendLabel={sendLabel}
          onApprove={() => dispatch({ type: "confirm", value: "econ" })}
        />
      )}

      {confirm === "grid" && (
        <ConfirmDialog
          count={changed.length}
          scope={`Serão atualizadas ${changed.length} célula(s) da aba ${sheetName}.`}
          onConfirm={() => void approve()}
          onCancel={() => dispatch({ type: "confirm", value: null })}
        />
      )}
      {confirm === "econ" && (
        <ConfirmDialog
          count={econChanged.length}
          scope={`Serão atualizados ${econChanged.length} mês(es) da aba Economia.`}
          onConfirm={() => void approveEcon()}
          onCancel={() => dispatch({ type: "confirm", value: null })}
        />
      )}
    </section>
  );
}
