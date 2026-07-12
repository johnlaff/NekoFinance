import { useEffect, useState } from "react";
import { Button } from "../../design-system/components/Button";
import {
  createAccount,
  getPockets,
  isTauri,
  type Pockets,
  type PocketType,
} from "../../lib/api";
import { parseBRLToCents } from "../../lib/format";
import { safeErrorMessage } from "../../lib/errors";
import { Money } from "../../design-system/components/Money";
import { invalidateCommands } from "../../lib/useCommand";
import { LIQUIDITY_LABELS, POCKET_TYPE_LABELS } from "./pocketLabels";

interface FormState {
  name: string;
  type: PocketType;
  balance: string;
}

const EMPTY_FORM: FormState = { name: "", type: "bank", balance: "" };

/** Settings panel: list pockets and register new ones. */
export function PocketsManager() {
  const [pockets, setPockets] = useState<Pockets | null>(null);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [status, setStatus] = useState<{ error: string | null; saving: boolean }>({
    error: null,
    saving: false,
  });

  function reload() {
    if (!isTauri) return;
    getPockets()
      .then(setPockets)
      .catch((e: unknown) =>
        setStatus({
          error: safeErrorMessage(e, "Não foi possível carregar os bolsos."),
          saving: false,
        }),
      );
  }

  // Load once on mount; subsequent reloads happen after each successful create.
  useEffect(reload, []);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!isTauri) return;
    const cents = parseBRLToCents(form.balance || "0");
    if (!form.name.trim()) {
      setStatus({ error: "Informe um nome para o bolso.", saving: false });
      return;
    }
    if (cents === null) {
      setStatus({ error: "Saldo inválido. Use o formato 1.234,56.", saving: false });
      return;
    }
    setStatus({ error: null, saving: true });
    try {
      await createAccount(form.name.trim(), form.type, cents);
      invalidateCommands(); // projected balance and pockets card must refresh
      setForm(EMPTY_FORM);
      setStatus({ error: null, saving: false });
      reload();
    } catch (err) {
      setStatus({
        error: safeErrorMessage(err, "Não foi possível adicionar o bolso."),
        saving: false,
      });
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
              <span className="pockets-list__balance">
                <Money cents={a.balance} size="sm" />
              </span>
            </li>
          ))}
        </ul>
      )}

      <form className="pockets-form" onSubmit={(e) => void onSubmit(e)}>
        <label className="pockets-form__field">
          <span>Nome</span>
          <input
            className="pockets-input"
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            placeholder="Ex.: Bolso demo"
            disabled={!isTauri}
          />
        </label>
        <label className="pockets-form__field">
          <span>Tipo</span>
          <select
            className="gs-select pockets-select"
            value={form.type}
            onChange={(e) => setForm({ ...form, type: e.target.value as PocketType })}
            disabled={!isTauri}
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
            value={form.balance}
            onChange={(e) => setForm({ ...form, balance: e.target.value })}
            placeholder="0,00"
            inputMode="decimal"
            disabled={!isTauri}
          />
        </label>
        <Button variant="primary" type="submit" disabled={status.saving || !isTauri}>
          {status.saving ? "Salvando…" : "Adicionar bolso"}
        </Button>
      </form>
      {status.error && (
        <p className="pockets-error" role="alert">
          {status.error}
        </p>
      )}
    </div>
  );
}
