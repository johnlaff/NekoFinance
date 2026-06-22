import React from "react";

// Inline-style pattern — matches production Badge.tsx which uses no CSS classes.

const TONE_STYLES = {
  success: { background: "var(--success-tint)", color: "var(--success-400)" },
  warning: { background: "var(--warning-tint)", color: "var(--warning-400)" },
  danger: { background: "var(--danger-tint)", color: "var(--danger-400)" },
  info: { background: "var(--info-tint)", color: "var(--info-400)" },
  primary: { background: "var(--primary-quiet)", color: "var(--primary)" },
  secondary: { background: "var(--secondary-quiet)", color: "var(--secondary)" },
};

const BASE = {
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  padding: "1px 7px",
  fontSize: "var(--fs-micro)",
  fontWeight: "var(--fw-bold)",
  letterSpacing: "var(--ls-caps)",
  textTransform: "uppercase",
  lineHeight: 1.3,
  whiteSpace: "nowrap",
};

const DOT_BASE = {
  width: 6,
  height: 6,
  borderRadius: "50%",
  background: "currentColor",
  display: "inline-block",
  flexShrink: 0,
};

export function Badge({
  tone = "primary",
  dot = false,
  square = false,
  children,
  className = "",
  ...rest
}) {
  const toneStyle = TONE_STYLES[tone] ?? TONE_STYLES["primary"];
  const style = {
    ...BASE,
    borderRadius: square ? "4px" : "999px",
    ...toneStyle,
  };
  return (
    <span className={className} style={style} {...rest}>
      {dot && <span style={DOT_BASE} />}
      {children}
    </span>
  );
}
