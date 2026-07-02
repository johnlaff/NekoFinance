use super::*;

#[derive(serde::Serialize)]
pub struct SheetInfo {
    pub title: String,
    pub sheet_id: i64,
}

#[tauri::command]
pub async fn list_sheet_names(
    app_dir: State<'_, AppDataDir>,
    spreadsheet_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<SheetInfo>, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let metadata = client.get_sheet_metadata(&spreadsheet_id).await?;
    let sheets = google_sheets::parse_sheet_names(&metadata);
    Ok(sheets
        .into_iter()
        .map(|s| SheetInfo {
            title: s.title,
            sheet_id: s.sheet_id,
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct SheetPreview {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

#[tauri::command]
pub async fn fetch_sheet_preview(
    app_dir: State<'_, AppDataDir>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<SheetPreview, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Grade inteira (não A1:Z21) — a planilha real vai até a coluna BO (~71 col); A:Z cortaria
    // JUNHO–DEZEMBRO no preview, como no import (auditoria vs planilha oficial, P1).
    let range = quote_sheet(&sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;

    let mut rows = values.values;
    let total_rows = rows.len();

    let headers = if rows.is_empty() {
        vec![]
    } else {
        rows.remove(0)
    };

    // O preview exibe só as primeiras linhas; o range completo garante todas as colunas/meses.
    rows.truncate(20);

    Ok(SheetPreview {
        headers,
        rows,
        total_rows: total_rows.saturating_sub(1),
    })
}

// Comando Tauri: a lista de parâmetros é plana por design (cada um vem de state/request); o
// `guard` (SyncGuard) é estado gerenciado — daí passar de 7 argumentos.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn import_sheet_data(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    spreadsheet_id: String,
    sheet_name: String,
    profile_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    if layout_detect::is_metric_tab(&sheet_name) {
        return Err(format!(
            "'{sheet_name}' é uma aba de métricas do método (não tem transações). \
             Importe as abas-ano; o import de métricas chega na spec 010."
        ));
    }
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Serializa contra o sync de fundo e o probe de foco no pool de 1 conexão (mesmo SyncGuard).
    // Segurado por TODO o import_one_tab (fetch → transação atômica → diff-delete).
    let _lock = guard.inner().lock().await;
    import_one_tab(
        pool.inner(),
        &client,
        &spreadsheet_id,
        &sheet_name,
        &profile_id,
    )
    .await
}

/// Pipeline completo de import de UMA aba-ano, partindo de um `SheetsClient` já autenticado.
/// Extraído de `import_sheet_data` para que o comando (caminho do usuário) e a tarefa de sync em
/// segundo plano reusem EXATAMENTE o mesmo pipeline (fetch → parse → checksum-skip → transação
/// atômica → UPSERT + merge de 3 vias → diff-delete), sem reimplementar nada. Pula abas de métricas.
pub(crate) async fn import_one_tab(
    pool: &SqlitePool,
    client: &SheetsClient,
    spreadsheet_id: &str,
    sheet_name: &str,
    profile_id: &str,
) -> Result<usize, String> {
    if layout_detect::is_metric_tab(sheet_name) {
        return Ok(0);
    }

    // Grade usada inteira: a planilha real tem 12 blocos mensais até a coluna BO (~71
    // colunas) — um range A:Z cortaria JUNHO–DEZEMBRO em silêncio (spec 010, slice 0).
    let range = quote_sheet(sheet_name);
    let values = client.get_sheet_values(spreadsheet_id, &range).await?;
    let rows = values.values;

    if rows.len() < 3 {
        return Ok(0);
    }

    // Detecção de layout (leituras no pool). Quando NOVO, o INSERT do layout/mappings é adiado para
    // DENTRO da transação externa (tudo-ou-nada); guardamos os mappings gerados para o parse, já que
    // ainda não estarão visíveis por uma leitura no pool.
    let (layout, new_mappings) = match import::get_layout_for_sheet(pool, sheet_name).await? {
        Some(l) => (l, None),
        None => {
            let detected = layout_detect::detect_layout(&rows, sheet_name)?;
            let mappings = layout_detect::generate_mappings(&detected);
            (detected, Some(mappings))
        }
    };

    // Mappings ativos para o parse: do pool (layout pré-existente) ou os recém-gerados (layout novo,
    // ainda não comitado). `generate_mappings` marca a coluna Saldo como inativa, então o parse só vê
    // os mappings ativos — espelhando `get_active_mappings_for_sheet`.
    let mappings: Vec<(String, i32)> = match &new_mappings {
        Some(generated) => generated
            .iter()
            .filter(|m| m.is_active != 0)
            .map(|m| (m.target_field.clone(), m.block_offset))
            .collect(),
        None => import::get_active_mappings_for_sheet(pool, sheet_name).await?,
    };

    // Notas de célula = a descrição real de cada lançamento (quem/o quê/quanto por item). Sem
    // elas, o parser só tem fallback estrutural ("Entrada/Saída {data}"). Se a API de notas
    // falhar, os valores ainda entram, mas essas descrições não são tratadas como fonte canônica.
    let (notes, descriptions_trusted) = match client
        .get_sheet_notes(spreadsheet_id, sheet_name)
        .await
    {
        Ok(notes) => (notes, true),
        Err(e) => {
            // Ciclo DEGRADADO: os valores ainda entram, mas itens
            // classificados e `source_note` ficam CONGELADOS (gate de confiança no
            // `import_rows_core`) e a `raw_note` sai do checksum — uma falha transitória da
            // API de notas não pode reimportar destrutivamente nem apagar classificação.
            eprintln!(
                "[import] notas de célula indisponíveis em '{sheet_name}': {e} — ciclo degradado (classificação preservada)"
            );
            (Vec::new(), false)
        }
    };
    // Sinaliza (ou limpa) o ciclo degradado para a UI (painel do Google Sheets) — sem isto a
    // degradação era invisível fora do stderr. KV local apenas.
    crate::commands::write_back_cmds::app_setting_set(
        pool,
        "notes_degraded_last_sheet",
        if descriptions_trusted { "" } else { sheet_name },
    )
    .await?;
    let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &notes)?;
    let options = import::ImportRowsOptions {
        descriptions_trusted,
    };

    // Checksum + checagem de duplicata ANTES de abrir a transação (leitura no pool; dentro da tx
    // daria read-your-writes falso-negativo). Dataset idêntico ao último import → idempotente.
    let checksum = import::compute_import_checksum(&imported_rows, descriptions_trusted);
    if !imported_rows.is_empty()
        && import::check_duplicate_import(pool, sheet_name, &checksum).await?
    {
        return Ok(0);
    }

    // Captura a coluna Saldo (o saldo corrente do método) → semente da projeção + visão histórica.
    // Sem isto a semente era 0 e o saldo de hoje aparecia zerado. `get_balance_offset_for_sheet` é
    // leitura no pool; pode rodar antes de abrir a tx.
    let balance_offset = import::get_balance_offset_for_sheet(pool, sheet_name).await?;
    let balances = import::parse_balance_series(&rows, &layout, balance_offset)?;

    // Transação externa única: layout + mappings + linhas + série de Saldo gravam tudo-ou-nada.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("begin import: {e}"))?;

    if let Some(generated) = &new_mappings {
        sqlx::query(
            "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
        )
        .bind(&layout.id).bind(&layout.sheet_name).bind(layout.year)
        .bind(layout.month_names_row).bind(layout.header_row).bind(layout.data_start_row)
        .bind(layout.day_column).bind(layout.block_size).bind(&layout.date_direction)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("save layout: {e}"))?;

        for m in generated {
            sqlx::query(
                "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
            )
            .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
            .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
            .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("save mapping: {e}"))?;
        }
    }

    let count = import::import_rows_with_options_in_tx(
        &mut tx,
        sheet_name,
        &imported_rows,
        profile_id,
        options,
        &checksum,
    )
    .await?;

    import::store_balance_series_in_tx(&mut tx, sheet_name, &balances).await?;

    tx.commit()
        .await
        .map_err(|e| format!("commit import: {e}"))?;

    Ok(count)
}

/// Células numéricas do calamine viram string decimal-com-ponto de 4 casas fixas: `123.456`
/// vira `123.4560`, que o `parse_number` nunca confunde com agrupamento de milhar
/// (spec 010, slice 0 — antes, `12.34` perdia o ponto e inflava 100×).
pub(crate) fn xlsx_cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Float(f) => format!("{f:.4}"),
        other => other.to_string().trim().to_string(),
    }
}

