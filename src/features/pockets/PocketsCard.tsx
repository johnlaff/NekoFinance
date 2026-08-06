import { fetchPockets } from "./pocketsView";
import { Money } from "../../design-system/components/Money";
import { useCommand } from "../../lib/useCommand";

const GROUPS = [
  { key: "liquid_cents", label: "Caixa", hint: "Entra no saldo projetado" },
  { key: "reserve_cents", label: "Reserva", hint: "Emergência, fora do caixa" },
  { key: "restricted_cents", label: "Vale", hint: "Uso restrito" },
  { key: "illiquid_cents", label: "Ilíquido", hint: "Previdência, FGTS" },
] as const;

/** Bolsos por liquidez + patrimônio total. Vive dentro do card Bolsos de
 *  Configurações — a seção é dona do título e do chrome; aqui só o corpo. */
export function PocketsCard() {
  const pocketsQ = useCommand("get_pockets", fetchPockets);
  const pockets = pocketsQ.data ?? null;

  return (
    <div>
      {pocketsQ.error ? (
        <p className="pockets-error" role="alert">
          Não foi possível carregar os bolsos: {pocketsQ.error}
        </p>
      ) : !pockets || pockets.accounts.length === 0 ? (
        <p className="pockets-empty">
          Nenhum bolso cadastrado. Adicione conta, poupança, vale, previdência e FGTS em
          Configurações para o saldo projetado considerar só dinheiro líquido.
        </p>
      ) : (
        <>
          <div className="pockets-grid">
            {GROUPS.map((g) => (
              <div key={g.key} className="pockets-cell">
                <span className="pockets-cell__label">{g.label}</span>
                <span className="pockets-cell__value">
                  <Money cents={pockets[g.key]} size="sm" />
                </span>
                <span className="pockets-cell__hint">{g.hint}</span>
              </div>
            ))}
          </div>
          <div className="pockets-networth">
            Patrimônio total{" "}
            <Money cents={pockets.net_worth_cents} size="sm" sign="auto" />
          </div>
        </>
      )}
    </div>
  );
}
