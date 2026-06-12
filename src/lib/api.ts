import { invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri shell (vs plain web preview). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Google OAuth client id baked at build time. Empty string when not configured. */
export const GOOGLE_CLIENT_ID =
  (import.meta.env["VITE_GOOGLE_CLIENT_ID"] as string) ?? "";

export type AuthStatus = "connected" | "expired" | "disconnected" | "loading";

export interface DashboardSummary {
  /** Projected end-of-current-month balance, in cents (forecast engine, spec 003). */
  balance: number;
  daily_budget: number;
  daily_spend_today: number;
  credit_spend_month: number;
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

/** Projection DTO from the deterministic engine (spec 005). All money in cents. */
export interface Forecast {
  today: string;
  horizon_end: string;
  safe_to_spend_today_cents: number;
  deepest_deficit: DayPoint | null;
  daily: ForecastDay[];
  month_end: MonthEnd[];
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
  return invoke("start_oauth_flow", { clientId });
}

export function disconnectGoogle(): Promise<void> {
  return invoke("disconnect_google");
}

export function listUserSpreadsheets(clientId: string): Promise<UserSpreadsheet[]> {
  return invoke("list_user_spreadsheets", { clientId });
}

export function listSheetNames(
  spreadsheetId: string,
  clientId: string,
): Promise<SheetInfo[]> {
  return invoke("list_sheet_names", { spreadsheetId, clientId });
}

export function fetchSheetPreview(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<SheetPreview> {
  return invoke("fetch_sheet_preview", { spreadsheetId, sheetName, clientId });
}

export function getSheetMappings(sheetName: string): Promise<SheetMappingEntry[]> {
  return invoke("get_sheet_mappings", { sheetName });
}

export function detectSheetLayout(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<SheetLayout> {
  return invoke("detect_sheet_layout", { spreadsheetId, sheetName, clientId });
}

export function saveSheetMapping(
  mappingId: string,
  blockOffset: number,
  isActive: boolean,
): Promise<void> {
  return invoke("save_sheet_mapping", { mappingId, blockOffset, isActive });
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
  });
}

export function importLocalXlsx(filePath: string, profileId: string): Promise<string> {
  return invoke("import_local_xlsx", { filePath, profileId });
}
