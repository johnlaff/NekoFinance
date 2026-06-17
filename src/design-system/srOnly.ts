import type { CSSProperties } from "react";

/**
 * Estilo "só para leitores de tela": tira do layout visual mas mantém no fluxo de acessibilidade.
 * Usado para dar a um glifo/composição decorativa (`aria-hidden`) um rótulo textual equivalente,
 * sem recorrer a `role="img"`.
 */
export const SR_ONLY: CSSProperties = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clipPath: "inset(50%)",
  whiteSpace: "nowrap",
  border: 0,
};
