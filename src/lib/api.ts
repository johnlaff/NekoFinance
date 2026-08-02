import { Channel, invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

/**
 * Credencial do cliente OAuth, repassada ao backend (que a usa no refresh do token).
 *
 * O client é do tipo "Desktop app" (app instalado). O Google emite um `client_secret` também
 * para esse tipo e o endpoint de token o EXIGE no refresh: sem ele o refresh falha com
 * `client_secret is missing` e a conexão cai em ~1h, forçando reconexão constante. Para apps
 * desktop esse valor NÃO é confidencial pela definição do próprio Google — ele acompanha o
 * binário instalado inevitavelmente, e a doc do Google assume isso. Daí o nome público
 * `VITE_GOOGLE_DESKTOP_CLIENT_KEY`: chamá-lo de segredo prometeria um sigilo que um bundle de
 * browser não pode cumprir.
 *
 * Vem do build (`.env` local gitignored) — `null` quando ausente, e aí o backend cai no valor
 * que carrega.
 */
const clientSecretOrNull =
  (import.meta.env["VITE_GOOGLE_DESKTOP_CLIENT_KEY"] as string) || null;

export type AuthStatus = "connected" | "expired" | "disconnected" | "loading";

export interface MiaProcessor {
  name: string;
  role: string;
}

export interface MiaChecklistItem {
  title: string;
  detail: string;
}

export interface MiaConsentText {
  headline: string;
  processors: MiaProcessor[];
  paragraphs: string[];
  checklist: MiaChecklistItem[];
}

export interface MiaConsentView {
  granted: boolean;
  needs_renewal: boolean;
  granted_at: string | null;
  has_key: boolean;
  linked: boolean;
  text: MiaConsentText;
}

export function getMiaConsent(): Promise<MiaConsentView> {
  return invoke("get_mia_consent");
}

export function grantMiaConsent(): Promise<MiaConsentView> {
  return invoke("grant_mia_consent");
}

export function revokeMiaConsent(): Promise<MiaConsentView> {
  return invoke("revoke_mia_consent");
}

export function setMiaApiKey(key: string): Promise<MiaConsentView> {
  return invoke("set_mia_api_key", { key });
}

// --- Rodada da conversa (runtime) ---
//
// Espelha `MiaScreenEvent` do backend (`src-tauri/src/mia/screen_events.rs`) linha a linha —
// `kind` em snake_case é a etiqueta discriminante do evento, os demais campos são exatamente o
// que aquela linha carrega. Sem texto token a token: `answer_ready` publica a resposta inteira.

export type MiaErrorCode =
  | "consent_missing"
  | "provider_unavailable"
  | "rate_limited"
  | "provider_refused"
  | "protocol_violation"
  | "turn_cap"
  | "tool_call_cap"
  | "cost_cap"
  | "time_cap"
  | "cancelled"
  | "ungrounded"
  | "context_cap";

export type MiaStopReason =
  | "consent_missing"
  | "answered"
  | "turn_cap"
  | "tool_call_cap"
  | "cost_cap"
  | "time_cap"
  | "cancelled"
  | "ungrounded"
  | "failed";

export type MiaScreenEvent =
  | { kind: "run_started"; run_id: string; model: string; endpoint: string }
  | { kind: "tool_started"; id: string; tool: string }
  | { kind: "tool_finished"; id: string; tool: string; ok: boolean }
  | { kind: "proposal_ready"; id: string; proposal: unknown }
  | { kind: "answer_ready"; text: string; provenance: "calculo" | "metodo" }
  | {
      kind: "usage";
      model: string;
      endpoint: string;
      prompt_tokens: number;
      completion_tokens: number;
      /** Nulo é lacuna declarada pelo provedor — nunca renderizar como zero. */
      cost_micro_usd: number | null;
      attempts: number;
    }
  | { kind: "error"; code: MiaErrorCode; message: string; fix: string }
  | { kind: "run_finished"; stop: MiaStopReason };

/**
 * Abre uma rodada da conversa ligada. Devolve o `run_id` assim que o backend a registra — antes
 * de qualquer evento, para o cancelamento poder alcançá-la mesmo antes da primeira resposta.
 * Fora do Tauri a promessa rejeita com um erro honesto: não existe rodada para simular.
 */
export function runMiaRound(
  question: string,
  onEvent: (event: MiaScreenEvent) => void,
): Promise<string> {
  if (!isTauri) {
    return Promise.reject(new Error("A conversa ligada só funciona no app desktop."));
  }
  const channel = new Channel<MiaScreenEvent>();
  channel.onmessage = onEvent;
  return invoke<string>("run_mia_round", { question, onEvent: channel });
}

/** Interrompe uma rodada em curso. Fora do Tauri não há rodada — gesto sem efeito. */
export function cancelMiaRound(runId: string): Promise<void> {
  if (!isTauri) return Promise.resolve();
  return invoke("cancel_mia_round", { runId });
}

/** Aprova a proposta da conversa (evento `proposal_ready`): o backend revalida hash,
 *  `data_revision` e validade antes de gravar, e devolve o id do lançamento criado. `hash`
 *  amarra a aprovação ao envelope que a pessoa viu — nunca o valor editado no cartão. */
export function approveMiaProposal(
  proposalId: number,
  payloadJson: string,
  hash: string,
): Promise<string> {
  return invoke("approve_mia_proposal", { proposalId, payloadJson, hash });
}

/** Recusa a proposta — gesto explícito, nunca inferido de texto no chat. */
export function rejectMiaProposal(proposalId: number): Promise<void> {
  return invoke("reject_mia_proposal", { proposalId });
}

/** Uma linha da conversa guardada, como o backend a devolve — espelha `StoredMessage`
 *  (`src-tauri/src/mia/store.rs`) linha a linha. `answer` é opaco: o formato é o que a
 *  própria interface gravou como `MiaAnswer` serializado, e valê-lo cabe a quem lê. */
export interface StoredMiaMessage {
  author: "voce" | "mia";
  question: string | null;
  answer: unknown;
  at_iso: string;
}

/** A conversa guardada, na ordem em que foi dita. */
export function loadMiaConversation(): Promise<StoredMiaMessage[]> {
  return invoke("load_mia_conversation");
}

/** Grava o par pergunta/resposta que a tela acabou de desenhar. `answerJson` é o `MiaAnswer`
 *  já serializado — o backend guarda o JSON como ele vem, sem conhecer a forma dele. */
export function appendMiaExchange(question: string, answerJson: string): Promise<void> {
  return invoke("append_mia_exchange", { question, answerJson });
}

/** Apaga a conversa de verdade: o que a pessoa leu e o rastro técnico das rodadas somem juntos. */
export function deleteMiaConversation(): Promise<void> {
  return invoke("delete_mia_conversation");
}

export interface UpcomingInvoice {
  account_id: string;
  card_name: string;
  due_date: string;
  amount_cents: number;
  status: "prevista" | "aberta" | "fechada" | "paga";
  owner_name: string;
  /** Existe Entrada vinculada à fatura — o vínculo, mesmo quando o valor esperado é zero. */
  has_refund_expectation: boolean;
  /** Σ das Entradas vinculadas a esta fatura, limitado ao total dela — a parte que volta. */
  refund_expected_cents: number;
}

export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents, from the forecast engine. */
  balance: number;
  daily_budget: number;
  /** Procedência do teto exibido: veredito escolhido, estimativa da média, ou sem registro. */
  daily_ceiling_source: "chosen" | "estimate" | "none";
  /**
   * Operandos da estimativa do teto — a tela imprime esta conta em vez de descrevê-la.
   * Ausente quando o teto é escolhido: número digitado não tem conta a mostrar.
   */
  daily_ceiling_estimate: {
    /** Gasto variável somado do mês da base (magnitude, centavos). */
    variable_cents: number;
    /** Dias do mês da base — o divisor da média. */
    days: number;
    /** Mês da base, `YYYY-MM`. */
    month: string;
  } | null;
  /** Overlay: existe proposta da cerimônia do teto aguardando confirmação. */
  ceiling_proposal_pending: boolean;
  daily_spend_today: number;
  /** Compras de cartão realizadas hoje (magnitude, centavos) — o total do bloco do dia no modo cartão. */
  card_spend_today_cents: number;
  reserve_months: number;
  /** Estado epistêmico da reserva (veredito · retrato vivo · zerada · sem registro). */
  reserve_state: "verdict" | "estimate" | "zero" | "no_record";
  /** Meses completos que sustentam o custo de vida da régua. */
  reserve_basis_months: number;
  /** Alvo da reserva (custo de vida × meses do método). Leitura patrimonial, não trava o dia. */
  reserve_target_cents: number;
  /** Quanto a reserva passa do alvo; `null` enquanto ela está sendo construída. */
  reserve_surplus_cents: number | null;
  reserve_trend: string;
  /** Modo de gasto derivado dos próprios dados. */
  spending_mode: "debit" | "card";
  /** `false` quando o modo é o default de dado insuficiente, não uma leitura da janela. */
  spending_mode_detected: boolean;
  /** Gate composto de legitimidade do modo cartão. */
  card_gate: "alive" | "below" | "unknown";
  card_gate_economy: "alive" | "below" | "unknown";
  /** Percentual bruto (bps) por trás da perna de economia — a matemática que o gate mostra
   * ("14%, falta 6 p/ 20%"); `null` só quando a perna é `unknown` (sem renda anual). */
  card_gate_economy_bps: number | null;
  card_gate_reserve: "alive" | "below" | "unknown";
  /** Cartão do mês corrente (realizado + projetado), magnitude em centavos. */
  cartao_month_cents: number;
  /** Próximo dia de fatura a partir de hoje, quando existe. */
  next_fatura_date: string | null;
  next_fatura_amount_cents: number;
  upcoming_invoices: UpcomingInvoice[];
  transaction_count: number;
  /** ISO date (YYYY-MM-DD) of the most recent non-projection transaction, or null if none. */
  last_real_tx_date: string | null;
}

