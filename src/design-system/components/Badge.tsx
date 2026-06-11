import type { ReactNode } from "react";

function toCamelCase(str: string): string {
  return str.replace(/-([a-z])/g, (_, g) => g.toUpperCase());
}

interface BadgeProps {
  tone?: "success" | "warning" | "danger" | "info" | "primary" | "secondary";
  dot?: boolean;
  square?: boolean;
  children: ReactNode;
  className?: string;
}

const toneMap: Record<string, string> = {
  success: "background:var(--success-tint);color:var(--success-400);",
  warning: "background:var(--warning-tint);color:var(--warning-400);",
  danger: "background:var(--danger-tint);color:var(--danger-400);",
  info: "background:var(--info-tint);color:var(--info-400);",
  primary: "background:var(--primary-quiet);color:var(--primary);",
  secondary: "background:var(--secondary-quiet);color:var(--secondary);",
};

export function Badge({
  tone = "primary",
  dot = false,
  square = false,
  children,
  className = "",
}: BadgeProps) {
  const style = (toneMap[tone] ?? toneMap["primary"]) || "";
  return (
    <span
      className={className}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        padding: "1px 7px",
        borderRadius: square ? "4px" : "999px",
        fontSize: "var(--fs-micro)",
        fontWeight: "var(--fw-bold)",
        letterSpacing: "var(--ls-caps)",
        textTransform: "uppercase",
        lineHeight: 1.3,
        ...Object.fromEntries(
          style
            .split(";")
            .filter(Boolean)
            .map((s) => {
              const parts = s.split(":").map((x) => x.trim());
              return [toCamelCase(parts[0] || ""), parts[1] || ""];
            }),
        ),
      }}
    >
      {dot && (
        <span
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background: "currentColor",
            display: "inline-block",
          }}
        />
      )}
      {children}
    </span>
  );
}
