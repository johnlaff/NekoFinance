import { Landmark } from "lucide-react";
import { getPockets } from "../../lib/api";
import { fmtBRL } from "../../lib/format";
import { useCommand } from "../../lib/useCommand";

const GROUPS = [
  { key: "liquid_cents", label: "Caixa", hint: "entra no saldo projetado" },
  { key: "reserve_cents", label: "Reserva", hint: "emergência, fora do caixa" },
  { key: "restricted_cents", label: "Vale", hint: "uso restrito" },
  { key: "illiquid_cents", label: "Ilíquido", hint: "previdência, FGTS" },
] as const;

/** Dashboard card: liquidity-grouped pockets + net worth (spec 007 US3). */
export function PocketsCard() {
  const pocketsQ = useCommand("get_pockets", getPockets);
  const pockets = pocketsQ.data ?? null;

  return (
    <div className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title">
          <Landmark size={16} strokeWidth={1.75} className="dash-card__ic" />
          Bolsos &amp; patrimônio
        </span>
        {pockets && pockets.accounts.length > 0 && (
          <span className="pockets-networth">
            Patrimônio <b>{fmtBRL(pockets.net_worth_cents)}</b>
          </span>
        )}
      </div>
      <div className="dash-card__body">
        {pocketsQ.error ? (
          <p className="pockets-error" role="alert">
            Não foi possível carregar os bolsos: {pocketsQ.error}
          </p>
        ) : !pockets || pockets.accounts.length === 0 ? (
          <p className="pockets-empty">
            Nenhum bolso cadastrado. Adicione conta, poupança, vale, previdência e FGTS
            em Ajustes para o saldo projetado considerar só dinheiro líquido.
          </p>
        ) : (
          <div className="pockets-grid">
            {GROUPS.map((g) => (
              <div key={g.key} className="pockets-cell">
                <span className="pockets-cell__label">{g.label}</span>
                <span className="pockets-cell__value">{fmtBRL(pockets[g.key])}</span>
                <span className="pockets-cell__hint">{g.hint}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
