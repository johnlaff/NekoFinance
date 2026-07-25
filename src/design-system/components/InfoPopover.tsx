import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { GLOSSARY, type GlossaryEntry } from "../glossary";

/**
 * InfoPopover — o explicador didático de termos do método (mandato "didático e calmo" do DS).
 * Envolve um termo financeiro e abre uma frase de contexto sob demanda. Portado do design system:
 * `position: fixed` portaled para o body (nunca cortado por overflow), flip/clamp ao viewport,
 * botão nativo (Enter/Space abrem; Esc fecha e devolve o foco), popover `role=tooltip`,
 * respeita prefers-reduced-motion (via CSS). Orçamento de conteúdo: título opcional + 1–2 frases.
 */

interface Pos {
  left: number;
  top: number;
  side: "top" | "bottom";
  arrowX: number;
}

interface InfoPopoverProps {
  /** Chave do glossário OU o conteúdo direto ({title?, body}). */
  term: string | GlossaryEntry;
  children: ReactNode;
  /** Esconde o marcador "i" (quando o trigger já é visualmente distinto). */
  hideMarker?: boolean;
  width?: number;
  className?: string;
}

export function InfoPopover({
  term,
  children,
  hideMarker = false,
  width = 280,
  className = "",
}: InfoPopoverProps) {
  const entry: GlossaryEntry =
    typeof term === "string" ? (GLOSSARY[term] ?? { body: term }) : term;
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<Pos | null>(null);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const popRef = useRef<HTMLSpanElement>(null);
  const hoverTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const id = useId();

  // `place` mora DENTRO do effect (depende só de open+width) → sem dep instável. Os fechamentos
  // (`dismiss`/`dismissAndRefocus`) também moram no effect e compartilham um único `setOpen(false)`,
  // de modo que o effect tem só dois setState lexicais (posicionar + fechar) — sem cascata síncrona
  // (os fechamentos rodam em handlers assíncronos) nem callbacks instáveis nas deps.
  useEffect(() => {
    if (!open) return;
    const dismiss = () => setOpen(false);
    const dismissAndRefocus = () => {
      dismiss();
      (wrapRef.current?.querySelector(".nk-term") as HTMLElement | null)?.focus();
    };
    const place = () => {
      const trigger = wrapRef.current?.querySelector(".nk-term");
      if (!trigger) return;
      const r = trigger.getBoundingClientRect();
      const MARGIN = 12;
      const GAP = 9;
      const popH = popRef.current ? popRef.current.offsetHeight : 96;
      const below =
        r.bottom + GAP + popH + MARGIN <= window.innerHeight ||
        r.top - GAP - popH < MARGIN;
      const top = below ? r.bottom + GAP : r.top - GAP - popH;
      let left = r.left;
      left = Math.min(left, window.innerWidth - width - MARGIN);
      left = Math.max(MARGIN, left);
      const arrowX = Math.max(12, Math.min(width - 20, r.left + r.width / 2 - left));
      setPos({ left, top, side: below ? "bottom" : "top", arrowX });
    };
    place();
    const raf = requestAnimationFrame(place); // re-place após medir a altura real
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismissAndRefocus();
    };
    const onDoc = (e: MouseEvent) => {
      const t = e.target as Node;
      if (wrapRef.current?.contains(t) || popRef.current?.contains(t)) return;
      dismiss();
    };
    const onScroll = () => place();
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDoc);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onScroll);
    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onDoc);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onScroll);
    };
  }, [open, width]);

  // Higiene: limpa o timer de hover pendente no unmount (evita callback órfão).
  useEffect(() => () => clearTimeout(hoverTimer.current), []);

  const show = () => {
    clearTimeout(hoverTimer.current);
    setOpen(true);
  };
  const hideSoon = () => {
    clearTimeout(hoverTimer.current);
    hoverTimer.current = setTimeout(() => setOpen(false), 140);
  };
  const marker = !hideMarker ? (
    <span className="nk-term__i" aria-hidden="true">
      i
    </span>
  ) : null;

  return (
    <span
      className="nk-term-wrap"
      ref={wrapRef}
      onMouseEnter={show}
      onMouseLeave={hideSoon}
    >
      {/* Botão NATIVO (não <span role=button>): foco e Enter/Espaço vêm de graça. `.nk-term` é
          inline-flex sem borda/fundo, então flui inline dentro do texto. Clique/teclado ABRE; o
          hover também abre; fecha por Esc/clique-fora/tirar o mouse. */}
      <button
        type="button"
        className={["nk-term", hideMarker ? "nk-term--plain" : "", className]
          .filter(Boolean)
          .join(" ")}
        // O popover é um explicador NÃO-modal (role="tooltip"), anunciado via aria-describedby
        // quando aberto. NÃO usamos aria-expanded (sugere região colapsável/disclosure, incoerente
        // com tooltip) nem aria-haspopup (promete um popup interativo dialog/menu inexistente).
        aria-describedby={open ? id : undefined}
        onClick={(e) => {
          e.stopPropagation();
          setOpen(true);
        }}
      >
        {children}
        {marker}
      </button>
      {open &&
        pos &&
        createPortal(
          <span
            className={`nk-pop nk-pop--${pos.side}`}
            role="tooltip"
            id={id}
            ref={popRef}
            style={
              {
                left: `${pos.left}px`,
                top: `${pos.top}px`,
                width: `${width}px`,
                "--arrow-x": `${pos.arrowX}px`,
              } as React.CSSProperties
            }
            onMouseEnter={show}
            onMouseLeave={hideSoon}
          >
            {entry.title ? <span className="nk-pop__title">{entry.title}</span> : null}
            <span className="nk-pop__body">{entry.body}</span>
            <span className="nk-pop__hint">Esc para fechar</span>
          </span>,
          document.body,
        )}
    </span>
  );
}
