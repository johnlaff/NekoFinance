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

export function getDashboardSummary(): Promise<DashboardSummary> {
  return invoke("get_dashboard_summary");
}

export function getRecentTransactions(limit: number): Promise<TransactionRow[]> {
  return invoke("get_recent_transactions", { limit });
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
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
