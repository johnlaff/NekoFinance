import React from "react";

const CSS = `
.nk-seg{display:inline-flex;padding:3px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-sm);gap:2px;font-family:var(--font-sans);}
.nk-seg__opt{appearance:none;border:none;background:none;cursor:pointer;height:28px;padding:0 13px;
  border-radius:4px;font-size:13px;font-weight:600;color:var(--text-muted);white-space:nowrap;
  display:inline-flex;align-items:center;gap:7px;transition:var(--t-hover);}
.nk-seg__opt:hover{color:var(--text);}
.nk-seg__opt[aria-selected="true"]{background:var(--surface-elevated);color:var(--text-strong);
  box-shadow:var(--shadow-1);}
.nk-seg__opt:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-seg__dot{width:8px;height:8px;border-radius:50%;flex:none;}
.nk-seg--sm .nk-seg__opt{height:24px;padding:0 10px;font-size:12px;}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-seg-css")) return;
    const s = document.createElement("style");
    s.id = "nk-seg-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function SegmentedControl({
  options = [],
  value,
  onChange = () => {},
  size = "md",
  className = "",
}) {
  useCSS();
  return (
    <div
      role="tablist"
      className={["nk-seg", size === "sm" ? "nk-seg--sm" : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      {options.map((o) => {
        const opt = typeof o === "string" ? { value: o, label: o } : o;
        const selected = opt.value === value;
        return (
          <button
            key={opt.value}
            role="tab"
            type="button"
            aria-selected={selected}
            className="nk-seg__opt"
            onClick={() => onChange(opt.value)}
          >
            {opt.dot ? (
              <span className="nk-seg__dot" style={{ background: opt.dot }} />
            ) : null}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
