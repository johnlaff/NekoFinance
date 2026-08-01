import { useEffect, useState } from "react";
import { getFlagSetting, SHOW_RECEIPT } from "./api";

/**
 * "Conta sempre à mostra" — a preferência de exibição do recibo, válida em todo o app.
 *
 * Ligada, a conta vem aberta; desligada, a superfície imprime o resultado e a aritmética abre
 * sob demanda. A regra que ela obedece é a mesma em qualquer tela: esconde ARITMÉTICA, nunca
 * estado do dado — o selo epistêmico sobrevive ao recolhimento.
 *
 * O default `true` mora aqui, e não em cada tela: duas telas com defaults diferentes para a
 * mesma chave discordariam sobre o que "nunca gravada" significa.
 */
export function useShowReceipt(): boolean {
  const [showReceipt, setShowReceipt] = useState(true);

  useEffect(() => {
    getFlagSetting(SHOW_RECEIPT, true)
      .then(setShowReceipt)
      .catch(() => setShowReceipt(true));
  }, []);

  return showReceipt;
}
