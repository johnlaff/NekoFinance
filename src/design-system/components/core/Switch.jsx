import React from "react";

const CSS = `
.nk-switch{display:inline-flex;align-items:center;gap:10px;cursor:pointer;font-family:var(--font-sans);
  font-size:13px;color:var(--text);user-select:none;}
.nk-switch input{position:absolute;opacity:0;width:0;height:0;}
.nk-switch__track{position:relative;width:38px;height:22px;border-radius:var(--radius-pill);
  background:var(--ink-600);border:1px solid var(--border-strong);transition:var(--t-hover);flex:none;}
.nk-switch__thumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;
  background:var(--text-muted);box-shadow:var(--shadow-1);transition:transform var(--dur-base) var(--ease-standard),background var(--dur-fast) var(--ease-standard);}
.nk-switch input:checked + .nk-switch__track{background:var(--primary);border-color:var(--primary);}
.nk-switch input:checked + .nk-switch__track .nk-switch__thumb{transform:translateX(16px);background:var(--text-on-primary);}
.nk-switch input:focus-visible + .nk-switch__track{box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-switch--disabled{opacity:.45;pointer-events:none;}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-switch-css")) return;
    const s = document.createElement("style");
    s.id = "nk-switch-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function Switch({
  checked,
  onChange = () => {},
  label,
  disabled = false,
  className = "",
  ...rest
}) {
  useCSS();
  return (
    <label
      className={["nk-switch", disabled ? "nk-switch--disabled" : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        {...rest}
      />
      <span className="nk-switch__track">
        <span className="nk-switch__thumb" />
      </span>
      {label ? <span>{label}</span> : null}
    </label>
  );
}
