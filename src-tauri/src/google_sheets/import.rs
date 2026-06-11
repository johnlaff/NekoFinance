use super::layout_detect::SheetLayout;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

#[derive(Debug)]
pub struct ImportedRow {
    pub date: String,
    pub amount: i64,
    pub description: String,
    pub is_projection: bool,
}

pub fn classify_row(date_str: &str, date_direction: &str) -> Result<bool, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_past = date_str < today.as_str();

    match date_direction {
        "past_only" => Ok(false),
        "future_only" => Ok(true),
        "both" => Ok(!is_past),
        _ => Err(format!("unknown date_direction: {date_direction}")),
    }
}

pub fn compute_checksum(rows: &[ImportedRow]) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.date.as_bytes());
        hasher.update(row.amount.to_le_bytes());
        hasher.update(row.description.as_bytes());
        hasher.update([row.is_projection as u8]);
    }
    hex::encode(hasher.finalize())
}

pub async fn check_duplicate_import(
    pool: &SqlitePool,
    sheet_name: &str,
    checksum: &str,
) -> Result<bool, String> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sync_log WHERE source_sheet = ?1 AND checksum = ?2")
            .bind(sheet_name)
            .bind(checksum)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("check duplicate: {e}"))?;

    Ok(count > 0)
}

pub async fn import_rows(
    pool: &SqlitePool,
    sheet_name: &str,
    rows: &[ImportedRow],
    profile_id: &str,
) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    let checksum = compute_checksum(rows);
    if check_duplicate_import(pool, sheet_name, &checksum).await? {
        return Ok(0);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut imported = 0usize;

    for row in rows {
        let txn_id = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, description, date, is_projection, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        )
        .bind(&txn_id)
        .bind(if row.amount >= 0 { "income" } else { "expense" })
        .bind(row.amount.abs())
        .bind(&row.description)
        .bind(&row.date)
        .bind(row.is_projection as i64)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                let log_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, metadata, source_sheet, checksum) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                )
                .bind(&log_id)
                .bind("import")
                .bind("transaction")
                .bind(&txn_id)
                .bind(profile_id)
                .bind(&now)
                .bind(format!(r#"{{"source":"{sheet_name}","date":"{}","amount":{}}}"#, row.date, row.amount))
                .bind(sheet_name)
                .bind(&checksum)
                .execute(pool)
                .await
                .map_err(|e| format!("sync_log error: {e}"))?;

                imported += 1;
            }
            Err(e) => {
                eprintln!("Failed to import row {:?}: {e}", row);
            }
        }
    }

    Ok(imported)
}

