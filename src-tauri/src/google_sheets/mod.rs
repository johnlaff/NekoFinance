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

        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .bearer_auth(&self.token.access_token),
        )
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

        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .bearer_auth(&self.token.access_token),
        )
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

    /// Sonda os metadados do arquivo no Drive para detectar barato se a planilha mudou desde o último
    /// import. Devolve a string RFC-3339 `modifiedTime` do endpoint `files.get` do Drive.
    ///
    /// Uma chamada ao Drive substitui N leituras do Sheets como sentinela de mudança — o
    /// `spreadsheets.values.batchGet` completo só roda quando o `modifiedTime` avançou. Usa o scope
    /// `drive.metadata.readonly` que o app já pede (oauth::pkce) — nenhum re-consentimento.
    pub async fn get_file_modified_time(&self, file_id: &str) -> Result<String, String> {
        let url =
            format!("https://www.googleapis.com/drive/v3/files/{file_id}?fields=modifiedTime");
        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .bearer_auth(&self.token.access_token),
        )
        .await
        .map_err(|e| format!("drive files.get error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Drive API error {status}: {body}"));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("drive modifiedTime parse: {e}"))?;

        json["modifiedTime"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "modifiedTime field absent from Drive response".into())
    }

    pub async fn get_sheet_metadata(
        &self,
        spreadsheet_id: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}");

        let resp = crate::http::send_with_retry(
            crate::http::client()
                .get(&url)
                .bearer_auth(&self.token.access_token),
        )
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
        // Fragmenta o lote em pedaços ≤ MAX_RANGES_PER_REQUEST: uma escrita anual completa pode
        // exceder o limite de ranges por requisição da API. Reportamos progresso PARCIAL numa falha
        // no meio (o erro inclui quantas células já foram confirmadas) em vez de um "0" enganoso.
        let mut written = 0usize;
        for chunk in chunk_update_ranges(updates) {
            match self.batch_update_chunk(spreadsheet_id, chunk).await {
                Ok(n) => written += n,
                Err(e) => {
                    return Err(format!(
                        "{e} (parcial: {written} célula(s) já escritas antes da falha)"
                    ));
                }
            }
        }
        Ok(written)
    }

    /// Envia UM pedaço (≤ limite) ao `values:batchUpdate` e CONFERE a resposta: o
    /// `totalUpdatedCells` precisa bater com o número de ranges pedidos, senão reportamos erro
    /// (uma escrita silenciosamente parcial é tão perigosa quanto um 4xx). Plano 028 Step 6.
    async fn batch_update_chunk(
        &self,
        spreadsheet_id: &str,
        chunk: &[(String, f64)],
    ) -> Result<usize, String> {
        let data: Vec<serde_json::Value> = chunk
            .iter()
            .map(|(range, value)| serde_json::json!({ "range": range, "values": [[value]] }))
            .collect();
        let body = serde_json::json!({ "valueInputOption": "RAW", "data": data });

        let url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values:batchUpdate",
        );
        let resp = crate::http::send_with_retry(
            crate::http::client()
                .post(&url)
                .bearer_auth(&self.token.access_token)
                .json(&body),
        )
        .await
        .map_err(|e| format!("batchUpdate request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sheets batchUpdate error {status}: {body}"));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("batchUpdate parse: {e}"))?;
        verify_batch_update_response(&json, chunk.len())
    }
}

/// Limite seguro de ranges por requisição `values:batchUpdate`. Mantém uma folga ampla sob qualquer
/// limite prático da API para que uma escrita anual completa nunca estoure numa única chamada.
pub(crate) const MAX_RANGES_PER_REQUEST: usize = 500;

/// Fragmenta o slice de updates em pedaços de no máximo `MAX_RANGES_PER_REQUEST` ranges.
pub(crate) fn chunk_update_ranges(
    updates: &[(String, f64)],
) -> std::slice::Chunks<'_, (String, f64)> {
    updates.chunks(MAX_RANGES_PER_REQUEST)
}

