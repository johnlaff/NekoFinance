import { invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

/**
 * Client secret do OAuth, repassado ao backend (que já o usa no refresh do token).
 *
 * O client é do tipo "Desktop app" (app instalado). O Google emite um `client_secret` também
 * para esse tipo e o endpoint de token o EXIGE no refresh: sem ele o refresh falha com
 * `client_secret is missing` e a conexão cai em ~1h, forçando reconexão constante. Para apps
 * desktop esse "secret" NÃO é confidencial pela definição do próprio Google — ele acompanha o
 * binário instalado inevitavelmente, e a doc do Google assume isso. Antes passávamos `null`
 * (confiando num env `GOOGLE_CLIENT_SECRET` do processo Rust, que NÃO existe no .exe em
 * runtime) — por isso o refresh nunca funcionava.
 *
 * Agora lemos do build (`VITE_GOOGLE_CLIENT_SECRET`, do `.env` local gitignored) — `null` quando
 * ausente (cai no fallback do backend).
 */
const clientSecretOrNull =
  (import.meta.env["VITE_GOOGLE_CLIENT_SECRET"] as string) || null;

export type AuthStatus = "connected" | "expired" | "disconnected" | "loading";

export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents (forecast engine, spec 003). */
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  reserve_months: number;
  reserve_trend: string;
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

/** Uma parte de um lançamento itemizado (breakdown da nota de célula, plano 035). */
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
}

export interface TransactionRow {
  id: string;
  type: string;
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
  /** Partes itemizadas da nota (vazio = lançamento não itemizado). Plano 035 — só leitura. */
  line_items: LineItem[];
  /** Vencimento opcional ("YYYY-MM-DD"); null = sem lembrete de conta (plano 045). Consultivo. */
  due_date: string | null;
  /** Posição 1-based na série de parcelas; null = não é lançamento de série (plano 045). */
  installment_index: number | null;
  /** Total de parcelas da série; null = não é lançamento de série (plano 045). */
  installment_total: number | null;
}

export interface SheetInfo {
  title: string;
  sheet_id: number;
}

/** Plano 070: torna visível uma nota que não deu para itemizar, ou itens cujo somatório diverge
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
  performance_cents: number;
  cost_of_living_cents: number;
  /** Saídas fixas realizadas (coluna Saída sem cartão/economia/patrimônio). */
  fixed_out_cents: number;
  /** Diário realizado (coluna Diário). */
  daily_out_cents: number;
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
  realized_income_cents: number;
  /** NET superávit (renda − saída) — o "colchão" do Neko, distinto da Economia registrada. */
  realized_savings_cents: number;
  realized_rate_bps: number;
  /** Economia REGISTRADA do ano (transfers→reserva) — numerador do Economizado% do método. */
  registered_economia_cents: number;
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

/** Projection DTO from the deterministic engine (spec 005). All money in cents. */
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

/** Liquidity-grouped balances (spec 007). All money in cents. */
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

/** Uma conta a vencer (plano 045): um lançamento com `due_date` na janela consultada. */
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

/** Partes itemizadas de um lançamento (breakdown da nota de célula, plano 035). */
// react-doctor-disable-next-line deslop/unused-export -- plano 035: ponte do frontend (comando Tauri pronto/testado); o Livro-razão usa o batch line_items, este getter sob demanda atende o plano 036
export function getLineItems(transactionId: string): Promise<LineItem[]> {
  return invoke<LineItem[]>("get_line_items_cmd", { transactionId });
}

/** Uma parte itemizada EDITÁVEL no form (plano 036). Sem `id`/`transaction_id`: o backend
 * recria as linhas (clear + reinsert) a cada edição. `position` é a ordem 0-based. */
export interface LineItemDraft {
  amount_cents: number; // magnitude positiva, centavos inteiros
  description: string;
  position: number;
}

/** Substitui TODAS as partes de um lançamento e fixa o total do pai = Σ partes (plano 036). As
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
  /** Vencimento opcional ("YYYY-MM-DD") p/ o calendário de contas (plano 045). Não afeta o Saldo. */
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

// --- Write-back (spec 018, atrás de flag desligada) ---

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
 * Resultado RICO da prévia (plano 028): o diff + um token de frescura + flags de pré-condição.
 * `preview_revision` é o `modifiedTime` do Drive no instante da prévia; o apply o re-verifica e
 * ABORTA se a planilha tiver mudado (edição concorrente → re-revisão). `conflicts_pending` espelha
 * o gate de conflito do backend (a UI desabilita o envio). `multi_card_warning` é não-bloqueante.
 */
export interface WriteBackPreviewResult {
  cells: CellWrite[];
  preview_revision: string;
  conflicts_pending: boolean;
  multi_card_warning: boolean;
}

