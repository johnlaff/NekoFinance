import { useState } from "react";
import type { CSSProperties } from "react";
import { CircleGauge, ListChecks, Plus, Trash2 } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Money } from "../design-system/components/Money";
import {
  acceptCeilingProposal,
  dismissCeilingProposal,
  getCeilingProposal,
  getDailyBudget,
  isTauri,
  upsertDailyBudgetWithCategories,
  type CeilingProposal,
  type DailyBudget,
} from "../lib/api";
import { fmtBRL } from "../lib/nkFormat";
import { parseBRLToCents } from "../lib/format";
import { invalidateCommands, useCommand } from "../lib/useCommand";

// A cerimônia do teto: itens mensais do gasto variável ÷ divisor de dias = teto/dia. O teto só
// vira veredito por gesto explícito do dono — a proposta importada da planilha é um banner de
// confirmação, nunca escrita silenciosa. Estilos içados (convenção do arquivo: nunca inline no
// JSX para não recriar objetos por render).
const FIELD: CSSProperties = {
  background: "var(--surface-2)",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-md)",
  color: "var(--text)",
  font: "inherit",
  padding: "8px 10px",
  width: "100%",
};

const FIELD_AMOUNT: CSSProperties = {
  ...FIELD,
  fontVariantNumeric: "tabular-nums",
  textAlign: "right",
  width: 120,
};

const FIELD_DIVISOR: CSSProperties = {
  ...FIELD,
  fontVariantNumeric: "tabular-nums",
  textAlign: "right",
  width: 76,
};

const ITEM_ROW: CSSProperties = {
  alignItems: "center",
  display: "flex",
  gap: 8,
};

const LIST: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  listStyle: "none",
  margin: 0,
  padding: 0,
};

const DERIVED_LINE: CSSProperties = {
  alignItems: "baseline",
  borderTop: "1px solid var(--border)",
  display: "flex",
  gap: 8,
  justifyContent: "space-between",
  marginTop: 12,
  paddingTop: 12,
};

const PROPOSAL_ITEMS: CSSProperties = {
  color: "var(--text-muted)",
  fontSize: 12.5,
  margin: "6px 0 0",
  paddingLeft: 18,
};

const ACTIONS_ROW: CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 8,
  marginTop: 12,
};

const REMOVE_BTN: CSSProperties = {
  alignItems: "center",
  background: "none",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-md)",
  color: "var(--text-muted)",
  cursor: "pointer",
  display: "inline-flex",
  justifyContent: "center",
  minHeight: 36,
  minWidth: 36,
};

const HINT: CSSProperties = {
  color: "var(--text-faint)",
  fontSize: 12.5,
  margin: "8px 0 0",
};

const CEREMONY_TERM = {
  title: "Cerimônia do teto",
  body: "Liste o que o mês variável comporta por categoria, some e divida pelos dias. O resultado é o teto diário estipulado — o número que o dia inteiro respeita.",
};

interface DraftItem {
  name: string;
  amountText: string;
}

function draftFromBudget(budget: DailyBudget): DraftItem[] {
  return budget.categories.map((c) => ({
    name: c.name,
    amountText: (c.amount_cents / 100).toLocaleString("pt-BR", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }),
  }));
}

export function TetoScreen() {
  const budgetQ = useCommand("get_daily_budget", getDailyBudget);
  const proposalQ = useCommand("get_ceiling_proposal", getCeilingProposal);
  const budget = budgetQ.data;

  return (
    <div className="teto neko-app">
      {proposalQ.data ? <ProposalBanner proposal={proposalQ.data} /> : null}
      {budget ? (
        <TetoEditor initial={budget} />
      ) : (
        <EmptyState variant="skeleton" skeletonRows={5} />
      )}
    </div>
  );
}

