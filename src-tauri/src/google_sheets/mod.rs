pub mod import;
pub mod layout_detect;

use crate::oauth::token_store::StoredToken;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SheetValues {
    #[serde(default)]
    pub values: Vec<Vec<String>>,
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
        let url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values/{range}",
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

        resp.json::<SheetValues>()
            .await
            .map_err(|e| format!("parse error: {e}"))
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
}