pub(crate) fn validate_local_xlsx_path(file_path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(file_path);
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|e| format!("read file metadata: {e}"))?;
    if metadata.file_type().is_symlink() {
        return Err("Arquivo .xlsx não pode ser um link simbólico.".into());
    }
    if !metadata.is_file() {
        return Err("Escolha um arquivo .xlsx regular.".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("canonicalize: {e}"))?;
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "xlsx" {
        return Err("Escolha um arquivo com extensão .xlsx.".into());
    }
    Ok(canonical)
}

#[tauri::command]
pub async fn import_local_xlsx(
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    file_path: String,
    profile_id: String,
) -> Result<String, String> {
    use calamine::{Reader, Xlsx, open_workbook};

    let workbook_path = validate_local_xlsx_path(&file_path)?;
    let mut workbook: Xlsx<_> =
        open_workbook(&workbook_path).map_err(|e| format!("open error: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut total = 0usize;
    let mut sheets_imported = Vec::new();

    // Serializa contra o sync de fundo e o probe de foco no pool de 1 conexão (mesmo SyncGuard).
    // Segurado por TODO o loop de abas (cada aba é uma transação atômica própria).
    let _lock = guard.inner().lock().await;
    for sheet_name in &sheet_names {
        if layout_detect::is_metric_tab(sheet_name) {
            if sheet_name.trim().eq_ignore_ascii_case("economia")
                && let Ok(range) = workbook.worksheet_range(sheet_name)
            {
                let rows: Vec<Vec<String>> = range
                    .rows()
                    .map(|row| row.iter().map(xlsx_cell_to_string).collect())
                    .collect();
                let entries = import::parse_economia_sheet(&rows);
                if !entries.is_empty() {
                    let count = store_economia_entries(pool.inner(), &entries).await?;
                    total += count;
                    sheets_imported.push(format!("{sheet_name} ({count} months)"));
                }
            }
            continue;
        }
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| row.iter().map(xlsx_cell_to_string).collect())
                .collect();

            if rows.len() < 3 {
                continue;
            }

            // Layout: pré-existente (leitura no pool) ou recém-detectado. Quando NOVO, o INSERT do
            // layout/mappings é adiado para DENTRO da transação externa; guardamos os mappings
            // gerados para o parse, já que ainda não estarão visíveis por uma leitura no pool.
            let (layout, new_mappings) =
                match import::get_layout_for_sheet(&pool, sheet_name).await? {
                    Some(l) => (l, None),
                    None => match layout_detect::detect_layout(&rows, sheet_name) {
                        Ok(detected) => {
                            let mappings = layout_detect::generate_mappings(&detected);
                            (detected, Some(mappings))
                        }
                        Err(_) => continue,
                    },
                };

            // Mappings ativos para o parse: do pool (layout pré-existente) ou os recém-gerados
            // (layout novo, ainda não comitado).
            let mappings: Vec<(String, i32)> = match &new_mappings {
                Some(generated) => generated
                    .iter()
                    .filter(|m| m.is_active != 0)
                    .map(|m| (m.target_field.clone(), m.block_offset))
                    .collect(),
                None => import::get_active_mappings_for_sheet(&pool, sheet_name).await?,
            };

            // xlsx (calamine) não expõe notas de célula → fallback "Entrada/Saída {data}". As
            // notas só vêm pelo caminho ao vivo (Sheets API), então o fallback não vira base
            // canônica de descrição.
            let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &[])?;
            if imported_rows.is_empty() {
                continue;
            }

            let options = import::ImportRowsOptions {
                descriptions_trusted: false,
            };
            let checksum = import::compute_import_checksum(&imported_rows, false);
            if import::check_duplicate_import(&pool, sheet_name, &checksum).await? {
                continue;
            }

            // Série de Saldo da aba (semente da projeção + visão histórica do livro-razão).
            let balance_offset = import::get_balance_offset_for_sheet(&pool, sheet_name).await?;
            let balances = import::parse_balance_series(&rows, &layout, balance_offset)?;

            // Transação externa única por aba: layout + mappings + linhas + Saldo, tudo-ou-nada.
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| format!("begin import: {e}"))?;

            if let Some(generated) = &new_mappings {
                sqlx::query(
                        "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
                    )
                    .bind(&layout.id).bind(&layout.sheet_name).bind(layout.year)
                    .bind(layout.month_names_row).bind(layout.header_row).bind(layout.data_start_row)
                    .bind(layout.day_column).bind(layout.block_size).bind(&layout.date_direction)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("save layout: {e}"))?;

                for m in generated {
                    sqlx::query(
                            "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
                        )
                        .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
                        .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
                        .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("save mapping: {e}"))?;
                }
            }

            let count = import::import_rows_with_options_in_tx(
                &mut tx,
                sheet_name,
                &imported_rows,
                &profile_id,
                options,
                &checksum,
            )
            .await?;

            import::store_balance_series_in_tx(&mut tx, sheet_name, &balances).await?;

            tx.commit()
                .await
                .map_err(|e| format!("commit import: {e}"))?;

            total += count;
            sheets_imported.push(format!("{sheet_name} ({count} rows)"));
        }
    }

    // Sem notas de célula o classificador de 5 tipos não roda — quem
    // importa só por .xlsx veria Cartão/Economia dobrados em Saída sem saber por quê. O aviso
    // torna a degradação explícita; a classificação do último import ao vivo é preservada
    // (gate de confiança no `import_rows_core`).
    Ok(format!(
        "Imported {} total rows from: {}. Aviso: arquivos .xlsx não carregam notas de célula — a \
         classificação por seção (Cartão/Economia/Patrimônio) exige o import ao vivo do Google \
         Sheets; itens já classificados foram preservados.",
        total,
        sheets_imported.join(", ")
    ))
}

