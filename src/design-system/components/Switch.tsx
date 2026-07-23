import { useEffect } from "react";

/* Desligado, o thumb usa tinta visível (--text-muted) e o trilho leva borda:
   thumb branco sobre trilho claro some no tema claro (~1:1). Ligado, o par
   atômico --primary/--text-on-primary garante o contraste em qualquer paleta. */
const CSS = `
.nk-switch{position:relative;width:40px;height:23px;flex:none;padding:0;cursor:pointer;
  border-radius:var(--radius-pill);background:var(--bg-subtle);
  border:var(--bw-hair) solid var(--border-input);
  transition:background var(--dur-fast) var(--ease-standard),border-color var(--dur-fast) var(--ease-standard);}
.nk-switch__k{position:absolute;top:2px;left:2px;width:17px;height:17px;border-radius:50%;
  background:var(--text-muted);
  transition:transform var(--dur-fast) var(--ease-standard),background var(--dur-fast) var(--ease-standard);}
.nk-switch[aria-checked="true"]{background:var(--primary);border-color:var(--primary);}
.nk-switch[aria-checked="true"] .nk-switch__k{transform:translateX(17px);background:var(--text-on-primary);}
.nk-switch:focus-visible{outline:2px solid var(--primary);outline-offset:2px;}
.nk-switch:disabled{opacity:.55;cursor:default;}
/* Alvo de toque ≥44px sem inflar o layout (área expandida invisível). */
.nk-switch::before{content:"";position:absolute;inset:-11px -4px;}
@media (prefers-reduced-motion:reduce){.nk-switch,.nk-switch__k{transition:none;}}
`;

function useCSS() {
  useEffect(() => {
    if (document.getElementById("nk-switch-css")) return;
    const s = document.createElement("style");
    s.id = "nk-switch-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

interface SwitchProps {
  on: boolean;
  /** O evento acompanha para quem precisa da origem do toque (reveal do tema). */
  onChange: (next: boolean, event: React.MouseEvent<HTMLButtonElement>) => void;
  /** Nome acessível do switch (a linha ao lado carrega o rótulo visível). */
  label: string;
  disabled?: boolean;
  className?: string;
}

/** Interruptor do design system: `role="switch"`, thumb por transform (nunca layout). */
export function Switch({ on, onChange, label, disabled, className }: SwitchProps) {
  useCSS();
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      className={className ? `nk-switch ${className}` : "nk-switch"}
      onClick={(event) => onChange(!on, event)}
    >
      <span className="nk-switch__k" aria-hidden="true" />
    </button>
  );
}
