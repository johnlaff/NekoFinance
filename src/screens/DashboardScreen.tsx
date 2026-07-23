import { useState } from "react";
import { CalendarRange, Clock3, CreditCard, Landmark, TrendingUp } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { ModeChip } from "../design-system/components/ModeChip";
import { Money } from "../design-system/components/Money";
import { NekoMark } from "../design-system/components/NekoMark";
import { NoRecordDash } from "../design-system/components/NoRecordDash";
import {
  getDashboardSummary,
  getForecast,
  getUpcomingBills,
  isTauri,
  listCards,
  type DashboardSummary,
  type Forecast,
  type UpcomingBill,
  type UpcomingInvoice,
} from "../lib/api";
import { invalidateCommands, useCommand } from "../lib/useCommand";
import { MES, monthOf, saldoBand } from "../lib/nkFormat";
import { currentMonthMetric } from "./totaisStatus";
import {
  dueLabel,
  faturaDayLabel,
  greetingForHour,
  joinNames,
  monthInsight,
  openInvoicesView,
  saldoBandPhrase,
  saldoGaugeFraction,
  upcomingIncome,
  type MonthInsight,
  type OpenInvoicesView,
} from "./hojeView";
import { useNekoApp } from "../shell/appContext";
import "./hoje.css";

// Didáticas dos estados epistêmicos (copy conceitual fixa; o resto da UI mostra dado derivado).
const TETO_ESTIMATE_TERM = {
  title: "Teto estimado",
  body: "Você ainda não estipulou um teto: este é o Diário médio do mês anterior, exibido como estimativa. Estipule o seu na cerimônia do teto para virar veredito.",
};
const RESERVE_ESTIMATE_TERM = {
  title: "Retrato vivo",
  body: "Ainda há poucos meses completos de custo de vida: a régua usa o retrato do que existe, como estimativa. Com 6 meses completos ela vira veredito.",
};
const RESERVE_NONE_TERM = {
  title: "Reserva sem registro",
  body: "Nenhuma conta de reserva mapeada (ou nenhum mês de custo de vida ainda). Marque nos bolsos qual conta é a sua reserva para a régua existir.",
};
const RESERVE_ZERO_TERM = {
  title: "Sem reserva",
  body: "Suas contas de reserva estão zeradas. A reserva é a fundação do método: o próximo passo é o ritual de guardar antes de gastar.",
};
const TETO_NONE_TERM = {
  title: "Sem teto estipulado",
  body: "Não há teto escolhido nem histórico de Diário para estimar um. A cerimônia do teto lista o mês variável por categoria e divide pelos dias.",
};

