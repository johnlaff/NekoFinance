import { InfoPopover, type GlossaryEntry } from "./InfoPopover";

/**
 * ProvBadge — proveniência de um lançamento (como ele chegou aqui), padrão spreadsheet-aware do
 * design system. Ponto colorido + rótulo, com explicação didática no InfoPopover (o badge é o
 * próprio trigger, sem o marcador "i"). Cor nunca é sinal único — sempre acompanha a palavra.
 */
// Nota: "conciliado/Conferido" foi removido até existir persistência de reconciliação. Hoje a
// proveniência é DERIVADA (projetado/importado/manual) em recent_transactions; o gate de conflito
// não grava um estado "conferido". Reintroduzir quando houver uma coluna/flag de reconciliação.
type Prov = "importado" | "manual" | "projetado";

const PROV: Record<Prov, { label: string; dot: string; entry: GlossaryEntry }> = {
  importado: {
    label: "Da planilha",
    dot: "var(--text-faint)",
    entry: {
      title: "Da planilha",
      body: "Você anotou na planilha e o app leu, igualzinho. Ainda não foi conferido com o banco.",
    },
  },
  manual: {
    label: "Do app",
    dot: "var(--info-400)",
    entry: {
      title: "Do app",
      body: "Você lançou aqui no app. Ele também é gravado na planilha, valor a valor.",
    },
  },
  projetado: {
    label: "Previsto",
    dot: "var(--secondary)",
    entry: {
      title: "Previsto",
      body: "Ainda não aconteceu. Pode ser um compromisso que você registrou ou uma projeção automática. Vira real quando o lançamento de verdade chega.",
    },
  },
};

const PROV_BADGE_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  height: 20,
  padding: "0 8px 0 6px",
  borderRadius: "var(--radius-pill)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border)",
  fontSize: "var(--fs-micro)",
  fontWeight: "var(--fw-medium)",
  color: "var(--text-muted)",
  whiteSpace: "nowrap",
};

export function ProvBadge({ provenance }: { provenance: string }) {
  const p = PROV[provenance as Prov];
  if (!p) return null;
  return (
    <InfoPopover term={p.entry} hideMarker>
      <span style={PROV_BADGE_STYLE}>
        <span
          aria-hidden="true"
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            flex: "none",
            background: p.dot,
          }}
        />
        {p.label}
      </span>
    </InfoPopover>
  );
}
