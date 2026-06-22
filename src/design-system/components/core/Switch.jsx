import React from "react";

const CSS = `
.nk-switch{display:inline-flex;align-items:center;gap:10px;cursor:pointer;font-family:var(--font-sans);
  font-size:var(--fs-body);color:var(--text);user-select:none;}
.nk-switch input{position:absolute;opacity:0;width:0;height:0;}
.nk-switch__track{position:relative;width:36px;height:20px;border-radius:10px;
  background:var(--ink-300);transition:var(--t-hover);flex:none;}
[data-theme="light"] .nk-switch__track{background:#727c77;}
.nk-switch__thumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;
  background:var(--ink-000);transition:var(--t-hover);box-shadow:var(--shadow-1);}
.nk-switch input:checked + .nk-switch__track{background:var(--primary);}
.nk-switch input:checked + .nk-switch__track .nk-switch__thumb{left:18px;background:var(--ink-000);}
.nk-switch input:focus-visible + .nk-switch__track{box-shadow:var(--shadow-focus);}
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
  checked = false,
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
