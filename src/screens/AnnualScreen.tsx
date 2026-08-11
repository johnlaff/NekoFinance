import "./ano.css";
import { useEffect, useState, type ReactNode } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Gauge,
  Flag,
  Table2,
  TrendingUp,
  Rows3,
} from "lucide-react";
import { isTauri } from "../lib/env";
import { useCommand } from "../lib/useCommand";
import { todayISO } from "../lib/format";
import { MES, MES_ABBR } from "../lib/nkFormat";
import { Money, SignedMoney } from "../design-system/components/Money";
import { RangeRuler } from "../design-system/components/RangeRuler";
import { Meter } from "../design-system/components/Meter";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { VerdictHero } from "../design-system/components/VerdictHero";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { setCrumb } from "../shell/crumbStore";
import {
  anoMiaObservation,
  annualMetricsCacheKey,
  annualMetricsFetcher,
  annualRulerCacheKey,
  annualRulerFetcher,
  buildAnoView,
  buildIncomeAcrossYears,
  fetchForecast,
  type AnoView,
  type AnoMonth,
  type MonthMetric,
} from "./anoView";

// A tela O ano é o tribunal do método: a única onde o Economizado% pode julgar, porque a
// régua é a média de 20% a 30% no ANO. A ordem narra pela prova — veredito → régua da faixa
// → onde o ano termina → os doze meses → o ano em números → renda ao longo dos anos. Toda a
// matemática vem do motor; a composição vive no view-model puro `anoView`.

const RULER_MARKS = [
  { at: 20, label: "20%" },
  { at: 30, label: "30%" },
  { at: 40, label: "40%" },
];

// ------------------------------------------------------------------ helpers --

function cap(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}
function pctTrunc(n: number | null): string {
  return n == null ? "—" : `${Math.trunc(n)}%`;
}
function monthAbbr(month: number): string {
  return cap(MES_ABBR[month - 1] ?? "");
}

/** "1 mês" / "7 meses" — a concordância quebra em janeiro, quando só um mês foi vivido. */
function mesesLabel(n: number): string {
  return `${n} ${n === 1 ? "mês" : "meses"}`;
}

/** Cor do pino da régua = status do MÉTODO (a única cor de status da tela): fora da faixa é
 *  atenção, dentro é aprovação. Nunca a cor de marca. */
function pinColor(pct: number): string {
  return pct < 20 ? "var(--warning-400)" : "var(--success-400)";
}

// ---------------------------------------------------------- ano navigation --

function YearNav({
  year,
  onPrev,
  onNext,
  canNext,
}: {
  year: number;
  onPrev: () => void;
  onNext: () => void;
  canNext: boolean;
}) {
  return (
    <div className="ano__yhead">
      <span className="ano__ynav" role="group" aria-label="Trocar de ano">
        <button type="button" aria-label="Ano anterior" onClick={onPrev}>
          <ChevronLeft size={15} strokeWidth={1.9} aria-hidden="true" />
        </button>
        <span className="ano__ycur">{year}</span>
        <button
          type="button"
          aria-label="Próximo ano"
          onClick={onNext}
          disabled={!canNext}
        >
          <ChevronRight size={15} strokeWidth={1.9} aria-hidden="true" />
        </button>
      </span>
    </div>
  );
}

// ------------------------------------------------------------------ verdict --

