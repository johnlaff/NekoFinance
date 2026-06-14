import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/**
 * InfoPopover — o explicador didático de termos do método (mandato "didático e calmo" do DS).
 * Envolve um termo financeiro e abre uma frase de contexto sob demanda. Portado do design system:
 * `position: fixed` portaled para o body (nunca cortado por overflow), flip/clamp ao viewport,
 * teclado completo (Enter/Space alternam, Esc fecha e devolve o foco), `role=button`/`tooltip`,
 * respeita prefers-reduced-motion (via CSS). Orçamento de conteúdo: título opcional + 1–2 frases.
 */

export interface GlossaryEntry {
  title?: string;
  body: string;
}

/** Glossário canônico PT-BR dos termos do método (teste dos 12 anos). */
export const GLOSSARY: Record<string, GlossaryEntry> = {
  pode_gastar: {
    title: "Pode gastar hoje",
    body: "O quanto dá para gastar hoje sem furar o mês. É o menor de dois limites: o que o caixa aguenta e o que respeita sua meta de poupança.",
  },
  piso_caixa: {
    title: "Limite do caixa",
    body: "O máximo por dia que mantém nenhum dia do mês no vermelho, olhando o saldo projetado.",
  },
  folga_poupanca: {
    title: "Limite da poupança",
    body: "O máximo por dia que ainda deixa você guardar a meta do ano (20% a 30% da renda).",
  },
  reserva: {
    title: "Reserva",
    body: "Quantos meses de gasto você consegue cobrir com o que tem guardado. O colchão para imprevistos.",
  },
  caixa: {
    title: "Caixa",
    body: "É dinheiro de passagem, não a sua riqueza. O que está na conta hoje, antes das contas do mês.",
  },
  previsibilidade: {
    title: "Previsibilidade",
    body: "O quanto do gasto típico de cada mês futuro já está lançado. Futuro vazio engana a previsão.",
  },
  colchao: {
    title: "Colchão",
    body: "O que sobra e você guarda para cobrir meses negativos sem sacar investimento. Adaptação válida do método.",
  },
  performance: {
    title: "Performance",
    body: "A foto do mês: o que entrou menos tudo que saiu (saídas fixas, diário, economia, cartão e a previsão do diário que ainda falta). Por isso o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
  },
  economizado: {
    title: "Economizado",
    body: "Quanto da renda você guardou como Economia. A meta do método é de 20% a 30% no ano.",
  },
  custo_de_vida: {
    title: "Custo de vida",
    body: "Saídas fixas, diário e cartão somados. O que custa manter sua vida no mês.",
  },
  diario_medio: {
    title: "Diário médio",
    body: "A média do gasto variável por dia até hoje. Ajuda a saber se o ritmo do mês está saudável.",
  },
  cartao: {
    title: "Cartão (Régua 2)",
    body: "Compras no cartão viram fatura no vencimento. Gastar hoje no crédito afunda os meses à frente.",
  },
};

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

  // `place` mora DENTRO do effect (depende só de open+width) → sem dep instável nem setState
  // síncrono no fechamento. Ao fechar, o effect não faz nada; o portal some pelo guard `open && pos`
  // e a `pos` velha (inofensiva) é recomputada no próximo open.
  useEffect(() => {
    if (!open) return;
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
      if (e.key === "Escape") {
        setOpen(false);
        (wrapRef.current?.querySelector(".nk-term") as HTMLElement | null)?.focus();
      }
    };
    const onDoc = (e: MouseEvent) => {
      const t = e.target as Node;
      if (wrapRef.current?.contains(t) || popRef.current?.contains(t)) return;
      setOpen(false);
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
        aria-expanded={open}
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