/** Tag anexada a um lançamento (chip do Livro-razão). */
export interface TagRef {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
}

/** Uma parte de um lançamento itemizado (breakdown da nota de célula). */
export type LineItemKind =
  "entrada" | "saida" | "cartao" | "diario" | "economia" | "patrimonio" | "ajuste";

export interface LineItem {
  id: string;
  transaction_id: string;
  amount_cents: number;
  description: string;
  position: number;
  /**
   * Classificação derivada da seção da nota; nunca de nome de banco/descrição.
   * Pai `income` → "entrada" (os kinds de seção só fatiam saídas).
   */
  kind: LineItemKind;
  /** Cabeçalho de seção cru da nota (ex.: "CONTAS:"); null = sem seção. Usado ao propor
   * `match_section` para marcar o item como obrigação recorrente. */
  section: string | null;
}

export interface TransactionRow {
  id: string;
  type: string;
  /** Magnitude em centavos; o SINAL armazenado não é contrato (linhas importadas e
   *  manuais divergem — o backend compara sempre por valor absoluto). Exibição
   *  deriva o sinal do tipo de movimento: entrada positiva, resto negativo. */
  amount: number;
  description: string;
  date: string;
  payment_method: string;
  is_projection: boolean;
  /** Despesa fixa (coluna Saída) vs variável (Diário). Distingue Saída × Diário no Livro-razão. */
  is_fixed: boolean;
  /** Titulares distintos das parcelas (multi-titular). Vazio = sem split por pessoa. */
  owners: string[];
  /** Tags anexadas (diagnóstico), mostradas como chips. */
  tags: TagRef[];
  /** Proveniência: "projetado" | "importado" | "manual" | "conciliado". */
  provenance: string;
  /** Partes itemizadas da nota (vazio = lançamento não itemizado); somente leitura. */
  line_items: LineItem[];
  /** Vencimento opcional ("YYYY-MM-DD"); null = sem lembrete de conta. Consultivo. */
  due_date: string | null;
  /** Posição 1-based na série de parcelas; null = não é lançamento de série. */
  installment_index: number | null;
  /** Total de parcelas da série; null = não é lançamento de série. */
  installment_total: number | null;
  /** Há dinheiro que volta ligado à linha (Entrada vinculada ↔ fatura/compra/série). */
  has_refund_link: boolean;
}

export interface SheetInfo {
  title: string;
  sheet_id: number;
}

/** Torna visível uma nota que não deu para itemizar, ou itens cujo somatório diverge
 * do total da célula (a célula continua dona do total — isto só reporta). */
export type DiagKind =
  "NoteNotItemized" | "ItemsDoNotSumToCell" | "MonthlyBudgetPlanNote";

export interface ImportDiagnostic {
  sheet: string;
  /** Rótulo sintético (sem endereço real de célula) — só para exibição. */
  cell: string;
  kind: DiagKind;
  detail: string;
}

/** Retorno estruturado dos comandos de import — `count` é sempre numérico (usado
 * aritmeticamente pelo `importAllTabs`); `diagnostics` nunca substitui a contagem. */
export interface ImportOutcome {
  count: number;
  summary: string;
  diagnostics: ImportDiagnostic[];
}

export interface UserSpreadsheet {
  id: string;
  name: string;
  modified_time: string;
}

export interface SheetPreview {
  headers: string[];
  rows: string[][];
  total_rows: number;
}

export interface SheetMappingEntry {
  id: string;
  sheet_name: string;
  column_letter: string;
  column_header: string | null;
  target_table: string;
  target_field: string;
  date_direction: string;
  layout_id: string | null;
  block_offset: number;
  is_active: number;
}

export interface SheetLayout {
  id: string;
  sheet_name: string;
  year: number | null;
  month_names_row: number;
  header_row: number;
  data_start_row: number;
  day_column: number;
  block_size: number;
  date_direction: string;
}

export interface AppInfo {
  version: string;
  db_path: string;
}

export interface ForecastDay {
  date: string;
  income_cents: number;
  fixed_out_cents: number;
  daily_out_cents: number;
  /** Economia (guardar) lançada no dia — sai do saldo de gasto, mas é poupança. */
  economia_cents: number;
  balance_cents: number;
}

export interface DayPoint {
  date: string;
  balance_cents: number;
}

export interface MonthEnd {
  year: number;
  month: number;
  balance_cents: number;
}

/** Per-month decision metrics (Caixa ≠ Performance). All money in cents. */
export interface MonthMetric {
  year: number;
  month: number;
  income_cents: number;
  /** Renda do mês na view PERFORMANCE (perna positiva de `performance_cents`) — diverge de
   * `income_cents` quando uma tag entra/sai da régua de Performance mas não da de Economia. */
  income_performance_cents: number;
  performance_cents: number;
  cost_of_living_cents: number;
  /** Saídas fixas realizadas (coluna Saída sem cartão/economia/patrimônio). */
  fixed_out_cents: number;
  /** Diário realizado (coluna Diário). */
  daily_out_cents: number;
  /** Diário realizado na view DIÁRIO MÉDIO — numerador de `real_daily_avg_cents` (diverge de
   * `daily_out_cents` quando uma tag entra/sai só da régua de Diário médio). */
  daily_avg_out_cents: number;
  /** Previsão de diário do mês (teto dos dias futuros + pré-lançados); desconta a Performance. */
  daily_projected_cents: number;
  /** Cartão realizado, bucket próprio dentro do custo de vida. */
  cartao_cents: number;
  /** Diário médio = Σ diário realizado ÷ dias decorridos (D/N). */
  real_daily_avg_cents: number;
  /** Economia lançada no mês (numerador do Economizado%). */
  economia_cents: number;
  /** Patrimônio/long-term/illiquid, fora de custo de vida e Economia% acessível. */
  patrimonio_cents: number;
  savings_rate_bps: number;
}

/** Poupança do ano: realizada (honesta) vs projetada (otimista se o futuro está incompleto). */
export interface AnnualSavings {
  /** Entradas dos meses vividos do ano, o corrente incluído — denominador da régua. */
  realized_income_cents: number;
  /** NET superávit (renda − saída) dos meses vividos — o "colchão", distinto da Economia. */
  realized_savings_cents: number;
  realized_rate_bps: number;
  /** Economia REGISTRADA do ano (transfers→reserva) — numerador do Economizado% do método. */
  registered_economia_cents: number;
  /** Patrimônio realizado do ano (previdência/ilíquido) — a outra leitura do popover. */
  patrimonio_cents: number;
  /** Numerador da régua: Economia lançada nos meses vividos. Patrimônio fica de fora. */
  economia_ruler_cents: number;
  economia_ruler_rate_bps: number;
  /** Estado da régua de economia: sem registro ⇒ a UI exibe a sobra como estimativa marcada. */
  economia_state: "verdict" | "no_record";
  projected_income_cents: number;
  projected_savings_cents: number;
  projected_rate_bps: number;
  target_bps: number;
}