// Manchete + UMA linha de corpo (regra 42): o selo do veredito, que muda com o estado. A
// didática do método — a régua anual e o convite de cada lado da faixa — mora no "Como
// funciona?" da régua, e os operandos da razão vivem no cabeçalho dela (regra 41).
function Verdict({ v }: { v: AnoView }) {
  const { verdict, year } = v;
  const pctTxt = pctTrunc(v.rulerPct);

  let title: string;
  let body: ReactNode;
  switch (verdict.kind) {
    case "no_record":
      title = `${year} não tem registro.`;
      body = <>Nenhum lançamento chegou da planilha para este ano.</>;
      break;
    case "zero_by_choice":
      title = "Você zerou a economia para não tocar na reserva.";
      body = (
        <>
          {v.livedCount === 1 ? "Foi 1 mês" : `Foram ${v.livedCount} meses`} sem guardar
          nada, e a reserva seguiu protegida.
        </>
      );
      break;
    case "in_band":
      title = `Você guardou ${pctTxt} do que ganhou.`;
      body = <>Dentro da faixa do método — dá para seguir a vida.</>;
      break;
    case "above_band":
      title = `Você guardou ${pctTxt} do que ganhou.`;
      body = <>Acima da faixa — dá para gastar um pouco mais, se quiser.</>;
      break;
    default: // below_band
      if (v.economiaLived === 0) {
        title = `Você não guardou nada em ${year}.`;
        body = (
          <>
            {v.surplusLived >= 0 ? "Sobraram " : "Faltaram "}
            <Money cents={Math.abs(v.surplusLived)} size="inherit" />{" "}
            {v.livedCount === 1 ? "no mês" : `nos ${v.livedCount} meses`} que você
            viveu.
          </>
        );
      } else {
        // O percentual segue a régua (realizado quando há suspeitos, ano inteiro quando não),
        // e o selo devolve a decisão pelas duas alavancas do método — nunca uma cobrança.
        title = `Você guardou ${pctTxt} até aqui.`;
        body = <>O que aproxima o ano dos 20 — soltar menos ou entrar mais?</>;
      }
  }

  return (
    <VerdictHero
      label={`Economizado · ${year}`}
      labelMark={
        v.estimate ? (
          <EstimateMark
            className="ano__estmark"
            term={{
              title: "Número em estimativa",
              body: "Há meses à frente com pouca saída lançada, então a projeção do ano não se sustenta. Vale o que já foi vivido até você confirmar os lançamentos.",
            }}
          />
        ) : null
      }
      headline={title}
    >
      <p>{body}</p>
    </VerdictHero>
  );
}

// --------------------------------------------------------------- faixa card --

function FaixaCard({ v }: { v: AnoView }) {
  const scopeText = v.rulerScopeLived
    ? `em ${v.livedCount} de 12 meses já vividos`
    : `em ${v.year}`;
  const pct = v.rulerPct;
  const situacao =
    pct == null
      ? "sem registro de economia"
      : pct < 20
        ? "abaixo da faixa do método"
        : pct > 30
          ? "acima da faixa do método"
          : "dentro da faixa do método";
  const firstFutureMonth = v.months.find((m) => m.future)?.month ?? null;
  const lastMonth = v.months[v.months.length - 1]?.month ?? 12;
  const monthName = (month: number): string => (MES[month - 1] ?? "").toLowerCase();
  const faixaFuturos =
    firstFutureMonth != null
      ? `${monthName(firstFutureMonth)} a ${monthName(lastMonth)}`
      : "";

  return (
    <section className="ano__card" aria-labelledby="ano-faixa-t">
      <header className="ano__cardhead">
        <Gauge size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h3 id="ano-faixa-t">A faixa do método</h3>
        <span className="ano__note">
          <Money cents={v.economiaLived} size="inherit" /> de{" "}
          <Money cents={v.incomeLived} size="inherit" />
        </span>
        <InfoPopover
          term={v.verdict.kind === "zero_by_choice" ? REGUA_TERM_ZERO : REGUA_TERM}
          label="Como funciona? — A faixa do método"
          hideMarker
        >
          <span className="ano__how">Como funciona?</span>
        </InfoPopover>
      </header>
      <RangeRuler
        className="ano__ruler"
        max={40}
        zone={{ from: 20, to: 30 }}
        marks={RULER_MARKS}
        pin={
          pct == null
            ? null
            : { value: pct, label: pctTrunc(pct), color: pinColor(pct) }
        }
        label={`Economizado: ${pctTrunc(pct)}, ${situacao}, ${scopeText}. A faixa vai de 20% a 30%, numa escala de 0% a 40%.`}
      />
      <p className="ano__gaugebase">{cap(scopeText)}</p>
      {/* Só o dado variável fica inline (quanto falta para os 20%); a didática fixa — "a
          régua é anual", o convite abaixo/acima da faixa — mora no "Como funciona?". A conta
          usa o denominador ANUAL: a falta dos meses vividos fecharia o ano num número menor. */}
      {pct != null && pct < 20 ? (
        <p className="ano__gaugefoot">
          Para <b>{v.year}</b> fechar em 20%,{" "}
          {v.futureCount > 0 ? "falta guardar" : "faltou guardar"}{" "}
          <Money cents={Math.max(0, v.shortfallYearCents)} size="inherit" />
          {v.perMonthShortfallCents != null && v.perMonthShortfallCents > 0 ? (
            <>
              {" "}
              — ou <Money cents={v.perMonthShortfallCents} size="inherit" /> por mês de{" "}
              {faixaFuturos}
            </>
          ) : null}
          .
        </p>
      ) : null}
    </section>
  );
}

