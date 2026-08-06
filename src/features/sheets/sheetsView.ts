//! Porta completa (ADR-0007) do domínio Sheets/write-back: conexão Google, import (planilha
//! remota + .xlsx local) e o fluxo de escrita de volta (prévia + apply, grade e Economia).
//! `GoogleSheetsPanel.tsx`, `LocalXlsxImport.tsx`, `WriteBackPreview.tsx`,
//! `screens/dashboard/WriteBackPending.tsx` e a seção Conexão de `SettingsScreen.tsx` importam
//! só daqui — nunca de `lib/api`. `hooks/useWriteBackPending.ts` é zona de exceção à parte (ADR-0006)
//! e continua lendo `lib/api` direto: ele já está no nível de funil que a view ocupa para uma tela.

import {
  applyEconomiaWriteBack,
  applyWriteBack,
  checkAuthStatus,
  detectSheetLayout,
  disconnectGoogle,
  fetchSheetPreview,
  getAppSetting,
  getImportConflicts,
  getSheetMappings,
  importEconomiaSheet,
  importLocalXlsx,
  importSheetData,
  listSheetNames,
  listUserSpreadsheets,
  previewEconomiaWriteBackStatus,
  previewWriteBackStatus,
  saveSheetMapping,
  setAppSetting,
  startOAuthFlow,
  writeBackEnabled,
  type AuthStatus,
  type CellWrite,
  type ImportDiagnostic,
  type ImportOutcome,
  type SheetInfo,
  type SheetMappingEntry,
  type SheetPreview,
  type UserSpreadsheet,
  type WriteBackPreviewResult,
  type WriteBackResult,
} from "../../lib/api";

export type {
  AuthStatus,
  CellWrite,
  ImportDiagnostic,
  ImportOutcome,
  SheetInfo,
  SheetMappingEntry,
  SheetPreview,
  UserSpreadsheet,
  WriteBackPreviewResult,
  WriteBackResult,
};

/** Última PLANILHA importada (preferência local, `sheets_last_import`). */
export const LAST_IMPORT_KEY = "sheets_last_import";
/** Última ABA-ano importada — o indicador de write-back pendente lê daqui. */
export const LAST_SHEET_KEY = "sheets_last_sheet";
/** Sync em segundo plano: ligado por padrão, separado do "Re-sincronizar" manual. */
export const BG_SYNC_KEY = "sheets_bg_sync_enabled";
/** Client id do OAuth persistido para a tarefa de sync em segundo plano renovar o token. */
export const CLIENT_ID_KEY = "sheets_client_id";
/** Marca o ciclo em que o backend não conseguiu ler notas de célula (classificação congelada). */
export const NOTES_DEGRADED_KEY = "notes_degraded_last_sheet";

// --- Leitura -----------------------------------------------------------------------------

export function fetchGoogleAuthStatus(): Promise<AuthStatus> {
  return checkAuthStatus();
}

export function fetchUserSpreadsheets(clientId: string): Promise<UserSpreadsheet[]> {
  return listUserSpreadsheets(clientId);
}

export function fetchSheetNames(
  spreadsheetId: string,
  clientId: string,
): Promise<SheetInfo[]> {
  return listSheetNames(spreadsheetId, clientId);
}

export function fetchSheetPreviewCmd(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<SheetPreview> {
  return fetchSheetPreview(spreadsheetId, sheetName, clientId);
}

export function fetchSheetMappings(sheetName: string): Promise<SheetMappingEntry[]> {
  return getSheetMappings(sheetName);
}

export function fetchWriteBackEnabled(): Promise<boolean> {
  return writeBackEnabled();
}

export function fetchWriteBackPreview(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
): Promise<WriteBackPreviewResult> {
  return previewWriteBackStatus(spreadsheetId, sheetName, clientId);
}

export function fetchEconomiaWriteBackPreview(
  spreadsheetId: string,
  year: number,
  clientId: string,
): Promise<WriteBackPreviewResult> {
  return previewEconomiaWriteBackStatus(spreadsheetId, year, clientId);
}

export function fetchImportConflictsCount(): Promise<number> {
  return getImportConflicts().then((c) => c.length);
}

/** Preferência local do domínio Sheets (repassa o shim genérico sob o vocabulário da tela). */
export function fetchSheetsSetting(key: string): Promise<string | null> {
  return getAppSetting(key);
}

// --- Escrita -------------------------------------------------------------------------------

export function connectGoogleCmd(clientId: string): Promise<string> {
  return startOAuthFlow(clientId);
}

export function disconnectGoogleCmd(): Promise<void> {
  return disconnectGoogle();
}

export function detectSheetLayoutCmd(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
) {
  return detectSheetLayout(spreadsheetId, sheetName, clientId);
}

export function saveSheetMappingCmd(
  mappingId: string,
  blockOffset: number,
  isActive: boolean,
): Promise<void> {
  return saveSheetMapping(mappingId, blockOffset, isActive);
}

export function importSheetDataCmd(
  spreadsheetId: string,
  sheetName: string,
  profileId: string,
  clientId: string,
): Promise<ImportOutcome> {
  return importSheetData(spreadsheetId, sheetName, profileId, clientId);
}

export function importEconomiaSheetCmd(
  spreadsheetId: string,
  clientId: string,
): Promise<number> {
  return importEconomiaSheet(spreadsheetId, clientId);
}

export function importLocalXlsxCmd(
  filePath: string,
  profileId: string,
): Promise<ImportOutcome> {
  return importLocalXlsx(filePath, profileId);
}

export function applyWriteBackCmd(
  spreadsheetId: string,
  sheetName: string,
  clientId: string,
  previewRevision?: string | null,
): Promise<WriteBackResult> {
  return applyWriteBack(spreadsheetId, sheetName, clientId, previewRevision);
}

export function applyEconomiaWriteBackCmd(
  spreadsheetId: string,
  year: number,
  clientId: string,
  previewRevision?: string | null,
): Promise<number> {
  return applyEconomiaWriteBack(spreadsheetId, year, clientId, previewRevision);
}

/** Grava uma preferência local do domínio Sheets. */
export function setSheetsSetting(key: string, value: string): Promise<void> {
  return setAppSetting(key, value);
}