pub async fn get_layout_for_sheet(
    pool: &SqlitePool,
    sheet_name: &str,
) -> Result<Option<SheetLayout>, String> {
    let result = sqlx::query_as::<_, (String, String, Option<i32>, i32, i32, i32, i32, i32, String)>(
        "SELECT id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction FROM sheet_layout WHERE sheet_name = ?1"
    )
    .bind(sheet_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query layout: {e}"))?;

    Ok(
        result.map(|(id, sn, year, mnr, hr, dsr, dc, bs, dd)| SheetLayout {
            id,
            sheet_name: sn,
            year,
            month_names_row: mnr,
            header_row: hr,
            data_start_row: dsr,
            day_column: dc,
            block_size: bs,
            date_direction: dd,
        }),
    )
}

pub async fn get_active_mappings_for_sheet(
    pool: &SqlitePool,
    sheet_name: &str,
) -> Result<Vec<(String, i32)>, String> {
    let rows = sqlx::query_as::<_, (String, i32)>(
        "SELECT target_field, block_offset FROM sheet_mapping WHERE sheet_name = ?1 AND is_active = 1 ORDER BY block_offset"
    )
    .bind(sheet_name)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query mappings: {e}"))?;

    Ok(rows)
}

pub fn parse_rows_with_layout(
    rows: &[Vec<String>],
    layout: &SheetLayout,
    mappings: &[(String, i32)],
) -> Vec<ImportedRow> {
    let mut imported = Vec::new();

    let year = layout.year.unwrap_or(2025);
    let data_start = layout.data_start_row as usize;
    let day_col = layout.day_column as usize;
    let block_size = layout.block_size as usize;
    let month_row = layout.month_names_row as usize;

    if month_row >= rows.len() {
        return imported;
    }

    let header_row_data = &rows[month_row];
    let mut month_offsets = Vec::new();
    for (i, cell) in header_row_data.iter().enumerate() {
        if !cell.trim().is_empty() && i > 0 {
            month_offsets.push(i);
        }
    }

    if month_offsets.is_empty() {
        for i in (0..header_row_data.len()).step_by(block_size) {
            if i > 0 {
                month_offsets.push(i);
            }
        }
    }

    let amount_in_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_in")
        .map(|(_, offset)| *offset as usize);
    let amount_out_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_out")
        .map(|(_, offset)| *offset as usize);

    for row in rows.iter().skip(data_start) {
        if row.is_empty() || row.get(day_col).is_none_or(|c| c.trim().is_empty()) {
            continue;
        }

        let day_str = row.get(day_col).map_or("", |c| c.trim());
        let day: f64 = day_str.parse().unwrap_or(0.0);
        if !(1.0..=31.0).contains(&day) {
            continue;
        }

        let day_num = day as u32;

        for (month_idx, &offset) in month_offsets.iter().enumerate() {
            let month = month_idx as u32 + 1;
            let date = format!("{:04}-{:02}-{:02}", year, month, day_num);
            let is_projection = classify_row(&date, &layout.date_direction).unwrap_or(false);

            if let Some(in_off) = amount_in_offset
                && offset + in_off < row.len()
            {
                let amount_in = parse_number(&row[offset + in_off]);
                if amount_in > 0 {
                    imported.push(ImportedRow {
                        date: date.clone(),
                        amount: amount_in,
                        description: format!("Entrada {}", layout.sheet_name),
                        is_projection,
                    });
                }
            }

            if let Some(out_off) = amount_out_offset
                && offset + out_off < row.len()
            {
                let amount_out = parse_number(&row[offset + out_off]);
                if amount_out > 0 {
                    imported.push(ImportedRow {
                        date: date.clone(),
                        amount: -amount_out,
                        description: format!("Saída {}", layout.sheet_name),
                        is_projection,
                    });
                }
            }
        }
    }

    imported
}

fn parse_number(s: &str) -> i64 {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == ',')
        .collect();
    let normalized = cleaned.replace('.', "").replace(',', ".");
    if let Ok(f) = normalized.parse::<f64>() {
        (f * 100.0).round() as i64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_row_past() {
        let past = "2020-01-15";
        assert!(!classify_row(past, "both").unwrap());
        assert!(!classify_row(past, "past_only").unwrap());
        assert!(classify_row(past, "future_only").unwrap());
    }

    #[test]
    fn test_classify_row_future() {
        let future = "2099-12-31";
        assert!(classify_row(future, "both").unwrap());
        assert!(classify_row(future, "future_only").unwrap());
    }

    #[test]
    fn test_classify_row_invalid_direction() {
        assert!(classify_row("2025-01-01", "invalid").is_err());
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("100"), 10000);
        assert_eq!(parse_number("1.234,56"), 123456);
        assert_eq!(parse_number("-50"), -5000);
        assert_eq!(parse_number(""), 0);
    }

    #[test]
    fn test_parse_rows_with_layout() {
        let rows = vec![
            vec![
                "".into(),
                "JANEIRO".into(),
                "".into(),
                "".into(),
                "".into(),
                "".into(),
                "FEVEREIRO".into(),
            ],
            vec![
                "".into(),
                "Data".into(),
                "Entrada".into(),
                "Saída".into(),
                "Diário".into(),
                "Saldo".into(),
                "Data".into(),
            ],
            vec![
                "1".into(),
                "".into(),
                "3500".into(),
                "".into(),
                "3500".into(),
                "3500".into(),
                "".into(),
            ],
            vec![
                "2".into(),
                "".into(),
                "".into(),
                "45".into(),
                "3455".into(),
                "3455".into(),
                "".into(),
            ],
        ];

        let layout = SheetLayout {
            id: "test".into(),
            sheet_name: "2025".into(),
            year: Some(2025),
            month_names_row: 0,
            header_row: 1,
            data_start_row: 2,
            day_column: 0,
            block_size: 6,
            date_direction: "past_only".into(),
        };

        let mappings = vec![("amount_in".to_string(), 1), ("amount_out".to_string(), 2)];

        let result = parse_rows_with_layout(&rows, &layout, &mappings);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].amount, 350000);
        assert_eq!(result[0].date, "2025-01-01");
        assert!(!result[0].is_projection);
        assert_eq!(result[1].amount, -4500);
        assert_eq!(result[1].date, "2025-01-02");
    }

    #[test]
    fn test_compute_checksum() {
        let rows = vec![ImportedRow {
            date: "2025-01-01".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
        }];
        let checksum1 = compute_checksum(&rows);
        let checksum2 = compute_checksum(&rows);
        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64);

        let different_rows = vec![ImportedRow {
            date: "2025-01-02".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
        }];
        let checksum3 = compute_checksum(&different_rows);
        assert_ne!(checksum1, checksum3);
    }
}
