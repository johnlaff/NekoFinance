import type { LoanBreakdown, ScenarioCompareDto } from "../lib/api";
import { fmtBRL, fmtCompactBRL, MES, monthOf, saldoBand } from "../lib/nkFormat";
import { custoVidaStatus, performanceStatus } from "./totaisStatus";

// View-model puro da superfície de comparação real × cenário. Consome o DTO do motor
// (`get_scenario_forecast`) e produz os cinco cards de KPI já DECIDIDOS: rótulo, copy didática,
// os dois valores, o delta, o sentido do delta e o estado de método de cada lado — e o gate de
// financiamento (reserva após financiar, economia após parcela) já resolvido em estado + textos
// formatados. Nenhuma fronteira mora na tela — o que a tela faz é pintar o que chega daqui.

// ------------------------------------------------------------------- tipos --

/** Sentido do delta: decide se um delta material é bom ou ruim (nunca o sinal cru). */
export type DeltaSense = "higher-better" | "lower-better";

/** Glifo do estado, em vocabulário do método — a tela escolhe o ícone concreto. */
export type MethodIcon = "ok" | "alert" | "none";

/** `key` decide TRANSIÇÃO (comparar real × cenário) — pode divergir do `label` renderizado
 * quando o rótulo embute um valor que muda toda hora sem ser uma mudança de ESTADO (ex.:
 * "Pode gastar hoje": "Livre até R$X" tem `key = "livre"` fixo). `line` é uma frase adicional
 * DATA-DERIVADA para a situação (nunca copy fixa de conceito — essa mora só no InfoPopover). */
export interface MethodState {
  key: string;
  label: string;
  color: string;
  icon: MethodIcon;
  line?: string;
}

/** Termo do InfoPopover que acompanha o rótulo do card. */
export interface MethodTerm {
  title: string;
  body: string;
}

/** Delta já julgado: `material` decide se o chip mostra "≈ Sem mudança" (ruído de arredondamento)
 * ou o valor com sinal; `better` decide a cor/ícone quando material — nunca o sinal cru do
 * delta, que sozinho não sabe se uma métrica é "maior é melhor" ou "menor é melhor". Sem
 * significado quando `material` é `false`. */
export interface DeltaVerdict {
  material: boolean;
  better: boolean;
}

export interface ScenarioKpi {
  label: string;
  term: MethodTerm;
  realCents: number;
  scenarioCents: number;
  deltaCents: number;
  sense: DeltaSense;
  delta: DeltaVerdict;
  realState: MethodState;
  scenarioState: MethodState;
  /** Cenário sem NENHUM ponto de projeção: `scenarioCents`/`deltaCents` são ruído e a tela
   * rende um vazio neutro em vez de fingir um valor. */
  emptyScenario: boolean;
}

export type VerdictTier = "risk" | "tight" | "ok";

export interface ScenarioVerdict {
  tier: VerdictTier;
  headline: string;
  subline: string;
}

export interface ScenariosView {
  /** Ordem por prioridade de decisão (padrão-Z): Buraco do futuro, Saldo no fim, Pode gastar
   *  hoje, Performance, Custo de vida. */
  kpis: ScenarioKpi[];
  /** Delta do saldo no fim do horizonte — muda a cada recomputo, então é ele que a região
   *  live da tela anuncia. */
  endDeltaCents: number;
  /** Resposta a "é seguro?" de relance, ANTES da grade de KPIs — já traduzida em manchete e
   *  subtítulo pela mesma fonte (`scenarioDeepestPoint`) que alimenta o card "Buraco do futuro". */
  verdict: ScenarioVerdict;
  /** Gate de financiamento (reserva pós-financiamento + economia após parcela), `null` sem
   *  empréstimo simulado — o componente só decide SE renderiza a linha, nunca a cor/rótulo. */
  loanGate: LoanGateView | null;
}