function ProposalBanner({ proposal }: { proposal: CeilingProposal }) {
  const [busy, setBusy] = useState(false);

  function resolve(action: "accept" | "dismiss") {
    if (!isTauri) return;
    setBusy(true);
    const call =
      action === "accept"
        ? acceptCeilingProposal(proposal.id)
        : dismissCeilingProposal(proposal.id);
    call
      .then(() => invalidateCommands())
      // eslint-disable-next-line @typescript-eslint/no-empty-function
      .catch(() => {})
      .finally(() => setBusy(false));
  }

  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <ListChecks size={16} strokeWidth={1.75} className="ic" />
          Proposta da sua planilha
        </span>
      </div>
      <div className="card__body">
        <p style={{ margin: 0 }}>
          A planilha documenta uma cerimônia de teto em notas do Diário:{" "}
          <strong>
            <Money cents={proposal.per_day_cents} size="inherit" /> por dia
          </strong>{" "}
          ({fmtBRL(proposal.items.reduce((s, i) => s + i.amount_cents, 0))} ÷{" "}
          {proposal.divisor_days} dias, anotada em {proposal.source_month}).
        </p>
        {proposal.items.length > 0 && (
          <ul style={PROPOSAL_ITEMS}>
            {proposal.items.map((it) => (
              <li key={it.name + it.amount_cents}>
                {it.name} — <Money cents={it.amount_cents} size="inherit" />
              </li>
            ))}
          </ul>
        )}
        <div style={ACTIONS_ROW}>
          <Button variant="primary" onClick={() => resolve("accept")} disabled={busy}>
            Usar este teto
          </Button>
          <Button variant="ghost" onClick={() => resolve("dismiss")} disabled={busy}>
            Agora não
          </Button>
        </div>
        <p style={HINT}>
          Nada é gravado sem a sua confirmação — o teto só vira veredito quando você escolhe.
        </p>
      </div>
    </section>
  );
}