/// Confere a resposta do `values:batchUpdate`: o `totalUpdatedCells` deve igualar `expected`
/// (uma célula por range pedido). Diferente → erro nomeando a divergência, em vez de reportar
/// sucesso sobre uma escrita parcial/silenciosa. Plano 028 Step 6.
pub(crate) fn verify_batch_update_response(
    json: &serde_json::Value,
    expected: usize,
) -> Result<usize, String> {
    let total = json["totalUpdatedCells"].as_u64().unwrap_or(0) as usize;
    if total != expected {
        return Err(format!(
            "Sheets batchUpdate confirmou {total} célula(s), esperado {expected} — escrita parcial; revise a planilha."
        ));
    }
    Ok(total)
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

    // Plano 028 Step 6: a fronteira de fragmentação respeita MAX_RANGES_PER_REQUEST. Um lote de
    // exatamente o limite vira 1 pedaço; +1 vira 2 (o último com 1 range).
    #[test]
    fn batch_update_chunks_at_the_safe_boundary() {
        let make = |n: usize| -> Vec<(String, f64)> {
            (0..n).map(|i| (format!("A{i}"), i as f64)).collect()
        };

        let exact = make(MAX_RANGES_PER_REQUEST);
        let chunks: Vec<_> = chunk_update_ranges(&exact).collect();
        assert_eq!(chunks.len(), 1, "exatamente no limite → 1 pedaço");
        assert_eq!(chunks[0].len(), MAX_RANGES_PER_REQUEST);

        let over = make(MAX_RANGES_PER_REQUEST + 1);
        let chunks: Vec<_> = chunk_update_ranges(&over).collect();
        assert_eq!(chunks.len(), 2, "um a mais → 2 pedaços");
        assert_eq!(chunks[0].len(), MAX_RANGES_PER_REQUEST);
        assert_eq!(chunks[1].len(), 1, "o resto cai no último pedaço");

        // Lote bem maior: ceil(1250/500) = 3 pedaços (500, 500, 250).
        let big = make(1250);
        let lens: Vec<usize> = chunk_update_ranges(&big).map(|c| c.len()).collect();
        assert_eq!(lens, vec![500, 500, 250]);
    }

    // Plano 028 Step 6: a conferência da resposta detecta escrita parcial (totalUpdatedCells != n).
    #[test]
    fn verify_batch_update_response_flags_partial_writes() {
        use serde_json::json;
        // Confirmação exata → Ok com a contagem.
        assert_eq!(
            verify_batch_update_response(&json!({"totalUpdatedCells": 3}), 3),
            Ok(3)
        );
        // Menos células confirmadas que o pedido → erro nomeando a divergência.
        let err = verify_batch_update_response(&json!({"totalUpdatedCells": 2}), 3).unwrap_err();
        assert!(err.contains("2"));
        assert!(err.contains("3"));
        // Campo ausente (resposta inesperada) é tratado como 0 confirmadas → erro.
        assert!(verify_batch_update_response(&json!({}), 1).is_err());
    }

    // Spec 010 slice 0: o path ao vivo normaliza números cru → 4 casas fixas, sem locale.
    #[test]
    fn json_cells_normalize_numbers_to_fixed_decimals() {
        use serde_json::json;
        assert_eq!(json_cell_to_string(&json!(12.34)), "12.3400");
        assert_eq!(json_cell_to_string(&json!(1234.56)), "1234.5600");
        assert_eq!(json_cell_to_string(&json!(5678.1234)), "5678.1234");
        assert_eq!(json_cell_to_string(&json!(1)), "1.0000");
        assert_eq!(json_cell_to_string(&json!(" JANEIRO ")), "JANEIRO");
        assert_eq!(json_cell_to_string(&json!("")), "");
        assert_eq!(json_cell_to_string(&serde_json::Value::Null), "");
    }
}
