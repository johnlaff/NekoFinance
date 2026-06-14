import { invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

/**
 * Client secret do app desktop: NÃO é mais embutido no bundle do frontend (era exposição
 * desnecessária — qualquer um lê um bundle JS). Para um cliente desktop o secret é opcional: o
 * fluxo usa PKCE (RFC 8252) e, se o secret for necessário, o backend Rust o lê do PRÓPRIO env
 * (`GOOGLE_CLIENT_SECRET`, sem prefixo `VITE_`). Os chamadores enviam `null`.
 */
const clientSecretOrNull = null;

export type AuthStatus = "connected" | "expired" | "disconnected" | "loading";

export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents (forecast engine, spec 003). */
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  credit_spend_month: number;
  /** Há rastreio de crédito (cartão ou gasto). `false` → mostrar "—" no tile, não R$0. */
  has_credit: boolean;
  reserve_months: number;
  reserve_trend: string;
  transaction_count: number;
}

export interface TransactionRow {
  id: string;
  type: string;
  amount: number;
  description: string;
  date: string;
  payment_method: string;
  is_projection: boolean;
  /** Titulares distintos das parcelas (multi-titular). Vazio = sem split por pessoa. */
  owners: string[];
  /** Proveniência: "projetado" | "importado" | "manual" | "conciliado". */
  provenance: string;
}

export interface SheetInfo {
  title: string;
  sheet_id: number;
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
  /** Diário médio = Σ diário realizado ÷ dias decorridos (D/N). */
  real_daily_avg_cents: number;
  /** Economia lançada no mês (numerador do Economizado%). */
  economia_cents: number;
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
  | "bank"
  | "wallet"
  | "business"
  | "savings"
  | "meal_voucher"
  | "pension"
  | "fgts";

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

/** Cria um lançamento manual. Com `recurrence`, gera a série projetada. Retorna o id criado. */
export function createTransaction(input: {
  txnType: "income" | "expense";
  amountCents: number;
  description: string | null;
  date: string;
  paymentMethod: string | null;
  isFixed: boolean;
  tagIds: string[];
  recurrence: { frequency: Frequency; repetitions: number } | null;
}): Promise<string> {
  return invoke("create_transaction", input);
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
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
  changed: boolean;
}

/** Estado da flag de write-back. `false` → envio ao Sheets desabilitado (só preview). */
export function writeBackEnabled(): Promise<boolean> {
  return invoke("write_back_enabled");
}

/** Pré-visualização READ-ONLY: transações → células (diff). Seguro mesmo com a flag desligada. */
export function previewWriteBack(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<CellWrite[]> {
  return invoke("preview_write_back", {
    spreadsheetId,
    sheetName,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

/** Aplica o write-back. Atrás da flag: rejeita enquanto desligado (nunca escreve). */
export function applyWriteBack(): Promise<void> {
  return invoke("apply_write_back");
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

export function importSheetData(
  spreadsheetId: string,
  sheetName: string,
  profileId: string,
  clientId: string,
): Promise<number> {
  return invoke("import_sheet_data", {
    spreadsheetId,
    sheetName,
    profileId,
    clientId,
    clientSecret: clientSecretOrNull,
  });
}

export function importLocalXlsx(filePath: string, profileId: string): Promise<string> {
  return invoke("import_local_xlsx", { filePath, profileId });
}

// --- Tags (spec 014) ---

export interface Tag {
  id: string;
  name: string;
  color: string;
  emoji: string | null;
  is_special: boolean;
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

export function listTags(): Promise<Tag[]> {
  return invoke("list_tags_cmd");
}

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

/** Apaga a ocorrência indicada e todas as posteriores ("deste ponto em diante"). */
export function deleteSeriesFrom(transactionId: string): Promise<number> {
  return invoke("delete_series_from_cmd", { transactionId });
}

/** Apaga toda a série + a linha de recorrência. */
export function deleteSeriesAll(recurrenceId: string): Promise<number> {
  return invoke("delete_series_all_cmd", { recurrenceId });
}

interface SeriesEdit {
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