/** Cobertura de um mês futuro: quanto do gasto típico já está lançado (previsibilidade). */
export interface MonthCoverage {
  year: number;
  month: number;
  projected_outflow_cents: number;
  baseline_outflow_cents: number;
  coverage_bps: number;
  is_complete: boolean;
  estimated_missing_cents: number;
}

/** Projection DTO from the deterministic engine. All money in cents. */
export interface Forecast {
  today: string;
  horizon_end: string;
  /** Poupança do ano — realizada vs projetada. */
  annual_savings: AnnualSavings;
  /** Cobertura por mês futuro (vazio se a projeção está completa). */
  coverage: MonthCoverage[];
  /** Gasto típico/mês (mediana realizada). 0 = sem histórico → previsibilidade indeterminada. */
  baseline_outflow_cents: number;
  /** Último mês cuja projeção é confiável ("YYYY-MM"); null se não há baseline para avaliar. */
  trusted_through_month: string | null;
  /** Soma do que falta lançar nos meses incompletos. */
  total_missing_cents: number;
  /** "Pode gastar hoje" honesto: o mais apertado de caixa × poupança. */
  safe_to_spend_today_cents: number;
  cash_headroom_cents: number;
  /** `null` quando a régua de poupança está inativa (mês sem renda) → só o caixa decide. */
  savings_headroom_cents: number | null;
  binding_guardrail: "cash" | "savings";
  savings_target_bps: number;
  deepest_deficit: DayPoint | null;
  daily: ForecastDay[];
  month_end: MonthEnd[];
  months: MonthMetric[];
}

/** Pocket types accepted by `create_account` (credit_card is the invoice slice). */
export type PocketType =
  "bank" | "wallet" | "business" | "savings" | "meal_voucher" | "pension" | "fgts";

export interface PocketAccount {
  id: string;
  name: string;
  type: string;
  liquidity: string | null;
  balance: number;
  institution: string | null;
}

/** Liquidity-grouped balances. All money in cents. */
export interface Pockets {
  liquid_cents: number;
  reserve_cents: number;
  restricted_cents: number;
  illiquid_cents: number;
  net_worth_cents: number;
  accounts: PocketAccount[];
}

export function getPockets(): Promise<Pockets> {
  return invoke("get_pockets");
}

export function createAccount(
  name: string,
  accountType: PocketType,
  balanceCents: number,
  institution?: string,
): Promise<string> {
  return invoke("create_account", {
    name,
    accountType,
    balanceCents,
    institution: institution ?? null,
  });
}

export function getDashboardSummary(): Promise<DashboardSummary> {
  return invoke("get_dashboard_summary");
}

export function getRecentTransactions(limit: number): Promise<TransactionRow[]> {
  return invoke("get_recent_transactions", { limit });
}

/** Uma conta a vencer: um lançamento com `due_date` na janela consultada. */
export interface UpcomingBill {
  id: string;
  description: string;
  amount: number; // magnitude (centavos)
  due_date: string;
  is_projection: boolean;
}

/** Contas com `due_date` nos próximos `days` dias (inclui hoje), ordenadas por vencimento. */
export function getUpcomingBills(days: number): Promise<UpcomingBill[]> {
  return invoke("get_upcoming_bills_cmd", { days });
}

/** Partes itemizadas de um lançamento (breakdown da nota de célula). */
// react-doctor-disable-next-line deslop/unused-export -- ponte para leitura unitária de itens; o Livro-razão usa o batch line_items, enquanto este getter expõe o comando Tauri por lançamento
export function getLineItems(transactionId: string): Promise<LineItem[]> {
  return invoke<LineItem[]>("get_line_items_cmd", { transactionId });
}

/** Uma parte itemizada EDITÁVEL no form. Sem `id`/`transaction_id`: o backend
 * recria as linhas (clear + reinsert) a cada edição. `position` é a ordem 0-based. */
export interface LineItemDraft {
  amount_cents: number; // magnitude positiva, centavos inteiros
  description: string;
  position: number;
}

/** Substitui TODAS as partes de um lançamento e fixa o total do pai = Σ partes. As
 * partes ficam marcadas como editadas localmente — sobrevivem ao próximo re-import enquanto a nota
 * da planilha não mudar. Lista vazia é rejeitada pelo backend (use o valor simples nesse caso). */
export function updateTransactionItems(
  transactionId: string,
  items: LineItemDraft[],
): Promise<void> {
  return invoke("update_transaction_items_cmd", { transactionId, items });
}

/** Cria um lançamento manual. Com `recurrence`, gera a série projetada. Para `transfer` (Economia),
 * `toAccountId` é obrigatório e precisa ser uma conta reserve/illiquid. Retorna o id criado. */
export function createTransaction(input: {
  txnType: "income" | "expense" | "transfer";
  amountCents: number;
  description: string | null;
  date: string;
  paymentMethod: string | null;
  isFixed: boolean;
  tagIds: string[];
  recurrence: { frequency: Frequency; repetitions: number } | null;
  /** Obrigatório (não-nulo) quando `txnType = "transfer"`. Ausente/nulo nos demais (income/expense). */
  toAccountId?: string | null;
  /** Vencimento opcional ("YYYY-MM-DD") p/ o calendário de contas. Não afeta o Saldo. */
  dueDate?: string | null;
}): Promise<string> {
  return invoke("create_transaction", input);
}

/** Apaga um lançamento manual pelo id. Importados da planilha são rejeitados pelo backend. */
export function deleteTransaction(id: string): Promise<void> {
  return invoke("delete_transaction_cmd", { id });
}

/** Edita um lançamento manual (tipo, valor, descrição, método, fixo, data). `txnType` precisa
 * viajar: trocar entrada↔saída muda renda↔despesa (sinal no forecast). */
export function updateTransaction(
  id: string,
  edit: {
    txnType: string;
    amountCents: number;
    description: string | null;
    paymentMethod: string | null;
    isFixed: boolean;
    date: string;
  },
): Promise<void> {
  return invoke("update_transaction_cmd", { id, ...edit });
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
}

/** Backup atômico (VACUUM INTO) do banco local em `destPath` (escolhido no save dialog). Retorna
 * o caminho gravado. Local-first: o dono do dado leva uma cópia íntegra para onde quiser. */
export function backupDatabase(destPath: string): Promise<string> {
  return invoke("backup_database", { destPath });
}

export function getForecast(): Promise<Forecast> {
  return invoke("get_forecast");
}

export function checkAuthStatus(): Promise<AuthStatus> {
  return invoke("check_auth_status");
}

export function startOAuthFlow(clientId: string): Promise<string> {
  return invoke("start_oauth_flow", { clientId, clientSecret: clientSecretOrNull });
}

export function disconnectGoogle(): Promise<void> {
  return invoke("disconnect_google");
}

