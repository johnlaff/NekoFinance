import type { CSSProperties } from "react";
import { CalendarClock } from "lucide-react";
import { getForecast, getUpcomingBills } from "../lib/api";
import { fmtDate, formatBRL } from "../lib/format";
import { useCommand } from "../lib/useCommand";
import { Money } from "../design-system/components/Money";
import { ProvBadge } from "../design-system/components/ProvBadge";
import { EmptyState } from "../design-system/components/EmptyState";
import { BalanceTrajectory } from "../design-system/components/BalanceTrajectory";
import {
  saldoBand,
  SALDO_BAND_FILL as BAND_FILL,
  SALDO_BAND_LABEL as BAND_LABEL,
  SALDO_BAND_LEGEND as BAND_LEGEND,
} from "../lib/saldoHeatmap";
import { groupByMonth } from "./horizonteData";

// Estilos estáticos hoistados do JSX: não recriam por render nem disparam o aviso de objeto de
// estilo inline exaustivo. A parte dinâmica do dia (cor da faixa + outline) entra por merge.
const MONTH_LABEL_STYLE: CSSProperties = {
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-bold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-muted)",
  padding: "var(--space-2) var(--space-3)",
  position: "sticky",
  top: 0,
};

const DAY_LIST_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "2px",
  listStyle: "none",
  margin: 0,
  padding: 0,
};

const DAY_CELL_BASE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "var(--space-3)",
  padding: "var(--space-2) var(--space-3)",
  borderRadius: "var(--radius-sm)",
  fontVariantNumeric: "tabular-nums",
};

// --- Vencimentos próximos (plano 045). Estilos hoistados (React Compiler). ---

const BILLS_TITLE_STYLE: CSSProperties = {
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "var(--ls-label)",
  textTransform: "uppercase",
  color: "var(--text-faint)",
  margin: "var(--space-6) 0 var(--space-3)",
};

const BILLS_LIST_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-2)",
  listStyle: "none",
  margin: 0,
  padding: 0,
};

const BILL_ROW_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "var(--space-3)",
  padding: "var(--space-3)",
  background: "var(--surface)",
  border: "var(--bw-hair) solid var(--border)",
  borderRadius: "var(--radius-sm)",
};

const BILL_DATE_CHIP_STYLE: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  padding: "2px 8px",
  borderRadius: "var(--radius-pill)",
  background: "var(--surface-2)",
  border: "var(--bw-hair) solid var(--border)",
  fontSize: "var(--fs-micro)",
  fontWeight: "var(--fw-medium)",
  color: "var(--text-muted)",
  whiteSpace: "nowrap",
  flex: "none",
};

