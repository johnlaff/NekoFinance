import { useState } from "react";
import { CalendarRange } from "lucide-react";
import { getMonthGrid } from "../../lib/api";
import { fmtDayMonth, monthNamePtBR } from "../../lib/format";
import { useCommand } from "../../lib/useCommand";
import { Money } from "../../design-system/components/Money";
import { MonthNav } from "../../design-system/components/MonthNav";
import { EmptyState } from "../../design-system/components/EmptyState";
import { saldoBand, SALDO_BAND_FILL, SALDO_BAND_LABEL } from "../../lib/saldoHeatmap";

/** Soma os fluxos da grade para o rodapé do mês (ENTRADAS | SAÍDAS | DIÁRIO → Saída Total).
 * Uma única passada (era 3 reduces sobre a mesma lista). */
function footerOf(
  grid: { income_cents: number; fixed_out_cents: number; daily_out_cents: number }[],
) {
  const t = grid.reduce(
    (a, d) => {
      a.income += d.income_cents;
      a.fixed += d.fixed_out_cents;
      a.daily += d.daily_out_cents;
      return a;
    },
    { income: 0, fixed: 0, daily: 0 },
  );
  const saidaTotal = t.fixed + t.daily;
  return { ...t, saidaTotal, performance: t.income - saidaTotal };
}

/** Próximo/anterior "YYYY-MM". */
function shiftYm(ym: string, delta: number): string {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(Date.UTC(y!, m! - 1 + delta, 1));
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

/**
 * Página do mês fiel à planilha: a grade Data | Entrada | Saída | Diário | Saldo de QUALQUER mês
 * (com seletor), o termômetro na coluna Saldo, e o rodapé ENTRADAS | SAÍDAS | DIÁRIO → Saída Total →
 * Performance. Diferente da projeção do dashboard, mostra também os dias já passados do mês.
 */
export function MonthLedgerCard({
  today,
  reloadKey = 0,
}: {
  today: string;
  /** Muda quando o dashboard é invalidado (ex.: após um check-in diário) para forçar o refetch da
   * grade — a chave do useCommand é estável por mês, então sem isto o "Dia a dia" ficava stale. */
  reloadKey?: number;
}) {
  const todayYm = today.slice(0, 7);
  const [ym, setYm] = useState(todayYm);
  const [year, month] = ym.split("-").map(Number);
  const monthName = monthNamePtBR(`${ym}-01`);
  const monthCap = monthName.charAt(0).toUpperCase() + monthName.slice(1);
  const gridQ = useCommand(`month_grid:${ym}:${reloadKey}`, () =>
    getMonthGrid(year!, month!),
  );
  const grid = gridQ.data ?? [];
  const hasData = grid.some(
    (d) =>
      d.income_cents ||
      d.fixed_out_cents ||
      d.daily_out_cents ||
      d.balance_cents != null,
  );
  const foot = footerOf(grid);

  return (
    <div className="dash-card">
      <div className="dash-card__head">
        <span className="dash-card__title">
          <CalendarRange size={16} strokeWidth={1.75} className="dash-card__ic" />
          Dia a dia
        </span>
        <MonthNav
          label={`${monthCap} de ${year}`}
          onPrev={() => setYm((v) => shiftYm(v, -1))}
          onNext={() => setYm((v) => shiftYm(v, 1))}
          onToday={() => setYm(todayYm)}
          atToday={ym === todayYm}
          prevLabel="Mês anterior"
          nextLabel="Próximo mês"
        />
      </div>
      <div className="dash-card__body" style={{ padding: 0 }}>
        {gridQ.loading ? (
          <EmptyState variant="skeleton" skeletonRows={6} />
        ) : !hasData ? (
          <EmptyState
            variant="empty"
            title="Mês sem lançamentos"
            description="Importe sua planilha ou navegue até um mês com dados."
          />
        ) : (
          <div className="fc-scroll">
            <table className="txn-table fc-table">
              <thead>
                <tr>
                  <th scope="col">Data</th>
                  <th scope="col">Entrada</th>
                  <th
                    scope="col"
                    title="Saídas fixas e a fatura do cartão no vencimento"
                  >
                    Saída
                  </th>
                  <th scope="col">Diário</th>
                  <th scope="col">Saldo</th>
                </tr>
              </thead>
              <tbody>
                {grid.map((d) => {
                  const isToday = d.date === today;
                  return (
                    <tr key={d.date} className={isToday ? "fc-today" : ""}>
                      <td>
                        {fmtDayMonth(d.date)}
                        {isToday && <span className="fc-today__tag">hoje</span>}
                      </td>
                      <td className="money">
                        {d.income_cents ? (
                          <Money cents={d.income_cents} size="sm" sign="auto" />
                        ) : (
                          "—"
                        )}
                      </td>
                      <td className="money">
                        {d.fixed_out_cents ? (
                          <Money cents={d.fixed_out_cents} size="sm" />
                        ) : (
                          "—"
                        )}
                      </td>
                      <td className="money">
                        {d.daily_out_cents ? (
                          <Money cents={d.daily_out_cents} size="sm" />
                        ) : (
                          "—"
                        )}
                      </td>
                      {/* Saldo com o termômetro da planilha (dias não importados ficam neutros). */}
                      {d.balance_cents == null ? (
                        <td className="money" style={{ color: "var(--text-faint)" }}>
                          —
                        </td>
                      ) : (
                        <td
                          className="money"
                          style={{
                            background: SALDO_BAND_FILL[saldoBand(d.balance_cents)],
                            color: "var(--text)",
                          }}
                          title={`Saldo ${SALDO_BAND_LABEL[saldoBand(d.balance_cents)]}`}
                        >
                          <Money cents={d.balance_cents} size="sm" sign="none" />
                        </td>
                      )}
                    </tr>
                  );
                })}
              </tbody>
              {/* Rodapé do mês, fiel ao da planilha (linhas 37–44): somas + Saída Total + Resultado do mês. */}
              <tfoot>
                <tr className="fc-foot">
                  <th scope="row">Total</th>
                  <td className="money">
                    <Money cents={foot.income} size="sm" sign="auto" />
                  </td>
                  <td className="money">
                    <Money cents={foot.fixed} size="sm" />
                  </td>
                  <td className="money">
                    <Money cents={foot.daily} size="sm" />
                  </td>
                  <td className="money" aria-label="Saldo não se aplica ao total">
                    —
                  </td>
                </tr>
                <tr className="fc-foot">
                  <th scope="row">Saída Total</th>
                  <td className="money" colSpan={3}>
                    <span
                      style={{
                        color: "var(--text-faint)",
                        fontSize: "var(--fs-micro)",
                      }}
                    >
                      saídas + diário
                    </span>
                  </td>
                  <td className="money">
                    <Money cents={foot.saidaTotal} size="sm" />
                  </td>
                </tr>
                <tr className="fc-foot">
                  <th
                    scope="row"
                    title="Resultado contábil do mês na grade: entradas menos saída total (fixas + diário). Diferente da Performance do método (Totais), que também desconta Economia e a projeção do diário restante."
                  >
                    Resultado do mês
                  </th>
                  <td className="money" colSpan={3}>
                    <span
                      style={{
                        color: "var(--text-faint)",
                        fontSize: "var(--fs-micro)",
                      }}
                    >
                      entradas − saída total
                    </span>
                  </td>
                  <td className="money">
                    <Money cents={foot.performance} size="sm" sign="auto" />
                  </td>
                </tr>
              </tfoot>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
