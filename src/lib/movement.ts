import type { MovKind } from "../design-system/components/MovBadge";

/** Os 5 tipos de movimento do método, na ordem canônica usada nos seletores (form e check-in). */
export const FORM_KINDS: MovKind[] = [
  "entrada",
  "saida",
  "diario",
  "cartao",
  "economia",
];

/** Campos do schema derivados de um tipo de movimento. */
export interface KindFields {
  txnType: "income" | "expense" | "transfer";
  isFixed: boolean;
  paymentMethod: string | null;
}

/** Mapeia o tipo de movimento do método para (type, is_fixed, payment_method) do schema. */
export function kindToFields(kind: MovKind): KindFields {
  switch (kind) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: "debit" };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
    case "economia":
      return { txnType: "transfer", isFixed: false, paymentMethod: null };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: "debit" };
  }
}

/** Inverso de `kindToFields`: deriva o tipo de movimento (chip) de um lançamento existente. */
export function fieldsToKind(
  type: string,
  isFixed: boolean,
  paymentMethod: string | null,
): MovKind {
  if (type === "transfer") return "economia";
  if (type === "income") return "entrada";
  if (isFixed) return "saida";
  if (paymentMethod === "credit") return "cartao";
  return "diario";
}
