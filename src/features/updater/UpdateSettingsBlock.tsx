import { useSyncExternalStore } from "react";
import { Button } from "../../design-system/components/Button";
import { InfoPopover } from "../../design-system/components/InfoPopover";
import { Meter } from "../../design-system/components/Meter";
import { isAndroid } from "../../lib/env";
import { Line } from "../../screens/configLine";
import {
  blockedSpaceExplainer,
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
    // Bloqueado por espaço: a única ação útil é re-checar depois de liberar disco —
    // a mesma checagem completa do launch, que revalida update E espaço.
    case "blocked-space":
      return { label: "Tentar de novo", onClick: () => machine.checkForUpdate() };
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

  // O Android não tem plugin de updater — a distribuição lateral por ADB é quem atualiza:
  // mostrar "ocioso" mentiria uma checagem que nunca roda, e um "Verificar agora" clicável
  // convidaria a uma chamada fadada a falhar.
  if (isAndroid) {
    return (
      <Line
        title="Verificar atualizações"
        sub="Atualização automática não está disponível no Android — instale a versão mais nova pelo ADB."
      />
    );
  }

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
        ) : state.status === "blocked-space" ? (
          <InfoPopover term={blockedSpaceExplainer} hideMarker>
            <span className="config__how">Como funciona?</span>
          </InfoPopover>
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
