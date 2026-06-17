import { useId, useState, type ReactNode } from "react";

/**
 * Disclosure — padrão "lump-expand" do design system: o resumo fica sempre visível; o detalhe denso
 * abre sob demanda. Anima `grid-template-rows: 0fr → 1fr` (sem height-jank), com `aria-expanded`/
 * `aria-controls`/`role=region` e chevron que rotaciona. Respeita prefers-reduced-motion (via CSS).
 *
 * `bare` (sem chrome de card) é o default aqui porque a maioria dos usos é DENTRO de um card do
 * dashboard, e card-dentro-de-card é proibido. Sem `bare`, vira o card autônomo do DS.
 */
type Accent = "ok" | "warn" | "brass";

interface DisclosureProps {
  title: ReactNode;
  summary?: ReactNode;
  icon?: ReactNode;
  accent?: Accent;
  badge?: ReactNode;
  defaultOpen?: boolean;
  /** Sem fundo/borda/sombra de card (uso interno a um card). Default true. */
  bare?: boolean;
  children: ReactNode;
  className?: string;
}

function Chevron() {
  return (
    <svg
      className="nk-disc__chev"
      width={16}
      height={16}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M6 9l6 6 6-6"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Disclosure({
  title,
  summary,
  icon,
  accent,
  badge,
  defaultOpen = false,
  bare = true,
  children,
  className = "",
}: DisclosureProps) {
  const [open, setOpen] = useState(defaultOpen);
  const id = useId();

  const classes = [
    "nk-disc",
    bare ? "nk-disc--bare" : "",
    open ? "is-open" : "",
    accent ? `nk-disc--${accent}` : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes}>
      <button
        type="button"
        className="nk-disc__head"
        aria-expanded={open}
        aria-controls={`${id}-b`}
        onClick={() => setOpen((o) => !o)}
      >
        {icon ? <span className="nk-disc__ic">{icon}</span> : null}
        <span className="nk-disc__titles">
          <span className="nk-disc__title" id={`${id}-t`}>
            {title}
            {badge}
          </span>
          {summary ? <span className="nk-disc__summary">{summary}</span> : null}
        </span>
        <Chevron />
      </button>
      <section
        className="nk-disc__bodywrap"
        id={`${id}-b`}
        aria-labelledby={`${id}-t`}
        inert={!open}
      >
        <div className="nk-disc__body">{children}</div>
      </section>
    </div>
  );
}
