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
}

export function SegmentedControl({
  options,
  value,
  onChange,
  size = "md",
  className = "",
  disabled = false,
}: SegmentedControlProps) {
  const handleClick = (v: string) => {
    onChange(v);
  };

  return (
    <div
      className={className}
      role="tablist"
      style={{
        display: "flex",
        gap: "2px",
        padding: "2px",
        background: "var(--bg-subtle)",
        borderRadius: "var(--radius-xs)",
      }}
    >
      {options.map((opt) => {
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
            role="tab"
            aria-selected={isActive}
            disabled={disabled}
            onClick={() => !disabled && handleClick(opt.value)}
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
