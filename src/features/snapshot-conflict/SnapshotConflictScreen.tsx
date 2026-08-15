import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Button } from "../../design-system/components/Button";
import { EmptyState } from "../../design-system/components/EmptyState";
import { GOOGLE_OAUTH_CLIENT_ID } from "../../lib/env";
import { errorText, safeErrorMessage } from "../../lib/errors";
import { invalidateCommands } from "../../lib/useCommand";
import { closeSnapshotConflict } from "./snapshotConflictStore";
import {
  CHECKIN_REFUSED_STALE_CONFLICT,
  conflictGestureGroupLabel,
  conflictRemoteDeviceLabel,
  fetchSnapshotConflictDetails,
  groupConsecutiveGestures,
  groupKeys,
  isAfterPoolClosedError,
  resolveConflictErrorMessage,
  resolveSnapshotConflictCmd,
  STALE_CONFLICT_RELOADED_NOTICE,
  type DriveConflictChoice,
  type DriveConflictDetails,
  type DriveConflictGesture,
} from "./snapshotConflictView";

// `<dialog open>` nativo (mesmo padrão de `OnboardingFlow`): a modalidade é visual (scrim +
// bloqueio do resto do shell via `inert` em `App.tsx`), sem `showModal()` (suporte irregular no
// WebView/jsdom). Sem Escape-para-fechar de propósito — esta tela nunca descarta a decisão em
// silêncio (a mesma regra que barra a sobrescrita silenciosa no backend).
const OVERLAY_STYLE: CSSProperties = {
  position: "fixed",
  inset: 0,
  // Acima do dock flutuante do shell mobile (`.sh-dock`, `z-index: 70`, só existe ≤700px): sem
  // isto o dock (Hoje/Lançamentos/…/FAB) desenha POR CIMA do rodapé de ações deste cartão em
  // qualquer viewport estreito — o defeito real por trás da issue #476 no aparelho, uma
  // sobreposição de camadas que nenhum ajuste de `max-height`/rolagem resolve sozinho.
  zIndex: 90,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "var(--space-5)",
  margin: 0,
  border: "none",
  maxWidth: "none",
  maxHeight: "none",
  width: "100%",
  height: "100%",
  background: "var(--bg-scrim)",
  backdropFilter: "blur(8px)",
};

// `dvh`, nunca `vh`: no WebView Android as barras de sistema (endereço/navegação) somem e voltam
// enquanto o dono rola — `vh` mede contra o viewport GRANDE (barras escondidas), então `92vh` pode
// exceder a área realmente visível quando as barras estão à mostra. `dvh` acompanha o viewport
// DINÂMICO atual (issue #476: a lista de gestos empurrou os botões de escolha para fora da tela, e
// como não havia rodapé próprio para as ações, não existia rolagem que os alcançasse).
const CARD_STYLE: CSSProperties = {
  width: "100%",
  maxWidth: 520,
  maxHeight: "92dvh",
  outline: "none",
  background: "var(--surface-elevated)",
  border: "var(--bw-hair) solid var(--border-strong)",
  borderRadius: "var(--radius-xl)",
  boxShadow: "var(--shadow-4)",
  display: "flex",
  flexDirection: "column",
  // Sem isto um filho flex nunca encolhe abaixo do tamanho do seu CONTEÚDO (default
  // `min-height: auto`) — o corpo rolável ignoraria `maxHeight` do cartão e cresceria com a
  // lista, empurrando o rodapé de ações para fora de novo.
  minHeight: 0,
};

// Cabeçalho e rodapé NUNCA rolam — só o corpo entre eles. As ações do rodapé (regra 21 do
// ui-standards, adaptada de "sticky" para "fora da área rolável"): visíveis sempre, nunca atrás de
// uma rolagem que a lista de gestos possa esgotar antes de chegar lá.
const CARD_HEADER_STYLE: CSSProperties = {
  flex: "0 0 auto",
  padding: "var(--space-7) var(--space-7) 0",
};

const CARD_BODY_STYLE: CSSProperties = {
  flex: "1 1 auto",
  minHeight: 0,
  overflowY: "auto",
  display: "grid",
  gap: "var(--space-4)",
  padding: "var(--space-5) var(--space-7)",
};

const CARD_FOOTER_STYLE: CSSProperties = {
  flex: "0 0 auto",
  display: "grid",
  gap: "var(--space-3)",
  padding: "var(--space-5) var(--space-7) var(--space-7)",
  borderTop: "var(--bw-hair) solid var(--border-strong)",
};