export function listUserSpreadsheets(clientId: string): Promise<UserSpreadsheet[]> {
  return invoke("list_user_spreadsheets", {
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function listSheetNames(
  spreadsheetId: string,
  clientId: string,
): Promise<SheetInfo[]> {
  return invoke("list_sheet_names", {
    spreadsheetId,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function fetchSheetPreview(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<SheetPreview> {
  return invoke("fetch_sheet_preview", {
    spreadsheetId,
    sheetName,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function getSheetMappings(sheetName: string): Promise<SheetMappingEntry[]> {
  return invoke("get_sheet_mappings", { sheetName });
}

export function detectSheetLayout(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<SheetLayout> {
  return invoke("detect_sheet_layout", {
    spreadsheetId,
    sheetName,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function saveSheetMapping(
  mappingId: string,
  blockOffset: number,
  isActive: boolean,
): Promise<void> {
  return invoke("save_sheet_mapping", { mappingId, blockOffset, isActive });
}

// --- Write-back (protegido por flag e aprovação humana) ---

/** Uma célula que o write-back tocaria: A1, valor atual, valor proposto, se mudou. */
export interface CellWrite {
  a1: string;
  row: number;
  col: number;
  date: string;
  kind: string;
  current: string;
  proposed: string;
  /** Magnitude em centavos efetivamente escrita (o número, não a string pt-BR de exibição). */
  value_cents: number;
  changed: boolean;
}

/**
 * Resultado RICO da prévia: o diff + um token de frescura + flags de pré-condição.
 * `preview_revision` é o `modifiedTime` do Drive no instante da prévia; o apply o re-verifica e
 * ABORTA se a planilha tiver mudado (edição concorrente → re-revisão). `conflicts_pending` espelha
 * o gate de conflito do backend (a UI desabilita o envio).
 */
export interface WriteBackPreviewResult {
  cells: CellWrite[];
  preview_revision: string;
  conflicts_pending: boolean;
}

/** Estado da flag de write-back. `false` → envio ao Sheets desabilitado (só preview). */
export function writeBackEnabled(): Promise<boolean> {
  return invoke("write_back_enabled");
}

/** Prévia RICA READ-ONLY: diff + `preview_revision` (frescura) + conflitos pendentes.
 * A UI endurecida usa isto para amarrar a aprovação ao que foi visto. */
export function previewWriteBackStatus(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<WriteBackPreviewResult> {
  return invoke("preview_write_back_status", {
    spreadsheetId,
    sheetName,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

/** Resultado do write-back: nº de células escritas + um aviso não-bloqueante quando a
 * NOTA de célula itemizada não pôde ser gravada (o valor/fórmula já foi escrito com sucesso). */
export interface WriteBackResult {
  written: number;
  note_warning: string | null;
}

/** Aplica o write-back (escreve as células divergentes). Atrás da flag: rejeita enquanto
 * desligado (nunca escreve). `previewRevision` (do `previewWriteBackStatus`) amarra a aprovação à
 * revisão vista: se a planilha mudou, o backend aborta com erro de re-revisão. Células itemizadas
 * (≥2 partes) escrevem `=SUM(...)` + nota por-parte; as normais seguem número cru. */
export function applyWriteBack(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
  previewRevision?: string | null,
): Promise<WriteBackResult> {
  return invoke("apply_write_back", {
    spreadsheetId,
    sheetName,
    clientId,
    clientSecret: clientSecretOrNull,
    previewRevision: previewRevision ?? null,
  });
}

/** Prévia RICA READ-ONLY da Economia: diff + `preview_revision` + conflitos pendentes. */
export function previewEconomiaWriteBackStatus(
  spreadsheetId: string,
  year: number,
  clientId: string,
): Promise<WriteBackPreviewResult> {
  return invoke("preview_economia_write_back_status", {
    spreadsheetId,
    year,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

/** Aplica o write-back da Economia. Mesma flag; `previewRevision` amarra a aprovação à revisão
 * vista (aborta em edição concorrente). Retorna nº de células escritas. */
export function applyEconomiaWriteBack(
  spreadsheetId: string,
  year: number,
  clientId: string,
  previewRevision?: string | null,
): Promise<number> {
  return invoke("apply_economia_write_back", {
    spreadsheetId,
    year,
    clientId,
    clientSecret: clientSecretOrNull,
    previewRevision: previewRevision ?? null,
  });
}

// --- Conciliação avançada: gate de conflito ---

/** Um conflito de import: um campo onde local e planilha divergiram do base (merge de 3 vias). */
export interface ImportConflict {
  id: string;
  transaction_id: string;
  field: string;
  base_value: string | null;
  local_value: string;
  sheet_value: string;
}

/** Conflitos de import pendentes para o gate humano. */
export function getImportConflicts(): Promise<ImportConflict[]> {
  return invoke("get_import_conflicts");
}

/** Resolve um conflito: "sheet" (planilha vence) ou "local" (mantém a edição). */
export function resolveImportConflict(
  id: string,
  choice: "sheet" | "local",
): Promise<void> {
  return invoke("resolve_import_conflict", { id, choice });
}

// --- Preferências locais (KV) ---

/** Lê uma preferência local. `null` quando nunca foi gravada. */
export function getAppSetting(key: string): Promise<string | null> {
  return invoke("get_app_setting", { key });
}

/** Grava uma preferência local (sobrescreve). */
export function setAppSetting(key: string, value: string): Promise<void> {
  return invoke("set_app_setting", { key, value });
}

/**
 * Chave da preferência de exibição do recibo, válida em todo o app. O nome persistido guarda
 * o prefixo da conversa, onde o recibo nasceu: renomeá-lo descartaria a escolha já gravada.
 */
export const SHOW_RECEIPT = "mia_show_receipt";

/**
 * Lê uma preferência de liga/desliga. O default mora aqui, e não em cada tela que consulta a
 * chave: quem lê e quem escreve precisam concordar sobre o que "nunca gravada" significa.
 */
export async function getFlagSetting(key: string, fallback: boolean): Promise<boolean> {
  const value = await getAppSetting(key);
  return value === null ? fallback : value !== "false";
}

/**
 * Grava o teto de gasto Diário por dia configurado pelo dono.
 * `amountCents = 0` desativa o teto explícito — o engine usa o fallback de média.
 */
export function upsertDailyBudget(amountCents: number): Promise<void> {
  return invoke("upsert_daily_budget", { amountCents });
}

// --- Quebra por categoria do orçamento Diário ---

/** Uma categoria do orçamento mensal do Diário (leitura). `amount_cents` é o alvo mensal positivo. */
export interface DailyBudgetCategory {
  id: string;
  name: string;
  amount_cents: number;
  position: number;
}

/** Categoria editável do orçamento Diário (escrita). Sem `id`: o backend recria as linhas. */
export interface DailyBudgetCategoryInput {
  name: string;
  amount_cents: number; // centavos positivos
  position: number;
}

/** Lê as categorias do orçamento Diário ativo. Vetor vazio = sem quebra definida. */
export function getDailyBudgetCategories(): Promise<DailyBudgetCategory[]> {
  return invoke("get_daily_budget_categories_cmd");
}

/**
 * Grava o teto total do Diário + uma quebra opcional por categoria.
 * `categories` pode ser vazio (mantém o total-only e limpa qualquer quebra anterior).
 * `amountCents = 0` desativa o teto explícito (o engine cai no fallback de média).
 * `divisorDays` persiste o divisor da cerimônia (total mensal ÷ dias = teto/dia).
 */
export function upsertDailyBudgetWithCategories(
  amountCents: number,
  categories: DailyBudgetCategoryInput[],
  divisorDays?: number | null,
): Promise<void> {
  return invoke("upsert_daily_budget_with_categories_cmd", {
    amountCents,
    categories,
    divisorDays: divisorDays ?? null,
  });
}

/** Orçamento Diário ativo por inteiro. `per_day_cents = 0` ⇒ sem teto estipulado. */
export interface DailyBudget {
  per_day_cents: number;
  divisor_days: number | null;
  /** `YYYY-MM` da cerimônia que produziu o teto — a idade que a tela conta. */
  ceremony_month: string | null;
  /** Nota crua da planilha que sustenta o teto; `null` quando a cerimônia foi feita no app. */
  source_note: string | null;
  categories: DailyBudgetCategory[];
}

export function getDailyBudget(): Promise<DailyBudget> {
  return invoke("get_daily_budget_cmd");
}

/** Proposta de teto lida da cerimônia documentada na planilha (uma pendente por vez). */
export interface CeilingProposal {
  id: string;
  per_day_cents: number;
  divisor_days: number;
  source_month: string;
  /** Nota crua da célula, reproduzida como prova; `null` em propostas anteriores à coluna. */
  raw_note: string | null;
  items: { name: string; amount_cents: number }[];
}

export function getCeilingProposal(): Promise<CeilingProposal | null> {
  return invoke("get_ceiling_proposal_cmd");
}

/** Aceite explícito: grava o orçamento (valor/dia + itens + divisor) e resolve a proposta. */
export function acceptCeilingProposal(proposalId: string): Promise<void> {
  return invoke("accept_ceiling_proposal_cmd", { proposalId });
}

/** Dispensa a proposta — a mesma nota da planilha não volta a propor. */
export function dismissCeilingProposal(proposalId: string): Promise<void> {
  return invoke("dismiss_ceiling_proposal_cmd", { proposalId });
}

// --- Lembrete agendado no nível do sistema ---

/**
 * Registra (ou atualiza) o lembrete agendado no nível do SISTEMA no horário `HH:MM`
 * informado, para que dispare mesmo com o app fechado. Idempotente. Melhor-esforço:
 * o laço em-app continua como fallback, então uma falha aqui não deve bloquear o salvamento.
 */
export function registerOsReminder(timeHhmm: string): Promise<void> {
  return invoke("register_os_reminder", { timeHhmm });
}

/** Remove o lembrete agendado no nível do sistema. No-op quando não há registro. */
export function unregisterOsReminder(): Promise<void> {
  return invoke("unregister_os_reminder");
}

// --- Eventos do backend (sync em segundo plano) ---

/** Carga útil do evento `neko://sync-done` emitido pela tarefa de sync de leitura. */
export interface SyncDonePayload {
  conflict_count: number;
}

/** Nome do evento emitido pelo backend quando um sync em segundo plano conclui. */
export const SYNC_DONE_EVENT = "neko://sync-done";

/** Função para cancelar uma assinatura de evento. */
export type UnlistenFn = () => void;

/**
 * Assina um evento do backend Tauri, degradando com elegância fora do shell (web preview, e2e
 * mockado): se não estamos no Tauri ou o IPC de eventos não existe no mock, retorna um cancelador
 * no-op em vez de quebrar. A importação é dinâmica para não puxar o módulo de eventos no bundle web.
 */
export function listenEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauri) return Promise.resolve(() => undefined);
  return import("@tauri-apps/api/event")
    .then(({ listen }) => listen<T>(event, (e) => handler(e.payload)))
    .catch(() => () => undefined);
}

export function importSheetData(
  spreadsheetId: string,
  sheetName: string,
  profileId: string,
  clientId: string,
): Promise<ImportOutcome> {
  return invoke("import_sheet_data", {
    spreadsheetId,
    sheetName,
    profileId,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

/** Importa a aba `Economia` como anotação mensal de Economia. Retorna nº de meses importados. */
export function importEconomiaSheet(
  spreadsheetId: string,
  clientId: string,
): Promise<number> {
  return invoke("import_economia_sheet", {
    spreadsheetId,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function importLocalXlsx(
  filePath: string,
  profileId: string,
): Promise<ImportOutcome> {
  return invoke("import_local_xlsx", { filePath, profileId });
}

// --- Tags ---

export interface Tag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
  /** Os 4 interruptores de régua — cada um tira os lançamentos desta tag da régua
   * correspondente (Performance, Custo de vida, Economia, Diário médio). O Saldo não tem
   * flag: é garantia estrutural, sempre conta o movimento de caixa real. */
  exclude_from_performance: boolean;
  exclude_from_cost_of_living: boolean;
  exclude_from_savings: boolean;
  exclude_from_daily_avg: boolean;
}

export interface AnnualMetrics {
  year: number;
  months: MonthMetric[];
}

export function getAnnualMetrics(year: number): Promise<AnnualMetrics> {
  return invoke("get_annual_metrics", { year });
}

/** Veredito do ano contra a faixa 20–30% do método. */
export type BandVerdict =
  "no_record" | "zero_by_choice" | "below_band" | "in_band" | "above_band";

/** Um mês do ano na ótica do método. */
export interface AnnualRulerMonth {
  month: number;
  /** Saída total do mês (renda − performance) — a figura que alimenta o gasto típico. */
  outflow_cents: number;
  lived: boolean;
  /** Mês à frente cuja saída lançada não alcança o piso de lastro. */
  suspect: boolean;
  /** Quanto faltaria lançar para o mês custar o típico; zero em mês vivido ou lastreado. */
  missing_cents: number;
}

export interface YearEnd {
  end_month: number | null;
  end_balance_cents: number | null;
  /** O saldo se os meses sem lastro até o fim custassem o típico; nulo sem silêncio a descontar. */
  end_balance_typical_cents: number | null;
}

/**
 * A régua anual do método, computada no motor (`forecast::annual_ruler`). É a MESMA leitura que a
 * conversa responde — a tela compõe a apresentação, nunca a régua.
 */
export interface AnnualRuler {
  year: number;
  lived_months: number;
  future_months: number;
  /** Gasto típico = mediana das saídas dos meses vividos. */
  typical_spend_cents: number;
  income_lived_cents: number;
  economia_lived_cents: number;
  /** Sobra dos meses vividos (Performance somada) — o colchão. */
  surplus_lived_cents: number;
  income_year_cents: number;
  economia_year_cents: number;
  recorded_months: number;
  avg_income_cents: number;
  lived_bps: number | null;
  projected_bps: number | null;
  /** O percentual que JULGA: o vivido enquanto houver mês sem lastro, senão o do ano. */
  bps: number | null;
  scope_lived: boolean;
  has_data: boolean;
  /** Falta para o piso de 20%; negativo = o piso já passou. */
  shortfall_lived_cents: number;
  shortfall_year_cents: number;
  per_month_shortfall_cents: number | null;
  verdict: BandVerdict;
  band: { floor_bps: number; target_bps: number; ceiling_bps: number };
  months: AnnualRulerMonth[];
  /** Saldo de fim de cada mês do ano (corrente da planilha até o vivido, projeção à frente). */
  month_end: MonthEnd[];
  year_end: YearEnd;
}

export function getAnnualRuler(year: number): Promise<AnnualRuler> {
  return invoke("get_annual_ruler", { year });
}

export interface MonthGridDay {
  date: string;
  day: number;
  income_cents: number;
  fixed_out_cents: number;
  daily_out_cents: number;
  balance_cents: number | null;
}

export function getMonthGrid(year: number, month: number): Promise<MonthGridDay[]> {
  return invoke("get_month_grid", { year, month });
}

/** Lançamentos do mês ("YYYY-MM") — o Livro-razão é mês-escopado; a janela
 *  recente pura cortaria meses antigos no limite. */
export function getMonthTransactions(month: string): Promise<TransactionRow[]> {
  return invoke("get_recent_transactions", { limit: 1000, month });
}

/** Timestamp UTC ("YYYY-MM-DD HH:MM:SS") da última sincronização com a planilha
 *  (import ou write-back), ou null quando não há histórico. */
export function lastSyncAt(): Promise<string | null> {
  return invoke("last_sync_at");
}

export function listTags(): Promise<Tag[]> {
  return invoke("list_tags_cmd");
}

/** Cria uma tag livre (nome + cor + emoji opcional). `isSpecial` fixa a tag no topo ("! Pagar"). */
export function createTag(
  name: string,
  color: string,
  emoji: string | null,
  isSpecial: boolean,
): Promise<string> {
  return invoke("create_tag_cmd", { name, color, emoji, isSpecial });
}

export function setTransactionTags(
  transactionId: string,
  tagIds: string[],
): Promise<void> {
  return invoke("set_transaction_tags_cmd", { transactionId, tagIds });
}

/** Renomeia/recolore uma tag (nome + cor + emoji). `is_special` re-deriva da convenção "!". */
export function updateTag(
  tagId: string,
  name: string,
  color: string,
  emoji: string | null,
): Promise<void> {
  return invoke("update_tag_cmd", { tagId, name, color, emoji });
}

/** Liga/desliga os 4 interruptores de régua de uma tag (UPDATE único, idempotente). */
export function updateTagRulers(
  tagId: string,
  excludeFromPerformance: boolean,
  excludeFromCostOfLiving: boolean,
  excludeFromSavings: boolean,
  excludeFromDailyAvg: boolean,
): Promise<void> {
  return invoke("update_tag_rulers_cmd", {
    tagId,
    excludeFromPerformance,
    excludeFromCostOfLiving,
    excludeFromSavings,
    excludeFromDailyAvg,
  });
}

/** Estado epistêmico de uma pessoa na seção "Dinheiro de terceiros" — deriva de vínculos
 * reais (marcador de nota, split, cartão adicional, expectativa de reembolso), nunca fabricado. */
export type ThirdPartyState = "favor" | "open" | "series" | "settled" | "none";

export interface TagsScreenThirdParty {
  person_id: string;
  name: string;
  /** Saiu no mês da tela (magnitude). */
  out_cents: number;
  /** Voltou no mês da tela, realizado (magnitude). */
  back_cents: number;
  /** Retorno esperado vinculado, ainda não realizado (magnitude). */
  expected_cents: number;
  state: ThirdPartyState;
  /** Dias desde a saída/expectativa — só em estado "open". */
  open_since_days: number | null;
  /** Parcelas já voltaram / total da série — só em estado "series". */
  series_done: number | null;
  series_total: number | null;
  /** Data da quitação — só em estado "settled". */
  settled_date: string | null;
}

/** Os 4 interruptores de régua de uma tag, na chave positiva do DTO da tela
 * (`true` = a régua CALCULA esta tag) — inverso do flag de escrita `exclude_from_*`. */
export interface TagRulerFlags {
  performance: boolean;
  cost_of_living: boolean;
  savings: boolean;
  daily_avg: boolean;
}

/** Contribuição marginal de uma tag a cada régua: o motor recomputa o mês com o flag
 * invertido e reporta a diferença — o mesmo número serve à frase ligada e desligada. */
export interface TagRulerEffects {
  performance_delta_cents: number;
  cost_delta_cents: number;
  /** Δ na renda-base da view Economia. */
  savings_base_delta_cents: number;
  /** Δ na economia registrada (reconciliada com a anotação da aba). */
  savings_amount_delta_cents: number;
  daily_avg_delta_cents: number;
}

export interface TagsScreenTag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
  counts_in: TagRulerFlags;
  /** O que a tag movimentou no mês (semântica do antigo tag_totals) — nunca a autoridade
   * do veredito, só o rótulo de "linha à direita". */
  month_total_cents: number;
  txn_count: number;
  effects: TagRulerEffects;
}

export interface TagsScreenVerdict {
  /** Custo de vida com os interruptores atuais — a manchete A/C. */
  cost_current_cents: number;
  /** Custo de vida se todas as tags contassem em todas as réguas — a cauda de A. */
  cost_all_on_cents: number;
  /** Média mensal de terceiros detectados (12 meses + corrente, meses com movimento) — manchete B. */
  third_party_avg_cents: number | null;
  third_party_people: number;
  has_exceptions: boolean;
}

export interface TagsScreenDto {
  /** "YYYY-MM" do mês da tela. */
  month: string;
  verdict: TagsScreenVerdict;
  third_parties: TagsScreenThirdParty[];
  tags: TagsScreenTag[];
  /** Última sincronização com a planilha — a idade do dado exibida na manchete F quando a
   * leitura atual falha (a falha em si é o erro da query, nunca este campo sozinho). */
  last_sync_at: string | null;
}

/** DTO completo da tela Tags: veredito, dinheiro de terceiros, exceções e rótulos —
 * exceção × rótulo é derivado de `tags[].counts_in` no frontend, não um campo à parte. */
export function getTagsScreen(year: number, month: number): Promise<TagsScreenDto> {
  return invoke("get_tags_screen", { year, month });
}

// --- Multi-titular / split (read-side) ---

export interface OwnerTotal {
  owner_person_id: string;
  owner_name: string;
  /** Soma (centavos, valor absoluto) das parcelas do titular no mês. */
  total_cents: number;
}

export function ownerTotalsForMonth(
  year: number,
  month: number,
): Promise<OwnerTotal[]> {
  return invoke("owner_totals_for_month_cmd", { year, month });
}

// --- Recorrências / séries ---

export type Frequency = "diaria" | "semanal" | "mensal";

export interface SeriesEdit {
  amount: number;
  description: string | null;
  paymentMethod: string | null;
  isFixed: boolean;
}

/** Reajusta a ocorrência indicada e todas as posteriores (o passado fica intacto). */
export function updateSeriesFrom(
  transactionId: string,
  edit: SeriesEdit,
): Promise<number> {
  return invoke("update_series_from_cmd", { transactionId, ...edit });
}

/** Reajusta toda a série de uma vez. */
export function updateSeriesAll(
  recurrenceId: string,
  edit: SeriesEdit,
): Promise<number> {
  return invoke("update_series_all_cmd", { recurrenceId, ...edit });
}

/** Apaga a ocorrência indicada e todas as posteriores ("deste ponto em diante"). O passado
 * realizado fica intacto. Retorna quantas ocorrências foram removidas. */
export function deleteSeriesFrom(transactionId: string): Promise<number> {
  return invoke("delete_series_from_cmd", { transactionId });
}

/** Apaga TODA a série recorrente + a linha de recorrência. Retorna quantas ocorrências foram
 * removidas. `recurrenceId` é o prefixo do id da ocorrência ("uuid:index" → "uuid"). */
export function deleteSeriesAll(recurrenceId: string): Promise<number> {
  return invoke("delete_series_all_cmd", { recurrenceId });
}

// --- Obrigações recorrentes ---
//
// `obligation` é uma EXTENSÃO do Neko, não um conceito do método/planilha: a planilha não
// guarda nenhum vínculo entre as 12 ocorrências mensais de um item recorrente ("Aluguel" é só
// uma linha repetida dentro da célula Saída, mês a mês, sem id nenhum ligando as doze). Aqui o
// usuário nomeia o item recorrente UMA vez e o Neko resolve quais `line_item`s casam — sempre
// via prévia confirmada pelo usuário, nunca por inferência silenciosa.

/** Uma ocorrência (`line_item`) que casou com a regra de uma obrigação. */
export interface ObligationLineItem {
  line_item_id: string;
  transaction_id: string;
  amount_cents: number;
  description: string;
  /** Data ISO ("YYYY-MM-DD") do lançamento pai — a nota não carrega data própria. */
  date: string;
}

/** Total de um mês (`YYYY-MM`) das ocorrências casadas de uma obrigação. */
export interface ObligationMonthTotal {
  year: number;
  month: number;
  total_cents: number;
  count: number;
}

/** Uma obrigação recorrente confirmada pelo usuário: regra de casamento (descrição/seção já
 * normalizadas) + o kind derivado da seção (mesmos kinds de `LineItemKind`, exceto "entrada"). */
export interface Obligation {
  id: string;
  person_id: string;
  name: string;
  match_desc: string;
  match_section: string | null;
  kind: string;
}

/** Prévia do casamento — SEM salvar nada. Chame antes de `createObligation` para mostrar "isto
 * vai agrupar N lançamentos" e só gravar após confirmação explícita do usuário. */
export function previewObligationMatches(
  matchDesc: string,
  matchSection: string | null,
): Promise<ObligationLineItem[]> {
  return invoke("preview_obligation_matches_cmd", { matchDesc, matchSection });
}

/** Cria a obrigação (regra normalizada no backend). Chame só após o usuário confirmar a prévia
 * de `previewObligationMatches`. */
export function createObligation(
  name: string,
  matchDesc: string,
  matchSection: string | null,
): Promise<string> {
  return invoke("create_obligation_cmd", { name, matchDesc, matchSection });
}

export function listObligations(): Promise<Obligation[]> {
  return invoke("list_obligations_cmd");
}

/** Apaga a obrigação (a regra). Nunca apaga/edita os `line_item`s que ela agrupava — é
 * view/índice, não dono do dado. */
export function deleteObligation(id: string): Promise<void> {
  return invoke("delete_obligation_cmd", { id });
}

/** Todas as ocorrências atualmente casadas por uma obrigação salva. */
// react-doctor-disable-next-line deslop/unused-export -- ponte para ocorrências cruas de uma obrigação; a UI consome o agregado mensal via obligationHistory, enquanto este getter preserva o detalhe por ocorrência
export function obligationItems(obligationId: string): Promise<ObligationLineItem[]> {
  return invoke("obligation_items_cmd", { obligationId });
}

/** Totais por mês das ocorrências de uma obrigação — a série que a planilha não guarda. */
export function obligationHistory(
  obligationId: string,
): Promise<ObligationMonthTotal[]> {
  return invoke("obligation_history_cmd", { obligationId });
}

// --- Cenários "e se" ---
//
// Um `scenario` é um rótulo para um conjunto de linhas HIPOTÉTICAS (`transaction.scenario_id`),
// invisíveis a todo o resto do app (forecast real, write-back, dashboard). Um `scenario_override`
// é uma ação (`suppress`/`replace`) sobre uma obrigação ou uma série recorrente,
// aplicada SÓ na projeção do cenário — nunca no livro-razão real.

export interface Scenario {
  id: string;
  name: string;
  person_id: string;
}

export interface ScenarioChange {
  op: "add" | "remove" | "replace";
  description: string;
  from_date: string;
  old_amount_cents: number | null;
  new_amount_cents: number | null;
}

export interface LoanBreakdown {
  loan_principal_cents: number;
  loan_installment_cents: number;
  loan_term_months: number;
  loan_monthly_rate_bps: number;
  loan_total_paid_cents: number;
  loan_total_cost_cents: number;
  /**
   * Régua canônica de reserva ANTES do financiamento: saldo das contas de reserva ÷ custo de
   * vida típico (mediana dos meses completos) — a mesma conta do dashboard. `null` quando não
   * há mês completo realizado.
   */
  reserve_months_before_financing: number | null;
  /** A mesma régua com a parcela somada ao denominador: reserva ÷ (típico + parcela). */
  reserve_months_after_financing: number | null;
  /**
   * Segunda perna do gate: percentual poupado típico ANTES da parcela, em bps (mediana da
   * economia registrada ÷ mediana das entradas, últimos 6 meses completos — mesma janela da
   * régua de reserva). `null` quando a mediana de entradas é 0 — a linha some.
   */
  savings_rate_before_bps: number | null;
  /**
   * O mesmo percentual com a parcela descontada da economia típica, BRUTO (sem clamp) —
   * negativo quando a parcela excede a economia típica; o clamp em 0% é só de exibição e a
   * escada julga sempre este valor.
   */
  savings_rate_after_bps: number | null;
  /** Mediana mensal da economia registrada (centavos): insumo da regra da metade e do popover. */
  economia_median_cents: number;
}

export interface ScenarioMonthEnd {
  year: number;
  month: number;
  real_balance_cents: number;
  scenario_balance_cents: number;
  delta_cents: number;
}

/** Compare de forecast real × cenário (o núcleo do "e se"). Todo dinheiro em centavos. */
export interface ScenarioCompareDto {
  scenario_id: string;
  scenario_name: string;

  real_today: string;
  real_horizon_end: string;
  real_month_end: MonthEnd[];
  real_deepest_deficit: DayPoint | null;
  real_performance_cents: number;
  real_safe_to_spend_today_cents: number;
  real_binding_guardrail: "cash" | "savings";
  real_cost_of_living_cents: number;
  /** Renda do mês corrente (Entradas): classifica Custo de vida
   * ("Dentro da renda"/"Acima da renda") sem re-derivar a renda no frontend. */
  real_income_cents: number;

  scenario_month_end: MonthEnd[];
  scenario_deepest_deficit: DayPoint | null;
  scenario_performance_cents: number;
  scenario_safe_to_spend_today_cents: number;
  scenario_binding_guardrail: "cash" | "savings";
  scenario_cost_of_living_cents: number;
  scenario_income_cents: number;

  month_end: ScenarioMonthEnd[];
  deepest_deficit_delta_cents: number | null;
  performance_delta_cents: number;
  safe_to_spend_delta_cents: number;
  cost_of_living_delta_cents: number;

  changes: ScenarioChange[];
  loan: LoanBreakdown | null;
}

export function createScenario(name: string): Promise<Scenario> {
  return invoke("create_scenario_cmd", { name });
}

export function listScenarios(): Promise<Scenario[]> {
  return invoke("list_scenarios_cmd");
}

/** Apaga o cenário — cascateia as linhas hipotéticas e os overrides (FK ON DELETE CASCADE). */
export function deleteScenario(id: string): Promise<void> {
  return invoke("delete_scenario_cmd", { id });
}

/** Insere uma linha hipotética "e se". `description` é obrigatória (aparece no compare). Nunca
 * muta `account.balance` (mesma política do lançamento manual real). `date` anterior ao mês
 * corrente é rejeitada — cairia fora da janela da projeção e sumiria em silêncio. */
export function addScenarioTransaction(input: {
  scenarioId: string;
  txnType: "income" | "expense" | "transfer";
  amountCents: number;
  description: string;
  date: string;
  paymentMethod?: string | null;
  isFixed?: boolean;
  toAccountId?: string | null;
  dueDate?: string | null;
}): Promise<string> {
  return invoke("add_scenario_transaction_cmd", {
    scenarioId: input.scenarioId,
    txnType: input.txnType,
    amountCents: input.amountCents,
    description: input.description,
    date: input.date,
    paymentMethod: input.paymentMethod ?? null,
    isFixed: input.isFixed ?? false,
    toAccountId: input.toAccountId ?? null,
    dueDate: input.dueDate ?? null,
  });
}

/** Parâmetros de um empréstimo hipotético — a série (principal + parcelas) é sempre derivada
 * deles pela tabela PRICE no backend, nunca editada linha a linha. */
export interface ScenarioLoanInput {
  scenarioId: string;
  principalCents: number;
  termMonths: number;
  rateBps: number;
  disbursementDate: string;
  firstInstallmentDate: string;
  description: string;
}

/** A entidade `scenario_loan` persistida: fonte do cabeçalho do grupo e do formulário de
 * edição pré-preenchido. */
export interface ScenarioLoanRow {
  id: string;
  scenario_id: string;
  principal_cents: number;
  rate_bps: number;
  term_months: number;
  disbursement_date: string;
  first_installment_date: string;
  description: string;
}

/** Cria a entidade + principal + todas as parcelas em uma única transação. Devolve o id do
 * empréstimo — a UI foca/realça o grupo recém-criado por ele. */
export function createScenarioLoan(input: ScenarioLoanInput): Promise<string> {
  return invoke("create_scenario_loan_cmd", { input });
}

/** Atualiza os parâmetros e REGENERA a série inteira sob a mesma identidade, em uma única
 * transação — parcelas removidas à mão são restauradas (a UI avisa antes). */
export function updateScenarioLoan(
  loanId: string,
  input: ScenarioLoanInput,
): Promise<void> {
  return invoke("update_scenario_loan_cmd", { loanId, input });
}

/** Remove o empréstimo inteiro (entidade + principal + parcelas), atomicamente. */
export function deleteScenarioLoan(scenarioId: string, loanId: string): Promise<void> {
  return invoke("delete_scenario_loan_cmd", { scenarioId, loanId });
}

export function listScenarioLoans(scenarioId: string): Promise<ScenarioLoanRow[]> {
  return invoke("list_scenario_loans_cmd", { scenarioId });
}

/** Apagar a última linha restante de um empréstimo apaga também o registro do empréstimo, na
 * mesma transação (um empréstimo existe enquanto tiver ao menos uma linha). */
export function deleteScenarioTransaction(
  scenarioId: string,
  txnId: string,
): Promise<void> {
  return invoke("delete_scenario_transaction_cmd", { scenarioId, txnId });
}

/** Uma linha hipotética crua do cenário. `loan_id` presente = linha de um empréstimo (agrupe
 * por ele); `override_id` presente = linha de uma série de substituição (some com o override em
 * cascata). */
export interface ScenarioTransactionRow {
  id: string;
  type: string;
  amount: number;
  description: string;
  date: string;
  loan_id: string | null;
  override_id: string | null;
}

/** Lista as linhas hipotéticas do cenário — a fonte da lista editável do side-sheet (permite
 * apagar uma linha adicionada em uma sessão anterior). */
export function listScenarioTransactions(
  scenarioId: string,
): Promise<ScenarioTransactionRow[]> {
  return invoke("list_scenario_transactions_cmd", { scenarioId });
}

/** A SÉRIE de substituição de um override `replace` (opcional): o backend gera UMA linha por
 * ocorrência suprimida (datas derivadas do alvo, `>= from_date`), todas donas do `override_id`
 * via FK — é isso que permite ao compare fundir velho→novo e faz a série morrer em cascata com o
 * alvo. Sem campo de data (as datas vêm do alvo). `amount_cents` é o novo valor de CADA
 * ocorrência. Defaults: `txn_type = "expense"`, `is_fixed = true`. */
export interface ReplacementInput {
  amount_cents: number;
  description?: string | null;
  txn_type?: string | null;
  payment_method?: string | null;
  is_fixed?: boolean | null;
}

/** Cria um override (`suppress`/`replace`) escopado a uma obrigação OU a uma recorrência —
 * exatamente uma das duas (o banco endurece via CHECK XOR). Um segundo override para o mesmo
 * alvo no mesmo cenário é rejeitado. Para `op = "replace"`, passe `replacement` para o backend
 * criar a linha de substituição pareada (compare emite UMA entrada fundida old→new). */
export function setScenarioOverride(input: {
  scenarioId: string;
  op: "suppress" | "replace";
  fromDate: string;
  obligationId?: string | null;
  recurrenceId?: string | null;
  replacement?: ReplacementInput | null;
}): Promise<string> {
  return invoke("set_scenario_override_cmd", {
    scenarioId: input.scenarioId,
    op: input.op,
    fromDate: input.fromDate,
    obligationId: input.obligationId ?? null,
    recurrenceId: input.recurrenceId ?? null,
    replacement: input.replacement ?? null,
  });
}

/** Uma recorrência REAL do livro-razão, oferecível como alvo de override. A tabela `recurrence`
 * não guarda rótulo — ele vem da descrição da ocorrência mais antiga. */
export interface RecurrenceTarget {
  id: string;
  description: string;
  frequency: string;
  first_date: string;
}

/** Recorrências com ≥ 1 ocorrência real, para o grupo "Recorrências" do seletor de alvo. */
export function listRecurrenceTargets(): Promise<RecurrenceTarget[]> {
  return invoke("list_recurrence_targets_cmd");
}

/** Uma ocorrência real de uma recorrência (data + magnitude) — o análogo de `obligationItems`
 * para recorrências: alimenta a prévia "afeta N ocorrências". */
export interface RecurrenceOccurrence {
  date: string;
  amount_cents: number;
}

export function recurrenceOccurrences(
  recurrenceId: string,
): Promise<RecurrenceOccurrence[]> {
  return invoke("recurrence_occurrences_cmd", { recurrenceId });
}

/** O compare de forecast real × cenário — o núcleo do "e se". */
export function getScenarioForecast(scenarioId: string): Promise<ScenarioCompareDto> {
  return invoke("get_scenario_forecast_cmd", { scenarioId });
}

// --- Cartões e faturas ---

/** Resumo derivado de uma fatura; o total declarado, quando existe, é a autoridade. */
export interface InvoiceSummary {
  id: string;
  cycle_month: string;
  closing_date: string;
  due_date: string;
  status: "prevista" | "aberta" | "fechada" | "paga";
  stated_total_cents: number | null;
  purchases_sum_cents: number;
  effective_total_cents: number;
  reconciliation_delta_cents: number | null;
}

export interface Card {
  id: string;
  name: string;
  institution: string | null;
  owner_name: string;
  linked_account_id: string | null;
  closing_day: number;
  due_day: number;
  credit_limit_cents: number | null;
  aliases: string[];
  open_invoice: InvoiceSummary | null;
  next_due: InvoiceSummary | null;
}

export interface CardPurchase {
  txn_id: string;
  date: string;
  description: string;
  amount_cents: number;
  owner_name: string;
  series_id: string | null;
  installment_label: string | null;
  is_projection: boolean;
}

export interface Refund {
  txn_id: string;
  date: string;
  amount_cents: number;
  description: string;
  is_projection: boolean;
}

export interface SubInvoice {
  account_id: string;
  card_name: string;
  owner_name: string;
  effective_total_cents: number;
}

/** `InvoiceSummary` é achatado pelo `#[serde(flatten)]` do DTO Rust. */
export interface InvoiceDetail extends InvoiceSummary {
  purchases: CardPurchase[];
  refunds: Refund[];
  sub_invoices: SubInvoice[];
  emitter_total_cents: number;
}

export interface CardProposal {
  id: string;
  alias: string;
  display_name: string;
  source_month: string;
  status: string;
  /** Todas as grafias com que a planilha nomeia este cartão, a identidade na frente. */
  aliases: string[];
}

export function listCards(): Promise<Card[]> {
  return invoke("list_cards");
}
export function listInvoices(accountId: string): Promise<InvoiceSummary[]> {
  return invoke("list_invoices", { accountId });
}
export function getInvoice(invoiceId: string): Promise<InvoiceDetail> {
  return invoke("get_invoice", { invoiceId });
}
export function registerCardPurchase(input: {
  cardAccountId: string;
  amountCents: number;
  description?: string | null;
  date: string;
  refundCents?: number | null;
  tagIds: string[];
}): Promise<string> {
  return invoke("register_card_purchase", {
    ...input,
    description: input.description ?? null,
    refundCents: input.refundCents ?? null,
  });
}
export function moveCardPurchase(
  txnId: string,
  targetCycleMonth: string,
): Promise<void> {
  return invoke("move_card_purchase", { txnId, targetCycleMonth });
}
export function setInvoiceStatedTotal(
  invoiceId: string,
  statedTotalCents: number | null,
): Promise<void> {
  return invoke("set_invoice_stated_total", { invoiceId, statedTotalCents });
}
/** Corrige as datas de UM ciclo, sem tocar no molde do cartão. */
export function setInvoiceDates(input: {
  invoiceId: string;
  closingDate: string;
  dueDate: string;
}): Promise<void> {
  return invoke("set_invoice_dates", input);
}
export function createCardSeries(input: {
  cardAccountId: string;
  description: string;
  amountCents: number;
  count: number | null;
  startDate: string;
  refundCents?: number | null;
  tagIds: string[];
}): Promise<string> {
  return invoke("create_card_series", {
    ...input,
    refundCents: input.refundCents ?? null,
  });
}
export function updateCardSeries(
  seriesId: string,
  description: string,
  amountCents: number,
): Promise<void> {
  return invoke("update_card_series", { seriesId, description, amountCents });
}
export function cancelCardSeries(
  seriesId: string,
  fromCycleMonth: string,
): Promise<void> {
  return invoke("cancel_card_series", { seriesId, fromCycleMonth });
}
export function deleteCardSeries(seriesId: string): Promise<void> {
  return invoke("delete_card_series", { seriesId });
}
export function createRefundExpectation(
  invoiceId: string,
  amountCents: number,
  description?: string | null,
): Promise<string> {
  return invoke("create_refund_expectation", {
    invoiceId,
    amountCents,
    description: description ?? null,
  });
}
export function linkRefund(input: {
  txnId: string;
  refundInvoiceId?: string | null;
  refundTxnId?: string | null;
  refundSeriesId?: string | null;
}): Promise<void> {
  return invoke("link_refund", {
    txnId: input.txnId,
    refundInvoiceId: input.refundInvoiceId ?? null,
    refundTxnId: input.refundTxnId ?? null,
    refundSeriesId: input.refundSeriesId ?? null,
  });
}
export function unlinkRefund(txnId: string): Promise<void> {
  return invoke("unlink_refund", { txnId });
}
export function listCardProposals(): Promise<CardProposal[]> {
  return invoke("list_card_proposals");
}
export function acceptCardProposal(input: {
  proposalId: string;
  closingDay?: number | null;
  dueDay?: number | null;
  ownerPersonName?: string | null;
  linkedAccountId?: string | null;
}): Promise<string> {
  return invoke("accept_card_proposal", {
    ...input,
    closingDay: input.closingDay ?? null,
    dueDay: input.dueDay ?? null,
    ownerPersonName: input.ownerPersonName ?? null,
    linkedAccountId: input.linkedAccountId ?? null,
  });
}
/** Resolve a proposta como apelido de um cartão que já existe, sem criar outra conta. */
export function attachCardProposal(input: {
  proposalId: string;
  accountId: string;
}): Promise<void> {
  return invoke("attach_card_proposal", input);
}
export function dismissCardProposal(proposalId: string): Promise<void> {
  return invoke("dismiss_card_proposal", { proposalId });
}
export function createCardAccount(input: {
  name: string;
  institution?: string | null;
  closingDay?: number | null;
  dueDay?: number | null;
  creditLimitCents?: number | null;
  ownerPersonName?: string | null;
  linkedAccountId?: string | null;
  aliases: string[];
}): Promise<string> {
  return invoke("create_card_account", {
    ...input,
    institution: input.institution ?? null,
    closingDay: input.closingDay ?? null,
    dueDay: input.dueDay ?? null,
    creditLimitCents: input.creditLimitCents ?? null,
    ownerPersonName: input.ownerPersonName ?? null,
    linkedAccountId: input.linkedAccountId ?? null,
  });
}
export function updateCardAccount(input: {
  accountId: string;
  name: string;
  institution?: string | null;
  closingDay?: number | null;
  dueDay?: number | null;
  creditLimitCents?: number | null;
  aliases: string[];
}): Promise<void> {
  return invoke("update_card_account", {
    ...input,
    institution: input.institution ?? null,
    closingDay: input.closingDay ?? null,
    dueDay: input.dueDay ?? null,
    creditLimitCents: input.creditLimitCents ?? null,
  });
}

/** Ferramenta determinística (tabela PRICE) para pré-visualizar a parcela de um empréstimo antes
 * de confirmar as linhas hipotéticas — nunca matemática livre de LLM. */
export function priceInstallment(
  principalCents: number,
  monthlyRateBps: number,
  termMonths: number,
): Promise<number> {
  return invoke("price_installment_cmd", {
    principalCents,
    monthlyRateBps,
    termMonths,
  });
}
