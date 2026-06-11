import React from "react";

const CSS = `
.nk-btn{--_h:36px;--_px:14px;--_fs:14px;display:inline-flex;align-items:center;justify-content:center;gap:8px;
  height:var(--_h);padding:0 var(--_px);font-family:var(--font-sans);font-size:var(--_fs);font-weight:600;
  line-height:1;border-radius:var(--radius-sm);border:1px solid transparent;cursor:pointer;white-space:nowrap;
  letter-spacing:-0.005em;transition:var(--t-hover),transform var(--dur-instant) var(--ease-standard);
  -webkit-tap-highlight-color:transparent;user-select:none;}
.nk-btn:active{transform:translateY(0.5px) scale(0.992);}
.nk-btn:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-btn[disabled]{opacity:.45;cursor:not-allowed;transform:none;}
.nk-btn--sm{--_h:30px;--_px:11px;--_fs:13px;}
.nk-btn--lg{--_h:44px;--_px:20px;--_fs:15px;}
.nk-btn--full{width:100%;}
.nk-btn__ic{display:inline-flex;width:16px;height:16px;flex:none;}
.nk-btn--primary{background:var(--primary);color:var(--text-on-primary);}
.nk-btn--primary:hover:not([disabled]){background:var(--primary-hover);}
.nk-btn--primary:active:not([disabled]){background:var(--primary-press);}
.nk-btn--secondary{background:var(--surface-elevated);color:var(--text);border-color:var(--border-strong);}
.nk-btn--secondary:hover:not([disabled]){background:var(--surface-hover);border-color:var(--border-strong);}
.nk-btn--ghost{background:transparent;color:var(--text-muted);}
.nk-btn--ghost:hover:not([disabled]){background:var(--surface-hover);color:var(--text);}
.nk-btn--danger{background:var(--danger-500);color:#fff;}
.nk-btn--danger:hover:not([disabled]){filter:brightness(1.08);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-btn-css")) return;
    const s = document.createElement("style");
    s.id = "nk-btn-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function Button({
  variant = "primary",
  size = "md",
  fullWidth = false,
  iconLeft = null,
  iconRight = null,
  disabled = false,
  type = "button",
  className = "",
  children,
  ...rest
}) {
  useCSS();
  const cls = [
    "nk-btn",
    `nk-btn--${variant}`,
    size !== "md" ? `nk-btn--${size}` : "",
    fullWidth ? "nk-btn--full" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type={type} className={cls} disabled={disabled} {...rest}>
      {iconLeft ? <span className="nk-btn__ic">{iconLeft}</span> : null}
      {children ? <span>{children}</span> : null}
      {iconRight ? <span className="nk-btn__ic">{iconRight}</span> : null}
    </button>
  );
}
