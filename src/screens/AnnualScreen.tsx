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
import {
  getAnnualMetrics,
  getForecast,
  getMonthGrid,
  getDashboardSummary,
  isTauri,
  type MonthMetric,
  type MonthEnd,
  type MonthGridDay,
} from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { todayISO } from "../lib/format";
import { MES, MES_ABBR } from "../lib/nkFormat";
import { Money, SignedMoney } from "../design-system/components/Money";
import { RangeRuler } from "../design-system/components/RangeRuler";
import { Meter } from "../design-system/components/Meter";
import { EmptyState } from "../design-system/components/EmptyState";
import { InfoPopover } from "../design-system/components/InfoPopover";
import { EstimateMark } from "../design-system/components/EstimateMark";
import { NekoMark } from "../design-system/components/NekoMark";
import { SR_ONLY } from "../design-system/srOnly";
import { setCrumb } from "../shell/crumbStore";
import {
  buildAnoView,
  buildIncomeAcrossYears,
  type AnoView,
  type AnoMonth,
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

// ---------------------------------------------------------------- fetchers --
// Identidade estável por chave (o contrato do useCommand rejeita closures novas a cada render).

function fetchForecast() {
  return getForecast();
}
function fetchSummary() {
  return getDashboardSummary();
}

const _annualCache = new Map<number, () => ReturnType<typeof getAnnualMetrics>>();
function annualFetcher(year: number): () => ReturnType<typeof getAnnualMetrics> {
  const cached = _annualCache.get(year);
  if (cached) return cached;
  const fn = () => getAnnualMetrics(year);
  _annualCache.set(year, fn);
  return fn;
}

const _histCache = new Map<string, () => Promise<MonthEnd[]>>();
function historicalEndFetcher(year: number, today: string): () => Promise<MonthEnd[]> {
  const key = `${year}:${today}`;
  const cached = _histCache.get(key);
  if (cached) return cached;
  const fn = async () => {
    const grids = await Promise.all(
      pastMonthNumbers(year, today).map(async (month) => ({
        month,
        days: await getMonthGrid(year, month),
      })),
    );
    return grids.flatMap(({ month, days }) => {
      const balance = lastNonNullBalance(days);
      return balance == null ? [] : [{ year, month, balance_cents: balance }];
    });
  };
  _histCache.set(key, fn);
  return fn;
}

// ------------------------------------------------------------------ helpers --

function yearOf(iso: string): number {
  return parseInt(iso.slice(0, 4), 10);
}
function monthOf(iso: string): number {
  return parseInt(iso.slice(5, 7), 10);
}
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

function pastMonthNumbers(year: number, today: string): number[] {
  const cy = yearOf(today);
  const cm = monthOf(today);
  if (year < cy) return Array.from({ length: 12 }, (_, i) => i + 1);
  if (year > cy) return [];
  return Array.from({ length: Math.max(0, cm - 1) }, (_, i) => i + 1);
}
function lastNonNullBalance(days: MonthGridDay[]): number | null {
  for (let i = days.length - 1; i >= 0; i -= 1) {
    const b = days[i]?.balance_cents;
    if (b != null) return b;
  }
  return null;
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

function Verdict({ v }: { v: AnoView }) {
  const { verdict, year } = v;
  const showEconomia = v.rulerScopeLived ? v.economiaLived : v.economiaYear;
  const showIncome = v.rulerScopeLived ? v.incomeLived : v.incomeYear;
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
          nada, e a reserva seguiu protegida.{" "}
          <span className="ano__cf">Na ordem do método, é a troca certa.</span>
        </>
      );
      break;
    case "in_band":
      title = `Você guardou ${pctTxt} do que ganhou.`;
      body = (
        <>
          São <Money cents={showEconomia} size="inherit" /> de{" "}
          <Money cents={showIncome} size="inherit" /> que entraram.{" "}
          <span className="ano__cf">
            Dentro da faixa do método — dá para seguir a vida.
          </span>
        </>
      );
      break;
    case "above_band":
      title = `Você guardou ${pctTxt} do que ganhou.`;
      body = (
        <>
          São <Money cents={showEconomia} size="inherit" /> de{" "}
          <Money cents={showIncome} size="inherit" /> que entraram.{" "}
          <span className="ano__cf">
            Acima da faixa — dá para gastar um pouco mais, se quiser.
          </span>
        </>
      );
      break;
    default: // below_band
      if (v.economiaLived === 0) {
        title = `Você não guardou nada em ${year}.`;
        body = (
          <>
            {v.surplusLived >= 0 ? "Sobraram " : "Faltaram "}
            <Money cents={Math.abs(v.surplusLived)} size="inherit" />{" "}
            {v.livedCount === 1 ? "no mês" : `nos ${v.livedCount} meses`} que você viveu
            — e nada virou economia.{" "}
            <span className="ano__cf">
              O método pede de 20% a 30% das entradas no ano.
            </span>
          </>
        );
      } else {
        // O percentual do título e a razão em reais do corpo precisam falar do MESMO recorte
        // (regra 6): ambos seguem a régua — realizado quando há suspeitos, ano inteiro quando não.
        title = `Você guardou ${pctTxt} do que ganhou.`;
        body = (
          <>
            São <Money cents={showEconomia} size="inherit" /> de{" "}
            <Money cents={showIncome} size="inherit" /> que entraram.{" "}
            <span className="ano__cf">
              Abaixo da faixa do método — o convite é cortar custo ou aumentar renda.
            </span>
          </>
        );
      }
  }

  return (
    <div className="ano__verdict">
      <p className="ano__vlabel">
        Economizado · {year}
        {v.estimate ? (
          <EstimateMark
            className="ano__estmark"
            term={{
              title: "Número em estimativa",
              body: "Há meses à frente com pouca saída lançada, então a projeção do ano não se sustenta. Vale o que já foi vivido até você confirmar os lançamentos.",
            }}
          />
        ) : null}
      </p>
      <h1 data-large-title>{title}</h1>
      <p>{body}</p>
    </div>
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
        <InfoPopover term={REGUA_TERM} hideMarker>
          <span className="ano__how">
            Como funciona?
            <span style={SR_ONLY}> — A faixa do método</span>
          </span>
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

// ------------------------------------------------------- onde o ano termina --

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

  return (
    <section className="ano__card" aria-labelledby="ano-dez-t">
      <header className="ano__cardhead">
        <Flag size={16} strokeWidth={1.75} className="ic" aria-hidden="true" />
        <h3 id="ano-dez-t">
          {isOpen ? `Onde ${endName} termina` : `Como ${v.year} fechou`}
        </h3>
        {isOpen ? <span className="ano__note">Projeção</span> : null}
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
      {suspects.length > 0 ? (
        <p className="ano__decnote">
          A diferença entre os dois é o que ainda não foi lançado:{" "}
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
          lançados, contra os{" "}
          <b>
            <Money cents={v.typicalSpendCents} size="inherit" />
          </b>{" "}
          que costumam sair por mês.{" "}
          <span className="ano__cf">
            Pode ser mês barato de verdade — ou pode faltar lançar. Enquanto não
            confirmar, o ano não tem veredito.
          </span>
        </p>
      ) : isOpen ? (
        <p className="ano__decnote">
          Todos os meses à frente têm saída lançada compatível com o seu gasto típico de{" "}
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
          <p className="ano__myhint">
            Toque num mês para ver entradas, saídas, economia e saldo.
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
          <p className="ano__gaugefoot ano__foldnote">
            <b>Entrou</b> e <b>Saiu</b> são tudo que passou pela conta, inclusive
            dinheiro de terceiros — as duas colunas, não só uma. O custo de vida limpo
            mora em <b>Tags</b>, onde as exceções são declaradas.
          </p>
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
        <span className="ano__note">Entradas por mês com registro</span>
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
              — e o quanto você guarda seguiu em <b>0%</b> em todos eles.{" "}
              <span className="ano__cf">Ganhar mais não vira economia sozinho.</span>
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

// A linha da Mia AVANÇA a história — não repete o número que o veredito já deu. Cada estado
// ganha a leitura que o método faria dele.
function miaLine(v: AnoView): { lead: string; teach: string } {
  if (v.economiaLived > 0) {
    return {
      lead: "Você já tira dinheiro da conta para a reserva.",
      teach:
        "É isso que o método chama de economia. A régua julga a média do ano, então um mês fraco não derruba o veredito.",
    };
  }
  if (v.surplusLived < 0) {
    return {
      lead: "Nos meses vividos, saiu mais do que entrou.",
      teach:
        "Antes de guardar, o passo é fechar essa conta — e a reserva não entra na conversa para cobrir o mês.",
    };
  }
  return {
    lead: "Sobrar não é guardar.",
    teach:
      "O método não conta dinheiro parado na conta corrente: economia é o que você tira de lá. Sem uma decisão, a sobra some no gasto do mês seguinte.",
  };
}

function MiaCard({ v }: { v: AnoView }) {
  const { lead, teach } = miaLine(v);
  return (
    <section className="ano__card ano__card--mia" aria-label="A linha da Mia">
      <div className="ano__mia">
        <span className="ano__mav" aria-hidden="true">
          <NekoMark width={19} height={19} />
        </span>
        <span className="ano__mtxt">
          {lead}
          <small>{teach}</small>
        </span>
      </div>
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
  const summaryQ = useCommand("get_dashboard_summary", fetchSummary);
  const annualQ = useCommand(`annual_metrics:${year}:ano`, annualFetcher(year));

  const today = forecastQ.data?.today ?? todayISO();
  const historicalQ = useCommand(
    `month_grid_ends:${year}:${today}`,
    historicalEndFetcher(year, today),
  );

  // Renda ao longo dos anos: o ano visto e os dois anteriores (o que o método manda comparar).
  const prevA = year - 1;
  const prevB = year - 2;
  const prevAQ = useCommand(`annual_metrics:${prevA}:ano`, annualFetcher(prevA));
  const prevBQ = useCommand(`annual_metrics:${prevB}:ano`, annualFetcher(prevB));

  useEffect(() => {
    setCrumb("ano", `Onde ${year} está na faixa`);
    return () => setCrumb("ano", null);
  }, [year]);

  // Erro só na carga inicial (sem nenhum dado) — a nav de ano não tem por que aparecer.
  if (forecastQ.error && !forecastQ.data) {
    return (
      <div className="ano">
        <EmptyState
          title="Sem dados para o ano"
          description="Importe a planilha ou lance um movimento para ver o ano no método."
        />
      </div>
    );
  }

  // A nav de ano permanece visível durante a troca de ano — só o corpo esqueletiza, para o
  // usuário seguir navegando sem a tela inteira piscar.
  const ready =
    !forecastQ.loading && !annualQ.loading && !historicalQ.loading && !!annualQ.data;

  const months: MonthMetric[] = annualQ.data?.months ?? [];
  const monthEnd: MonthEnd[] = [
    ...(forecastQ.data?.month_end ?? []),
    ...(historicalQ.data ?? []),
  ];
  const reserveMonths = summaryQ.data?.reserve_months ?? null;

  const v = ready
    ? buildAnoView({ year, today, months, monthEnd, reserveMonths })
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