/** Uma perna do gate já resolvida: estado de método + textos formatados prontos pro badge. */
export interface LoanGateLeg {
  state: MethodState;
  /** `null` quando a fonte não tem "antes" (reserva sem mês completo realizado). */
  beforeText: string | null;
  afterText: string;
}

export interface LoanGateView {
  /** `null` quando `reserve_months_after_financing` é `null` (sem mês completo realizado). */
  reserve: LoanGateLeg | null;
  /** `null` quando alguma das duas pontas de `savings_rate_*_bps` é `null` (mediana de entradas
   *  zero). */
  savings: (LoanGateLeg & { popoverBody: string }) | null;
}

// ------------------------------------------------------- estados do método --

/** Buraco do futuro & Saldo no fim: o Termômetro canônico (`saldoBand`, limiares ABSOLUTOS,
 * nunca relativos ao baseline) — rótulos e cores usados verbatim. */
export function saldoState(cents: number): MethodState {
  const band = saldoBand(cents);
  const ok = band.key === "comfortable" || band.key === "ok";
  return {
    key: band.key,
    label: band.label,
    color: band.text,
    icon: ok ? "ok" : "alert",
  };
}

/** Performance: `performanceStatus` verbatim ("Sobrou dinheiro"/"Faltou dinheiro" — ambos
 * método). "Faltou dinheiro" é uma quebra real de limiar (disciplina do vermelho: cor cheia). */
export function performanceState(cents: number): MethodState {
  const s = performanceStatus(cents);
  const ok = s.level === "strong";
  return {
    key: s.label,
    label: s.label,
    color: ok ? "var(--success-400)" : "var(--danger-400)",
    icon: ok ? "ok" : "alert",
  };
}

/** Custo de vida: `custoVidaStatus` verbatim ("Dentro da renda" é método; "Acima da renda" é
 * copy do Neko para o estado ruim — ver totaisStatus.ts). Nesta superfície de decisão de alto
 * risco, "Acima da renda" é tratada como quebra real de limiar (disciplina do vermelho: cor
 * cheia) — mais rígida que o âmbar ambiente do card "Este mês" (TotaisScreen). */
export function custoVidaState(cost: number, income: number): MethodState {
  const s = custoVidaStatus(cost, income);
  const ok = s.label === "Dentro da renda";
  return {
    key: s.label,
    label: s.label,
    color: ok ? "var(--success-400)" : "var(--danger-400)",
    icon: ok ? "ok" : "alert",
  };
}

/** Pode gastar hoje: sem helper de método pronto — estado por valor+régua. `cents` nunca é
 * negativo (o motor já despeja no piso 0), então só há duas categorias. */
export function podeGastarState(
  cents: number,
  guardrail: "cash" | "savings",
): MethodState {
  if (cents > 0) {
    return {
      key: "livre",
      label: `Livre até ${fmtCompactBRL(cents)}`,
      color: "var(--success-400)",
      icon: "ok",
    };
  }
  return {
    key: "segure",
    label: "Segure hoje",
    color: "var(--warning-400)",
    icon: "alert",
    line:
      guardrail === "savings"
        ? "Limitado pela régua de poupança (20–30% ao ano), não pelo caixa."
        : "Limitado pelo caixa do mês, não pela régua de poupança.",
  };
}

/** Estado neutro para quando o cenário não tem NENHUM ponto de projeção —
 * nem `deepest_deficit` diário, nem `month_end` mensal. Nunca reutilizar `saldoState(0)` aqui:
 * 0 cai na banda "apertado" do Termômetro por coincidência aritmética do `?? 0`, não porque o
 * cenário tenha de fato um menor saldo — mostraria "Apertado" colorido sobre um dado inexistente.
 * `--text-faint` é a MESMA cor "sem valor" que `saldoBand(null)` já usa (nkFormat.ts). */
export const EMPTY_SCENARIO_STATE: MethodState = {
  key: "none",
  label: "—",
  color: "var(--text-faint)",
  icon: "none",
};

