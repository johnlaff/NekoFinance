import React from "react";

const CSS = `
.nk-field{display:flex;flex-direction:column;gap:6px;font-family:var(--font-sans);}
.nk-field__label{font-size:12px;font-weight:600;color:var(--text-muted);letter-spacing:.01em;}
.nk-field__req{color:var(--danger-400);margin-left:3px;}
.nk-input{display:flex;align-items:center;gap:8px;height:36px;padding:0 11px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-sm);transition:var(--t-hover),box-shadow var(--dur-fast) var(--ease-standard);}
.nk-input:hover{border-color:var(--border-strong);}
.nk-input:focus-within{border-color:var(--border-focus);box-shadow:0 0 0 3px var(--focus-ring);}
.nk-input--err{border-color:var(--danger-500);}
.nk-input--err:focus-within{box-shadow:0 0 0 3px var(--danger-tint);}
.nk-input input{flex:1;min-width:0;background:none;border:none;outline:none;color:var(--text);
  font-family:inherit;font-size:14px;}
.nk-input input::placeholder{color:var(--text-faint);}
.nk-input--money input{font-family:var(--font-money);font-variant-numeric:tabular-nums;text-align:right;}
.nk-input__affix{color:var(--text-faint);font-size:13px;display:inline-flex;align-items:center;flex:none;}
.nk-input__icon{width:16px;height:16px;color:var(--text-faint);flex:none;display:inline-flex;}
.nk-input[disabled],.nk-input--disabled{opacity:.5;pointer-events:none;}
.nk-field__hint{font-size:11.5px;color:var(--text-faint);}
.nk-field__hint--err{color:var(--danger-400);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-input-css")) return;
    const s = document.createElement("style");
    s.id = "nk-input-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

export function Input({
  label,
  required = false,
  prefix = null,
  suffix = null,
  icon = null,
  money = false,
  error = "",
  hint = "",
  disabled = false,
  className = "",
  id,
  ...rest
}) {
  useCSS();
  const fid = id || React.useId();
  return (
    <div className={["nk-field", className].filter(Boolean).join(" ")}>
      {label ? (
        <label className="nk-field__label" htmlFor={fid}>
          {label}
          {required ? <span className="nk-field__req">*</span> : null}
        </label>
      ) : null}
      <div
        className={[
          "nk-input",
          money ? "nk-input--money" : "",
          error ? "nk-input--err" : "",
          disabled ? "nk-input--disabled" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {icon ? <span className="nk-input__icon">{icon}</span> : null}
        {prefix ? <span className="nk-input__affix">{prefix}</span> : null}
        <input id={fid} disabled={disabled} {...rest} />
        {suffix ? <span className="nk-input__affix">{suffix}</span> : null}
      </div>
      {error ? (
        <span className="nk-field__hint nk-field__hint--err">{error}</span>
      ) : hint ? (
        <span className="nk-field__hint">{hint}</span>
      ) : null}
    </div>
  );
}
