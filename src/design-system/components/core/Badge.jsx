import React from "react";

const CSS = `
.nk-badge{display:inline-flex;align-items:center;gap:6px;height:22px;padding:0 9px;border-radius:var(--radius-pill);
  font-family:var(--font-sans);font-size:11.5px;font-weight:600;letter-spacing:.01em;white-space:nowrap;
  border:1px solid transparent;line-height:1;}
.nk-badge__dot{width:6px;height:6px;border-radius:50%;flex:none;}
.nk-badge--solid{color:#fff;}
.nk-badge--square{border-radius:var(--radius-xs);}
.nk-badge--neutral{background:var(--surface-elevated);color:var(--text-muted);border-color:var(--border);}
.nk-badge--success{background:var(--success-tint);color:var(--success-400);}
.nk-badge--warning{background:var(--warning-tint);color:var(--warning-400);}
.nk-badge--danger{background:var(--danger-tint);color:var(--danger-400);}
.nk-badge--info{background:var(--info-tint);color:var(--info-400);}
.nk-badge--primary{background:var(--primary-quiet);color:var(--primary);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-badge-css")) return;
    const s = document.createElement("style");
    s.id = "nk-badge-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const DOTS = {
  success: "var(--success-500)",
  warning: "var(--warning-500)",
  danger: "var(--danger-500)",
  info: "var(--info-500)",
  primary: "var(--primary)",
  neutral: "var(--text-faint)",
};

export function Badge({
  tone = "neutral",
  dot = false,
  square = false,
  children,
  className = "",
  ...rest
}) {
  useCSS();
  return (
    <span
      className={[
        "nk-badge",
        `nk-badge--${tone}`,
        square ? "nk-badge--square" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      {...rest}
    >
      {dot ? (
        <span className="nk-badge__dot" style={{ background: DOTS[tone] }} />
      ) : null}
      {children}
    </span>
  );
}
