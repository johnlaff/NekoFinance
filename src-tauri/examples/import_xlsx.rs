use calamine::{Reader, Xlsx, open_workbook};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xlsx_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/example/Financas.xlsx".into());
    println!("Importing: {xlsx_path}");

    let db_path = "/tmp/neko-import.db";
    let _ = std::fs::remove_file(db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{db_path}?mode=rwc"))
        .await?;

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await?;
    migrator.run(&pool).await?;

    let pid = uuid::Uuid::new_v4().to_string();
    let fid = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'ImportUser')")
        .bind(&pid)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO profile (id, person_id) VALUES (?1, ?2)")
        .bind(&fid)
        .bind(&pid)
        .execute(&pool)
        .await?;

    let mut wb: Xlsx<_> = open_workbook(&xlsx_path)?;
    let names = wb.sheet_names().to_vec();
    let now = chrono::Utc::now().to_rfc3339();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut grand_income = 0i64;
    let mut grand_expense = 0i64;
    let mut grand_count = 0usize;

    for sheet_name in &names {
        let Ok(range) = wb.worksheet_range(sheet_name) else {
            continue;
        };
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|r| r.iter().map(|c| c.to_string().trim().to_string()).collect())
            .collect();
        if rows.len() < 3 {
            continue;
        }

        let mut offsets = Vec::new();
        for (i, c) in rows[0].iter().enumerate() {
            if !c.is_empty() && i > 0 {
                offsets.push(i);
            }
        }
        if offsets.is_empty() {
            for i in (0..rows[0].len()).step_by(6) {
                if i > 0 {
                    offsets.push(i);
                }
            }
        }

        let year: u32 = sheet_name.parse().unwrap_or(2025);
        let mut sheet_count = 0usize;
        let mut sheet_income = 0i64;
        let mut sheet_expense = 0i64;

        for row in rows.iter().skip(2) {
            if row.is_empty() || row[0].is_empty() {
                continue;
            }
            let Ok(day) = row[0].parse::<f64>() else {
                continue;
            };
            if !(1.0..=31.0).contains(&day) {
                continue;
            }
            let day_num = day as u32;

            for (mi, &off) in offsets.iter().enumerate() {
                if off + 3 >= row.len() {
                    continue;
                }
                let amount_in = parse_brl(&row.get(off + 1).cloned().unwrap_or_default());
                let amount_out = parse_brl(&row.get(off + 2).cloned().unwrap_or_default());
                let month = mi as u32 + 1;
                let date = format!("{year:04}-{month:02}-{day_num:02}");
                let is_proj = date >= today;

                if amount_in > 0 {
                    let id = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO \"transaction\" (id,type,amount,description,date,is_projection,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")
                        .bind(&id).bind("income")                        .bind(amount_in).bind(sheet_name.to_string()).bind(&date).bind(is_proj as i64).bind(&now).bind(&now)
                        .execute(&pool).await?;
                    sheet_income += amount_in;
                    sheet_count += 1;
                }
                if amount_out > 0 {
                    let id = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO \"transaction\" (id,type,amount,description,date,is_projection,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")
                        .bind(&id).bind("expense")                        .bind(amount_out).bind(sheet_name.to_string()).bind(&date).bind(is_proj as i64).bind(&now).bind(&now)
                        .execute(&pool).await?;
                    sheet_expense += amount_out;
                    sheet_count += 1;
                }
            }
        }

        println!(
            "  {sheet_name}: {sheet_count} rows | +R${:.2} -R${:.2} | net R${:.2}",
            sheet_income as f64 / 100.0,
            sheet_expense as f64 / 100.0,
            (sheet_income - sheet_expense) as f64 / 100.0
        );
        grand_income += sheet_income;
        grand_expense += sheet_expense;
        grand_count += sheet_count;
    }

    let (past,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE is_projection=0")
            .fetch_one(&pool)
            .await?;
    let (proj,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE is_projection=1")
            .fetch_one(&pool)
            .await?;

    println!("\n============= SUMMARY =============");
    println!("Total rows imported:  {grand_count}");
    println!("  Past transactions:  {past}");
    println!("  Future projections: {proj}");
    println!("Total income:        R$ {:.2}", grand_income as f64 / 100.0);
    println!(
        "Total expense:       R$ {:.2}",
        grand_expense as f64 / 100.0
    );
    println!(
        "Net:                 R$ {:.2}",
        (grand_income - grand_expense) as f64 / 100.0
    );
    println!("DB path:             {db_path}");

    Ok(())
}

fn parse_brl(s: &str) -> i64 {
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