// A didática da régua vive atrás de "Como funciona?" (nunca em parágrafo permanente): a
// regra do método é anual, e o convite muda com o lado da faixa.
const REGUA_TERM = {
  title: "A régua da faixa",
  body: "A régua é anual: um mês fraco é normal — o que precisa fechar entre 20% e 30% é a média do ano. Abaixo de 20%, o convite é cortar custo ou aumentar renda; acima de 30%, dá para gastar um pouco mais.",
};
// Quem zerou a economia para proteger a reserva não está abaixo da faixa por descuido: a
// leitura do método muda, e é ela que a régua explica nesse estado.
const REGUA_TERM_ZERO = {
  title: "A régua da faixa",
  body: "A régua é anual: o que precisa fechar entre 20% e 30% é a média do ano. Zerar a economia para não tocar na reserva é, na ordem do método, a troca certa.",
};

// ------------------------------------------------------- onde o ano termina --

// O que os dois cenários significam é didática invariável — o card imprime os operandos e
// guarda a leitura atrás da pergunta.
const DEZEMBRO_TERM = {
  title: "Os dois cenários do fim do ano",
  body: "A diferença entre os dois é o que ainda não foi lançado nos meses sem lastro: pode ser mês barato de verdade, ou pode faltar lançar. Enquanto não confirmar, o ano não tem veredito.",
};

function DezembroCard({ v }: { v: AnoView }) {
  if (v.endBalanceCents == null || v.endMonth == null) return null;
  const endMonth = v.endMonth;
  const endName = (MES[endMonth - 1] ?? "dezembro").toLowerCase();
  // Só há PROJEÇÃO enquanto o ano tem meses à frente. Num ano fechado o saldo final é fato
  // realizado — chamá-lo de projeção (e falar em "meses à frente") seria mentira.
  const isOpen = v.futureCount > 0;
  const suspects = v.months.filter((m) => m.suspect && m.month <= endMonth);
  const negEnd = v.endBalanceCents < 0;
  const negTip = (v.endBalanceTypicalCents ?? 0) < 0;
  const outflows = suspects.map((m) => m.outflow);
  const minOut = suspects.length > 0 ? Math.min(...outflows) : 0;
  const maxOut = suspects.length > 0 ? Math.max(...outflows) : 0;
  const title = isOpen ? `Onde ${endName} termina` : `Como ${v.year} fechou`;

  return (
    <section className="ano__card" aria-labelledby="ano-dez-t">
      <header className="ano__cardhead">
        <Flag size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h3 id="ano-dez-t">{title}</h3>
        {isOpen ? <span className="ano__note">Projeção</span> : null}
        <InfoPopover
          term={DEZEMBRO_TERM}
          label={`Como funciona? — ${title}`}
          hideMarker
        >
          <span className="ano__how">Como funciona?</span>
        </InfoPopover>
      </header>
      <div className="ano__dec">
        <span className={`ano__decval${negEnd ? " neg" : ""}`}>
          <Money cents={v.endBalanceCents} size="inherit" />
        </span>
        <span className="ano__est-inline">
          {isOpen
            ? "Se o resto do ano custar o que está lançado"
            : `Saldo no fim de ${endName}`}
        </span>
      </div>
      {v.endBalanceTypicalCents != null ? (
        <div className="ano__dec ano__dec--alt">
          <span className={`ano__decval ano__decval--sm${negTip ? " neg" : ""}`}>
            <Money cents={v.endBalanceTypicalCents} size="inherit" />
          </span>
          <span className="ano__decalt-lbl">
            Se os meses a conferir custarem o de sempre
          </span>
        </div>
      ) : null}
      {/* Legenda de cálculo: os operandos que separam os dois cenários. A leitura deles é
          didática e mora no "Como funciona?" do cabeçalho. */}
      {suspects.length > 0 ? (
        <p className="ano__decnote">
          <b>{suspects.map((m) => monthAbbr(m.month)).join(", ")}</b>{" "}
          {suspects.length === 1 ? "tem" : "têm"}{" "}
          {minOut === maxOut ? (
            <Money cents={minOut} size="inherit" />
          ) : (
            <>
              entre <Money cents={minOut} size="inherit" /> e{" "}
              <Money cents={maxOut} size="inherit" />
            </>
          )}{" "}
          lançados, contra{" "}
          <b>
            <Money cents={v.typicalSpendCents} size="inherit" />
          </b>{" "}
          que costumam sair por mês.
        </p>
      ) : isOpen ? (
        <p className="ano__decnote">
          Meses à frente com saída compatível com o gasto típico de{" "}
          <b>
            <Money cents={v.typicalSpendCents} size="inherit" />
          </b>{" "}
          — a projeção se sustenta.
        </p>
      ) : null}
    </section>
  );
}

