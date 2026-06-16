import { useRef, useState } from "react";
import { createTag, tagTotalsForMonth } from "../lib/api";
import { monthNamePtBR } from "../lib/format";
import { useCommand, invalidateCommands } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";

// Cor + nome humano (o aria-label precisa de um nome legível, não da string do token CSS).
const PALETTE: { value: string; name: string }[] = [
  { value: "var(--cat-jade)", name: "Verde" },
  { value: "var(--cat-sky)", name: "Azul" },
  { value: "var(--cat-orchid)", name: "Orquídea" },
  { value: "var(--cat-violet)", name: "Violeta" },
  { value: "var(--cat-teal)", name: "Turquesa" },
  { value: "var(--cat-amber)", name: "Âmbar" },
  { value: "var(--cat-coral)", name: "Coral" },
];

export function TagsScreen() {
  const now = new Date();
  const year = now.getFullYear();
  const month = now.getMonth() + 1;
  const [reload, setReload] = useState(0);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [emoji, setEmoji] = useState("");
  const [color, setColor] = useState(PALETTE[0]!.value);
  // Padrão WAI-ARIA radiogroup: roving tabindex (só o selecionado é tabbable) + setas navegam.
  const swatchRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const onSwatchKey = (e: React.KeyboardEvent, i: number) => {
    const last = PALETTE.length - 1;
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = i === last ? 0 : i + 1;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp")
      next = i === 0 ? last : i - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    if (next === null) return;
    e.preventDefault();
    setColor(PALETTE[next]!.value);
    swatchRefs.current[next]?.focus();
  };

  const totalsQ = useCommand(`tag_totals:${year}-${month}:${reload}`, () =>
    tagTotalsForMonth(year, month),
  );
  const tags = totalsQ.data ?? [];

  async function submit() {
    const trimmed = name.trim();
    if (!trimmed) return;
    await createTag(trimmed, color, emoji.trim() || null, trimmed.startsWith("!"));
    invalidateCommands();
    setName("");
    setEmoji("");
    setOpen(false);
    setReload((r) => r + 1);
  }

  return (
    <div style={{ maxWidth: 720, margin: "0 auto", padding: "var(--space-2)" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-4)",
          marginBottom: "var(--space-6)",
        }}
      >
        <div>
          <h1
            style={{
              fontSize: "var(--fs-h2)",
              fontWeight: "var(--fw-bold)",
              letterSpacing: "var(--ls-tight)",
              margin: 0,
            }}
          >
            Tags
          </h1>
          <p
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              margin: "var(--space-1) 0 0",
            }}
          >
            Totais de {monthNamePtBR(`${year}-${String(month).padStart(2, "0")}-01`)} de{" "}
            {year}. Rótulos livres para diagnóstico; "! Pagar" e similares ficam no
            topo.
          </p>
        </div>
        <Button onClick={() => setOpen((o) => !o)}>
          {open ? "Cancelar" : "Nova tag"}
        </Button>
      </header>

      {open ? (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-4)",
            padding: "var(--space-6)",
            marginBottom: "var(--space-6)",
            background: "var(--surface)",
            border: "var(--bw-hair) solid var(--border)",
            borderRadius: "var(--radius-md)",
          }}
        >
          <div style={{ display: "flex", gap: "var(--space-3)", flexWrap: "wrap" }}>
            <input
              aria-label="Nome da tag"
              placeholder="Nome (ex.: Viagem, ! Pagar)"
              value={name}
              onChange={(e) => setName(e.target.value)}
              style={inputStyle}
            />
            <input
              aria-label="Emoji da tag"
              placeholder="Emoji"
              value={emoji}
              onChange={(e) => setEmoji(e.target.value)}
              style={{ ...inputStyle, width: 80 }}
            />
          </div>
          <div
            role="radiogroup"
            aria-label="Cor da tag"
            style={{ display: "flex", gap: "var(--space-2)" }}
          >
            {PALETTE.map((c, i) => (
              <button
                key={c.value}
                ref={(el) => {
                  swatchRefs.current[i] = el;
                }}
                type="button"
                role="radio"
                aria-checked={color === c.value}
                aria-label={c.name}
                tabIndex={color === c.value ? 0 : -1}
                onClick={() => setColor(c.value)}
                onKeyDown={(e) => onSwatchKey(e, i)}
                style={{
                  width: 24,
                  height: 24,
                  borderRadius: "50%",
                  background: c.value,
                  border:
                    color === c.value
                      ? "2px solid var(--text)"
                      : "2px solid transparent",
                  cursor: "pointer",
                }}
              />
            ))}
          </div>
          <div>
            <Button onClick={() => void submit()}>Criar tag</Button>
          </div>
        </div>
      ) : null}

      {totalsQ.loading ? (
        <div style={{ color: "var(--text-muted)" }}>Carregando tags…</div>
      ) : tags.length === 0 ? (
        <EmptyState
          title="Nenhuma tag ainda"
          description='Crie tags livres (com emoji e cor) para marcar lançamentos, como "! Pagar", "Viagem", "Delivery".'
        />
      ) : (
        <ul
          style={{
            listStyle: "none",
            margin: 0,
            padding: 0,
            display: "flex",
            flexDirection: "column",
            gap: "2px",
          }}
        >
          {tags.map((t) => (
            <li
              key={t.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
                padding: "var(--space-4) var(--space-3)",
                borderBottom: "var(--bw-hair) solid var(--border)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 14,
                  height: 22,
                  borderRadius: "3px 6px 6px 3px",
                  background: t.color,
                  flexShrink: 0,
                }}
              />
              {t.emoji ? <span aria-hidden="true">{t.emoji}</span> : null}
              <span
                style={{
                  flex: 1,
                  fontWeight: t.is_special ? "var(--fw-bold)" : "var(--fw-semibold)",
                  color: "var(--text)",
                }}
              >
                {t.name}
              </span>
              <Money cents={t.total_cents} size="sm" />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 160,
  padding: "var(--space-3) var(--space-4)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-hair) solid var(--border)",
  background: "var(--surface-2)",
  color: "var(--text)",
  fontSize: "var(--fs-body)",
  fontFamily: "var(--font-sans)",
};
