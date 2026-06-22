import React from "react";

const CSS = `
.nk-btn{--_h:var(--hit-min);--_px:14px;--_fs:var(--fs-sm);display:inline-flex;align-items:center;justify-content:center;gap:var(--space-2);
  height:var(--_h);padding:0 var(--_px);font-family:var(--font-sans);font-size:var(--_fs);font-weight:var(--fw-semibold);
  line-height:1;border-radius:var(--radius-sm);border:var(--bw-hair) solid transparent;cursor:pointer;white-space:nowrap;
  transition:var(--t-hover);}
.nk-btn:active:not([disabled]){transform:translateY(0.5px) scale(0.992);}
.nk-btn:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-btn[disabled]{opacity:.5;cursor:not-allowed;}
.nk-btn--sm{--_h:28px;--_px:10px;--_fs:var(--fs-sm);}
.nk-btn--lg{--_h:44px;--_px:18px;--_fs:var(--fs-body);}
.nk-btn__ic{display:inline-flex;width:16px;height:16px;flex:none;}
.nk-btn--primary{background:var(--primary);color:var(--text-on-primary);}
.nk-btn--primary:hover:not([disabled]){background:var(--primary-hover);}
.nk-btn--primary:active:not([disabled]){background:var(--primary-press);}
.nk-btn--secondary{background:var(--secondary-quiet);color:var(--secondary);border-color:transparent;}
.nk-btn--secondary:hover:not([disabled]){filter:brightness(1.06);}
.nk-btn--ghost{background:transparent;color:var(--text);border-color:var(--border);}
.nk-btn--ghost:hover:not([disabled]){background:var(--surface-hover);}
.nk-btn--danger{background:var(--danger-tint);color:var(--danger-400);border-color:transparent;}
.nk-btn--danger:hover:not([disabled]){filter:brightness(1.08);}
@media(prefers-reduced-motion:reduce){.nk-btn,.nk-btn:active:not([disabled]){transition:none;transform:none;}}
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
