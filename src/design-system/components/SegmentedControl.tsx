interface Option {
  value: string;
  label: string;
}

// Base estática do botão de segmento (não recria por render); estado/tamanho entram por merge.
const SEG_BTN_BASE: React.CSSProperties = {
  border: "none",
  borderRadius: "calc(var(--radius-xs) - 1px)",
  fontFamily: "var(--font-sans)",
  fontWeight: "var(--fw-medium)",
  transition: "var(--t-hover)",
  whiteSpace: "nowrap",
};

interface SegmentedControlProps {
  options: Option[];
  value: string;
  onChange: (value: string) => void;
  size?: "sm" | "md";
  className?: string;
  disabled?: boolean;
  /** Nome acessível do grupo (radiogroup). Opcional, mas recomendado. */
  ariaLabel?: string;
}

/**
 * Seletor de opção única (ex.: Dia/Semana/Mês). Semântica `radiogroup`/`radio` — não `tablist`,
 * que exigiria painéis associados (`aria-controls`) que não existem aqui. Navegação por teclado
 * completa: o grupo expõe UM ponto de tab (roving tabindex) e as setas movem a seleção, como o
 * usuário espera de um grupo de rádios (antes, `tablist` sem setas prendia o usuário de teclado).
 */
export function SegmentedControl({
  options,
  value,
  onChange,
  size = "md",
  className = "",
  disabled = false,
  ariaLabel,
}: SegmentedControlProps) {
  const handleKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, idx: number) => {
    if (disabled || options.length === 0) return;
    let next: number;
    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
        next = (idx + 1) % options.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        next = (idx - 1 + options.length) % options.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = options.length - 1;
        break;
      default:
        return;
    }
    e.preventDefault();
    const target = options[next];
    if (!target) return;
    onChange(target.value);
    // Move o foco para o rádio recém-selecionado (roving tabindex).
    const group = e.currentTarget.parentElement;
    const radios = group?.querySelectorAll<HTMLButtonElement>('[role="radio"]');
    radios?.[next]?.focus();
  };

  return (
    <div
      className={className}
      role="radiogroup"
      aria-label={ariaLabel}
      style={{
        display: "flex",
        gap: "2px",
        padding: "2px",
        background: "var(--bg-subtle)",
        borderRadius: "var(--radius-xs)",
      }}
    >
      {options.map((opt, idx) => {
        const isActive = value === opt.value;
        const btnStyle: React.CSSProperties = {
          ...SEG_BTN_BASE,
          background: isActive ? "var(--surface-selected)" : "transparent",
          color: isActive ? "var(--primary)" : "var(--text-muted)",
          fontSize: size === "sm" ? "var(--fs-sm)" : "var(--fs-body)",
          minHeight: size === "sm" ? "28px" : "32px",
          padding: size === "sm" ? "2px 10px" : "4px 14px",
          cursor: disabled ? "not-allowed" : "pointer",
          opacity: disabled ? 0.5 : 1,
        };
        return (
          <button
            key={opt.value}
            role="radio"
            aria-checked={isActive}
            // Roving tabindex: só o rádio selecionado entra na ordem de tab; os demais (-1) são
            // alcançados pelas setas. Sem isso, Tab pararia em cada segmento.
            tabIndex={isActive ? 0 : -1}
            disabled={disabled}
            onClick={() => !disabled && onChange(opt.value)}
            onKeyDown={(e) => handleKeyDown(e, idx)}
            type="button"
            style={btnStyle}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