// ------------------------------------------------------------- doze meses ----

function MonthBar({ m, maxIncome }: { m: AnoMonth; maxIncome: number }) {
  const w = maxIncome > 0 ? (m.income / maxIncome) * 100 : 0;
  const savedW = m.income > 0 ? w * (m.economia / m.income) : 0;
  // Barra decorativa (aria-hidden): o valor viaja como TEXTO ao lado; a geometria nunca é a
  // única portadora do dado. Trilho = renda do mês; preenchimento = economia; tique = 20%.
  return (
    <span className={`ano__mbar${m.future ? " fut" : ""}`} aria-hidden="true">
      <i className="inflow" style={{ width: `${w.toFixed(1)}%` }} />
      {m.economia > 0 ? (
        <i className="saved" style={{ width: `${savedW.toFixed(1)}%` }} />
      ) : null}
      <i className="target" style={{ left: `${(w * 0.2).toFixed(1)}%` }} />
    </span>
  );
}

function MesesCard({ v, children }: { v: AnoView; children: ReactNode }) {
  const maxIncome = Math.max(...v.months.map((m) => m.income), 1);
  return (
    <section className="ano__card" aria-labelledby="ano-meses-t">
      <header className="ano__cardhead">
        <Rows3 size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h3 id="ano-meses-t">Os doze meses</h3>
        <span className="ano__note">Entradas × o que virou economia</span>
      </header>
      <div className="ano__months">
        {v.months.map((m) => (
          <div key={m.month} className={`ano__mrow${m.current ? " now" : ""}`}>
            <span className="ano__mname">{monthAbbr(m.month)}</span>
            <MonthBar m={m} maxIncome={maxIncome} />
            <span className="ano__mval">
              <Money cents={m.income} size="inherit" />
            </span>
            <span className="ano__mpct">{m.lived ? pctTrunc(m.savedPct) : "—"}</span>
            {m.suspect ? <span className="ano__mflag">Conferir</span> : null}
          </div>
        ))}
      </div>
      <div className="ano__mlegend" aria-hidden="true">
        <span>
          <i className="k-in" />O que entrou
        </span>
        <span>
          <i className="k-sv" />O que você guardou
        </span>
        <span>
          <i className="k-tg" />
          Referência de 20%
        </span>
        <span>
          <i className="k-fu" />
          Ainda não aconteceu
        </span>
      </div>
      {children}
    </section>
  );
}

// -------------------------------------------------------- o ano em números --

