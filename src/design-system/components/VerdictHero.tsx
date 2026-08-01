import type { ReactNode } from "react";

/**
 * VerdictHero — a escala grande do veredito-primeiro: a manchete que abre a tela.
 *
 * O contrato do veredito, em qualquer escala: a palavra vem antes do número, o estado é
 * sempre texto (cor nunca é o único sinal) e o nível sai de um vocabulário fechado. A
 * escala pequena do mesmo contrato é o `HealthBadge`, que marca um bloco — um título que
 * abre a tela e uma pílula dentro de um card não são o mesmo componente.
 *
 * Cor de status fica FORA daqui: a manchete é a voz da marca, não o semáforo do método.
 *
 * `data-large-title` é contrato com o shell — a appbar mobile só assume o título quando
 * este sai de vista.
 */
export function VerdictHero({
  label,
  labelMark,
  headline,
  children,
  actions,
  footer,
}: {
  /** O olho: contexto e recorte ("Horizonte · Hoje → 30/06"). */
  label?: ReactNode;
  /** Selo epistêmico do olho, quando o recorte é estimativa. */
  labelMark?: ReactNode;
  /** A manchete: o estado em palavras. */
  headline: ReactNode;
  /** O corpo que prova a manchete. */
  children?: ReactNode;
  /** Ações que a manchete convida. */
  actions?: ReactNode;
  /** Proveniência ou rodapé de estado do dado. */
  footer?: ReactNode;
}) {
  return (
    <div className="nk-verdict">
      {label ? (
        <p className="nk-verdict__label">
          {label}
          {labelMark}
        </p>
      ) : null}
      <h1 className="nk-verdict__headline" data-large-title>
        {headline}
      </h1>
      {children ? <div className="nk-verdict__body">{children}</div> : null}
      {actions ? <div className="nk-verdict__actions">{actions}</div> : null}
      {footer}
    </div>
  );
}