#[tauri::command]
pub async fn detect_sheet_layout(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<layout_detect::SheetLayout, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);

    // Grade inteira — A1:Z10 podia cortar a linha de dados/cabeçalhos da detecção (P1).
    let range = quote_sheet(&sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;
    let rows = values.values;

    let layout = layout_detect::detect_layout(&rows, &sheet_name)?;

    sqlx::query(
        "INSERT OR REPLACE INTO sheet_layout (id, sheet_name, year, month_names_row, header_row, data_start_row, day_column, block_size, date_direction) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
    )
    .bind(&layout.id).bind(&layout.sheet_name).bind(layout.year)
    .bind(layout.month_names_row).bind(layout.header_row).bind(layout.data_start_row)
    .bind(layout.day_column).bind(layout.block_size).bind(&layout.date_direction)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("save layout: {e}"))?;

    let mappings = layout_detect::generate_mappings(&layout);
    for m in &mappings {
        sqlx::query(
            "INSERT OR REPLACE INTO sheet_mapping (id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"
        )
        .bind(&m.id).bind(&m.sheet_name).bind(&m.column_letter).bind(&m.column_header)
        .bind(&m.target_table).bind(&m.target_field).bind(&m.date_direction)
        .bind(&m.layout_id).bind(m.block_offset).bind(m.is_active)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("save mapping: {e}"))?;
    }

    Ok(layout)
}

