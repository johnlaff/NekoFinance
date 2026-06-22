import React from "react";

const CSS = `
.nk-state{display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;
  gap:12px;padding:36px 28px;font-family:var(--font-sans);max-width:380px;margin:0 auto;}
.nk-state__ic{width:44px;height:44px;border-radius:var(--radius-md);display:flex;align-items:center;
  justify-content:center;}
.nk-state__ic--empty{background:var(--surface-elevated);border:1px solid var(--border);color:var(--text-faint);}
.nk-state__ic--error{background:var(--danger-tint);color:var(--danger-400);}
.nk-state__title{font-size:15px;font-weight:700;color:var(--text-strong);}
.nk-state__desc{font-size:13px;line-height:1.5;color:var(--text-muted);}
.nk-state__action{margin-top:4px;}
.nk-state__spin{width:30px;height:30px;border-radius:50%;border:2.5px solid var(--border);
  border-top-color:var(--primary);animation:nk-spin .8s linear infinite;}
@keyframes nk-spin{to{transform:rotate(360deg)}}
@media (prefers-reduced-motion:reduce){.nk-state__spin{animation:none;}}
.nk-skel{display:flex;flex-direction:column;gap:9px;width:100%;padding:16px;}
.nk-skel__row{height:13px;border-radius:5px;background:linear-gradient(90deg,var(--surface-2) 25%,var(--surface-hover) 37%,var(--surface-2) 63%);
  background-size:400% 100%;animation:nk-shimmer 1.4s ease infinite;}
@keyframes nk-shimmer{0%{background-position:100% 0}100%{background-position:0 0}}
@media (prefers-reduced-motion:reduce){.nk-skel__row{animation:none;}}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-state-css")) return;
    const s = document.createElement("style");
    s.id = "nk-state-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function EmptyState({
  variant = "empty",
  icon = null,
  title,
  description,
  action = null,
  skeletonRows = 4,
  className = "",
}) {
  useCSS();
  if (variant === "skeleton") {
    return (
      <div className={["nk-skel", className].filter(Boolean).join(" ")}>
        {Array.from({ length: skeletonRows }).map((_, i) => (
          <div
            className="nk-skel__row"
            key={i}
            style={{ width: `${100 - (i % 3) * 14}%` }}
          />
        ))}
      </div>
    );
  }
  return (
    <div className={["nk-state", className].filter(Boolean).join(" ")}>
      {variant === "loading" ? (
        <div className="nk-state__spin" />
      ) : (
        <div
          className={`nk-state__ic nk-state__ic--${variant === "error" ? "error" : "empty"}`}
        >
          {icon ?? (
            <svg
              width="22"
              height="22"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              {variant === "error" ? (
                <>
                  <circle cx="12" cy="12" r="9" />
                  <path d="M12 8v4M12 16h.01" />
                </>
              ) : (
                <>
                  <rect x="3" y="4" width="18" height="16" rx="2" />
                  <path d="M3 10h18M8 4v16" />
                </>
              )}
            </svg>
          )}
        </div>
      )}
      {title ? <div className="nk-state__title">{title}</div> : null}
      {description ? <div className="nk-state__desc">{description}</div> : null}
      {action ? <div className="nk-state__action">{action}</div> : null}
    </div>
  );
}
