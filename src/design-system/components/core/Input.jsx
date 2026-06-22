import React from "react";

// Tokens used:
//   --font-sans, --font-money
//   --fs-label, --fs-body, --fs-sm, --fs-micro
//   --fw-semibold
//   --ls-label
//   --text, --text-muted, --text-faint
//   --bg-subtle, --surface-2
//   --border-input, --border-strong, --border-focus
//   --danger-400, --danger-500, --danger-tint
//   --focus-ring
//   --radius-xs
//   --hit-min, --bw-hair
//   --space-3, --space-2
//   --t-hover, --dur-fast, --ease-standard

const CSS = `
.nk-field{display:flex;flex-direction:column;gap:var(--space-2);font-family:var(--font-sans);}
.nk-field__label{font-size:var(--fs-label);font-weight:var(--fw-semibold);color:var(--text-muted);
  letter-spacing:var(--ls-label);text-transform:uppercase;}
.nk-field__req{color:var(--danger-400);margin-left:3px;}
.nk-input{display:flex;align-items:center;gap:var(--space-2);height:var(--hit-min);
  padding:0 var(--space-3);background:var(--bg-subtle);
  border:var(--bw-hair) solid var(--border-input);border-radius:var(--radius-xs);
  transition:var(--t-hover),box-shadow var(--dur-fast) var(--ease-standard);}
.nk-input:hover{border-color:var(--border-strong);}
.nk-input:focus-within{border-color:var(--border-focus);box-shadow:0 0 0 3px var(--focus-ring);}
.nk-input--err{border-color:var(--danger-500);}
.nk-input--err:focus-within{box-shadow:0 0 0 3px var(--danger-tint);}
.nk-input input{flex:1;min-width:0;background:none;border:none;outline:none;color:var(--text);
  font-family:inherit;font-size:var(--fs-body);}
.nk-input input::placeholder{color:var(--text-faint);}
.nk-input--money input{font-family:var(--font-money);font-variant-numeric:tabular-nums;text-align:right;}
.nk-input__affix{color:var(--text-faint);font-size:var(--fs-sm);display:inline-flex;align-items:center;flex:none;}
.nk-input__icon{width:16px;height:16px;color:var(--text-faint);flex:none;display:inline-flex;}
.nk-input--disabled,.nk-input[disabled]{opacity:.5;pointer-events:none;}
.nk-field__hint{font-size:var(--fs-micro);color:var(--text-faint);}
.nk-field__hint--err{color:var(--danger-400);}
.nk-input--readonly{background:var(--surface-2);color:var(--text-muted);}
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
  readOnly = false,
  error = "",
  hint = "",
  disabled = false,
  className = "",
  id,
  ...rest
}) {
  useCSS();
  // useId must be called unconditionally (Rules of Hooks); fall back to the external id after.
  const autoId = React.useId();
  const fid = id || autoId;
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
          readOnly ? "nk-input--readonly" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {icon ? <span className="nk-input__icon">{icon}</span> : null}
        {prefix ? <span className="nk-input__affix">{prefix}</span> : null}
        <input id={fid} disabled={disabled} readOnly={readOnly} {...rest} />
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