function TetoEditor({ initial }: { initial: DailyBudget }) {
  const hasCeremony = initial.categories.length > 0 || initial.divisor_days != null;
  const [mode, setMode] = useState<"items" | "direct">(
    initial.per_day_cents > 0 && !hasCeremony ? "direct" : "items",
  );
  const [items, setItems] = useState<DraftItem[]>(() => draftFromBudget(initial));
  const [divisorText, setDivisorText] = useState(String(initial.divisor_days ?? 30));
  const [directText, setDirectText] = useState(
    initial.per_day_cents > 0 && !hasCeremony
      ? (initial.per_day_cents / 100).toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        })
      : "",
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const guided = initial.per_day_cents === 0 && initial.categories.length === 0;
  const divisor = Number.parseInt(divisorText, 10);
  const monthlyTotal = items.reduce(
    (sum, it) => sum + (parseBRLToCents(it.amountText) ?? 0),
    0,
  );
  const perDayFromItems =
    Number.isFinite(divisor) && divisor > 0 ? Math.floor(monthlyTotal / divisor) : 0;

  function save() {
    if (!isTauri) return;
    setError(null);
    if (mode === "direct") {
      const cents = parseBRLToCents(directText);
      if (!cents || cents <= 0) {
        setError("Informe um valor por dia maior que zero.");
        return;
      }
      persist(cents, [], null);
      return;
    }
    const categories = items
      .map((it, position) => ({
        name: it.name.trim(),
        amount_cents: parseBRLToCents(it.amountText) ?? 0,
        position,
      }))
      .filter((c) => c.name !== "" || c.amount_cents > 0);
    if (categories.some((c) => c.name === "" || c.amount_cents <= 0)) {
      setError("Cada categoria precisa de nome e valor mensal maior que zero.");
      return;
    }
    if (categories.length === 0) {
      setError("Liste ao menos uma categoria mensal, ou use o valor direto.");
      return;
    }
    if (!Number.isFinite(divisor) || divisor <= 0) {
      setError("O divisor de dias precisa ser maior que zero.");
      return;
    }
    if (perDayFromItems <= 0) {
      setError("A soma mensal dividida pelos dias precisa resultar em um teto maior que zero.");
      return;
    }
    persist(perDayFromItems, categories, divisor);
  }

  function persist(
    perDay: number,
    categories: { name: string; amount_cents: number; position: number }[],
    div: number | null,
  ) {
    setSaving(true);
    upsertDailyBudgetWithCategories(perDay, categories, div)
      .then(() => invalidateCommands())
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setSaving(false));
  }

  function removeCeiling() {
    if (!isTauri) return;
    setSaving(true);
    upsertDailyBudgetWithCategories(0, [], null)
      .then(() => {
        setItems([]);
        setDirectText("");
        invalidateCommands();
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setSaving(false));
  }

  return (
    <section className="card">
      <div className="card__head">
        <span className="card__title">
          <CircleGauge size={16} strokeWidth={1.75} className="ic" />
          <InfoPopover term={CEREMONY_TERM} hideMarker>
            Teto do diário
          </InfoPopover>
        </span>
        {initial.per_day_cents > 0 && (
          <span style={{ color: "var(--text-muted)", fontSize: 12.5 }}>
            Hoje: <Money cents={initial.per_day_cents} size="inherit" /> por dia
          </span>
        )}
      </div>
      <div className="card__body">
        {guided && (
          <p style={{ margin: "0 0 12px", color: "var(--text-muted)" }}>
            Você ainda não estipulou um teto. A cerimônia é simples: liste o que o mês
            variável comporta por categoria, some e divida pelos dias — ou informe um valor
            direto por dia.
          </p>
        )}

        <div className="ci-types" role="radiogroup" aria-label="Como estipular o teto">
          <button
            type="button"
            role="radio"
            aria-checked={mode === "items"}
            className="ci-type"
            onClick={() => setMode("items")}
          >
            Por itens (cerimônia)
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={mode === "direct"}
            className="ci-type"
            onClick={() => setMode("direct")}
          >
            Valor direto
          </button>
        </div>

        {mode === "items" ? (
          <>
            <ul style={LIST}>
              {items.map((it, i) => (
                // Índice como chave: a lista é um draft posicional (sem identidade própria).
                <li key={i} style={ITEM_ROW}>
                  <input
                    style={FIELD}
                    aria-label={`Nome da categoria ${i + 1}`}
                    placeholder="Categoria (ex.: alimentação)"
                    value={it.name}
                    onChange={(e) =>
                      setItems((prev) =>
                        prev.map((p, j) => (j === i ? { ...p, name: e.target.value } : p)),
                      )
                    }
                  />
                  <input
                    style={FIELD_AMOUNT}
                    inputMode="decimal"
                    aria-label={`Valor mensal da categoria ${i + 1}`}
                    placeholder="R$ mensal"
                    value={it.amountText}
                    onChange={(e) =>
                      setItems((prev) =>
                        prev.map((p, j) =>
                          j === i ? { ...p, amountText: e.target.value } : p,
                        ),
                      )
                    }
                  />
                  <button
                    type="button"
                    style={REMOVE_BTN}
                    aria-label={`Remover categoria ${i + 1}`}
                    onClick={() => setItems((prev) => prev.filter((_, j) => j !== i))}
                  >
                    <Trash2 size={14} strokeWidth={1.75} aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
            <div style={ACTIONS_ROW}>
              <Button
                variant="ghost"
                onClick={() => setItems((prev) => [...prev, { name: "", amountText: "" }])}
              >
                <Plus size={14} strokeWidth={1.75} />
                Adicionar categoria
              </Button>
            </div>
            <div style={DERIVED_LINE}>
              <span style={{ color: "var(--text-muted)" }}>
                Soma mensal <Money cents={monthlyTotal} size="inherit" /> ÷{" "}
                <input
                  style={FIELD_DIVISOR}
                  inputMode="numeric"
                  aria-label="Divisor de dias"
                  value={divisorText}
                  onChange={(e) => setDivisorText(e.target.value)}
                />{" "}
                dias
              </span>
              <span>
                Teto: <Money cents={perDayFromItems} size="inherit" /> por dia
              </span>
            </div>
          </>
        ) : (
          <div style={ITEM_ROW}>
            <label htmlFor="teto-direct" style={{ color: "var(--text-muted)" }}>
              Teto por dia (R$)
            </label>
            <input
              id="teto-direct"
              style={FIELD_AMOUNT}
              inputMode="decimal"
              value={directText}
              onChange={(e) => setDirectText(e.target.value)}
            />
          </div>
        )}

        {error && (
          <p role="alert" style={{ color: "var(--danger-400)", fontSize: 12.5 }}>
            {error}
          </p>
        )}

        <div style={ACTIONS_ROW}>
          <Button variant="primary" onClick={save} disabled={saving}>
            Salvar teto
          </Button>
          {initial.per_day_cents > 0 && (
            <Button variant="ghost" onClick={removeCeiling} disabled={saving}>
              Remover teto
            </Button>
          )}
        </div>
        <p style={HINT}>
          O teto orienta o velocímetro do dia e o forecast dos dias futuros. No modo cartão
          ele permanece visível como referência.
        </p>
      </div>
    </section>
  );
}
