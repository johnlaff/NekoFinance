import React from "react";

// ProvBadge — proveniência de um lançamento (Da planilha · Do app · Previsto).
// Ponto colorido + rótulo em badge pill. Inclui popover educativo inline
// (sem dependência externa — a produção usa <InfoPopover>, aqui é recreação
// self-contained). Cor nunca é sinal único: sempre acompanha a palavra.

const CSS = `
.nk-prov{position:relative;display:inline-flex;}
.nk-prov__badge{display:inline-flex;align-items:center;gap:5px;height:20px;
  padding:0 8px 0 6px;border-radius:var(--radius-pill);
  background:var(--bg-subtle);border:var(--bw-hair) solid var(--border);
  font-size:var(--fs-micro);font-weight:var(--fw-medium);
  color:var(--text-muted);white-space:nowrap;cursor:default;
  font-family:var(--font-sans);}
.nk-prov__dot{width:6px;height:6px;border-radius:50%;flex:none;}
.nk-prov__tip{position:absolute;bottom:calc(100% + 6px);left:50%;
  transform:translateX(-50%);z-index:200;min-width:220px;max-width:260px;
  padding:10px 12px;border-radius:var(--radius-md);
  background:var(--surface-elevated,var(--surface));
  border:var(--bw-hair) solid var(--border-strong);
  box-shadow:var(--shadow-2,0 4px 16px rgba(0,0,0,.4));
  font-family:var(--font-sans);font-size:var(--fs-micro);
  line-height:1.5;color:var(--text-muted);pointer-events:none;}
.nk-prov__tip-title{display:block;font-size:11.5px;font-weight:600;
  color:var(--text-strong);margin-bottom:4px;}
@media (prefers-reduced-motion:no-preference){
  .nk-prov__tip{animation:nk-prov-fade 0.12s ease;}}
@keyframes nk-prov-fade{from{opacity:0;transform:translateX(-50%) translateY(3px);}
  to{opacity:1;transform:translateX(-50%) translateY(0);}}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-prov-css")) return;
    const s = document.createElement("style");
    s.id = "nk-prov-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const PROV = {
  importado: {
    label: "Da planilha",
    dot: "var(--text-faint)",
    title: "Da planilha",
    body: "Você anotou na planilha e o app leu, igualzinho. Ainda não foi conferido com o banco.",
  },
  manual: {
    label: "Do app",
    dot: "var(--info-400)",
    title: "Do app",
    body: "Você lançou aqui no app. Ele também é gravado na planilha, valor a valor.",
  },
  projetado: {
    label: "Previsto",
    dot: "var(--secondary)",
    title: "Previsto",
    body: "Ainda não aconteceu. Pode ser um compromisso que você registrou ou uma projeção automática. Vira real quando o lançamento de verdade chega.",
  },
};

export function ProvBadge({ provenance = "importado" }) {
  useCSS();
  const [open, setOpen] = React.useState(false);
  const p = PROV[provenance];
  if (!p) return null;

  return (
    <span
      className="nk-prov"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      <span
        className="nk-prov__badge"
        tabIndex={0}
        role="button"
        aria-expanded={open}
        aria-label={`Proveniência: ${p.title}. ${p.body}`}
      >
        <span
          aria-hidden="true"
          className="nk-prov__dot"
          style={{ background: p.dot }}
        />
        {p.label}
      </span>
      {open && (
        <span className="nk-prov__tip" role="tooltip" aria-hidden="true">
          <span className="nk-prov__tip-title">{p.title}</span>
          {p.body}
        </span>
      )}
    </span>
  );
}
