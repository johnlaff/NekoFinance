import { useState } from "react";

interface Option {
  value: string;
  label: string;
}

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
  const [active, setActive] = useState(value);

  const handleClick = (v: string) => {
    setActive(v);
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
        const isActive = active === opt.value;
        return (
          <button
            key={opt.value}
            role="tab"
            aria-selected={isActive}
            disabled={disabled}
            onClick={() => !disabled && handleClick(opt.value)}
            type="button"
            style={{
              background: isActive ? "var(--surface-selected)" : "transparent",
              color: isActive ? "var(--primary)" : "var(--text-muted)",
              border: "none",
              borderRadius: "calc(var(--radius-xs) - 1px)",
              fontFamily: "var(--font-sans)",
              fontSize: size === "sm" ? "var(--fs-sm)" : "var(--fs-body)",
              fontWeight: "var(--fw-medium)",
              minHeight: size === "sm" ? "28px" : "32px",
              padding: size === "sm" ? "2px 10px" : "4px 14px",
              cursor: disabled ? "not-allowed" : "pointer",
              opacity: disabled ? 0.5 : 1,
              transition: "var(--t-hover)",
              whiteSpace: "nowrap",
            }}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