function MonthDetail({ m }: { m: AnoMonth }) {
  const [open, setOpen] = useState(false);
  return (
    <div className={`ano__my${m.future ? " fut" : ""}`}>
      <button
        type="button"
        className="ano__mysum"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="ano__myname">{monthAbbr(m.month)}</span>
        {m.suspect ? <span className="ano__mflag">Conferir</span> : null}
        <span className={`ano__myres${m.performance < 0 ? " neg" : ""}`}>
          <SignedMoney cents={m.performance} size="inherit" />
        </span>
        <ChevronRight
          size={15}
          strokeWidth={2}
          className="ano__exchev"
          aria-hidden="true"
        />
      </button>
      {open ? (
        <div className="ano__mydet">
          <MDetail label="Entrou" value={<Money cents={m.income} size="inherit" />} />
          <MDetail label="Saiu" value={<Money cents={m.outflow} size="inherit" />} />
          <MDetail
            label="Economia"
            value={m.lived ? <Money cents={m.economia} size="inherit" /> : <>—</>}
          />
          <MDetail label="Guardado" value={m.lived ? pctTrunc(m.savedPct) : "—"} />
          <MDetail
            label="Saldo no fim do mês"
            value={
              m.endBalance != null ? (
                <Money cents={m.endBalance} size="inherit" />
              ) : (
                <>—</>
              )
            }
          />
        </div>
      ) : null}
    </div>
  );
}

function MDetail({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="ano__mdl">
      <span>{label}</span>
      <b>{value}</b>
    </div>
  );
}

// A chave de leitura do card: o que as duas colunas medem, e onde mora o custo de vida limpo.
const NUMEROS_TERM = {
  title: "O ano em números",
  body: "Entrou e Saiu são tudo que passou pela conta, inclusive dinheiro de terceiros — as duas colunas, não só uma. O custo de vida limpo mora em Tags, onde as exceções são declaradas.",
};

function AnoEmNumeros({ v }: { v: AnoView }) {
  const [open, setOpen] = useState(false);
  const firstFuture = v.months.find((m) => m.future)?.month ?? null;
  const livedOutflow = v.months
    .filter((m) => m.lived)
    .reduce((s, m) => s + m.outflow, 0);
  return (
    <div className="ano__fold">
      <button
        type="button"
        className="ano__foldsum"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Table2 size={16} strokeWidth={1.75} aria-hidden="true" />
        <b>O ano em números</b>
        <span className="ano__fc">12 meses · realizado e previsto</span>
        <ChevronRight
          size={15}
          strokeWidth={2}
          className="ano__exchev"
          aria-hidden="true"
        />
      </button>
      {open ? (
        <div className="ano__foldbody">
          {/* A chave de leitura das duas colunas é didática fixa: entra atrás da pergunta.
              Que dá para abrir cada mês, a própria linha anuncia (botão com aria-expanded). */}
          <p className="ano__foldhow">
            <InfoPopover
              term={NUMEROS_TERM}
              label="Como funciona? — O ano em números"
              hideMarker
            >
              <span className="ano__how">Como funciona?</span>
            </InfoPopover>
          </p>
          {v.months.map((m) => (
            <div key={m.month}>
              {m.month === firstFuture ? (
                <p className="ano__mydiv">Daqui para frente é previsão</p>
              ) : null}
              <MonthDetail m={m} />
            </div>
          ))}
          <div className="ano__mytot">
            <span className="ano__myname">Vivido</span>
            <span className="ano__mytot-meta">
              {mesesLabel(v.livedCount)} · entrou{" "}
              <Money cents={v.incomeLived} size="inherit" /> · saiu{" "}
              <Money cents={livedOutflow} size="inherit" />
            </span>
            <span className={`ano__myres${v.surplusLived < 0 ? " neg" : ""}`}>
              <SignedMoney cents={v.surplusLived} size="inherit" />
            </span>
          </div>
        </div>
      ) : null}
    </div>
  );
}

// -------------------------------------------------- renda ao longo dos anos --

