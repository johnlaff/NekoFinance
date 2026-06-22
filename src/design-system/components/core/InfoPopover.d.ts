import * as React from "react";

/**
 * Didactic term-explainer popover for Neko-method finance concepts.
 * Wraps a trigger inline; opens a positioned tooltip on click, hover, or
 * keyboard (Enter/Space). Closes on Esc, outside click, or mouse-leave.
 * Built-in PT-BR glossary covers 12 canonical method terms.
 * @startingPoint section="Core" subtitle="InfoPopover — method-term explainer" viewport="420x160"
 */
export interface GlossaryEntry {
  /** Optional bold title inside the popover. */
  title?: string;
  /** Required explanation body (1–2 sentences). */
  body: string;
}

export interface InfoPopoverProps {
  /**
   * Glossary key (one of the 12 built-in PT-BR terms) OR an inline
   * `{title?, body}` object for ad-hoc explanations.
   *
   * Built-in keys: `pode_gastar` · `piso_caixa` · `folga_poupanca` ·
   * `reserva` · `caixa` · `previsibilidade` · `colchao` · `performance` ·
   * `economizado` · `custo_de_vida` · `diario_medio` · `cartao`.
   */
  term?: string | GlossaryEntry;
  /**
   * The inline trigger content (usually a plain text label). Renders the
   * glossary key name as a default when omitted.
   */
  children?: React.ReactNode;
  /**
   * Hide the circular "i" badge to the right of the trigger text.
   * Use when the trigger element is already visually distinct.
   * @default false
   */
  hideMarker?: boolean;
  /**
   * Pixel width of the popover panel.
   * @default 280
   */
  width?: number;
  /** Extra class names added to the trigger `<button>`. */
  className?: string;
}

export function InfoPopover(props: InfoPopoverProps): JSX.Element;