// ------------------------------------------------------ gate de financiamento --

/**
 * Semáforo de meses de reserva pós-financiamento (`LoanBreakdown.reserve_months_after_
 * financing`) — a escada do gate de financiamento do método, em 3 faixas: abaixo de 6 meses
 * é abaixo do mínimo; 6–12 é zona amarela; 12+ é paz (assumir compromisso novo sobe o alvo
 * de reserva para 12 meses). **12,0 exato = Paz**: a fonte define a faixa como "12+", então
 * a fronteira inferior é INCLUSIVA — divergência deliberada da convenção
 * limite-superior-inclusivo do Termômetro (`saldoBand`).
 */
export function reserveMonthsState(months: number): MethodState {
  if (months < 6) {
    return {
      key: "below-min",
      label: "Abaixo do mínimo",
      color: "var(--danger-400)",
      icon: "alert",
    };
  }
  if (months < 12) {
    return {
      key: "amber",
      label: "Zona amarela",
      color: "var(--warning-400)",
      icon: "alert",
    };
  }
  return {
    key: "peace",
    label: "Paz",
    color: "var(--primary-quiet-text)",
    icon: "ok",
  };
}

function formatReserveMonths(months: number): string {
  return months.toLocaleString("pt-BR", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

/**
 * Escada composta da régua "Economia após parcela" (2ª perna do gate de financiamento),
 * julgada sempre sobre o `afterBps` BRUTO (pode ser negativo; o clamp em 0% é só de exibição):
 * abaixo de 2000 bps a parcela fura o piso de 20% de poupança; passando o piso, uma parcela
 * que consome MAIS da metade da economia típica ainda trava o ritmo de patrimônio (regra da
 * metade — parcela exatamente igual à metade é paz). Fronteiras: 20,00% exato passa o piso.
 * Nenhuma das duas regras sozinha cobre os dois perfis: quem poupa pouco fura primeiro no
 * piso; quem poupa muito fura primeiro na metade.
 */
export function savingsAfterState(
  afterBps: number,
  installmentCents: number,
  economiaMedianCents: number,
): MethodState {
  if (afterBps < 2000) {
    return {
      key: "below-floor",
      label: "Abaixo do piso",
      color: "var(--danger-400)",
      icon: "alert",
    };
  }
  if (installmentCents * 2 > economiaMedianCents) {
    return {
      key: "half-rule",
      label: "Mais da metade da economia",
      color: "var(--warning-400)",
      icon: "alert",
    };
  }
  return {
    key: "peace",
    label: "Paz",
    color: "var(--primary-quiet-text)",
    icon: "ok",
  };
}

// TRUNCA (floor), nunca arredonda: 1999 bps arredondado viraria "20%" ao lado do rótulo
// "Abaixo do piso" — número e veredito se contradiriam na fronteira exata que o gate julga.
// Truncar nunca superestima a poupança, o viés conservador certo para um gate financeiro.
function formatSavingsRate(bps: number): string {
  return `${Math.floor(bps / 100)}%`;
}

/**
 * Copy didática do popover da 2ª perna. A frase final é DATA-DERIVADA: aparece quando a regra
 * da metade (ou a exaustão da economia) se materializa NESTA simulação — o estado amarelo e o
 * vermelho por excesso precisam da evidência em R$ que os disparou, nunca só do julgamento.
 */
function savingsPopoverBody(
  installmentCents: number,
  economiaMedianCents: number,
): string {
  const base =
    "Mediana da economia registrada menos a parcela nova, dividida pela mediana das entradas — últimos 6 meses completos, a mesma janela da reserva. Abaixo de 20% a parcela fura o piso de poupança do método (a meta de 20–30% se julga na média do ano). E mesmo acima do piso, uma parcela que consome mais da metade da sua economia típica trava o ritmo do patrimônio — pelo menos metade dela precisa continuar sobrando.";
  if (installmentCents > economiaMedianCents) {
    return `${base} A parcela (${fmtBRL(installmentCents)}) excede sua economia típica (${fmtBRL(economiaMedianCents)}).`;
  }
  if (installmentCents * 2 > economiaMedianCents) {
    return `${base} Nesta simulação, a parcela (${fmtBRL(installmentCents)}) consome mais da metade da sua economia típica (${fmtBRL(economiaMedianCents)}).`;
  }
  return base;
}

/** Resolve as duas pernas do gate de financiamento em estado + textos formatados. `null` sem
 *  empréstimo simulado — o componente só decide SE a linha renderiza. */
export function loanGateView(loan: LoanBreakdown | null): LoanGateView {
  if (!loan) {
    return { reserve: null, savings: null };
  }

  const reserve: LoanGateLeg | null =
    loan.reserve_months_after_financing != null
      ? {
          state: reserveMonthsState(loan.reserve_months_after_financing),
          beforeText:
            loan.reserve_months_before_financing != null
              ? formatReserveMonths(loan.reserve_months_before_financing)
              : null,
          afterText: formatReserveMonths(loan.reserve_months_after_financing),
        }
      : null;

  const savings =
    loan.savings_rate_before_bps != null && loan.savings_rate_after_bps != null
      ? {
          state: savingsAfterState(
            loan.savings_rate_after_bps,
            loan.loan_installment_cents,
            loan.economia_median_cents,
          ),
          beforeText: formatSavingsRate(loan.savings_rate_before_bps),
          // O "depois" negativo EXIBE 0%; o estado já foi julgado no bruto acima.
          afterText: formatSavingsRate(Math.max(0, loan.savings_rate_after_bps)),
          popoverBody: savingsPopoverBody(
            loan.loan_installment_cents,
            loan.economia_median_cents,
          ),
        }
      : null;

  return { reserve, savings };
}

// --------------------------------------------------------- delta & veredito --

/** Abaixo de R$1 de diferença é ruído de arredondamento, não um resultado — um card mostrando
 * "−R$ 0,09" em vermelho alarma por nada. Este limiar é sobre MATERIALIDADE (existe mudança
 * que importa?), então usa o valor absoluto em centavos direto, sem depender do sentido
 * (`sense`) — que só decide se um delta material é bom ou ruim, não se ele é relevante. */
export const DELTA_MATERIALITY_CENTS = 100;

/** Julga um delta em materialidade + sentido de melhora/piora. `better` só tem significado
 * quando `material` é `true`. */
export function deltaVerdict(deltaCents: number, sense: DeltaSense): DeltaVerdict {
  return {
    material: Math.abs(deltaCents) > DELTA_MATERIALITY_CENTS,
    // O sentido de melhora/piora vem do QUE o KPI considera bom (`sense`), NUNCA do sinal cru
    // do delta — o mesmo delta positivo é melhora num "maior é melhor" e piora num "menor é
    // melhor" (custo de vida).
    better: sense === "higher-better" ? deltaCents > 0 : deltaCents < 0,
  };
}

/** Veredito (Nível 1): a resposta a "é seguro?" de relance, ANTES da grade de KPIs —
 * determinístico a partir do menor saldo do CENÁRIO (`scenarioDeepestPoint`, o mesmo dado que
 * alimenta o card "Buraco do futuro"). O TOM vem do MESMO predicado do card (`saldoBand`, o
 * Termômetro canônico), em três níveis: banda negativa/crítica → risco; banda apertada →
 * intermediário honesto — sem isto o banner diria "no azul o ano todo" enquanto o card logo
 * abaixo mostra "Apertado" sobre o MESMO número; banda ok/folga → tranquilo. Tom
 * GPS-não-ameaça: cada ramo ruim sugere uma ação, não um alarme. Sem NENHUM ponto de projeção:
 * nível ok com a subline dizendo isso, em vez de inventar um menor saldo. */
export function scenarioVerdict(compare: ScenarioCompareDto): ScenarioVerdict {
  const point = scenarioDeepestPoint(compare);
  if (point == null) {
    return {
      tier: "ok",
      headline: "Este cenário se mantém no azul o ano todo.",
      subline: "Sem pontos de projeção no horizonte para apontar um menor saldo.",
    };
  }
  const { minCents, monthIdx } = point;
  const band = saldoBand(minCents);
  const monthLabel = (MES[monthIdx] ?? "").toLowerCase();
  if (band.key === "negative" || band.key === "critical") {
    return {
      tier: "risk",
      headline: `Fura o caixa em ${monthLabel} — faltam ${fmtCompactBRL(Math.abs(minCents))}.`,
      subline:
        "Antecipe uma entrada, reduza uma parcela ou cubra com um empréstimo antes desse mês.",
    };
  }
  if (band.key === "tight") {
    return {
      tier: "tight",
      headline: `Fica apertado em ${monthLabel} — menor saldo ${fmtCompactBRL(minCents)}.`,
      subline: "Segure gastos grandes perto dessa data ou reforce o colchão antes.",
    };
  }
  return {
    tier: "ok",
    headline: "Este cenário se mantém no azul o ano todo.",
    subline: `Menor saldo no período: ${fmtBRL(minCents)} — ${band.label}.`,
  };
}

// ------------------------------------------------------------- construção --

/** Menor saldo do CENÁRIO + mês (0–11) na melhor resolução disponível: `deepest_deficit`
 * (diária) quando o motor o tem; quando null, o mínimo do `scenario_month_end` (mensal — o
 * mesmo dado do gráfico); `null` sem projeção nenhuma. FONTE ÚNICA do banner de veredito E do
 * card "Buraco do futuro". Uma fonte única impede que o card use o fallback `?? 0` enquanto o
 * banner usa o mínimo mensal, evitando que ambos discordem sobre o mesmo dado. */
export function scenarioDeepestPoint(
  compare: ScenarioCompareDto,
): { minCents: number; monthIdx: number } | null {
  const deficit = compare.scenario_deepest_deficit;
  if (deficit) {
    return { minCents: deficit.balance_cents, monthIdx: monthOf(deficit.date) };
  }
  if (compare.scenario_month_end.length > 0) {
    const worst = compare.scenario_month_end.reduce((a, b) =>
      b.balance_cents < a.balance_cents ? b : a,
    );
    return { minCents: worst.balance_cents, monthIdx: worst.month - 1 };
  }
  return null;
}

export function scenariosView(compare: ScenarioCompareDto): ScenariosView {
  const lastMonthEnd = compare.month_end[compare.month_end.length - 1] ?? null;
  const endRealCents = lastMonthEnd?.real_balance_cents ?? 0;
  const endScenarioCents = lastMonthEnd?.scenario_balance_cents ?? 0;
  const endDeltaCents = lastMonthEnd?.delta_cents ?? 0;

  const realDeficit = compare.real_deepest_deficit?.balance_cents ?? 0;
  // Menor saldo do cenário pela MESMA derivação do banner (`scenarioDeepestPoint`) — nunca o
  // `?? 0` cru sobre o deficit diário: com deficit nulo mas `scenario_month_end` presente, o
  // card fabricava "cenário R$ 0,00" (+ delta fake) enquanto o banner logo acima caía
  // honestamente no mínimo mensal — banner e card discordando sobre o MESMO dado. Só sem
  // projeção NENHUMA (deficit E month_end vazios) o card rende o vazio neutro.
  const scenarioPoint = scenarioDeepestPoint(compare);
  const noScenarioProjection = scenarioPoint == null;
  // O 0 do fallback nunca renderiza: `emptyScenario` suprime manchete/evidência/delta.
  const scenarioDeficit = scenarioPoint?.minCents ?? 0;
  // Delta do backend quando existe (deficit diário nos DOIS ramos); senão derivado dos mesmos
  // números que a linha de evidência mostra — o chip nunca pode discordar da evidência.
  const deficitDelta =
    compare.deepest_deficit_delta_cents ??
    (scenarioPoint != null ? scenarioPoint.minCents - realDeficit : 0);

  return {
    endDeltaCents,
    verdict: scenarioVerdict(compare),
    loanGate: loanGateView(compare.loan),
    kpis: [
      {
        label: "Buraco do futuro",
        term: {
          title: "Buraco do futuro",
          body: "O menor saldo que sua projeção alcança daqui pra frente — o pior momento de caixa. Se ele fica negativo, você precisa de um plano antes de chegar lá.",
        },
        realCents: realDeficit,
        scenarioCents: scenarioDeficit,
        deltaCents: deficitDelta,
        sense: "higher-better",
        delta: deltaVerdict(deficitDelta, "higher-better"),
        realState: saldoState(realDeficit),
        scenarioState: noScenarioProjection
          ? EMPTY_SCENARIO_STATE
          : saldoState(scenarioDeficit),
        emptyScenario: noScenarioProjection,
      },
      {
        label: "Saldo no fim do horizonte",
        term: {
          title: "Saldo no fim",
          body: "O saldo projetado no último mês do horizonte se nada mudar.",
        },
        realCents: endRealCents,
        scenarioCents: endScenarioCents,
        deltaCents: endDeltaCents,
        sense: "higher-better",
        delta: deltaVerdict(endDeltaCents, "higher-better"),
        realState: saldoState(endRealCents),
        scenarioState: saldoState(endScenarioCents),
        emptyScenario: false,
      },
      {
        label: "Pode gastar hoje",
        term: {
          title: "Pode gastar hoje",
          body: "Quanto dá pra gastar agora sem furar o caixa do mês nem a régua de poupança de 20–30%.",
        },
        realCents: compare.real_safe_to_spend_today_cents,
        scenarioCents: compare.scenario_safe_to_spend_today_cents,
        deltaCents: compare.safe_to_spend_delta_cents,
        sense: "higher-better",
        delta: deltaVerdict(compare.safe_to_spend_delta_cents, "higher-better"),
        realState: podeGastarState(
          compare.real_safe_to_spend_today_cents,
          compare.real_binding_guardrail,
        ),
        scenarioState: podeGastarState(
          compare.scenario_safe_to_spend_today_cents,
          compare.scenario_binding_guardrail,
        ),
        emptyScenario: false,
      },
      {
        label: "Performance · mês atual",
        term: {
          title: "Performance",
          body: "Entradas menos as saídas do mês — fixas, diário, economia, cartão e a previsão do diário que ainda falta. A economia e essa previsão contam como saída, então o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
        },
        realCents: compare.real_performance_cents,
        scenarioCents: compare.scenario_performance_cents,
        deltaCents: compare.performance_delta_cents,
        sense: "higher-better",
        delta: deltaVerdict(compare.performance_delta_cents, "higher-better"),
        realState: performanceState(compare.real_performance_cents),
        scenarioState: performanceState(compare.scenario_performance_cents),
        emptyScenario: false,
      },
      {
        label: "Custo de vida",
        term: {
          title: "Custo de vida",
          body: "Quanto sai por mês pra manter sua vida — fixas + diário + cartão. Não inclui economia (poupança não é custo), e é sobre ele que a reserva se dimensiona.",
        },
        realCents: compare.real_cost_of_living_cents,
        scenarioCents: compare.scenario_cost_of_living_cents,
        deltaCents: compare.cost_of_living_delta_cents,
        sense: "lower-better",
        delta: deltaVerdict(compare.cost_of_living_delta_cents, "lower-better"),
        realState: custoVidaState(
          compare.real_cost_of_living_cents,
          compare.real_income_cents,
        ),
        scenarioState: custoVidaState(
          compare.scenario_cost_of_living_cents,
          compare.scenario_income_cents,
        ),
        emptyScenario: false,
      },
    ],
  };
}