// Contenção própria por lista (issue #476): sem ela, uma corrida longa de gestos consome sozinha
// todo o corpo rolável do cartão, empurrando a segunda lista (e a copy entre elas) para fora da
// vista antes mesmo de chegar ao rodapé. `dvh` pela mesma razão do cartão — nunca `vh`.
const LIST_STYLE: CSSProperties = {
  display: "grid",
  gap: "var(--space-2)",
  margin: 0,
  padding: 0,
  listStyle: "none",
  maxHeight: "min(288px, 34dvh)",
  overflowY: "auto",
};

const ITEM_STYLE: CSSProperties = {
  padding: "var(--space-3)",
  borderRadius: "var(--radius-md)",
  background: "var(--surface-2)",
  fontSize: "var(--fs-sm)",
  color: "var(--text-strong)",
};

const GESTURE_LIST_HEADING_STYLE: CSSProperties = {
  margin: 0,
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  color: "var(--text-strong)",
};

const GESTURE_LIST_HINT_STYLE: CSSProperties = {
  fontWeight: "var(--fw-regular)",
  color: "var(--text-muted)",
};

/** Uma das duas listas simétricas da tela: o que se perde num dos dois sentidos da escolha. Vazia
 *  é um estado honesto, não um erro — `EmptyState` é para carregamento/falha (regra 16 do
 *  ui-standards), não para "não há nada aqui de verdade". */