export function DashboardScreen() {
  const { navigate } = useNekoApp();
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);
  const billsQ = useCommand("get_upcoming_bills", () => getUpcomingBills(45));
  const cardsQ = useCommand("list_cards", listCards);
  // A saudação lê o relógio UMA vez por montagem: cumprimento não muda no meio da visita.
  const [greeting] = useState(() => greetingForHour(new Date().getHours()));

  const summary = summaryQ.data;
  const forecast = forecastQ.data;
  const fetchError = summaryQ.error ?? forecastQ.error;

  // Nunca renderizar R$ 0,00 fingindo dado real: se QUALQUER uma das duas fontes do herói
  // falhou sem dado (nem cache), os números não podem ser calculados — estado de erro com
  // retry. O banner "dados antigos" fica só para quando todas as fontes têm cache.
  const summaryMissing = Boolean(summaryQ.error) && !summary;
  const forecastMissing = Boolean(forecastQ.error) && !forecast;
  if (summaryMissing || forecastMissing) {
    return (
      <div className="hoje neko-app">
        <EmptyState
          variant="error"
          title="Não foi possível carregar o painel"
          description="Os números de hoje não puderam ser calculados. Verifique o app e tente de novo."
          action={
            <Button variant="primary" onClick={() => invalidateCommands()}>
              Tentar novamente
            </Button>
          }
        />
      </div>
    );
  }

  // Primeira carga sem cache: esqueleto em vez de estados fabricados (R$ 0,00, "Sem registro"
  // e modo débito nasceriam do `undefined` antes do DTO chegar).
  if (!summary || !forecast) {
    return (
      <div className="hoje neko-app">
        <EmptyState variant="skeleton" skeletonRows={6} />
      </div>
    );
  }

  const today = forecast.today;
  const month = monthOf(today);
  // Recorte por ano-mês (prefixo ISO): só o número do mês colidiria o mesmo mês
  // de anos diferentes quando o horizonte cruza a virada.
  const monthKey = today.slice(0, 7);
  const monthDaily = forecast.daily.filter((d) => d.date.slice(0, 7) === monthKey);
  const insight = monthInsight(monthDaily, today);
  const invoices = openInvoicesView(summary.upcoming_invoices);
  const metric = currentMonthMetric(forecast.months, today);
  const nextIncome = upcomingIncome(forecast.daily, today, 45);
  const cardMode = summary.spending_mode === "card";
  const safeToSpend = Math.max(0, forecast.safe_to_spend_today_cents);
  const monthEndLabel = monthDaily.length
    ? faturaDayLabel(monthDaily[monthDaily.length - 1]!.date)
    : "o fim do mês";
  const saldoHoje =
    monthDaily.find((d) => d.date === today)?.balance_cents ?? summary.balance;

  return (
    <div className="hoje neko-app">
      {fetchError ? (
        <p role="status" className="hoje__stale">
          Não foi possível atualizar agora — mostrando os últimos dados carregados.
        </p>
      ) : null}

      <section className="hoje__greet" data-large-title aria-label="Veredito de hoje">
        <span className="hoje__greet-cat" aria-hidden="true">
          <NekoMark width={72} height={72} />
        </span>
        <h1>{greeting}</h1>
        <p className="hoje__verdict">
          Pode gastar hoje{" "}
          <span className="hoje__verdict-money">
            <Money cents={safeToSpend} size="inherit" />
          </span>
          <b>
            {forecast.binding_guardrail === "savings"
              ? "Sem tocar na economia planejada do ano."
              : "Sem deixar nenhum dia no vermelho."}
          </b>
        </p>
        <p className="hoje__teach">
          <TeachLine
            summary={summary}
            forecast={forecast}
            cardMode={cardMode}
            monthEndLabel={monthEndLabel}
            onOpenTeto={() => navigate("teto")}
          />
        </p>
      </section>

      <p className="hoje__curated">
        <span className="hoje__curated-cat" aria-hidden="true">
          <MiaAvatar width={14} height={14} />
        </span>
        A Mia separou o que importa hoje — a ordem muda com o seu dia, os números nunca.
      </p>

      <div className="hoje__grid">
        <BlockDay
          summary={summary}
          invoices={invoices}
          cardMode={cardMode}
          baselineOutflowCents={forecast.baseline_outflow_cents}
          cards={cardsQ.data ?? []}
          onSeeAll={() => navigate("lancamentos")}
          onOpenTeto={() => navigate("teto")}
        />

        {insight && <MonthInsightNote insight={insight} month={month} today={today} />}

        <section
          className="hoje__card hoje__card--moves"
          aria-labelledby="hoje-moves-title"
        >
          <header className="hoje__cardhead">
            <span className="ic" aria-hidden="true">
              <CalendarRange size={17} strokeWidth={1.75} />
            </span>
            <h2 id="hoje-moves-title">Próximos movimentos</h2>
            <button
              type="button"
              className="hoje__more"
              onClick={() => navigate("lancamentos")}
            >
              Ver tudo ›
            </button>
          </header>
          <UpcomingMoves
            bills={billsQ.data ?? []}
            nextIncome={nextIncome}
            today={today}
          />
          {metric && (
            <div className="hoje__pair">
              <div className="side">
                <div className="v">
                  <Money cents={metric.cost_of_living_cents} size="inherit" />
                </div>
                <div className="l">Custo de vida no mês</div>
              </div>
              <div className="side">
                <div className="v">
                  {(metric.savings_rate_bps / 100).toLocaleString("pt-BR", {
                    maximumFractionDigits: 1,
                  })}
                  %
                </div>
                <div className="l">Guardado no mês</div>
              </div>
            </div>
          )}
        </section>

        <section
          className="hoje__card hoje__card--saldo"
          aria-labelledby="hoje-saldo-title"
        >
          <header className="hoje__cardhead">
            <span className="ic" aria-hidden="true">
              <Landmark size={17} strokeWidth={1.75} />
            </span>
            <h2 id="hoje-saldo-title">Saldo e reserva</h2>
            <button
              type="button"
              className="hoje__more"
              onClick={() => navigate("mes")}
            >
              Ver o mês ›
            </button>
          </header>
          <SaldoReserva
            saldoHoje={saldoHoje}
            summary={summary}
            onMapReserve={() => navigate("config")}
          />
        </section>
      </div>

      {!isTauri && (
        <p className="hoje__preview">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}

/** Camada didática do veredito: o que é o número + o estado do teto. */
function TeachLine({
  summary,
  forecast,
  cardMode,
  monthEndLabel,
  onOpenTeto,
}: {
  summary: DashboardSummary;
  forecast: Forecast;
  cardMode: boolean;
  monthEndLabel: string;
  onOpenTeto: () => void;
}) {
  const savingsBound = forecast.binding_guardrail === "savings";
  const ceiling = summary.daily_budget;
  const source = summary.daily_ceiling_source;

  // O número é SÓ o guardrail que morde (caixa ou economia) — o teto nunca entra
  // nele; é o segundo limite do dia e a didática o apresenta como tal.
  const numberPhrase = savingsBound
    ? "Este é o limite da economia: o maior gasto que mantém a meta do ano viva — hoje é ela quem manda, não o caixa."
    : `Este é o limite do caixa: o maior gasto que o saldo aguenta até ${monthEndLabel} sem nenhum dia no vermelho.`;

  const tetoClause =
    source === "chosen" ? (
      <>
        {" "}
        O{" "}
        <button type="button" className="hoje__link" onClick={onOpenTeto}>
          teto que você estipulou
        </button>{" "}
        — <Money cents={ceiling} size="inherit" /> por dia —{" "}
        {cardMode
          ? "segue como referência."
          : "é o segundo limite do dia: vale o mais apertado dos dois."}
      </>
    ) : source === "estimate" ? (
      <>
        {" "}
        Teto de referência: <Money cents={ceiling} size="inherit" />{" "}
        <EstimateMark term={TETO_ESTIMATE_TERM} />.
      </>
    ) : summary.ceiling_proposal_pending ? (
      <>
        {" "}
        <button type="button" className="hoje__link" onClick={onOpenTeto}>
          A planilha propõe um teto — revisar.
        </button>
      </>
    ) : (
      <>
        {" "}
        O método pede um segundo limite, o teto diário que você estipula — e ele ainda
        não está definido.{" "}
        <button type="button" className="hoje__link" onClick={onOpenTeto}>
          Estipular o teto
        </button>
      </>
    );

  return (
    <>
      {numberPhrase}
      {cardMode
        ? " No cartão, a compra pesa na fatura seguinte — este número protege o caixa deste mês."
        : ""}
      {tetoClause}
    </>
  );
}

/** Bloco do dia: no modo cartão o corpo são as faturas em aberto por vencimento. */
function BlockDay({
  summary,
  invoices,
  cardMode,
  baselineOutflowCents,
  cards,
  onSeeAll,
  onOpenTeto,
}: {
  summary: DashboardSummary;
  invoices: OpenInvoicesView;
  cardMode: boolean;
  baselineOutflowCents: number;
  cards: { id: string; name: string }[];
  onSeeAll: () => void;
  onOpenTeto: () => void;
}) {
  const spentToday = cardMode
    ? summary.card_spend_today_cents
    : summary.daily_spend_today;
  const openAccounts = new Set(
    invoices.groups.flatMap((g) => g.invoices.map((i) => i.account_id)),
  );
  const zeroed: string[] = [];
  for (const card of cards) {
    if (!openAccounts.has(card.id)) zeroed.push(card.name);
  }
  // O texto/aria dizem o percentual REAL (150% acima do típico é dado, não ruído);
  // só a barra satura em 100 — largura é reforço visual, nunca o número.
  const pct =
    baselineOutflowCents > 0
      ? Math.round((invoices.totalCents / baselineOutflowCents) * 100)
      : null;

  const ceiling = summary.daily_budget;
  const source = summary.daily_ceiling_source;
  const overCeiling = !cardMode && ceiling > 0 && spentToday > ceiling;
  const ciPct =
    ceiling > 0 ? Math.min(100, Math.round((spentToday / ceiling) * 100)) : 0;

  return (
    <section
      className={`hoje__card hoje__blockday ${
        cardMode && invoices.count > 0 ? "hoje__blockday--deck" : ""
      }`}
      aria-labelledby="hoje-day-title"
    >
      <header className="hoje__cardhead">
        <span className="ic" aria-hidden="true">
          <Clock3 size={17} strokeWidth={1.75} />
        </span>
        <h2 id="hoje-day-title">Gasto variável de hoje</h2>
        <button type="button" className="hoje__more" onClick={onSeeAll}>
          Ver tudo ›
        </button>
      </header>

      <p className="hoje__daytotal">
        <Money cents={spentToday} size="inherit" />
        <small>
          {cardMode
            ? spentToday > 0
              ? "— somado às faturas de hoje"
              : "— nada somado à fatura hoje"
            : spentToday > 0
              ? "— lançado no Diário hoje"
              : "— nada lançado no Diário hoje"}
        </small>
      </p>

      <div className="hoje__modo">
        <ModeChip mode={summary.spending_mode} gate={summary.card_gate} />
      </div>

      {cardMode ? (
        <>
          {invoices.count > 0 ? (
            <>
              <div className="hoje__fatura">
                <div className="hoje__fatura-head">
                  <span>
                    Faturas em aberto — {invoices.count}{" "}
                    {invoices.count === 1 ? "cartão" : "cartões"}
                  </span>
                  <span className="money">
                    <Money cents={invoices.totalCents} size="inherit" />
                  </span>
                </div>
                {pct !== null && (
                  <div
                    className="hoje__fatura-bar"
                    role="img"
                    aria-label={`Faturas em aberto somam ${pct}% do gasto típico de um mês`}
                  >
                    <i style={{ width: `${Math.min(100, pct)}%` }} />
                  </div>
                )}
                <p className="hoje__fatura-note">
                  É aqui que o seu gasto variável mora: cada compra soma na fatura do
                  cartão usado. O método manda deixá-las à vista — a fatura crescendo é
                  o velocímetro de quem gasta no crédito.
                  {pct !== null ? ` Até aqui: ${pct}% do gasto típico de um mês.` : ""}
                </p>
              </div>
              {invoices.groups.map((group) => (
                <div key={group.dueDate}>
                  <h3 className="hoje__venc">
                    {group.invoices.length === 1 ? "Vence" : "Vencem"} em{" "}
                    {faturaDayLabel(group.dueDate)}
                  </h3>
                  <ul className="hoje__rows">
                    {group.invoices.map((invoice) => (
                      <InvoiceRow
                        key={invoice.account_id}
                        invoice={invoice}
                        isLargest={invoice.account_id === invoices.largestAccountId}
                      />
                    ))}
                  </ul>
                </div>
              ))}
            </>
          ) : (
            <div className="hoje__fatura">
              <div className="hoje__fatura-head">
                <span>Cartão do mês</span>
                <span className="money">
                  <Money cents={summary.cartao_month_cents} size="inherit" />
                </span>
              </div>
              <p className="hoje__fatura-note">
                {summary.next_fatura_date ? (
                  <>
                    Próxima fatura:{" "}
                    <Money cents={summary.next_fatura_amount_cents} size="inherit" /> em{" "}
                    {faturaDayLabel(summary.next_fatura_date)}.
                  </>
                ) : (
                  <>Nenhuma fatura em aberto agora.</>
                )}
              </p>
            </div>
          )}
          {zeroed.length > 0 && (
            <p className="hoje__zerados">
              {joinNames(zeroed)} {zeroed.length === 1 ? "está" : "estão"} sem fatura em
              aberto — cartão parado sai da lista sozinho e volta quando você usar.
            </p>
          )}
          <p className="hoje__gloss">
            O Diário fica zerado de propósito: ele é para débito e Pix, que mexem o
            saldo na hora. Se um dia você migrar para o débito, o check-in do diário
            volta a governar este bloco — sozinho.
          </p>
        </>
      ) : (
        <>
          <div className="hoje__fatura-head">
            <span style={{ color: "var(--text-muted)" }}>Diário de hoje</span>
            <span className="money">
              <Money cents={spentToday} size="inherit" />
              <span style={{ color: "var(--text-faint)", fontWeight: 400 }}>
                {" "}
                /{" "}
                {source === "none" ? (
                  // Com proposta pendente o convite é ÚNICO em toda a tela: revisar.
                  <button type="button" className="hoje__link" onClick={onOpenTeto}>
                    {summary.ceiling_proposal_pending
                      ? "Proposta do teto aguardando — revisar"
                      : "Sem teto — estipular"}
                  </button>
                ) : (
                  <>
                    <Money cents={ceiling} size="inherit" />{" "}
                    {source === "estimate" && (
                      <EstimateMark term={TETO_ESTIMATE_TERM} />
                    )}
                  </>
                )}
              </span>
            </span>
          </div>
          {ceiling > 0 && (
            <div
              className="hoje__ci-track"
              role="img"
              aria-label={
                overCeiling
                  ? "Diário de hoje acima do teto"
                  : `Diário de hoje em ${ciPct}% do teto`
              }
            >
              <span
                className="hoje__ci-fill"
                style={{
                  width: `${ciPct}%`,
                  background: overCeiling ? "var(--danger-500)" : "var(--type-diario)",
                }}
              />
            </div>
          )}
          {source === "none" && !summary.ceiling_proposal_pending && (
            <p className="hoje__gloss">
              <NoRecordDash term={TETO_NONE_TERM} label="Sem teto estipulado" />
            </p>
          )}
          <p className="hoje__gloss">
            Lance o gasto de hoje para manter o saldo fiel — o registro vive no botão
            Registrar lançamento.
          </p>
        </>
      )}
    </section>
  );
}

function InvoiceRow({
  invoice,
  isLargest,
}: {
  invoice: UpcomingInvoice;
  isLargest: boolean;
}) {
  const statusContext = isLargest
    ? "A maior fatura em aberto"
    : invoice.status === "fechada"
      ? "Fechada — aguarda pagamento"
      : "Acumulando";
  // "Eu" é a pessoa-padrão do domínio: só dono de cartão adicional merece prefixo.
  const foreignOwner = invoice.owner_name && invoice.owner_name !== "Eu";
  const context = foreignOwner
    ? `De ${invoice.owner_name} · ${statusContext.charAt(0).toLowerCase()}${statusContext.slice(1)}`
    : statusContext;
  return (
    <li>
      <span className="ic" aria-hidden="true">
        <CreditCard size={18} strokeWidth={1.75} />
      </span>
      <span className="hoje__what">
        <b>
          {invoice.card_name}
          {invoice.has_refund_expectation && <i className="hoje__reemb">Reembolso</i>}
        </b>
        <small>{context}</small>
      </span>
      <span className="hoje__val">
        <Money cents={invoice.amount_cents} size="inherit" />
      </span>
    </li>
  );
}

/** Insight do mês na voz da Mia — leitura em linguagem natural da corrente de saldo. */
function MonthInsightNote({
  insight,
  month,
  today,
}: {
  insight: MonthInsight;
  month: number;
  today: string;
}) {
  const band = saldoBand(insight.endBalanceCents);
  const minDay = Number(insight.minDate.split("-")[2] ?? 0);
  const incomeDay = insight.nextIncomeDate
    ? Number(insight.nextIncomeDate.split("-")[2] ?? 0)
    : null;
  const minIsToday = insight.minDate === today;
  return (
    <aside className="hoje__insight hoje__insight--month" aria-label="Leitura da Mia">
      <span className="hoje__insight-cat" aria-hidden="true">
        <MiaAvatar width={15} height={15} />
      </span>
      <p>
        Fechando o dia assim, {(MES[month] ?? "").toLowerCase()} termina em{" "}
        <b>{band.label}</b> — saldo previsto de{" "}
        <b className={insight.endBalanceCents < 0 ? "hoje__money-neg" : undefined}>
          <Money cents={insight.endBalanceCents} size="inherit" />
        </b>
        . O ponto mais apertado do mês é{" "}
        {insight.minIsOngoing ? <b>hoje</b> : <b>dia {minDay}</b>}:{" "}
        <b className={insight.minCents < 0 ? "hoje__money-neg" : undefined}>
          <Money cents={insight.minCents} size="inherit" />
        </b>
        {insight.minIsOngoing && !minIsToday ? <> desde o dia {minDay}</> : null}
        {incomeDay !== null ? (
          <>, e a próxima entrada chega dia {incomeDay}</>
        ) : null}.{" "}
        {insight.deficitDaysAhead === 0 ? (
          <>
            Nenhum dia no vermelho à vista — no método, isso é ficar sem "buraco do
            futuro".
          </>
        ) : (
          <>
            {insight.deficitDaysAhead}{" "}
            {insight.deficitDaysAhead === 1 ? "dia fica" : "dias ficam"} no vermelho — o
            "buraco do futuro" do método. Antecipar uma entrada ou segurar o variável
            desfaz o buraco.
          </>
        )}
      </p>
    </aside>
  );
}

/** Próximos movimentos: contas a vencer + a próxima entrada prevista, por data. */
function UpcomingMoves({
  bills,
  nextIncome,
  today,
}: {
  bills: UpcomingBill[];
  nextIncome: { date: string; cents: number } | null;
  today: string;
}) {
  interface Move {
    id: string;
    name: string;
    cents: number;
    income: boolean;
    date: string;
  }
  const moves: Move[] = [
    ...bills.map((bill) => ({
      id: bill.id,
      name: bill.description || "Sem descrição",
      cents: bill.amount,
      income: false,
      date: bill.due_date,
    })),
    ...(nextIncome
      ? [
          {
            id: "hoje-next-income",
            name: "Entrada prevista",
            cents: nextIncome.cents,
            income: true,
            date: nextIncome.date,
          },
        ]
      : []),
  ]
    .sort((a, b) => a.date.localeCompare(b.date))
    .slice(0, 6);

  if (moves.length === 0) {
    return <p className="hoje__moves-empty">Nada vencendo nos próximos dias.</p>;
  }
  return (
    <ul className="hoje__moves" aria-label="Próximos movimentos">
      {moves.map((move) => {
        const due = dueLabel(today, move.date);
        return (
          <li className="hoje__move" key={move.id}>
            <span
              className={`hoje__move-logo ${move.income ? "is-income" : ""}`}
              aria-hidden="true"
            >
              {move.income ? (
                <TrendingUp size={17} strokeWidth={1.75} />
              ) : (
                <CalendarRange size={17} strokeWidth={1.75} />
              )}
            </span>
            <span className="hoje__move-nm">{move.name}</span>
            <span className={`hoje__move-vl ${move.income ? "is-income" : ""}`}>
              <Money
                cents={move.income ? move.cents : -move.cents}
                size="inherit"
                sign={move.income ? "auto" : "none"}
              />
            </span>
            <span className={`hoje__move-dt ${due.soon ? "is-soon" : ""}`}>
              {due.label}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** Par saldo + reserva, cada um com sua régua, + o insight de reserva por estado. */
function SaldoReserva({
  saldoHoje,
  summary,
  onMapReserve,
}: {
  saldoHoje: number;
  summary: DashboardSummary;
  onMapReserve: () => void;
}) {
  const band = saldoBand(saldoHoje);
  const reserveState = summary.reserve_state;
  const reserveMonths = summary.reserve_months;
  const reserveFraction = Math.max(0, Math.min(1, reserveMonths / 6));
  const reserveOk = reserveState !== "no_record" && reserveMonths >= 6;

  return (
    <>
      <div className="hoje__statpair">
        <div className="hoje__stat">
          <div className="sv">
            <Money cents={saldoHoje} size="inherit" />
          </div>
          <div className="sl">Saldo hoje</div>
          {band.key !== "none" && (
            <>
              <div className="hoje__gauge" aria-hidden="true">
                <i
                  style={{
                    width: `${saldoGaugeFraction(band.key) * 100}%`,
                    background: band.text,
                  }}
                />
              </div>
              <div className="hoje__stat-note">
                <span
                  className="dot"
                  style={{ background: band.text }}
                  aria-hidden="true"
                />
                <span>
                  Termômetro {band.label} — {saldoBandPhrase(band.key)}
                </span>
              </div>
            </>
          )}
        </div>
        <div className="hoje__stat">
          <div className="sv">
            {reserveState === "no_record" ? (
              <NoRecordDash
                term={RESERVE_NONE_TERM}
                cta={
                  <button type="button" className="hoje__link" onClick={onMapReserve}>
                    Mapear
                  </button>
                }
              />
            ) : reserveState === "zero" ? (
              <NoRecordDash term={RESERVE_ZERO_TERM} label="Sem reserva" />
            ) : (
              <>
                {reserveMonths.toLocaleString("pt-BR", {
                  minimumFractionDigits: 1,
                  maximumFractionDigits: 1,
                })}{" "}
                meses{" "}
                {reserveState === "estimate" && (
                  <EstimateMark term={RESERVE_ESTIMATE_TERM} />
                )}
              </>
            )}
          </div>
          <div className="sl">Reserva de emergência</div>
          {(reserveState === "verdict" || reserveState === "estimate") && (
            <>
              <div className="hoje__gauge" aria-hidden="true">
                <i
                  style={{
                    width: `${reserveFraction * 100}%`,
                    background: reserveOk ? "var(--ok)" : "var(--warn)",
                  }}
                />
              </div>
              <div className="hoje__stat-note">
                <span
                  className="dot"
                  style={{ background: reserveOk ? "var(--ok)" : "var(--warn)" }}
                  aria-hidden="true"
                />
                <span>
                  {reserveOk
                    ? "Acima dos 6 meses que o método pede"
                    : "O método pede 6 meses de custo de vida"}
                </span>
              </div>
            </>
          )}
        </div>
      </div>
      <ReserveInsight state={reserveState} onMapReserve={onMapReserve} />
    </>
  );
}

function ReserveInsight({
  state,
  onMapReserve,
}: {
  state: DashboardSummary["reserve_state"];
  onMapReserve: () => void;
}) {
  if (state === "verdict") return null;
  return (
    <aside className="hoje__insight" aria-label="Leitura da Mia sobre a reserva">
      <span className="hoje__insight-cat" aria-hidden="true">
        <MiaAvatar width={15} height={15} />
      </span>
      <p>
        {state === "no_record" ? (
          <>
            A planilha não informa uma reserva guardada — e saldo em conta não é
            reserva. O método pede o equivalente a <b>6 meses do custo de vida</b> num
            lugar separado; quando você{" "}
            <button type="button" className="hoje__link" onClick={onMapReserve}>
              mapear a conta
            </button>
            , o Neko acompanha aqui.
          </>
        ) : state === "zero" ? (
          <>
            Suas contas de reserva estão zeradas. A reserva é a fundação do método — o
            ritual é guardar antes de gastar, até chegar a{" "}
            <b>6 meses do custo de vida</b>.
          </>
        ) : (
          <>
            A régua ainda é um retrato vivo: poucos meses completos de custo de vida.
            Ela já orienta — e vira veredito quando a janela de 6 meses fechar.
          </>
        )}
      </p>
    </aside>
  );
}
