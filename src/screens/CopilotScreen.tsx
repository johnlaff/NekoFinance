import { Badge } from "../design-system/components/Badge";
import { MiaAvatar } from "../design-system/components/MiaAvatar";

export function CopilotScreen() {
  return (
    <div className="dash">
      <div className="assistant-panel cop-panel">
        <div className="assistant-header">
          <MiaAvatar width={48} height={48} />
          <div>
            <p className="assistant-label">Copiloto</p>
            <h2 className="assistant-name">Mia</h2>
          </div>
          <span className="cop-panel__badge">
            <Badge tone="warning">Em desenvolvimento</Badge>
          </span>
        </div>
        <p>
          O chat da Mia ainda não está disponível nesta versão. Tudo o que você vê no
          app hoje é calculado pelo motor determinístico — nada é gerado por IA.
        </p>
      </div>

      <div className="roadmap-panel">
        <h2>O que a Mia vai fazer</h2>
        <ol>
          <li>
            Diagnóstico em linguagem natural: padrões de gasto, evolução da reserva e o
            peso real do crédito — sempre em modo leitura.
          </li>
          <li>
            Respostas a decisões: “posso comprar?”, “à vista ou parcelado?” — usando o
            saldo projetado, nunca cálculo improvisado.
          </li>
          <li>
            Escrita na planilha somente com a sua aprovação explícita, mostrando um diff
            antes → depois de cada alteração.
          </li>
        </ol>
      </div>
    </div>
  );
}
