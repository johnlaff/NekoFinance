pub mod import;
pub mod layout_detect;
pub mod reconcile;
pub mod write_back;

use crate::oauth::token_store::StoredToken;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SheetValues {
    #[serde(default)]
    pub values: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawSheetValues {
    #[serde(default)]
    values: Vec<Vec<serde_json::Value>>,
}

/// Números do Sheets chegam crus (UNFORMATTED_VALUE) e são normalizados com 4 casas fixas,
/// exatamente como as células do xlsx (`xlsx_cell_to_string`) — o `parse_number` nunca vê
/// string dependente do locale da planilha (spec 010, slice 0).
fn json_cell_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => match n.as_f64() {
            Some(f) => format!("{f:.4}"),
            None => n.to_string(),
        },
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

pub struct SheetsClient {
    token: StoredToken,
}

impl SheetsClient {
    pub fn new(token: StoredToken) -> Self {
        Self { token }
    }

    pub async fn get_sheet_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
    ) -> Result<SheetValues, String> {
        // UNFORMATTED_VALUE: valores numéricos crus, independentes do locale da planilha —
        // com FORMATTED, um locale pt-BR exibindo 3 casas ("65,280") viraria milhar (100×).
        let url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values/{range}?valueRenderOption=UNFORMATTED_VALUE",
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(&self.token.access_token)
            .send()
            .await
            .map_err(|e| format!("request error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sheets API error {status}: {body}"));
        }

        let raw = resp
            .json::<RawSheetValues>()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        Ok(SheetValues {
            values: raw
                .values
                .iter()
                .map(|row| row.iter().map(json_cell_to_string).collect())
                .collect(),
        })
    }

    /// Notas de célula da aba, alinhadas por `[linha][coluna]` (A1-based, "" quando sem nota).
    /// O método guarda o detalhe real de cada lançamento (quem, o quê, quanto por item) nas
    /// NOTAS — o endpoint `values` não as devolve, então usamos `spreadsheets.get` com
    /// `includeGridData`, pedindo só o campo `note` para o payload ficar enxuto.
    pub async fn get_sheet_notes(
        &self,
        spreadsheet_id: &str,
        sheet_name: &str,
    ) -> Result<Vec<Vec<String>>, String> {
        // Encode mínimo do nome da aba (espaços) — as abas-ano são alfanuméricas e seguras.
        let range = sheet_name.replace(' ', "%20");
        let url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}?ranges={range}&includeGridData=true&fields=sheets.data.rowData.values.note",
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(&self.token.access_token)
            .send()
            .await
            .map_err(|e| format!("notes request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sheets notes error {status}: {body}"));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| format!("notes parse: {e}"))?;

        let rows = json["sheets"][0]["data"][0]["rowData"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row["values"]
                            .as_array()
                            .map(|cells| {
                                cells
                                    .iter()
                                    .map(|c| c["note"].as_str().unwrap_or("").to_string())
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(rows)
    }

    pub async fn get_sheet_metadata(
        &self,
        spreadsheet_id: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}");

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(&self.token.access_token)
            .send()
            .await
            .map_err(|e| format!("request error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sheets API error {status}: {body}"));
        }

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("parse error: {e}"))
    }

    /// Escreve células via `values:batchUpdate`. `updates` = lista de (range A1 COM nome de aba,
    /// ex.: `'2026'!E3`, valor numérico em reais). Escrevemos NÚMERO cru com `valueInputOption=RAW`
    /// — o Sheets armazena o número exato e o display pt-BR ("75,00") vem do formato da célula;
    /// assim a escrita é independente do locale (espelha o `UNFORMATTED_VALUE` da leitura). Esta é
    /// a ÚNICA via de escrita real; só roda atrás de `WRITE_BACK_ENABLED` + aprovação humana.
    pub async fn batch_update_values(
        &self,
        spreadsheet_id: &str,
        updates: &[(String, f64)],
    ) -> Result<usize, String> {
        if updates.is_empty() {
            return Ok(0);
        }
        let data: Vec<serde_json::Value> = updates
            .iter()
            .map(|(range, value)| serde_json::json!({ "range": range, "values": [[value]] }))
            .collect();
        let body = serde_json::json!({ "valueInputOption": "RAW", "data": data });

        let url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values:batchUpdate",
        );
        let resp = reqwest::Client::new()
            .post(&url)
            .bearer_auth(&self.token.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("batchUpdate request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sheets batchUpdate error {status}: {body}"));
        }
        Ok(updates.len())
    }
}

#[derive(Debug, Deserialize)]
pub struct SheetInfo {
    pub title: String,
    pub sheet_id: i64,
}

pub fn parse_sheet_names(metadata: &serde_json::Value) -> Vec<SheetInfo> {
    let mut sheets = Vec::new();
    if let Some(sheet_list) = metadata["sheets"].as_array() {
        for sheet in sheet_list {
            let title = sheet["properties"]["title"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let sheet_id = sheet["properties"]["sheetId"].as_i64().unwrap_or(0);
            if !title.is_empty() {
                sheets.push(SheetInfo { title, sheet_id });
            }
        }
    }
    sheets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sheet_names() {
        let metadata = serde_json::json!({
            "sheets": [
                {"properties": {"title": "2025", "sheetId": 1}},
                {"properties": {"title": "2026", "sheetId": 2}},
                {"properties": {"title": "Economia", "sheetId": 3}}
            ]
        });
        let sheets = parse_sheet_names(&metadata);
        assert_eq!(sheets.len(), 3);
        assert_eq!(sheets[0].title, "2025");
        assert_eq!(sheets[1].title, "2026");
        assert_eq!(sheets[2].title, "Economia");
    }

    #[test]
    fn test_parse_sheet_names_empty() {
        let metadata = serde_json::json!({"sheets": []});
        let sheets = parse_sheet_names(&metadata);
        assert!(sheets.is_empty());
    }

    // Spec 010 slice 0: o path ao vivo normaliza números cru → 4 casas fixas, sem locale.
    #[test]
    fn json_cells_normalize_numbers_to_fixed_decimals() {
        use serde_json::json;
        assert_eq!(json_cell_to_string(&json!(65.28)), "65.2800");
        assert_eq!(json_cell_to_string(&json!(6012.73)), "6012.7300");
        assert_eq!(json_cell_to_string(&json!(10805.5048)), "10805.5048");
        assert_eq!(json_cell_to_string(&json!(1)), "1.0000");
        assert_eq!(json_cell_to_string(&json!(" JANEIRO ")), "JANEIRO");
        assert_eq!(json_cell_to_string(&json!("")), "");
        assert_eq!(json_cell_to_string(&serde_json::Value::Null), "");
    }
}
