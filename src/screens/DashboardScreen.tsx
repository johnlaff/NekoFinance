import { useState, type ReactNode } from "react";
import { CalendarRange, Clock3, CreditCard, Landmark, TrendingUp } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { EmptyState } from "../design-system/components/EmptyState";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { Meter } from "../design-system/components/Meter";
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
  spendCapReason,
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
// Didática do veredito recolhida (padrão do método: pergunta tocável, resposta sob demanda).
const VERDICT_HOW_DEBIT = {
  title: "Como funciona",
  body: "O número é o menor entre o que o caixa aguenta e o que preserva a economia do ano. O teto diário é o segundo limite: no dia a dia vale o mais apertado dos dois.",
};
const VERDICT_HOW_CARD = {
  title: "Como funciona",
  body: "O número é o limite que protege o caixa e a economia do ano. No cartão, a compra pesa na fatura seguinte — por isso o dia acompanha as faturas, e o teto fica como referência.",
};
const FATURAS_TERM = {
  title: "O velocímetro de quem vive no crédito",
  body: "Cada compra soma na fatura do cartão usado — é aqui que o seu gasto variável mora. O Diário fica zerado de propósito: ele é para débito e Pix, que mexem o saldo na hora.",
};

export function DashboardScreen() {
  const { navigate, openCompose } = useNekoApp();
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
  // O selo do veredito nomeia a MESMA régua que a didática logo abaixo: as duas descrevendo
  // cálculos diferentes é como a tela passou a afirmar "sem dia no vermelho" ao lado de
  // "nenhum dia no vermelho à vista".
  const capReason = spendCapReason({
    bindingGuardrail: forecast.binding_guardrail,
    deepestBalanceCents: forecast.deepest_deficit?.balance_cents ?? 0,
    deepestDate: forecast.deepest_deficit?.date ?? null,
  });
  const verdictSeal =
    capReason.kind === "savings"
      ? "Sem tocar na economia planejada do ano."
      : capReason.kind === "deficit"
        ? "O mês já abre o bico — o teto de hoje é zero."
        : "Sem deixar nenhum dia no vermelho.";

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
          <b>{verdictSeal}</b>
        </p>
        <p className="hoje__teach">
          <TeachLine
            summary={summary}
            forecast={forecast}
            cardMode={cardMode}
            monthEndLabel={monthEndLabel}
            onOpenTeto={() => navigate("teto")}
            onUseReserve={(shortfallCents, date) =>
              openCompose({
                mode: "new",
                type: "entrada",
                date,
                description: "Saque da reserva de emergência",
                amountCents: shortfallCents,
              })
            }
          />
        </p>
      </section>

      <p className="hoje__curated">
        <span className="hoje__curated-cat" aria-hidden="true">
          <MiaAvatar width={14} height={14} />
        </span>
        A Mia separou o que importa hoje — a ordem muda com o seu dia, os números nunca.
      </p>

      <HojeGrid
        deckMode={cardMode && invoices.count > 0}
        blockDay={
          <BlockDay
            summary={summary}
            invoices={invoices}
            cardMode={cardMode}
            baselineOutflowCents={forecast.baseline_outflow_cents}
            cards={cardsQ.data ?? []}
            // No modo cartão o corpo do bloco são as faturas — "ver tudo" delas é a
            // tela Cartões, não o livro-razão.
            onSeeAll={() => navigate(cardMode ? "cartoes" : "lancamentos")}
            onOpenTeto={() => navigate("teto")}
          />
        }
        monthInsightNote={
          insight ? (
            <MonthInsightNote insight={insight} month={month} today={today} />
          ) : null
        }
        moves={
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
                aria-label="Ver tudo — próximos movimentos"
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
        }
        saldo={
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
        }
      />

      {!isTauri && (
        <p className="hoje__preview">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}

/**
 * Malha de cards em COLUNAS INDEPENDENTES no desktop: cada coluna empilha seus
 * cards com altura natural — nada estica para casar linha, nada abre buraco.
 * A ordem do DOM é SEMPRE a ordem de leitura (dia → insight → movimentos →
 * saldo), em qualquer modo: o `deckMode` só muda ONDE a quebra de coluna cai
 * (depois do bloco alto de faturas, ou depois do insight) — leitor de tela e
 * tab order nunca divergem do visual. No mobile os wrappers somem
 * (`display: contents`) e a pilha segue o DOM.
 */
function HojeGrid({
  deckMode,
  blockDay,
  monthInsightNote,
  moves,
  saldo,
}: {
  deckMode: boolean;
  blockDay: ReactNode;
  monthInsightNote: ReactNode;
  moves: ReactNode;
  saldo: ReactNode;
}) {
  return deckMode ? (
    <div className="hoje__grid">
      <div className="hoje__col">{blockDay}</div>
      <div className="hoje__col">
        {monthInsightNote}
        {moves}
        {saldo}
      </div>
    </div>
  ) : (
    <div className="hoje__grid">
      <div className="hoje__col">
        {blockDay}
        {monthInsightNote}
      </div>
      <div className="hoje__col">
        {moves}
        {saldo}
      </div>
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
  onUseReserve,
}: {
  summary: DashboardSummary;
  forecast: Forecast;
  cardMode: boolean;
  monthEndLabel: string;
  onOpenTeto: () => void;
  onUseReserve: (shortfallCents: number, date: string) => void;
}) {
  const ceiling = summary.daily_budget;
  const source = summary.daily_ceiling_source;

  // O número é SÓ o guardrail que morde (caixa ou economia) — o teto nunca entra
  // nele; é o segundo limite do dia. Uma frase sempre visível; a mecânica completa
  // mora no "Como funciona" (didática atrás de pergunta, o padrão do método).
  //
  // Caixa zerado significa que o mês já abre o bico, e no método esse é o momento de ACIONAR a
  // reserva — não de proibir o gasto. A frase aponta para o gesto, com o tamanho do buraco.
  const reason = spendCapReason({
    bindingGuardrail: forecast.binding_guardrail,
    deepestBalanceCents: forecast.deepest_deficit?.balance_cents ?? 0,
    deepestDate: forecast.deepest_deficit?.date ?? null,
  });
  // A reserva só é oferecida quando ela existe: sem conta mapeada ou zerada, a saída é outra
  // (subir a performance), e sugerir um saque impossível seria conselho vazio.
  const hasReserve =
    summary.reserve_state === "verdict" || summary.reserve_state === "estimate";
  const numberPhrase =
    reason.kind === "savings" ? (
      "Este é o limite da economia: o maior gasto que mantém a meta do ano viva."
    ) : reason.kind === "deficit" ? (
      <>
        Falta <Money cents={reason.shortfallCents} size="inherit" /> em{" "}
        {faturaDayLabel(reason.date)}.{" "}
        {hasReserve ? (
          <>
            É para isso que a reserva existe —{" "}
            <button
              type="button"
              className="hoje__link"
              onClick={() => onUseReserve(reason.shortfallCents, reason.date)}
            >
              lançar o saque
            </button>{" "}
            e programar a reposição.
          </>
        ) : (
          "Sem reserva mapeada, o caminho é a performance do mês: entrar mais, ou sair menos."
        )}
      </>
    ) : (
      `Este é o limite do caixa: o maior gasto que o saldo aguenta até ${monthEndLabel} sem nenhum dia no vermelho.`
    );

  // Ação nunca se esconde: os estados sem teto mantêm o CTA visível; os estados
  // informados encolhem a um rótulo curto com o valor.
  const tetoClause =
    source === "chosen" ? (
      <>
        {" "}
        Teto:{" "}
        <button type="button" className="hoje__link" onClick={onOpenTeto}>
          <Money cents={ceiling} size="inherit" /> por dia
        </button>
        .
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
          Proposta do teto — revisar.
        </button>
      </>
    ) : (
      <>
        {" "}
        Ainda sem teto diário.{" "}
        <button type="button" className="hoje__link" onClick={onOpenTeto}>
          Estipular o teto
        </button>
      </>
    );

  return (
    <>
      {numberPhrase}
      {tetoClause}{" "}
      <InfoPopover term={cardMode ? VERDICT_HOW_CARD : VERDICT_HOW_DEBIT} hideMarker>
        <span className="hoje__how">Como funciona?</span>
      </InfoPopover>
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
    <section className="hoje__card hoje__blockday" aria-labelledby="hoje-day-title">
      <header className="hoje__cardhead">
        <span className="ic" aria-hidden="true">
          <Clock3 size={17} strokeWidth={1.75} />
        </span>
        <h2 id="hoje-day-title">Gasto variável de hoje</h2>
        <button
          type="button"
          className="hoje__more"
          aria-label={
            cardMode ? "Ver tudo — faturas dos cartões" : "Ver tudo — gasto variável"
          }
          onClick={onSeeAll}
        >
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
        <ModeChip
          mode={summary.spending_mode}
          gate={summary.card_gate}
          detected={summary.spending_mode_detected}
        />
      </div>

      {cardMode ? (
        <>
          {invoices.count > 0 ? (
            <>
              <div className="hoje__fatura">
                <div className="hoje__fatura-head">
                  <InfoPopover term={FATURAS_TERM} hideMarker>
                    <span className="hoje__how hoje__how--label">
                      Faturas em aberto — {invoices.count}{" "}
                      {invoices.count === 1 ? "cartão" : "cartões"}
                    </span>
                  </InfoPopover>
                  <span className="money">
                    <Money cents={invoices.totalCents} size="inherit" />
                  </span>
                </div>
                {pct !== null && (
                  <>
                    <Meter
                      className="hoje__fatura-bar"
                      fraction={pct / 100}
                      color="var(--accent)"
                    />
                    <p className="hoje__fatura-note">
                      Até aqui: {pct}% do gasto típico de um mês.
                    </p>
                  </>
                )}
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
        </>
      ) : (
        <>
          <div className="hoje__ci-head">
            <span className="hoje__ci-label">Diário de hoje</span>
            <span className="money">
              <Money cents={spentToday} size="inherit" />
              <span className="hoje__ci-denom">
                {" "}
                /{" "}
                {source === "none" ? (
                  // Com proposta pendente o convite é ÚNICO em toda a tela: revisar
                  // (mesma frase do herói).
                  <button type="button" className="hoje__link" onClick={onOpenTeto}>
                    {summary.ceiling_proposal_pending
                      ? "Proposta do teto — revisar"
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
            <Meter
              className="hoje__ci-track"
              fraction={ciPct / 100}
              height={9}
              color={overCeiling ? "var(--danger-500)" : "var(--type-diario)"}
              label={
                overCeiling
                  ? "Diário de hoje acima do teto"
                  : `Diário de hoje em ${ciPct}% do teto`
              }
            />
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
        <b>
          <Money
            cents={insight.endBalanceCents}
            size="inherit"
            sign={insight.endBalanceCents < 0 ? "negative" : "none"}
          />
        </b>
        . O ponto mais apertado do mês é{" "}
        {insight.minIsOngoing ? <b>hoje</b> : <b>dia {minDay}</b>}:{" "}
        <b>
          <Money
            cents={insight.minCents}
            size="inherit"
            sign={insight.minCents < 0 ? "negative" : "none"}
          />
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
    // 8 na lista do desktop; o mobile esconde 7–8 via CSS (carrossel de polegar
    // pede menos peças que a lista de mouse).
    .slice(0, 8);

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
              <Meter
                className="hoje__gauge"
                fraction={saldoGaugeFraction(band.key)}
                color={band.text}
                trackColor="var(--surface)"
              />
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
              <Meter
                className="hoje__gauge"
                fraction={reserveFraction}
                color={reserveOk ? "var(--ok)" : "var(--warn)"}
                trackColor="var(--surface)"
              />
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
              {/* Alcançado o alvo, a pergunta do método deixa de ser "quanto falta" e passa a
                  ser o que fazer com o excedente — é ele que financia o próximo movimento. */}
              {summary.reserve_surplus_cents != null && (
                <p className="hoje__stat-note">
                  <Money cents={summary.reserve_surplus_cents} size="inherit" /> além do
                  alvo — é com esse excedente que o próximo passo se decide.
                </p>
              )}
            </>
          )}
        </div>
      </div>
      <ReserveInsight
        state={reserveState}
        basisMonths={summary.reserve_basis_months}
        onMapReserve={onMapReserve}
      />
    </>
  );
}

function ReserveInsight({
  state,
  basisMonths,
  onMapReserve,
}: {
  state: DashboardSummary["reserve_state"];
  basisMonths: number;
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
          // O selo "Estimativa" já explica o conceito de retrato vivo; aqui entra o
          // DADO que o selo não tem — quanto da janela já existe e quanto falta.
          <>
            Retrato vivo com <b>{basisMonths} de 6 meses completos</b> de custo de vida
            —{" "}
            {6 - basisMonths === 1 ? "falta 1 mês" : `faltam ${6 - basisMonths} meses`}{" "}
            para a régua virar veredito.
          </>
        )}
      </p>
    </aside>
  );
}
