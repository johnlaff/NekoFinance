import * as React from "react";

/**
 * Badge de tipo de movimento — exibe o glifo colorido (círculo com letra) de cada
 * um dos 5 pilares do método: Entrada, Saída, Diário, Economia, Cartão.
 * O rótulo textual sempre está disponível para leitores de tela; `showLabel` o torna visível.
 * Entrada e Economia compartilham o glifo "E" — a cor os distingue (jade vs. verde-escuro).
 * @startingPoint section="Finance" subtitle="MovBadge — tipo de movimento (5 pilares)" viewport="360x200"
 */
export interface MovBadgeProps {
  /**
   * Tipo de movimento: "entrada" | "saida" | "diario" | "economia" | "cartao".
   * @default "saida"
   */
  kind?: "entrada" | "saida" | "diario" | "economia" | "cartao";
  /**
   * Quando true, exibe o nome do tipo ao lado do glifo.
   * Quando false (padrão), o nome ainda é exposto a leitores de tela via sr-only.
   * @default false
   */
  showLabel?: boolean;
  /**
   * Diâmetro do círculo glifo em px. O tamanho da fonte do glifo é calculado
   * automaticamente como 56% do diâmetro.
   * @default 18
   */
  size?: number;
  className?: string;
}

/**
 * Badge de tipo de movimento — exibe o glifo colorido (círculo com letra) de cada
 * um dos 5 pilares do método: Entrada, Saída, Diário, Economia, Cartão.
 */
export function MovBadge(props: MovBadgeProps): JSX.Element;
