import type { ReactNode } from "react";

function toCamelCase(str: string): string {
  return str.replace(/-([a-z])/g, (_, g) => g.toUpperCase());
}

interface ButtonProps {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md" | "lg";
  iconLeft?: ReactNode;
  iconRight?: ReactNode;
  children: ReactNode;
  onClick?: () => void;
  type?: "button" | "submit";
  className?: string;
  disabled?: boolean;
}

const variantStyles: Record<string, string> = {
  primary:
    "background:var(--primary);color:var(--text-on-primary);border-color:transparent;",
  secondary:
    "background:var(--secondary-quiet);color:var(--secondary);border-color:transparent;",
  ghost: "background:transparent;color:var(--text);border-color:var(--border);",
  danger:
    "background:var(--danger-tint);color:var(--danger-400);border-color:transparent;",
};

const sizeStyles: Record<string, string> = {
  sm: "height:28px;padding:0 10px;font-size:var(--fs-sm);",
  md: "height:var(--hit-min);padding:0 14px;font-size:var(--fs-sm);",
  lg: "height:44px;padding:0 18px;font-size:var(--fs-body);",
};

export function Button({
  variant = "primary",
  size = "md",
  iconLeft,
  iconRight,
  children,
  onClick,
  type = "button",
  className = "",
  disabled = false,
}: ButtonProps) {
  const vStyle = (variantStyles[variant] ?? variantStyles["primary"]) || "";
  const sStyle = (sizeStyles[size] ?? sizeStyles["md"]) || "";

  return (
    <button
      type={type}
      disabled={disabled}
      onClick={onClick}
      className={className}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-2)",
        justifyContent: "center",
        borderRadius: "var(--radius-sm)",
        border: "var(--bw-hair) solid transparent",
        fontFamily: "var(--font-sans)",
        fontWeight: "var(--fw-semibold)",
        lineHeight: 1,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.5 : 1,
        transition: "var(--t-hover)",
        whiteSpace: "nowrap",
        ...Object.fromEntries(
          [...vStyle.split(";"), ...sStyle.split(";")].filter(Boolean).map((s) => {
            const parts = s.split(":").map((x) => x.trim());
            return [toCamelCase(parts[0] || ""), parts[1] || ""];
          }),
        ),
      }}
    >
      {iconLeft}
      {children}
      {iconRight}
    </button>
  );
}
