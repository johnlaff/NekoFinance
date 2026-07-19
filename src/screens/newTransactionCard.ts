import { useEffect, useState } from "react";
import {
  createCardSeries,
  createRefundExpectation,
  listCards,
  listInvoices,
  registerCardPurchase,
  type Card,
} from "../lib/api";
import { parseBRLToCents } from "../lib/format";

const LAST_CARD_KEY = "neko:lastCardId";

/**
 * Cartões disponíveis para o registro de compra: um cartão só é pré-selecionado
 * quando a escolha é inequívoca (cartão único) ou já foi feita antes (último usado).
 */
export function useCardOptions(onPreselect: (cardId: string) => void) {
  const [cards, setCards] = useState<Card[]>([]);
  useEffect(() => {
    let alive = true;
    listCards()
      .then((items) => {
        if (!alive) return;
        setCards(items);
        if (items.length === 1) {
          onPreselect(items[0]?.id ?? "");
          return;
        }
        const last = window.localStorage.getItem(LAST_CARD_KEY);
        if (last && items.some((card) => card.id === last)) {
          onPreselect(last);
        }
      })
      .catch(() => alive && setCards([]));
    return () => {
      alive = false;
    };
    // A pré-seleção só faz sentido na montagem — depois, a escolha é do usuário.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return cards;
}

export interface CardSubmitInput {
  cardId: string;
  cardRepeat: "never" | "subscription" | "installments";
  installments: number;
  refundAmount: string;
  amountCents: number;
  description: string;
  date: string;
  tagIds: string[];
}

/**
 * Caminho de escrita do tipo cartão: compra avulsa soma na fatura do ciclo;
 * assinatura/parcelado viram série; o reembolso esperado pré-lança a Entrada
 * vinculada na fatura aberta.
 */
export async function submitCardPurchase(input: CardSubmitInput): Promise<void> {
  window.localStorage.setItem(LAST_CARD_KEY, input.cardId);
  if (input.cardRepeat === "never") {
    await registerCardPurchase({
      cardAccountId: input.cardId,
      amountCents: input.amountCents,
      description: input.description.trim() || null,
      date: input.date,
      tagIds: input.tagIds,
    });
  } else {
    await createCardSeries({
      cardAccountId: input.cardId,
      description: input.description.trim(),
      amountCents: input.amountCents,
      count: input.cardRepeat === "subscription" ? null : input.installments,
      startDate: input.date,
    });
  }
  const refundCents = parseBRLToCents(input.refundAmount);
  if (refundCents && refundCents > 0) {
    const invoices = await listInvoices(input.cardId);
    const invoice = invoices.find((item) => item.status === "aberta") ?? invoices[0];
    if (invoice) await createRefundExpectation(invoice.id, refundCents, null);
  }
}
