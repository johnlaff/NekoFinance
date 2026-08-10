import { useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
import { Button } from "../../design-system/components/Button";
import { InfoPopover } from "../../design-system/components/InfoPopover";
import { Meter } from "../../design-system/components/Meter";
import { isTauri } from "../../lib/env";
import {
  blockedSpaceExplainer,
  downloadFraction,
  downloadLabel,
  updateStatusCopy,
  updaterMachine,
  type UpdaterMachine,
  type UpdaterState,
} from "./updaterView";

/** Identifica a OFERTA (não só a fase): duas versões distintas em "available"/"ready" são
 *  convites diferentes — dispensar a v1.2.0 não deve silenciar uma v1.3.0 encontrada depois. */
function offerKey(state: UpdaterState): string {
  return "version" in state ? `${state.status}:${state.version}` : state.status;
}

// Card compartilhado pelos estados com algo a dizer — título + (barra opcional) + corpo +
// (didática opcional atrás de pergunta) + (ações opcionais), sempre a mesma casca (regra 15
// do ui-standards: componente em vez de reimplementação dispersa). O papel ARIA muda: erro e
// bloqueio interrompem (alert), o resto é polido (status).
function InviteCard({
  alert = false,
  title,
  body,
  explainer,
  meter,
  actions,
}: {
  alert?: boolean;
  title: string;
  body: string;
  explainer?: ReactNode;
  meter?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <aside
      className="upd-invite"
      role={alert ? "alert" : "status"}
      aria-live={alert ? undefined : "polite"}
    >
      <p className="upd-invite__title">{title}</p>
      {meter}
      <p className="upd-invite__body">
        {body}
        {explainer ? <> {explainer}</> : null}
      </p>
      {actions ? <div className="upd-invite__actions">{actions}</div> : null}
    </aside>
  );
}

/**
 * Convite calmo de atualização — monta uma vez no root do app (como o Compose) e checa em
 * silêncio no launch. Só aparece quando a máquina tem algo a dizer (nunca em ocioso/checando);
 * recusar marca aquela oferta como dispensada para o resto da sessão, sem insistir de novo.
 */
export function UpdateInvitation({
  machine = updaterMachine,
}: {
  machine?: UpdaterMachine;
}) {
  const state = useSyncExternalStore(
    (listener) => machine.subscribe(listener),
    () => machine.getState(),
  );
  const [dismissedOffer, setDismissedOffer] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    void machine.checkForUpdate();
  }, [machine]);

  if (dismissedOffer === offerKey(state)) return null;

  if (state.status === "available") {
    return (
      <InviteCard
        title={updateStatusCopy(state).headline}
        body={`Uma nova versão do Neko Finance está pronta: v${state.version}.`}
        actions={
          <>
            <Button size="sm" onClick={() => void machine.downloadAndInstall()}>
              Baixar e instalar
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setDismissedOffer(offerKey(state))}
            >
              Agora não
            </Button>
          </>
        }
      />
    );
  }

  if (state.status === "blocked-space") {
    const copy = updateStatusCopy(state);
    return (
      <InviteCard
        alert
        title={copy.headline}
        body={copy.detail ?? ""}
        explainer={
          <InfoPopover term={blockedSpaceExplainer} hideMarker>
            <span className="upd-invite__how">Como funciona?</span>
          </InfoPopover>
        }
        actions={
          <>
            <Button size="sm" onClick={() => void machine.checkForUpdate()}>
              Tentar de novo
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setDismissedOffer(offerKey(state))}
            >
              Agora não
            </Button>
          </>
        }
      />
    );
  }

  if (state.status === "downloading") {
    const fraction = downloadFraction(state.progress);
    return (
      <InviteCard
        title={updateStatusCopy(state).headline}
        body={downloadLabel(state.progress)}
        meter={
          fraction != null ? (
            <Meter
              fraction={fraction}
              label={`${Math.round(fraction * 100)}% baixado`}
            />
          ) : null
        }
      />
    );
  }

  if (state.status === "ready") {
    return (
      <InviteCard
        title={updateStatusCopy(state).headline}
        body={`O Neko Finance vai fechar e abrir de novo na v${state.version}.`}
        actions={
          <>
            <Button size="sm" onClick={() => void machine.relaunch()}>
              Reiniciar agora
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setDismissedOffer(offerKey(state))}
            >
              Depois
            </Button>
          </>
        }
      />
    );
  }

  if (state.status === "error") {
    return (
      <InviteCard
        alert
        title={updateStatusCopy(state).headline}
        body={state.message}
        actions={
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setDismissedOffer(offerKey(state))}
          >
            Fechar
          </Button>
        }
      />
    );
  }

  return null;
}
