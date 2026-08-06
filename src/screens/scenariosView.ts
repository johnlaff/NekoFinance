import type { ScenarioCompareDto } from "../lib/api";
import { fmtCompactBRL, monthOf, saldoBand } from "../lib/nkFormat";
import { custoVidaStatus, performanceStatus } from "./totaisStatus";

// View-model puro da superfície de comparação real × cenário. Consome o DTO do motor
// (`get_scenario_forecast`) e produz os cinco cards de KPI já DECIDIDOS: rótulo, copy didática,
// os dois valores, o delta, o sentido do delta e o estado de método de cada lado. Nenhuma
// fronteira da grade de KPI mora na tela — o que a tela faz é pintar o que chega daqui. As
// réguas do gate de financiamento (reserva após financiar, economia após parcela) seguem em
// `scenarios.tsx`, ao lado dos badges que só elas alimentam.

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

export interface ScenarioKpi {
  label: string;
  term: MethodTerm;
  realCents: number;
  scenarioCents: number;
  deltaCents: number;
  sense: DeltaSense;
  realState: MethodState;
  scenarioState: MethodState;
  /** Cenário sem NENHUM ponto de projeção: `scenarioCents`/`deltaCents` são ruído e a tela
   * rende um vazio neutro em vez de fingir um valor. */
  emptyScenario: boolean;
}

export interface ScenariosView {
  /** Ordem por prioridade de decisão (padrão-Z): Buraco do futuro, Saldo no fim, Pode gastar
   *  hoje, Performance, Custo de vida. */
  kpis: ScenarioKpi[];
  /** Delta do saldo no fim do horizonte — muda a cada recomputo, então é ele que a região
   *  live da tela anuncia. */
  endDeltaCents: number;
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
