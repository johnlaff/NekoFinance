import { InfoPopover, type GlossaryEntry } from "./InfoPopover";

/**
 * ProvBadge — proveniência de um lançamento (como ele chegou aqui), padrão spreadsheet-aware do
 * design system. Ponto colorido + rótulo, com explicação didática no InfoPopover (o badge é o
 * próprio trigger, sem o marcador "i"). Cor nunca é sinal único — sempre acompanha a palavra.
 */
type Prov = "importado" | "manual" | "projetado" | "conciliado";

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
      body: "Ainda não aconteceu. É uma previsão que o app criou para completar o futuro. Vira real quando o lançamento de verdade chega.",
    },
  },
  conciliado: {
    label: "Conferido",
    dot: "var(--success-400)",
    entry: {
      title: "Conferido",
      body: "Já foi cruzado com o detalhe do banco e bate certinho. É o nível mais alto de confiança.",
    },
  },
};

export function ProvBadge({ provenance }: { provenance: string }) {
  const p = PROV[provenance as Prov];
  if (!p) return null;
  return (
    <InfoPopover term={p.entry} hideMarker>
      <span
        style={{
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
        }}
      >
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
