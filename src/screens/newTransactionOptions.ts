import { useEffect, useState } from "react";
import { getPockets, listTags, type PocketAccount, type Tag } from "../lib/api";

/**
 * Dados de apoio do formulário de lançamento: tags para o seletor e as
 * contas-destino elegíveis da Economia (reserve/illiquid — a mesma fronteira
 * que o backend valida no transfer).
 */
export function useFormOptions() {
  const [tags, setTags] = useState<Tag[]>([]);
  const [reserveAccounts, setReserveAccounts] = useState<PocketAccount[]>([]);
  useEffect(() => {
    let alive = true;
    listTags()
      .then((t) => alive && setTags(t))
      .catch(() => alive && setTags([]));
    getPockets()
      .then((p) => {
        if (!alive) return;
        setReserveAccounts(
          p.accounts.filter(
            (a) => a.liquidity === "reserve" || a.liquidity === "illiquid",
          ),
        );
      })
      .catch(() => alive && setReserveAccounts([]));
    return () => {
      alive = false;
    };
  }, []);
  return { tags, reserveAccounts };
}
