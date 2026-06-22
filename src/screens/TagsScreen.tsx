import "./tags.css";
import { Tags, Sparkles } from "lucide-react";
import { tagTotalsForMonth, isTauri } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { fmtBRL, MES } from "../lib/nkFormat";

/** Palette mirrors the prototype's `palette` array in ScreenTags. */
const PALETTE = [
  "var(--cat-jade)",
  "var(--cat-sky)",
  "var(--cat-orchid)",
  "var(--cat-amber)",
  "var(--cat-coral)",
  "var(--cat-teal)",
  "var(--cat-violet)",
];

/** Stable fetcher for the current month — created once per mount to satisfy useCommand's
 *  referential-stability requirement (see useCommand JSDoc). */
function makeTagFetcher(year: number, month: number) {
  return () => tagTotalsForMonth(year, month);
}

export function TagsScreen() {
  const now = new Date();
  const year = now.getFullYear();
  const month = now.getMonth() + 1; // 1-based
  const monthIndex = now.getMonth(); // 0-based for MES[]

  // Stable fetcher: created with stable year/month values captured at module scope of
  // this render. Because TagsScreen never changes year/month (it always shows the
  // current month), the fetcher reference is stable across re-renders.
  const fetcher = makeTagFetcher(year, month);
  const key = `tag_totals:${year}-${String(month).padStart(2, "0")}`;
  const totalsQ = useCommand(key, fetcher);

  // Sort descending by total_cents (highest spend first), matching the prototype's
  // sort((a, b) => b[1] - a[1]).
  const sorted = (totalsQ.data ?? [])
    .slice()
    .sort((a, b) => b.total_cents - a.total_cents);

  const max = sorted.length > 0 ? sorted[0]!.total_cents : 1;
  const grand = sorted.reduce((s, t) => s + t.total_cents, 0);

  return (
    <div className="xs">
      <div className="xs-title">Tags · {MES[monthIndex]}</div>

      <section className="card">
        <div className="card__head">
          <span className="card__title">
            <Tags size={16} strokeWidth={1.75} className="ic" />
            Gasto por tag
          </span>
          <span
            style={{
              fontFamily: "var(--font-money)",
              fontSize: 12.5,
              color: "var(--text-faint)",
            }}
          >
            Total {fmtBRL(grand)}
          </span>
        </div>

        <div className="card__body">
          {totalsQ.loading ? (
            /* Loading skeleton — quiet, no flash */
            <p style={{ color: "var(--text-faint)", fontSize: 13 }}>Carregando…</p>
          ) : sorted.length === 0 ? (
            <p style={{ color: "var(--text-faint)", fontSize: 13 }}>
              {isTauri
                ? "Sem tags classificadas neste mês."
                : "Preview web — abra o app desktop para ver seus dados."}
            </p>
          ) : (
            sorted.map((tag, i) => {
              const color = PALETTE[i % PALETTE.length]!;
              const pct = max > 0 ? (tag.total_cents / max) * 100 : 0;
              return (
                <div className="tg-row" key={tag.id}>
                  <span
                    className="tg-dot"
                    style={{ background: color }}
                    aria-hidden="true"
                  />
                  <span className="tg-name" title={tag.name}>
                    {tag.emoji ? `${tag.emoji} ` : ""}
                    {tag.name}
                  </span>
                  <span className="tg-track" aria-hidden="true">
                    <span
                      className="tg-fill"
                      style={{ width: `${pct}%`, background: color }}
                    />
                  </span>
                  <span className="tg-amt">{fmtBRL(tag.total_cents)}</span>
                </div>
              );
            })
          )}
        </div>
      </section>

      <p
        style={{
          fontSize: 12.5,
          color: "var(--text-faint)",
          display: "flex",
          alignItems: "center",
          gap: 7,
          margin: 0,
        }}
      >
        <Sparkles size={14} strokeWidth={1.75} />
        Tags classificam um lançamento. Uma saída pode ter várias, e a Mia sugere tags
        ao importar.
      </p>
    </div>
  );
}
