/**
 * Extrai o spreadsheet ID de uma URL do Google Sheets colada pelo usuário —
 * fallback do picker via Drive (spec 010, slice 2): conectar à planilha real
 * sem depender do scope/listagem do Drive.
 *
 * Aceita a URL completa (`https://docs.google.com/spreadsheets/d/<ID>/edit#gid=0`)
 * ou o ID puro. Devolve `null` quando o texto não parece nem URL de Sheets nem ID.
 */
export function extractSpreadsheetId(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  const fromUrl = /\/spreadsheets\/(?:u\/\d+\/)?d\/([a-zA-Z0-9_-]+)/.exec(trimmed);
  if (fromUrl?.[1]) return fromUrl[1];

  // IDs de planilha são longos (44 chars tipicamente); 20+ evita capturar texto solto.
  if (/^[a-zA-Z0-9_-]{20,}$/.test(trimmed)) return trimmed;

  return null;
}