interface IncomeRow {
  year: number;
  recordedMonths: number;
  avgIncomeCents: number;
  savedPct: number | null;
}

// A leitura da série é didática invariável: o que a comparação mede e por que renda maior não
// vira economia sozinha.
const RENDA_TERM = {
  title: "Sua renda ao longo dos anos",
  body: "A comparação é de renda: o que entrou por mês com registro, ano a ano. Ganhar mais não vira economia sozinho — sem a decisão de tirar da conta, a renda maior vira gasto maior.",
};

function RendaCard({ rows }: { rows: IncomeRow[] }) {
  if (rows.length === 0) return null;
  const maxAvg = Math.max(...rows.map((r) => r.avgIncomeCents), 1);
  const first = rows[0]!;
  const last = rows[rows.length - 1]!;
  const delta =
    first.avgIncomeCents > 0
      ? Math.round((last.avgIncomeCents / first.avgIncomeCents - 1) * 100)
      : null;
  const allZeroSaved = rows.every((r) => (r.savedPct ?? 0) < 0.5);

  return (
    <section className="ano__card" aria-labelledby="ano-renda-t">
      <header className="ano__cardhead">
        <TrendingUp size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h3 id="ano-renda-t">Sua renda ao longo dos anos</h3>
        <span className="ano__note">Por mês com registro</span>
        <InfoPopover
          term={RENDA_TERM}
          label="Como funciona? — Sua renda ao longo dos anos"
          hideMarker
        >
          <span className="ano__how">Como funciona?</span>
        </InfoPopover>
      </header>
      <div className="ano__years">
        {rows.map((r) => (
          <div className="ano__yr" key={r.year}>
            <span className="ano__yrname">{r.year}</span>
            <Meter
              className="ano__yrmeter"
              fraction={r.avgIncomeCents / maxAvg}
              color="var(--text-faint)"
              height={8}
            />
            <span className="ano__yrval">
              <Money cents={r.avgIncomeCents} size="inherit" />
            </span>
            <span className={`ano__yrpct${(r.savedPct ?? 0) < 0.5 ? " zero" : ""}`}>
              {pctTrunc(r.savedPct)} guardado
            </span>
          </div>
        ))}
      </div>
      {rows.length >= 2 && delta != null ? (
        <p className="ano__yrfoot">
          Suas entradas médias {delta >= 0 ? "subiram" : "caíram"}{" "}
          <b>{Math.abs(delta)}%</b> de {first.year} para {last.year}
          {allZeroSaved ? (
            <>
              {" "}
              — e o quanto você guarda seguiu em <b>0%</b> em todos eles.
            </>
          ) : (
            <>.</>
          )}
        </p>
      ) : null}
    </section>
  );
}

// -------------------------------------------------------------- linha da Mia --

/**
 * A linha da Mia: uma observação que muda com o mês vivido — o card merece releitura diária.
 * A didática que a acompanhava duplicava o "Como funciona?" da régua (regra 41) e morreu ali.
 */
function MiaCard({ v }: { v: AnoView }) {
  const obs = anoMiaObservation(v);
  return (
    <section className="ano__card ano__card--mia" aria-label="A linha da Mia">
      {obs ? (
        <div className="ano__mia">
          {/* O rosto da Mia, não a marca do app: aqui o gato atribui a frase a quem a
              interpretou. `NekoMark` fica reservado ao shell, onde marca o produto. */}
          <span className="ano__mav" aria-hidden="true">
            <MiaAvatar width={19} height={19} />
          </span>
          <span className="ano__mtxt">
            <b>{obs.month}</b> {obs.clause} — a média do ano{" "}
            {obs.ongoing ? "segue" : "ficou"} em <b>{obs.yearPct}</b>.
          </span>
        </div>
      ) : null}
      <button className="ano__miaact" type="button">
        Perguntar à Mia sobre {v.year}
      </button>
    </section>
  );
}

// ------------------------------------------------------------- main screen --

