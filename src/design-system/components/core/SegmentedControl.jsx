import React from "react";

const CSS = `
.nk-seg{display:inline-flex;padding:2px;background:var(--bg-subtle);
  border-radius:var(--radius-xs);gap:2px;font-family:var(--font-sans);}
.nk-seg__opt{appearance:none;border:none;background:transparent;cursor:pointer;
  min-height:32px;padding:4px 14px;
  border-radius:calc(var(--radius-xs) - 1px);
  font-family:var(--font-sans);font-size:var(--fs-body);font-weight:var(--fw-medium);
  color:var(--text-muted);white-space:nowrap;transition:var(--t-hover);}
.nk-seg__opt:hover:not(:disabled){color:var(--text);}
.nk-seg__opt[aria-checked="true"]{background:var(--surface-selected);color:var(--primary);}
.nk-seg__opt:focus-visible{outline:none;box-shadow:var(--shadow-focus);}
.nk-seg__opt:disabled{cursor:not-allowed;opacity:0.5;}
.nk-seg--sm .nk-seg__opt{min-height:28px;padding:2px 10px;font-size:var(--fs-sm);}
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
  options = [
    { value: "dia", label: "Dia" },
    { value: "semana", label: "Semana" },
    { value: "mes", label: "Mês" },
  ],
  value = "mes",
  onChange = () => {},
  size = "md",
  className = "",
  disabled = false,
  ariaLabel,
}) {
  useCSS();

  const handleKeyDown = (e, idx) => {
    if (disabled || options.length === 0) return;
    let next;
    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
        next = (idx + 1) % options.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        next = (idx - 1 + options.length) % options.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = options.length - 1;
        break;
      default:
        return;
    }
    e.preventDefault();
    const target = options[next];
    if (!target) return;
    onChange(target.value);
    const group = e.currentTarget.parentElement;
    const radios = group && group.querySelectorAll('[role="radio"]');
    if (radios && radios[next]) radios[next].focus();
  };

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={["nk-seg", size === "sm" ? "nk-seg--sm" : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      {options.map((opt, idx) => {
        const isActive = value === opt.value;
        return (
          <button
            key={opt.value}
            role="radio"
            type="button"
            aria-checked={isActive}
            tabIndex={isActive ? 0 : -1}
            disabled={disabled}
            className="nk-seg__opt"
            onClick={() => !disabled && onChange(opt.value)}
            onKeyDown={(e) => handleKeyDown(e, idx)}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
