import { useCallback, useEffect, useState } from "react";
import { Button } from "../../design-system/components/Button";
import {
  createAccount,
  getPockets,
  isTauri,
  type Pockets,
  type PocketType,
} from "../../lib/api";
import { fmtBRL, parseBRLToCents } from "../../lib/format";
import { invalidateCommands } from "../../lib/useCommand";
import { LIQUIDITY_LABELS, POCKET_TYPE_LABELS } from "./pocketLabels";

/** Settings panel: list pockets and register new ones (spec 007 US2). */
export function PocketsManager() {
  const [pockets, setPockets] = useState<Pockets | null>(null);
  const [name, setName] = useState("");
  const [type, setType] = useState<PocketType>("bank");
  const [balance, setBalance] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(() => {
    if (!isTauri) return;
    getPockets()
      .then(setPockets)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  useEffect(reload, [reload]);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    const cents = parseBRLToCents(balance || "0");
    if (!name.trim()) {
      setError("Informe um nome para o bolso.");
      return;
    }
    if (cents === null) {
      setError("Saldo inválido. Use o formato 1.234,56.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await createAccount(name.trim(), type, cents);
      invalidateCommands(); // projected balance and pockets card must refresh
      setName("");
      setBalance("");
      reload();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="set-panel set-panel--pad">
      {pockets && pockets.accounts.length > 0 && (
        <ul className="pockets-list">
          {pockets.accounts.map((a) => (
            <li key={a.id} className="pockets-list__item">
              <span className="pockets-list__name">{a.name}</span>
              <span className="pockets-list__type">
                {POCKET_TYPE_LABELS[a.type as PocketType] ?? a.type}
                {a.liquidity
                  ? ` · ${LIQUIDITY_LABELS[a.liquidity] ?? a.liquidity}`
                  : ""}
              </span>
              <span className="pockets-list__balance money">{fmtBRL(a.balance)}</span>
            </li>
          ))}
        </ul>
      )}

      <form className="pockets-form" onSubmit={(e) => void onSubmit(e)}>
        <label className="pockets-form__field">
          <span>Nome</span>
          <input
            className="pockets-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Ex.: Vale refeição"
          />
        </label>
        <label className="pockets-form__field">
          <span>Tipo</span>
          <select
            className="gs-select pockets-select"
            value={type}
            onChange={(e) => setType(e.target.value as PocketType)}
          >
            {(Object.keys(POCKET_TYPE_LABELS) as PocketType[]).map((t) => (
              <option key={t} value={t}>
                {POCKET_TYPE_LABELS[t]}
              </option>
            ))}
          </select>
        </label>
        <label className="pockets-form__field">
          <span>Saldo (R$)</span>
          <input
            className="pockets-input"
            value={balance}
            onChange={(e) => setBalance(e.target.value)}
            placeholder="0,00"
            inputMode="decimal"
          />
        </label>
        <Button variant="primary" type="submit" disabled={saving}>
          {saving ? "Salvando…" : "Adicionar bolso"}
        </Button>
      </form>
      {error && (
        <p className="pockets-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
