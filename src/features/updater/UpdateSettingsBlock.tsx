import { useSyncExternalStore } from "react";
import { Button } from "../../design-system/components/Button";
import { Meter } from "../../design-system/components/Meter";
import { Line } from "../../screens/configLine";
import {
  downloadFraction,
  updateStatusCopy,
  updaterMachine,
  type UpdaterMachine,
  type UpdaterState,
} from "./updaterView";

/** Ação alcançável a partir da linha, uma por estado — as mesmas frases do convite calmo
 *  (`UpdateInvitation`), para nunca existir uma segunda invitation para o mesmo ato. */
function rowAction(machine: UpdaterMachine, status: UpdaterState["status"]) {
  switch (status) {
    case "checking":
      return { label: "Verificando…", onClick: null };
    case "downloading":
      return { label: "Baixando…", onClick: null };
    case "available":
      return {
        label: "Baixar e instalar",
        onClick: () => machine.downloadAndInstall(),
      };
    case "ready":
      return { label: "Reiniciar agora", onClick: () => machine.relaunch() };
    case "idle":
    case "error":
      return { label: "Verificar agora", onClick: () => machine.checkForUpdate() };
  }
}

/**
 * Bloco de Configurações (issue #383) — mesma máquina de estados do convite calmo
 * (`UpdateInvitation`), sempre visível: aqui o usuário lê onde está sem esperar o
 * convite e age (checar, baixar, reiniciar) sem depender de um convite ainda de pé.
 */
export function UpdateSettingsBlock({
  machine = updaterMachine,
}: {
  machine?: UpdaterMachine;
}) {
  const state = useSyncExternalStore(
    (listener) => machine.subscribe(listener),
    () => machine.getState(),
  );
  const { headline, detail } = updateStatusCopy(state);
  const fraction =
    state.status === "downloading" ? downloadFraction(state.progress) : null;
  const { label, onClick } = rowAction(machine, state.status);

  return (
    <Line
      title="Verificar atualizações"
      sub={detail ? `${headline} · ${detail}` : headline}
      subExtra={
        fraction != null ? (
          <Meter
            className="config__meter"
            fraction={fraction}
            label={`${Math.round(fraction * 100)}% baixado`}
          />
        ) : null
      }
      right={
        <Button
          size="sm"
          variant="ghost"
          onClick={onClick ? () => void onClick() : undefined}
          disabled={!onClick}
        >
          {label}
        </Button>
      }
    />
  );
}