const BILL_DESC_STYLE: CSSProperties = {
  flex: 1,
  fontSize: "var(--fs-sm)",
  color: "var(--text)",
  minWidth: 0,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

/** Janela do calendário de contas a vencer (dias a partir de hoje). */
const BILLS_WINDOW_DAYS = 60;

const fetchUpcomingBills = () => getUpcomingBills(BILLS_WINDOW_DAYS);

/**
 * Calendário/lista de contas a vencer (plano 045): lê os lançamentos com `due_date` nos próximos
 * {@link BILLS_WINDOW_DAYS} dias e os lista por vencimento. Conteúdo aditivo do Horizonte — NÃO
 * altera o Saldo/forecast (o vencimento é metadado consultivo; o caixa usa a data de caixa).
 */
function UpcomingBillsSection() {
  const billsQ = useCommand("get_upcoming_bills:60", fetchUpcomingBills);
  const bills = billsQ.data ?? [];

  return (
    <section aria-labelledby="horizon-bills-title">
      <h2 id="horizon-bills-title" style={BILLS_TITLE_STYLE}>
        Vencimentos próximos
      </h2>
      {bills.length === 0 ? (
        <EmptyState
          title="Nenhum vencimento nos próximos 60 dias"
          description="Lançamentos de Saída ou Cartão com uma data de vencimento aparecem aqui como lembrete de conta a pagar."
        />
      ) : (
        <ul style={BILLS_LIST_STYLE}>
          {bills.map((b) => (
            <li key={b.id} style={BILL_ROW_STYLE}>
              <span style={BILL_DATE_CHIP_STYLE}>
                <CalendarClock size={12} strokeWidth={1.75} aria-hidden="true" />
                {fmtDate(b.due_date)}
              </span>
              <span style={BILL_DESC_STYLE}>{b.description || "—"}</span>
              {b.is_projection && <ProvBadge provenance="projetado" />}
              <Money cents={-Math.abs(b.amount)} size="sm" sign="auto" />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function HorizonteScreen() {
  const forecastQ = useCommand("get_forecast", getForecast);
  const forecast = forecastQ.data ?? null;

  if (forecastQ.loading) {
    return <EmptyState variant="skeleton" skeletonRows={6} />;
  }
  if (forecastQ.error || !forecast || forecast.daily.length === 0) {
    return (
      <EmptyState
        title="Sem horizonte para projetar"
        description="O Horizonte mostra o saldo dia a dia no mesmo termômetro da planilha (verde = folga, vermelho = aperto). Para ver o futuro, lance as entradas e saídas dos próximos meses."
      />
    );
  }

  const cols = groupByMonth(forecast.daily, forecast.today);

  return (
    <div style={{ padding: "var(--space-2)" }}>
      <header style={{ marginBottom: "var(--space-6)" }}>
        <h1
          style={{
            fontSize: "var(--fs-h2)",
            fontWeight: "var(--fw-bold)",
            letterSpacing: "var(--ls-tight)",
            margin: 0,
          }}
        >
          Horizonte de saldos
        </h1>
        <p
          style={{
            color: "var(--text-muted)",
            fontSize: "var(--fs-sm)",
            margin: "var(--space-1) 0 0",
          }}
        >
          Saldo projetado dia a dia, no mesmo termômetro da planilha: quanto mais verde,
          mais folga; quanto mais vermelho, mais aperto.
        </p>
      </header>

      {/* Trajetória do saldo — a leitura principal, preenche a largura */}
      <section
        style={{
          background: "var(--surface)",
          border: "var(--bw-hair) solid var(--border)",
          borderRadius: "var(--radius-lg)",
          boxShadow: "var(--shadow-1)",
          padding: "var(--space-5) var(--space-5) var(--space-3)",
          marginBottom: "var(--space-6)",
        }}
      >
        <BalanceTrajectory daily={forecast.daily} today={forecast.today} />
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "var(--space-4)",
            marginTop: "var(--space-3)",
            paddingTop: "var(--space-3)",
            borderTop: "var(--bw-hair) solid var(--border)",
          }}
        >
          {BAND_LEGEND.map((l) => (
            <span
              key={l.band}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 7,
                fontSize: "var(--fs-sm)",
                color: "var(--text-muted)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: "var(--radius-xs)",
                  background: BAND_FILL[l.band],
                }}
              />
              {l.label}
            </span>
          ))}
        </div>
      </section>

      <h2
        style={{
          fontSize: "var(--fs-label)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--ls-label)",
          textTransform: "uppercase",
          color: "var(--text-faint)",
          margin: "0 0 var(--space-3)",
        }}
      >
        Detalhe diário
      </h2>

      {/* `<section>` rotulado (região) + `<ul>`/`<li>` por mês: semântica nativa de lista no lugar
          de role="group"/role="img". Cada dia é um item rotulado; o conteúdo visual fica aria-hidden. */}
      <section
        aria-label="Saldo projetado por dia, agrupado por mês"
        className="nk-horizon-cols"
        style={{
          display: "flex",
          gap: "var(--space-4)",
          overflowX: "auto",
          paddingBottom: "var(--space-4)",
        }}
      >
        {cols.map((col) => (
          <div key={col.ym} style={{ minWidth: 140, flexShrink: 0 }}>
            <div aria-hidden="true" style={MONTH_LABEL_STYLE}>
              {col.label}
            </div>
            <ul aria-label={col.label} style={DAY_LIST_STYLE}>
              {col.days.map((d) => {
                const cellStyle: CSSProperties = {
                  ...DAY_CELL_BASE,
                  background: BAND_FILL[saldoBand(d.balance)],
                  outline: d.isToday ? "2px solid var(--border-focus)" : "none",
                };
                return (
                  <li
                    key={d.day}
                    aria-current={d.isToday ? "date" : undefined}
                    aria-label={`Dia ${d.day}: saldo ${formatBRL(d.balance)} (${BAND_LABEL[saldoBand(d.balance)]})`}
                    style={cellStyle}
                  >
                    {/* --text (não --text-muted): garante >=4.5:1 sobre TODAS as faixas-fundo do
                        heatmap (as faixas verdes/vermelhas fortes derrubavam o muted < AA). */}
                    <span
                      aria-hidden="true"
                      style={{
                        fontSize: "var(--fs-sm)",
                        color: "var(--text)",
                        width: 22,
                      }}
                    >
                      {d.day}
                    </span>
                    {/* `sign="none"` + --text: o saldo herda alto contraste (>=4.5:1 em TODAS as
                        faixas). O sinal +/− já é carregado pela COR da faixa-fundo; colorir o número
                        por cima caía abaixo de AA nas faixas fortes (vermelho/verde 0.3+). */}
                    <span aria-hidden="true" style={{ color: "var(--text)" }}>
                      <Money cents={d.balance} size="sm" sign="none" />
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </section>

      <UpcomingBillsSection />
    </div>
  );
}
