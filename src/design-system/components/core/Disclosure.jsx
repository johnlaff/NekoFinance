import React from "react";

const CSS = `
.nk-disc{font-family:var(--font-sans);width:100%;}

/* card variant (bare=false): own background, border, shadow */
.nk-disc:not(.nk-disc--bare){
  background:var(--surface);
  border:var(--bw-default) solid var(--border);
  border-radius:var(--radius-md);
  box-shadow:var(--shadow-1);
  overflow:hidden;
}

/* accent left border strip */
.nk-disc--ok:not(.nk-disc--bare){border-left:3px solid var(--success-500);}
.nk-disc--warn:not(.nk-disc--bare){border-left:3px solid var(--warning-500);}
.nk-disc--brass:not(.nk-disc--bare){border-left:3px solid var(--secondary);}

/* trigger button */
.nk-disc__head{
  display:flex;
  align-items:center;
  gap:10px;
  width:100%;
  padding:12px 14px;
  background:transparent;
  border:none;
  cursor:pointer;
  text-align:left;
  color:var(--text);
  font-family:inherit;
  font-size:var(--fs-body);
  line-height:var(--lh-snug);
  border-radius:inherit;
  transition:background-color var(--dur-fast) var(--ease-standard);
  min-height:var(--hit-min);
}
.nk-disc__head:hover{background:var(--surface-hover);}
.nk-disc__head:focus-visible{
  outline:none;
  box-shadow:var(--shadow-focus);
  border-radius:var(--radius-sm);
}

/* icon slot */
.nk-disc__ic{
  display:inline-flex;
  align-items:center;
  justify-content:center;
  flex:none;
  width:20px;
  height:20px;
  color:var(--text-faint);
}

/* text column */
.nk-disc__titles{
  display:flex;
  flex-direction:column;
  gap:2px;
  flex:1;
  min-width:0;
}

.nk-disc__title{
  display:flex;
  align-items:center;
  gap:8px;
  font-size:var(--fs-body);
  font-weight:var(--fw-semibold);
  color:var(--text-strong);
  white-space:nowrap;
  overflow:hidden;
  text-overflow:ellipsis;
}

.nk-disc__summary{
  font-size:var(--fs-sm);
  color:var(--text-muted);
  white-space:nowrap;
  overflow:hidden;
  text-overflow:ellipsis;
}

/* chevron */
.nk-disc__chev{
  flex:none;
  color:var(--text-faint);
  transition:transform var(--dur-base) var(--ease-standard);
  margin-left:auto;
}
@media (prefers-reduced-motion:reduce){
  .nk-disc__chev{transition:none;}
}
.nk-disc.is-open .nk-disc__chev{transform:rotate(180deg);}

/* body wrapper: grid-rows collapse trick — no height-jank */
.nk-disc__bodywrap{
  display:grid;
  grid-template-rows:0fr;
  transition:grid-template-rows var(--dur-base) var(--ease-standard);
  overflow:hidden;
}
@media (prefers-reduced-motion:reduce){
  .nk-disc__bodywrap{transition:none;}
}
.nk-disc.is-open .nk-disc__bodywrap{grid-template-rows:1fr;}

/* inner must have min-height:0 for grid trick */
.nk-disc__body{
  min-height:0;
  overflow:hidden;
}

/* bare variant: inner content gets standard padding */
.nk-disc--bare .nk-disc__body > *{
  padding-top:0;
}

/* card variant: divider + padding inside body */
.nk-disc:not(.nk-disc--bare) .nk-disc__body{
  border-top:var(--bw-hair) solid var(--border);
  padding:14px;
}

/* accent ok/warn/brass on bare — subtle tinted title text */
.nk-disc--bare.nk-disc--ok .nk-disc__title{color:var(--success-400);}
.nk-disc--bare.nk-disc--warn .nk-disc__title{color:var(--warning-400);}
.nk-disc--bare.nk-disc--brass .nk-disc__title{color:var(--secondary);}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-disc-css")) return;
    const s = document.createElement("style");
    s.id = "nk-disc-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

let _idCounter = 0;
function useStableId() {
  const [id] = React.useState(() => `nk-disc-${++_idCounter}`);
  return id;
}

function Chevron() {
  return (
    <svg
      className="nk-disc__chev"
      width={16}
      height={16}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M6 9l6 6 6-6"
        stroke="currentColor"
        strokeWidth={1.75}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Disclosure({
  title = "Detalhes da transação",
  summary,
  icon,
  accent,
  badge,
  defaultOpen = false,
  bare = true,
  children,
  className = "",
}) {
  useCSS();
  const [open, setOpen] = React.useState(defaultOpen);
  const id = useStableId();

  const classes = [
    "nk-disc",
    bare ? "nk-disc--bare" : "",
    open ? "is-open" : "",
    accent ? `nk-disc--${accent}` : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes}>
      <button
        type="button"
        className="nk-disc__head"
        aria-expanded={open}
        aria-controls={`${id}-b`}
        onClick={() => setOpen((o) => !o)}
      >
        {icon ? <span className="nk-disc__ic">{icon}</span> : null}
        <span className="nk-disc__titles">
          <span className="nk-disc__title" id={`${id}-t`}>
            {title}
            {badge}
          </span>
          {summary ? <span className="nk-disc__summary">{summary}</span> : null}
        </span>
        <Chevron />
      </button>
      <section
        className="nk-disc__bodywrap"
        id={`${id}-b`}
        aria-labelledby={`${id}-t`}
        role="region"
        {...(!open ? { inert: "" } : {})}
      >
        <div className="nk-disc__body">
          {children ?? (
            <p
              style={{
                margin: 0,
                color: "var(--text-muted)",
                fontSize: "var(--fs-sm)",
              }}
            >
              Nenhum detalhe disponível.
            </p>
          )}
        </div>
      </section>
    </div>
  );
}