function GestureList({
  title,
  hint,
  gestures,
  emptyText,
}: {
  title: string;
  hint: string;
  gestures: DriveConflictGesture[];
  emptyText: string;
}) {
  // Corridas consecutivas do mesmo tipo de gesto colapsam numa linha com contagem — sem isto, um
  // import automático recorrente da mesma aba (o caso real da issue #476, ~18 vezes) repete a
  // MESMA frase uma vez por gesto, sem levar informação nova a cada linha.
  const groups = groupConsecutiveGestures(gestures);
  const keys = groupKeys(groups);
  return (
    <section aria-label={title} style={{ display: "grid", gap: "var(--space-2)" }}>
      <h3 style={GESTURE_LIST_HEADING_STYLE}>
        {title} <span style={GESTURE_LIST_HINT_STYLE}>— {hint}</span>
      </h3>
      {gestures.length === 0 ? (
        <p style={{ margin: 0, fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>
          {emptyText}
        </p>
      ) : (
        <ul style={LIST_STYLE}>
          {groups.map((group, i) => (
            <li key={keys[i]} style={ITEM_STYLE}>
              {conflictGestureGroupLabel(group)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

type Phase =
  | { kind: "loading" }
  | { kind: "load-error"; message: string }
  // `staleNotice` só existe quando este `ready` veio de um recarregamento automático depois de
  // `CHECKIN_REFUSED_STALE_CONFLICT` (nunca do mount inicial) — a tela precisa dizer ao dono que
  // o clique dele causou uma atualização, não nada.
  | { kind: "ready"; details: DriveConflictDetails; staleNotice?: string }
  | { kind: "resolving"; details: DriveConflictDetails; choice: DriveConflictChoice }
  | { kind: "resolve-error"; details: DriveConflictDetails; message: string }
  | { kind: "restart-required" }
  // O pool do banco ativo já foi fechado no backend (`resolve_conflict_use_remote_core` passou do
  // ponto de não-retorno) antes de a resolução falhar — nenhuma nova tentativa é possível a partir
  // daqui, então a tela nunca reoferece os botões de escolha, só a saída de reiniciar.
  | { kind: "restart-required-error"; message: string };

/**
 * Tela de conflito do snapshot no Drive (ADR-0015): os dois aparelhos avançaram a
 * partir da mesma base — o dono escolhe o vencedor vendo antes o que se perde em CADA sentido da
 * escolha (as duas listas de gestos, deste aparelho e do outro). Estado do shell (ADR-0006/0008),
 * montada CONDICIONALMENTE ao lado de
 * `<AppShell>` em `App.tsx` (só enquanto `snapshotConflictOpenSnapshot()` for verdadeiro) — nunca
 * um membro de `Screen`, o mesmo padrão de `OnboardingFlow`. Montar/desmontar em vez de um `open`
 * interno é o que dá o fetch inicial de graça (estado FRESCO a cada abertura) sem precisar
 * resetar a fase num `setState` síncrono dentro do efeito.
 */
export function SnapshotConflictScreen() {
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const cardRef = useRef<HTMLDivElement>(null);
  // Compartilhado pelas DUAS chamadas de `loadDetails` (mount E recuperação de consentimento
  // obsoleto) — um guard por closure sempre-`true` na segunda chamada seria assimétrico do mount
  // e convidaria um bug silencioso se um caminho de fechamento novo surgisse ali (issue #446 item
  // 9a). `mountedRef.current = true` dentro do efeito (não só a inicialização do `useRef`) cobre
  // o mount→cleanup→mount duplo do StrictMode em dev.
  const mountedRef = useRef(true);

  // Extraída de propósito: o mount inicial E a recuperação de um consentimento obsoleto (veredito
  // `CHECKIN_REFUSED_STALE_CONFLICT` abaixo) precisam do MESMO fetch fresco — o dono nunca decide
  // em cima do manifest velho que a tela mostrou antes. Nunca chama `setPhase` de forma síncrona
  // aqui dentro (regra `react-hooks/set-state-in-effect`): o mount já parte de `{ kind: "loading" }`
  // pelo estado inicial do `useState`, e o chamador de recuperação (dentro de `resolve`, um
  // manipulador de evento, não um efeito) seta a fase ANTES de invocar isto.
  function loadDetails(staleNotice?: string) {
    fetchSnapshotConflictDetails(GOOGLE_OAUTH_CLIENT_ID)
      .then((details) => {
        // `exactOptionalPropertyTypes` distingue "propriedade ausente" de "propriedade
        // presente com valor `undefined`" — o spread condicional constrói o objeto sem a chave
        // quando não há nota, em vez de atribuir `staleNotice: undefined` explicitamente.
        if (mountedRef.current) {
          setPhase({ kind: "ready", details, ...(staleNotice ? { staleNotice } : {}) });
        }
      })
      .catch((e: unknown) => {
        if (mountedRef.current)
          setPhase({ kind: "load-error", message: safeErrorMessage(e) });
      });
  }

  useEffect(() => {
    mountedRef.current = true;
    loadDetails();
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    cardRef.current?.focus();
  }, []);

  async function resolve(details: DriveConflictDetails, choice: DriveConflictChoice) {
    setPhase({ kind: "resolving", details, choice });
    try {
      const result = await resolveSnapshotConflictCmd(
        GOOGLE_OAUTH_CLIENT_ID,
        choice,
        details.remote_manifest.sequence,
      );
      if (result.requires_restart) {
        // `use_remote` fecha o pool do banco ativo para trocar o arquivo debaixo dele — nenhum
        // outro comando volta a funcionar até o reinício. A tela trava aqui de propósito, nunca
        // finge que o app segue operável (ver `resolve_conflict_use_remote_core`, Rust).
        setPhase({ kind: "restart-required" });
      } else {
        invalidateCommands();
        closeSnapshotConflict();
      }
    } catch (e) {
      if (errorText(e) === CHECKIN_REFUSED_STALE_CONFLICT) {
        // Consentimento obsoleto (ADR-0015): o outro aparelho publicou de novo entre esta tela
        // abrir e o clique — nunca publica/restaura por cima do que o dono nunca viu. Recarrega
        // os detalhes em vez de mostrar um erro parado, com uma nota visível: sem ela, o dono
        // clica, vê o spinner e a tela volta com listas diferentes sem uma palavra.
        setPhase({ kind: "loading" });
        loadDetails(STALE_CONFLICT_RELOADED_NOTICE);
        return;
      }
      if (isAfterPoolClosedError(e)) {
        // O pool já foi fechado no backend para a troca de arquivo antes de falhar — não há mais
        // pool para uma nova tentativa. Trava em "reinicie", nunca reoferece os botões de escolha.
        setPhase({
          kind: "restart-required-error",
          message:
            "A troca para o snapshot do outro aparelho parou no meio do caminho, com o banco " +
            "já fechado para a troca — não há como tentar de novo nesta tela. Feche e abra o " +
            "Neko Finance de novo.",
        });
        return;
      }
      // Verbatim só atrás do prefixo de contrato conhecido (schema mais nova, consentimento
      // obsoleto de novo tipo); qualquer outro erro cai no fallback calmo — nunca uma mensagem
      // técnica crua na tela.
      setPhase({
        kind: "resolve-error",
        details,
        message: resolveConflictErrorMessage(e),
      });
    }
  }

  const details =
    phase.kind === "ready" ||
    phase.kind === "resolving" ||
    phase.kind === "resolve-error"
      ? phase.details
      : null;
  const resolving = phase.kind === "resolving" ? phase.choice : null;
  const resolveError = phase.kind === "resolve-error" ? phase.message : null;
  const staleNotice = phase.kind === "ready" ? phase.staleNotice : undefined;

  return (
    <dialog
      open
      aria-modal="true"
      aria-label="Conflito de sincronização entre aparelhos"
      style={OVERLAY_STYLE}
    >
      <div ref={cardRef} tabIndex={-1} style={CARD_STYLE}>
        <div style={CARD_HEADER_STYLE}>
          <h2
            style={{
              margin: 0,
              fontSize: "var(--fs-h3)",
              fontWeight: "var(--fw-bold)",
              color: "var(--text-strong)",
            }}
          >
            Os dois aparelhos mudaram desde a última vez que se falaram
          </h2>
        </div>

        <div style={CARD_BODY_STYLE}>
          {phase.kind === "loading" && (
            <EmptyState
              variant="loading"
              description="Buscando o que mudou em cada lado…"
            />
          )}

          {phase.kind === "load-error" && (
            <EmptyState
              variant="error"
              title="Não foi possível carregar o conflito"
              description={phase.message}
              action={
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => closeSnapshotConflict()}
                >
                  Fechar
                </Button>
              }
            />
          )}

          {phase.kind === "restart-required" && (
            <EmptyState
              title="Feche e abra o Neko Finance de novo"
              description="Os dados do outro aparelho já estão prontos neste — o app precisa reiniciar para
                voltar a funcionar com eles."
            />
          )}

          {phase.kind === "restart-required-error" && (
            <EmptyState
              variant="error"
              title="Reinicie o Neko Finance"
              description={phase.message}
            />
          )}

          {details && (
            <>
              {staleNotice && (
                <p
                  role="status"
                  style={{
                    margin: 0,
                    fontSize: "var(--fs-sm)",
                    color: "var(--text-muted)",
                  }}
                >
                  {staleNotice}
                </p>
              )}

              <p
                role="status"
                style={{ margin: 0, fontSize: "var(--fs-body)", color: "var(--text)" }}
              >
                Isto é{" "}
                {conflictRemoteDeviceLabel(
                  details.remote_manifest,
                  details.this_device_id,
                )}
                . As listas abaixo cobrem só importações e escritas na planilha — split,
                tag, reembolso, fatura, teto e cenário ainda não ficam registrados aqui.
                Os horários do lado do outro aparelho vêm do relógio dele, não deste.
              </p>

              <GestureList
                title="Gestos deste aparelho"
                hint="perdidos se você usar o outro aparelho"
                gestures={details.local_gestures}
                emptyText="Não há registro de importação ou escrita na planilha neste aparelho desde a última base em comum."
              />
              <GestureList
                title="Gestos do outro aparelho"
                hint="perdidos se você mantiver este aparelho"
                gestures={details.remote_gestures}
                emptyText="Não há registro de importação ou escrita na planilha no outro aparelho desde a última base em comum."
              />

              {resolveError && (
                <p role="alert" style={{ margin: 0, color: "var(--danger-400)" }}>
                  {resolveError}
                </p>
              )}
            </>
          )}
        </div>

        {details && (
          <div style={CARD_FOOTER_STYLE}>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
                justifyContent: "space-between",
              }}
            >
              <p
                style={{
                  margin: 0,
                  fontSize: "var(--fs-sm)",
                  color: "var(--text-muted)",
                }}
              >
                Decidir depois fecha esta tela sem escolher — o conflito volta a
                aparecer no próximo check-in.
              </p>
              <Button
                variant="ghost"
                disabled={resolving !== null}
                onClick={() => closeSnapshotConflict()}
              >
                Decidir depois
              </Button>
            </div>

            <div
              style={{
                display: "flex",
                gap: "var(--space-3)",
                justifyContent: "flex-end",
              }}
            >
              <Button
                variant="ghost"
                disabled={resolving !== null}
                onClick={() => void resolve(details, "use_remote")}
              >
                {resolving === "use_remote" ? "Usando…" : "Usar o outro aparelho"}
              </Button>
              <Button
                variant="primary"
                disabled={resolving !== null}
                onClick={() => void resolve(details, "keep_local")}
              >
                {resolving === "keep_local" ? "Publicando…" : "Manter este aparelho"}
              </Button>
            </div>
          </div>
        )}
      </div>
    </dialog>
  );
}