/// Importa a aba `Economia` da planilha → ANOTAÇÃO de métrica em `economia_annotation` (plano 052):
/// o Economizado% (= Economia/Entradas) é uma anotação manual, NÃO um movimento de caixa. A poupança
/// já é lançada como Saída no grid (→ cost_of_living → Saldo uma vez); gravar a aba como transação
/// duplicaria o desconto no Saldo. Chave `(perfil, ano, mês)` ⇒ re-import ATUALIZA, não duplica.
/// Alimenta o Economizado%/ColchaoCard e o write-back da Economia sem tocar o Saldo.
#[tauri::command]
pub async fn import_economia_sheet(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let values = client
        .get_sheet_values(&spreadsheet_id, "'Economia'")
        .await?;
    let entries = import::parse_economia_sheet(&values.values);
    if entries.is_empty() {
        return Ok(0);
    }

    store_economia_entries(pool.inner(), &entries).await
}

#[tauri::command]
pub async fn save_sheet_mapping(
    pool: State<'_, SqlitePool>,
    mapping_id: String,
    block_offset: i32,
    is_active: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE sheet_mapping SET block_offset = ?1, is_active = ?2 WHERE id = ?3")
        .bind(block_offset)
        .bind(is_active as i32)
        .bind(&mapping_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("update mapping: {e}"))?;
    Ok(())
}

type MappingRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    i32,
    i32,
);

#[tauri::command]
pub async fn get_sheet_mappings(
    pool: State<'_, SqlitePool>,
    sheet_name: String,
) -> Result<Vec<layout_detect::SheetMappingEntry>, String> {
    let rows: Vec<MappingRow> = sqlx::query_as(
            "SELECT id, sheet_name, column_letter, column_header, target_table, target_field, date_direction, layout_id, block_offset, is_active FROM sheet_mapping WHERE sheet_name = ?1 ORDER BY block_offset"
        )
        .bind(&sheet_name)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("query: {e}"))?;

    Ok(rows
        .into_iter()
        .map(
            |(id, sn, cl, ch, tt, tf, dd, lid, bo, ia)| layout_detect::SheetMappingEntry {
                id,
                sheet_name: sn,
                column_letter: cl,
                column_header: ch,
                target_table: tt,
                target_field: tf,
                date_direction: dd,
                layout_id: lid,
                block_offset: bo,
                is_active: ia,
            },
        )
        .collect())
}