export function AnnualScreen() {
  const thisYear = new Date().getFullYear();
  const [year, setYear] = useState(thisYear);

  const forecastQ = useCommand("get_forecast", fetchForecast);
  const annualQ = useCommand(annualMetricsCacheKey(year), annualMetricsFetcher(year));
  // A régua do método chega decidida do motor — inclusive o veredito, que lê a reserva no
  // backend. É a mesma leitura que a conversa dá quando perguntam pelo ano.
  const rulerQ = useCommand(annualRulerCacheKey(year), annualRulerFetcher(year));

  const today = forecastQ.data?.today ?? todayISO();

  // Renda ao longo dos anos: o ano visto e os dois anteriores (o que o método manda comparar).
  const prevA = year - 1;
  const prevB = year - 2;
  const prevAQ = useCommand(annualMetricsCacheKey(prevA), annualMetricsFetcher(prevA));
  const prevBQ = useCommand(annualMetricsCacheKey(prevB), annualMetricsFetcher(prevB));

  useEffect(() => {
    setCrumb("ano", `Onde ${year} está na faixa`);
    return () => setCrumb("ano", null);
  }, [year]);

  // Erro só na carga inicial (sem nenhum dado) — a nav de ano não tem por que aparecer.
  if (forecastQ.error && !forecastQ.data) {
    return (
      <div className="ano">
        {/* Falha de carga é erro, não vazio: a variante certa anuncia por `role="alert"`,
            e a copy não pode mandar importar planilha quando o que quebrou foi a consulta. */}
        <EmptyState
          variant="error"
          title="Não foi possível carregar o ano"
          description="A leitura dos dados do ano falhou. Tente de novo em instantes."
        />
      </div>
    );
  }

  // A nav de ano permanece visível durante a troca de ano — só o corpo esqueletiza, para o
  // usuário seguir navegando sem a tela inteira piscar.
  const ready =
    !forecastQ.loading &&
    !annualQ.loading &&
    !rulerQ.loading &&
    !!annualQ.data &&
    !!rulerQ.data;

  const months: MonthMetric[] = annualQ.data?.months ?? [];

  const v =
    ready && rulerQ.data
      ? buildAnoView({ year, today, months, ruler: rulerQ.data })
      : null;

  // "Sua renda ao longo dos anos" só monta quando TODOS os anos comparados carregaram — senão
  // as linhas apareceriam em corrida (uma por ano que chega), piscando estados parciais.
  const incomeReady = ready && !prevAQ.loading && !prevBQ.loading;
  const incomeRows = incomeReady
    ? buildIncomeAcrossYears(
        [
          { year: prevB, months: prevBQ.data?.months ?? [] },
          { year: prevA, months: prevAQ.data?.months ?? [] },
          { year, months },
        ],
        today,
      ).filter((r) => r.recordedMonths > 0)
    : [];

  return (
    <div className="ano">
      <YearNav
        year={year}
        onPrev={() => setYear((y) => y - 1)}
        onNext={() => setYear((y) => y + 1)}
        canNext={year < thisYear}
      />

      {v == null ? (
        <EmptyState variant="skeleton" skeletonRows={6} />
      ) : (
        <>
          <Verdict v={v} />
          {v.hasData ? (
            <>
              {/* Veredito e régua da faixa em largura cheia — a régua é o instrumento herói
                  e a leitura pede a largura. Os cards de apoio descem para um bento de 2
                  colunas INDEPENDENTES (massas díspares → colunas independentes, nunca
                  row-alignment); no mobile as colunas se dissolvem (`display: contents`) e os
                  cards fluem na ordem do DOM (a narrativa aprovada). */}
              <FaixaCard v={v} />
              <div className="ano__bento">
                <div className="ano__col">
                  <DezembroCard v={v} />
                  <MesesCard v={v}>
                    <AnoEmNumeros v={v} />
                  </MesesCard>
                </div>
                <div className="ano__col">
                  <RendaCard rows={incomeRows} />
                  <MiaCard v={v} />
                </div>
              </div>
            </>
          ) : null}
        </>
      )}

      {!isTauri && (
        <p className="ano__webhint">
          Preview web — abra o app desktop para ver seus dados.
        </p>
      )}
    </div>
  );
}