/** Estado da flag de write-back. `false` → envio ao Sheets desabilitado (só preview). */
export function writeBackEnabled(): Promise<boolean> {
  return invoke("write_back_enabled");
}

/** Prévia RICA READ-ONLY (plano 028): diff + `preview_revision` (frescura) + conflitos pendentes +
 * aviso de multi-cartão. A UI endurecida usa isto para amarrar a aprovação ao que foi visto. */
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

/** Resultado do write-back (plano 036): nº de células escritas + um aviso não-bloqueante quando a
 * NOTA de célula itemizada não pôde ser gravada (o valor/fórmula já foi escrito com sucesso). */
export interface WriteBackResult {
  written: number;
  note_warning: string | null;
}

/** Aplica o write-back (escreve as células divergentes). Atrás da flag: rejeita enquanto
 * desligado (nunca escreve). `previewRevision` (do `previewWriteBackStatus`) amarra a aprovação à
 * revisão vista: se a planilha mudou, o backend aborta com erro de re-revisão. Células itemizadas
 * (≥2 partes) escrevem `=SUM(...)` + nota por-parte; as normais seguem número cru (plano 036). */
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

/** Prévia RICA READ-ONLY da Economia (plano 028): diff + `preview_revision` + conflitos pendentes. */
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

// --- Conciliação avançada: gate de conflito (spec 013) ---

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
 * Grava o teto de gasto Diário por dia configurado pelo dono.
 * `amountCents = 0` desativa o teto explícito — o engine usa o fallback de média.
 */
export function upsertDailyBudget(amountCents: number): Promise<void> {
  return invoke("upsert_daily_budget", { amountCents });
}

// --- Quebra por categoria do orçamento Diário (plano 045) ---

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
 */
export function upsertDailyBudgetWithCategories(
  amountCents: number,
  categories: DailyBudgetCategoryInput[],
): Promise<void> {
  return invoke("upsert_daily_budget_with_categories_cmd", { amountCents, categories });
}

// --- Lembrete agendado no nível do sistema (plano 039) ---

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

// --- Eventos do backend (sync em segundo plano, plano 026) ---

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

// --- Tags (spec 014) ---

export interface Tag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
  /** Quando true, lançamentos com esta tag saem das métricas (Performance, Custo de vida,
   * Economizado%) — mas continuam no Saldo (movimento de caixa real). */
  exclude_from_totals: boolean;
}

export interface TagTotal extends Tag {
  /** Soma (centavos, valor absoluto) dos lançamentos do mês com esta tag. */
  total_cents: number;
}

export interface AnnualMetrics {
  year: number;
  months: MonthMetric[];
}

export function getAnnualMetrics(year: number): Promise<AnnualMetrics> {
  return invoke("get_annual_metrics", { year });
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

/** Liga/desliga "Ignorar nos cálculos" para uma tag (sai das métricas, não do Saldo). Plano 034. */
export function updateTagExclude(tagId: string, exclude: boolean): Promise<void> {
  return invoke("update_tag_exclude_cmd", { tagId, exclude });
}

export function tagTotalsForMonth(year: number, month: number): Promise<TagTotal[]> {
  return invoke("tag_totals_for_month_cmd", { year, month });
}

// --- Multi-titular / split (read-side, spec 017) ---

export interface SplitRow {
  id: string;
  transaction_id: string;
  amount: number;
  owner_person_id: string;
  owner_name: string;
  note: string | null;
}

export interface OwnerTotal {
  owner_person_id: string;
  owner_name: string;
  /** Soma (centavos, valor absoluto) das parcelas do titular no mês. */
  total_cents: number;
}

/** @internal Tauri bridge — UI pending (spec 016/017). */
// react-doctor-disable-next-line deslop/unused-export -- spec 017: ponte do frontend (comando Tauri pronto/testado), UI multi-titular pendente
export function splitsForTransaction(transactionId: string): Promise<SplitRow[]> {
  return invoke("splits_for_transaction_cmd", { transactionId });
}

export function ownerTotalsForMonth(
  year: number,
  month: number,
): Promise<OwnerTotal[]> {
  return invoke("owner_totals_for_month_cmd", { year, month });
}

// --- Recorrências / séries (spec 016) ---

export type Frequency = "diaria" | "semanal" | "mensal";

/** @internal Tauri bridge — UI pending (spec 016/017). */
// react-doctor-disable-next-line deslop/unused-export -- spec 016: ponte do frontend (comando Tauri pronto/testado), UI de recorrências pendente
export function createRecurringSeries(input: {
  txnType: string;
  amount: number;
  description: string | null;
  start: string;
  paymentMethod: string | null;
  isFixed: boolean;
  frequency: Frequency;
  repetitions: number;
}): Promise<string> {
  return invoke("create_recurring_series_cmd", input);
}

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
