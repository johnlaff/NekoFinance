use crate::forecast::{self, CashflowEvent};
use crate::google_sheets::write_back::{self, CellWrite, WriteBackTxn};
use crate::google_sheets::{self, SheetsClient, import, layout_detect};
use crate::oauth::{self, AppDataDir, OAuthStateStore};
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

/// Aba entre aspas simples para um range A1 do Sheets, com as aspas internas escapadas (`'` → `''`).
/// Sem isto, uma aba chamada `O'Brien` quebraria o range (`'O'Brien'`) e a chamada à API falharia.
fn quote_sheet(name: impl AsRef<str>) -> String {
    format!("'{}'", name.as_ref().replace('\'', "''"))
}

#[tauri::command]
pub async fn start_oauth_flow(
    state: tauri::State<'_, OAuthStateStore>,
    app_dir: tauri::State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<String, String> {
    // Shell: o secret pode vir do env do processo (não do bundle do frontend) — ver resolve_*.
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let config = oauth::pkce::OAuthConfig::google(client_id, client_secret);
    // Liga o listener UMA vez e o mantém: a porta do redirect_uri e a que vamos escutar são a
    // mesma conexão — sem janela TOCTOU entre descobrir a porta e voltar a ligá-la.
    let (listener, port) = bind_loopback_listener()?;
    let oauth_state = oauth::pkce::OAuthState::new(port);

    let app_dir_path = app_dir.0.clone();
    let config_for_bg =
        oauth::pkce::OAuthConfig::google(config.client_id.clone(), config.client_secret.clone());

    // Store state and spawn flow
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        let flow_state = oauth_state.clone();
        *guard = Some(oauth_state);

        tokio::spawn(async move {
            match oauth::run_oauth_flow(config_for_bg, flow_state, app_dir_path, listener).await {
                Ok(_token) => {}
                Err(e) => eprintln!("OAuth flow error: {e}"),
            }
        });
    }

    // Clear state after spawn
    {
        let mut guard = state.0.lock().map_err(|e| format!("lock: {e}"))?;
        *guard = None;
    }

    Ok("oauth_started".to_string())
}

#[tauri::command]
pub async fn check_auth_status(app_dir: tauri::State<'_, AppDataDir>) -> Result<String, String> {
    match crate::oauth::token_store::load_token(&app_dir.0) {
        Ok(Some(token)) => {
            // Access token expirado mas com refresh_token disponível segue "connected":
            // ensure_valid_token renova sob demanda no próximo uso (spec 010, slice 2).
            if crate::oauth::token_store::is_token_expired(&token) && token.refresh_token.is_empty()
            {
                Ok("expired".to_string())
            } else {
                Ok("connected".to_string())
            }
        }
        Ok(None) => Ok("disconnected".to_string()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn disconnect_google(app_dir: tauri::State<'_, AppDataDir>) -> Result<(), String> {
    // Revoga no Google (best-effort) ANTES de apagar localmente — desconectar de verdade.
    crate::oauth::token_store::revoke_token(&app_dir.0).await;
    crate::oauth::token_store::delete_token(&app_dir.0)
}

/// Liga um socket de loopback numa porta efêmera e devolve `(listener, porta)`. O listener NÃO é
/// dropado: quem chama o usa para escutar o callback — eliminando o rebind (TOCTOU) do fluxo OAuth.
fn bind_loopback_listener() -> Result<(std::net::TcpListener, u16), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| format!("port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    Ok((listener, port))
}

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

#[tauri::command]
pub async fn import_sheet_data(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
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

    // Grade usada inteira: a planilha real tem 12 blocos mensais até a coluna BO (~71
    // colunas) — um range A:Z cortaria JUNHO–DEZEMBRO em silêncio (spec 010, slice 0).
    let range = quote_sheet(&sheet_name);
    let values = client.get_sheet_values(&spreadsheet_id, &range).await?;
    let rows = values.values;

    if rows.len() < 3 {
        return Ok(0);
    }

    // Detecção de layout (leituras no pool). Quando NOVO, o INSERT do layout/mappings é adiado para
    // DENTRO da transação externa (tudo-ou-nada); guardamos os mappings gerados para o parse, já que
    // ainda não estarão visíveis por uma leitura no pool.
    let (layout, new_mappings) = match import::get_layout_for_sheet(&pool, &sheet_name).await? {
        Some(l) => (l, None),
        None => {
            let detected = layout_detect::detect_layout(&rows, &sheet_name)?;
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
        None => import::get_active_mappings_for_sheet(&pool, &sheet_name).await?,
    };

    // Notas de célula = a descrição real de cada lançamento (quem/o quê/quanto por item). Sem
    // elas, o parser só tem fallback estrutural ("Entrada/Saída {data}"). Se a API de notas
    // falhar, os valores ainda entram, mas essas descrições não são tratadas como fonte canônica.
    let (notes, descriptions_trusted) =
        match client.get_sheet_notes(&spreadsheet_id, &sheet_name).await {
            Ok(notes) => (notes, true),
            Err(_) => (Vec::new(), false),
        };
    let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &notes);
    let options = import::ImportRowsOptions {
        descriptions_trusted,
    };

    // Checksum + checagem de duplicata ANTES de abrir a transação (leitura no pool; dentro da tx
    // daria read-your-writes falso-negativo). Dataset idêntico ao último import → idempotente.
    let checksum = import::compute_import_checksum(&imported_rows, descriptions_trusted);
    if !imported_rows.is_empty()
        && import::check_duplicate_import(&pool, &sheet_name, &checksum).await?
    {
        return Ok(0);
    }

    // Captura a coluna Saldo (o saldo corrente do método) → semente da projeção + visão histórica.
    // Sem isto a semente era 0 e o saldo de hoje aparecia zerado. `get_balance_offset_for_sheet` é
    // leitura no pool; pode rodar antes de abrir a tx.
    let balance_offset = import::get_balance_offset_for_sheet(&pool, &sheet_name).await?;
    let balances = import::parse_balance_series(&rows, &layout, balance_offset);

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
        &sheet_name,
        &imported_rows,
        &profile_id,
        options,
        &checksum,
    )
    .await?;

    import::store_balance_series_in_tx(&mut tx, &sheet_name, &balances).await?;

    tx.commit()
        .await
        .map_err(|e| format!("commit import: {e}"))?;

    Ok(count)
}

/// Células numéricas do calamine viram string decimal-com-ponto de 4 casas fixas: `123.456`
/// vira `123.4560`, que o `parse_number` nunca confunde com agrupamento de milhar
/// (spec 010, slice 0 — antes, `12.34` perdia o ponto e inflava 100×).
fn xlsx_cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Float(f) => format!("{f:.4}"),
        other => other.to_string().trim().to_string(),
    }
}

fn validate_local_xlsx_path(file_path: &str) -> Result<std::path::PathBuf, String> {
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
            let imported_rows = import::parse_rows_with_layout(&rows, &layout, &mappings, &[]);
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
            let balances = import::parse_balance_series(&rows, &layout, balance_offset);

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

    Ok(format!(
        "Imported {} total rows from: {}",
        total,
        sheets_imported.join(", ")
    ))
}

// --- App info ---

#[derive(serde::Serialize)]
pub struct AppInfo {
    pub version: String,
    pub db_path: String,
}

#[tauri::command]
pub async fn get_app_info(app_dir: State<'_, AppDataDir>) -> Result<AppInfo, String> {
    Ok(app_info_for_dir(&app_dir.0))
}

/// Pure helper so the command stays a thin adapter (testable without Tauri `State`).
fn app_info_for_dir(app_dir: &std::path::Path) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        db_path: app_dir.join("neko-finance.db").display().to_string(),
    }
}

// --- Forecast projection (spec 005) ---

/// Sum of liquid cash accounts — the projection seed (spec 003 US2).
/// Spec 007: only `liquidity = 'liquid'` pockets are cash; reserve/restricted/illiquid
/// money must not inflate the projected balance.
async fn liquid_seed(pool: &SqlitePool) -> Result<i64, String> {
    let seed: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'liquid'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query: {e}"))?;
    Ok(seed.0)
}

/// Semente da projeção — o saldo de partida do qual a engine encadeia o futuro.
///
/// Método da planilha: a coluna `Saldo` É o saldo que "bate com o banco". Quando há série
/// importada, a semente = `Saldo` do dia mais recente ≤ hoje; quaisquer lançamentos realizados
/// ENTRE esse dia e hoje são somados (cobre o caso de a planilha ainda não ter hoje preenchido),
/// de modo que o carregador de eventos pode seguir usando `date > today` sem perder o intervalo.
/// Sem planilha importada, cai nos Bolsos líquidos (spec 007). Precedência: planilha > bolsos —
/// quem importa a planilha quer que a projeção continue a própria linha dela.
async fn projection_seed(pool: &SqlitePool, today_naive: NaiveDate) -> Result<i64, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    let latest: Option<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date <= ?1 ORDER BY date DESC LIMIT 1",
    )
    .bind(&today)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("seed query: {e}"))?;

    let Some((seed_date, balance)) = latest else {
        return liquid_seed(pool).await;
    };

    let gap: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE -amount END), 0) \
         FROM \"transaction\" WHERE date > ?1 AND date <= ?2 AND type IN ('income','expense')",
    )
    .bind(&seed_date)
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("seed gap: {e}"))?;

    Ok(balance + gap.0)
}

/// Meta de poupança do método: piso de 25% (faixa 20–30%, MÉDIA ANUAL — o ano todo deve ficar
/// na faixa, os meses variam). Régua do guardrail ANUAL "pode gastar".
/// O badge MENSAL "Dentro do ideal" (src/screens/TotaisScreen.tsx) usa 20% (piso da faixa), por ser
/// leniente a variações de um mês; ambos ficam dentro da faixa canônica 20–30%.
const SAVINGS_TARGET_BPS: i64 = 2500;

/// Limiar de cobertura: um mês futuro com menos de 60% do gasto típico já lançado é tratado como
/// INCOMPLETO (projeção otimista demais — o "chá revelação" do método). Margem ampla porque o
/// método aceita variação mês a mês; abaixo disso é quase certo que falta fatura/variável.
const COVERAGE_COMPLETE_BPS: i64 = 6_000;

/// Renda e poupança REALIZADAS do ano corrente até hoje (`is_projection = 0`). Proxy de
/// Entradas/Economia da aba Economia (que ainda não é importada — slice 7): a poupança é o net
/// `renda − saída` realizado. Usada no guardrail anual: o ano PROJETADO mente quando os meses
/// futuros estão incompletos (só fixas/salário), então a régua olha só o que já aconteceu.
/// `transfer` é IGNORADO (não há linha Economia explícita ainda) — a poupança real virá do saldo
/// da reserva quando o slice de Economia existir; até lá o net é um proxy conservador (review P2).
///
/// Conta só **meses COMPLETOS** do ano (`substr(date) < mês corrente`), nunca o mês em andamento.
/// No meio do mês as contas fixas já podem ter entrado mas o salário ainda não, o que daria um
/// net negativo de timing e um "pode gastar R$ 0" de falso pânico.
///
/// NÃO filtra `is_projection`: ele é congelado no import (data vs hoje DAQUELE dia) e fica STALE
/// quando o dono não re-importa por dias/meses. Um mês completo já passou — é realizado por
/// definição —, então a janela de DATA é a fonte de verdade, não o flag congelado.
async fn realized_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND substr(date,1,7) < ?2 \
           AND type IN ('income','expense')",
    )
    .bind(&year_start)
    .bind(&cur_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized annual: {e}"))?;
    Ok((row.0, row.0 - row.1)) // (renda, poupança=net) dos meses completos
}

/// Economia REGISTRADA do ano até hoje (meses completos): transfers cujo destino é conta
/// reserva/ilíquida — mesma classificação de `forecast::classify`. É o numerador do "Economizado"
/// do método (Economia/Entradas), DISTINTO do net superávit de `realized_annual_savings` (que é o
/// "colchão" do Neko). Existir os dois lado a lado sem se confundir foi um achado da review.
async fn realized_annual_economia(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let year_start = format!("{}-01-01", today_naive.year());
    let cur_ym = today_naive.format("%Y-%m").to_string();
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND substr(t.date,1,7) < ?2 \
           AND t.type='transfer' AND a.liquidity IN ('reserve','illiquid')",
    )
    .bind(&year_start)
    .bind(&cur_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("realized economia: {e}"))?;
    Ok(row.0)
}

/// Renda e net do ANO INTEIRO projetado (todas as linhas do ano). Exibido só como contraste com
/// o realizado — é OTIMISTA quando os meses futuros estão incompletos (não usar no guardrail).
async fn projected_annual_savings(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<(i64, i64), String> {
    // Range explícito (não `LIKE 'YYYY%'`) — consistente com o realizado e rejeita data
    // malformada que começa com o ano mas não é ISO válida (review P2).
    let start = format!("{}-01-01", today_naive.year());
    let end = format!("{}-12-31", today_naive.year());
    let row: (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
           COALESCE(SUM(CASE WHEN type='expense' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date >= ?1 AND date <= ?2 AND type IN ('income','expense')",
    )
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("projected annual: {e}"))?;
    Ok((row.0, row.0 - row.1))
}

/// Gasto típico de um mês = MEDIANA da saída dos meses realizados COMPLETOS (anteriores ao mês
/// corrente), dos **últimos 6 meses** (recentes representam melhor o padrão atual que meses
/// antigos de anos anteriores — review ui-vs-planilha). Mediana para ser robusta a um mês atípico.
async fn realized_monthly_baseline(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<i64, String> {
    let cur_ym = today_naive.format("%Y-%m").to_string();
    // Sem filtro `is_projection` (congelado/stale): meses completos já passaram, a data decide.
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT SUM(amount) FROM \"transaction\" \
         WHERE type='expense' AND substr(date,1,7) < ?1 \
         GROUP BY substr(date,1,7) ORDER BY substr(date,1,7) DESC LIMIT 6",
    )
    .bind(&cur_ym)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("baseline: {e}"))?;
    let mut vals: Vec<i64> = rows.into_iter().map(|(s,)| s).collect();
    if vals.is_empty() {
        return Ok(0);
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    let median = if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2
    };
    Ok(median)
}

/// Teto de diário "típico" por dia. Fonte única para (a) projetar o gasto diário nos dias futuros do
/// mês corrente (driver do forecast — senão o saldo nasce otimista assumindo zero gasto) e (b) a
/// referência exibida no tile "Diário de hoje" (`de R$X`). Regra:
/// 1. se houver orçamento diário explícito ativo (> 0), ele vence (o dono definiu um teto);
/// 2. senão, o Diário médio do último mês COMPLETO = Σ diário realizado (despesa não-fixa, não-crédito)
///    ÷ dias do mês. Espelha o `real_daily_avg_cents` do método ("Diário médio") sobre o mês anterior.
///
/// Sem mês anterior com diário, retorna 0 (usuário novo — nada a assumir).
async fn effective_daily_ceiling(pool: &SqlitePool, today_naive: NaiveDate) -> Result<i64, String> {
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT amount FROM daily_budget WHERE status='active' AND amount > 0 \
         ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("daily ceiling (budget): {e}"))?;
    if let Some((amount,)) = active {
        return Ok(amount);
    }
    // Mês anterior completo: primeiro dia do mês corrente − 1 dia.
    let first_this = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data inválida")?;
    let last_prev = match first_this.pred_opt() {
        Some(d) => d,
        None => return Ok(0),
    };
    let prev_ym = last_prev.format("%Y-%m").to_string();
    let days_prev = last_prev.day() as i64;
    let sum: (i64,) = sqlx::query_as(
        "SELECT ABS(COALESCE(SUM(amount), 0)) FROM \"transaction\" \
         WHERE type='expense' AND is_fixed=0 AND is_projection=0 \
           AND (payment_method IS NULL OR payment_method <> 'credit') \
           AND substr(date,1,7) = ?1",
    )
    .bind(&prev_ym)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("daily ceiling (avg): {e}"))?;
    Ok(if days_prev > 0 { sum.0 / days_prev } else { 0 })
}

/// Piso de reserva = colchão intocável que a folga de caixa não pode comer. Por ora = soma dos
/// Bolsos marcados como reserva (spec 007, `liquidity = 'reserve'`); esses NÃO entram na semente
/// líquida, então subtraí-los aqui não dobra. O ideal metodológico (custo de vida × 12) fica
/// para quando a reserva for modelada como meta — ver limitações na spec 010. Hoje, sem reserva
/// configurada, retorna 0 e a régua de poupança é a que morde.
async fn reserve_floor(pool: &SqlitePool) -> Result<i64, String> {
    let floor: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("reserve floor: {e}"))?;
    Ok(floor.0)
}

/// Fim do horizonte da projeção = o último dia com dado pré-lançado (transação futura ou Saldo
/// importado) ≥ hoje. A planilha do método já lança o ano inteiro à frente, então varremos ATÉ
/// O FIM DOS DADOS, não só o mês corrente — senão o "pode gastar" fica cego ao buraco do futuro
/// e às faturas dos meses seguintes (decisão do dono). Piso: fim do mês corrente.
async fn forecast_horizon_end(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<NaiveDate, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let max_txn: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM \"transaction\" WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon txn: {e}"))?;
    let max_bal: (Option<String>,) =
        sqlx::query_as("SELECT MAX(date) FROM sheet_daily_balance WHERE date >= ?1")
            .bind(&today)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("horizon bal: {e}"))?;

    let mut horizon = forecast::last_day_of_month(today_naive.year(), today_naive.month());
    for (candidate,) in [max_txn, max_bal] {
        if let Some(date_str) = candidate
            && let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            && d > horizon
        {
            horizon = d;
        }
    }
    Ok(horizon)
}

/// Loads forward cashflow events for the projection window: future transactions (date > today,
/// avoiding double-counting today's already-realized spending baked into the balance snapshot)
/// plus credit-cycle lumps aggregated from `daily_checkin` at the card due date (Régua 2).
/// Single source of row→event mapping, shared by `dashboard_summary` and `forecast_dto`.
async fn load_cashflow_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();
    let horizon = horizon_end.format("%Y-%m-%d").to_string();

    // Liquidez da conta-destino entra no SELECT para classificar `transfer` → Economia (guardar
    // num bolso não-líquido) vs net-zero (entre contas líquidas).
    let txn_rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date > ?1 AND t.date <= ?2",
    )
    .bind(&today)
    .bind(&horizon)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    let mut all_events: Vec<CashflowEvent> = txn_rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let to_liq = (!liq.is_empty()).then_some(liq);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
            Some(CashflowEvent {
                date,
                kind,
                // Invariante: o evento guarda MAGNITUDE positiva; o sinal vem do `kind` (ver
                // `forecast::signed`). `.abs()` blinda contra um `transfer` gravado negativo.
                amount_cents: amount.abs(),
                realized: is_proj == 0,
            })
        })
        .collect();

    // `closing_day`/`due_day` são NULL-áveis (um cartão pode ser criado sem ciclo). Filtra no SQL
    // só os cartões com ciclo COMPLETO, em ordem determinística — um NULL faria o decode (i32,i32)
    // estourar, e sem ORDER BY a escolha do "primeiro" cartão (multi-card é slice futura) variaria.
    let credit_cards: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT closing_day, due_day FROM account \
         WHERE type = 'credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query credit cards: {e}"))?;

    if !credit_cards.is_empty() {
        // Use the first card's closing/due days (multi-card aggregation is a later slice)
        let (closing_day, due_day) = credit_cards[0];
        let closing_day = closing_day as u32;
        let due_day = due_day as u32;

        let checkins: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT date, daily_spend, credit_spend FROM daily_checkin WHERE date > ?1 AND date <= ?2",
        )
        .bind(&today)
        .bind(&horizon)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("query checkins: {e}"))?;

        let mut credit_by_due: std::collections::HashMap<NaiveDate, i64> =
            std::collections::HashMap::new();

        for (date_str, daily_spend, credit_spend) in checkins {
            if let Ok(checkin_date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                // Daily spend (Régua 1) → Daily event on its day
                if daily_spend > 0 {
                    all_events.push(CashflowEvent {
                        date: checkin_date,
                        kind: forecast::EventKind::Daily,
                        amount_cents: daily_spend,
                        realized: true,
                    });
                }

                // Credit spend (Régua 2) → aggregate by due_date
                if credit_spend > 0 {
                    let due_date = forecast::cycle_due_date(checkin_date, closing_day, due_day);
                    *credit_by_due.entry(due_date).or_insert(0) += credit_spend;
                }
            }
        }

        for (due_date, total_credit) in credit_by_due {
            all_events.push(CashflowEvent {
                date: due_date,
                kind: forecast::EventKind::FixedOut,
                amount_cents: total_credit,
                realized: false, // future projection
            });
        }
    }

    Ok(all_events)
}

/// Eventos que alimentam projeções de caixa: lançamentos reais/futuros + Diário típico futuro do
/// mês corrente. Usado por `forecast_dto` e `dashboard_summary` para manter o saldo projetado
/// idêntico em todas as telas.
async fn load_forecast_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    horizon_end: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let mut events = load_cashflow_events(pool, today_naive, horizon_end).await?;
    // Previsão de diário como DRIVER: injeta o teto/dia nos dias futuros do mês corrente, para o
    // saldo projetado e a Performance não nascerem otimistas (assumem o gasto típico até o fim do mês).
    let daily_ceiling = effective_daily_ceiling(pool, today_naive).await?;
    let days_with_daily: std::collections::HashSet<NaiveDate> = events
        .iter()
        .filter(|e| e.kind == forecast::EventKind::Daily)
        .map(|e| e.date)
        .collect();
    events.extend(forecast::project_daily_ceiling(
        daily_ceiling,
        today_naive,
        horizon_end,
        &days_with_daily,
    ));
    Ok(events)
}

/// Eventos JÁ REALIZADOS do mês corrente (`month_start..=today`), classificados como os futuros.
/// O encadeamento de caixa não os usa (a semente já os embute), mas a performance do mês precisa
/// deles — senão o mês corrente aparece pela metade (review adversarial P0). Só transações; os
/// lumps de fatura realizados deste mês já estão na coluna Saída da planilha como transação.
async fn load_realized_month_events(
    pool: &SqlitePool,
    month_start: NaiveDate,
    today_naive: NaiveDate,
) -> Result<Vec<CashflowEvent>, String> {
    let start = month_start.format("%Y-%m-%d").to_string();
    let today = today_naive.format("%Y-%m-%d").to_string();

    let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE t.date >= ?1 AND t.date <= ?2",
    )
    .bind(&start)
    .bind(&today)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query realized month: {e}"))?;

    Ok(rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let to_liq = (!liq.is_empty()).then_some(liq);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
            Some(CashflowEvent {
                date,
                kind,
                // Invariante: o evento guarda MAGNITUDE positiva; o sinal vem do `kind` (ver
                // `forecast::signed`). `.abs()` blinda contra um `transfer` gravado negativo.
                amount_cents: amount.abs(),
                realized: is_proj == 0,
            })
        })
        .collect())
}

/// Eventos para as MÉTRICAS por mês = futuros (encadeamento) + realizados do mês corrente.
/// Cobre o mês inteiro de hoje (realizado + projetado); meses à frente já são todos futuros.
async fn load_metric_events(
    pool: &SqlitePool,
    today_naive: NaiveDate,
    future_events: &[CashflowEvent],
) -> Result<Vec<CashflowEvent>, String> {
    let month_start = NaiveDate::from_ymd_opt(today_naive.year(), today_naive.month(), 1)
        .ok_or("data de hoje inválida")?;
    let mut metric = load_realized_month_events(pool, month_start, today_naive).await?;
    metric.extend_from_slice(future_events);
    Ok(metric)
}

#[derive(serde::Serialize)]
pub struct ForecastDayDto {
    pub date: String,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub economia_cents: i64,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct DayPointDto {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthEndDto {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(serde::Serialize)]
pub struct MonthMetricDto {
    pub year: i32,
    pub month: u32,
    pub income_cents: i64,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    /// Saídas fixas realizadas (coluna Saída; cartão entra como lump). Para o rodapé ENTRADAS|SAÍDAS|DIÁRIO.
    pub fixed_out_cents: i64,
    /// Diário realizado (coluna Diário). `cost_of_living = fixed_out + daily_out`.
    pub daily_out_cents: i64,
    /// Diário médio do mês = Σ diário realizado ÷ dias decorridos (D/N). Antes morria no DTO.
    pub real_daily_avg_cents: i64,
    /// Economia lançada no mês (numerador do Economizado%).
    pub economia_cents: i64,
    pub savings_rate_bps: i64,
}

/// Poupança do ano: realizada (honesta) vs projetada (otimista quando o futuro está incompleto).
/// ATENÇÃO a dois conceitos distintos (não confundir na UI): `*_savings_cents` é o NET superávit
/// (renda − saída), o "colchão" do Neko; `registered_economia_cents` é a Economia REGISTRADA do
/// método (transfers→reserva), numerador do Economizado%. O guardrail usa o net (colchão); o
/// Economizado mensal usa a Economia registrada.
#[derive(serde::Serialize)]
pub struct AnnualSavingsDto {
    pub realized_income_cents: i64,
    pub realized_savings_cents: i64,
    pub realized_rate_bps: i64,
    /// Economia REGISTRADA do ano (transfers→reserva/ilíquido), meses completos. Distinta do net.
    pub registered_economia_cents: i64,
    pub projected_income_cents: i64,
    pub projected_savings_cents: i64,
    pub projected_rate_bps: i64,
    pub target_bps: i64,
}

/// Cobertura de um mês futuro (quanto do gasto típico já está lançado).
#[derive(serde::Serialize)]
pub struct MonthCoverageDto {
    pub year: i32,
    pub month: u32,
    pub projected_outflow_cents: i64,
    pub baseline_outflow_cents: i64,
    pub coverage_bps: i64,
    pub is_complete: bool,
    pub estimated_missing_cents: i64,
}

#[derive(serde::Serialize)]
pub struct ForecastDto {
    pub today: String,
    pub horizon_end: String,
    /// Poupança do ano — realizada vs projetada (previsibilidade).
    pub annual_savings: AnnualSavingsDto,
    /// Cobertura por mês futuro (vazio se a projeção está completa).
    pub coverage: Vec<MonthCoverageDto>,
    /// Gasto típico/mês (mediana realizada). `0` = sem histórico → previsibilidade indeterminada.
    pub baseline_outflow_cents: i64,
    /// Último mês cuja projeção é confiável ("YYYY-MM"); `null` se não há baseline para avaliar.
    pub trusted_through_month: Option<String>,
    /// Soma do que falta lançar nos meses incompletos (fatura + variáveis).
    pub total_missing_cents: i64,
    /// "Pode gastar hoje" honesto: o MAIS APERTADO de caixa × poupança (guardrail duplo).
    pub safe_to_spend_today_cents: i64,
    /// Folga de caixa (menor saldo projetado no horizonte − piso de reserva).
    pub cash_headroom_cents: i64,
    /// Folga da meta de poupança do mês corrente (negativa = já abaixo da meta). `null` quando a
    /// régua de poupança está inativa (mês sem renda) → só o caixa decide.
    pub savings_headroom_cents: Option<i64>,
    /// Qual régua limita: "cash" ou "savings".
    pub binding_guardrail: String,
    /// Meta de poupança em basis points (2500 = 25%).
    pub savings_target_bps: i64,
    pub deepest_deficit: Option<DayPointDto>,
    pub daily: Vec<ForecastDayDto>,
    pub month_end: Vec<MonthEndDto>,
    /// Performance/poupança por mês (Caixa ≠ Performance; expõe meses futuros magros).
    pub months: Vec<MonthMetricDto>,
}

#[tauri::command]
pub async fn get_forecast(pool: State<'_, SqlitePool>) -> Result<ForecastDto, String> {
    forecast_dto(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation with an injected `today` (deterministic, integration-testable).
/// Maps the pure engine output to ISO-8601-string DTOs; the core stays serde-free.
async fn forecast_dto(pool: &SqlitePool, today_naive: NaiveDate) -> Result<ForecastDto, String> {
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let seed = projection_seed(pool, today_naive).await?;
    let events = load_forecast_events(pool, today_naive, horizon_end).await?;
    let metric_events = load_metric_events(pool, today_naive, &events).await?;
    let fc =
        forecast::project_with_metrics(seed, today_naive, &events, &metric_events, horizon_end);

    let reserve_floor_cents = reserve_floor(pool).await?;
    // Poupança ANUAL realizada (não o mês isolado, não o ano projetado-incompleto).
    let (annual_income, annual_savings_amt) = realized_annual_savings(pool, today_naive).await?;
    let sts = forecast::safe_to_spend_today(
        &fc,
        annual_income,
        annual_savings_amt,
        SAVINGS_TARGET_BPS,
        reserve_floor_cents,
    );
    let binding_guardrail = match sts.binding {
        forecast::Guardrail::Cash => "cash",
        forecast::Guardrail::Savings => "savings",
    }
    .to_string();

    // Previsibilidade: poupança realizada vs projetada + cobertura dos meses futuros.
    let (proj_income, proj_savings) = projected_annual_savings(pool, today_naive).await?;
    // Taxa em bps para EXIBIÇÃO (round half-up, não trunca — senão 25,00% vira 2499/abaixo da
    // meta; review P3). Nunca usada em decisão (o guardrail compara centavos diretos).
    let rate_bps = |save: i64, inc: i64| {
        if inc > 0 {
            (save * 10_000 + inc / 2) / inc
        } else {
            0
        }
    };
    let annual_economia = realized_annual_economia(pool, today_naive).await?;
    let annual_savings = AnnualSavingsDto {
        realized_income_cents: annual_income,
        realized_savings_cents: annual_savings_amt,
        realized_rate_bps: rate_bps(annual_savings_amt, annual_income),
        registered_economia_cents: annual_economia,
        projected_income_cents: proj_income,
        projected_savings_cents: proj_savings,
        projected_rate_bps: rate_bps(proj_savings, proj_income),
        target_bps: SAVINGS_TARGET_BPS,
    };

    let baseline = realized_monthly_baseline(pool, today_naive).await?;
    let coverage_raw =
        forecast::month_coverage(&fc.months, today_naive, baseline, COVERAGE_COMPLETE_BPS);
    // Sem baseline (nenhum mês realizado) não dá para afirmar "confiável até X" → `None`. Com
    // baseline, o mês corrente é sempre confiável (tem o realizado) e estende pelos meses futuros
    // completos até o primeiro incompleto.
    let trusted_through_month = if baseline <= 0 {
        None
    } else {
        let mut trusted = format!("{:04}-{:02}", today_naive.year(), today_naive.month());
        for c in coverage_raw.iter() {
            if c.is_complete {
                trusted = format!("{:04}-{:02}", c.year, c.month);
            } else {
                break;
            }
        }
        Some(trusted)
    };
    let total_missing_cents = coverage_raw
        .iter()
        .filter(|c| !c.is_complete)
        .map(|c| c.estimated_missing_cents)
        .sum();
    let coverage: Vec<MonthCoverageDto> = coverage_raw
        .iter()
        .map(|c| MonthCoverageDto {
            year: c.year,
            month: c.month,
            projected_outflow_cents: c.projected_outflow_cents,
            baseline_outflow_cents: c.baseline_outflow_cents,
            coverage_bps: c.coverage_bps,
            is_complete: c.is_complete,
            estimated_missing_cents: c.estimated_missing_cents,
        })
        .collect();

    // Per-day flow sums (income, fixed out, daily out), keyed by the same dates the engine emits.
    let mut flows: std::collections::HashMap<NaiveDate, (i64, i64, i64, i64)> =
        std::collections::HashMap::new();
    for e in &events {
        let entry = flows.entry(e.date).or_default();
        match e.kind {
            forecast::EventKind::Income => entry.0 += e.amount_cents,
            forecast::EventKind::FixedOut => entry.1 += e.amount_cents,
            forecast::EventKind::Daily => entry.2 += e.amount_cents,
            forecast::EventKind::Economia => entry.3 += e.amount_cents,
        }
    }

    let daily = fc
        .daily
        .iter()
        .map(|p| {
            let (income, fixed_out, daily_out, economia) =
                flows.get(&p.date).copied().unwrap_or_default();
            ForecastDayDto {
                date: p.date.format("%Y-%m-%d").to_string(),
                income_cents: income,
                fixed_out_cents: fixed_out,
                daily_out_cents: daily_out,
                economia_cents: economia,
                balance_cents: p.balance_cents,
            }
        })
        .collect();

    Ok(ForecastDto {
        today: today_naive.format("%Y-%m-%d").to_string(),
        horizon_end: horizon_end.format("%Y-%m-%d").to_string(),
        annual_savings,
        coverage,
        baseline_outflow_cents: baseline,
        trusted_through_month,
        total_missing_cents,
        safe_to_spend_today_cents: sts.amount_cents,
        cash_headroom_cents: sts.cash_headroom_cents,
        savings_headroom_cents: sts.savings_headroom_cents,
        binding_guardrail,
        savings_target_bps: SAVINGS_TARGET_BPS,
        deepest_deficit: fc.deepest_deficit.as_ref().map(|p| DayPointDto {
            date: p.date.format("%Y-%m-%d").to_string(),
            balance_cents: p.balance_cents,
        }),
        daily,
        month_end: fc
            .month_end
            .iter()
            .map(|m| MonthEndDto {
                year: m.year,
                month: m.month,
                balance_cents: m.balance_cents,
            })
            .collect(),
        months: fc
            .months
            .iter()
            .map(|m| MonthMetricDto {
                year: m.year,
                month: m.month,
                income_cents: m.income_cents,
                performance_cents: m.performance_cents,
                cost_of_living_cents: m.cost_of_living_cents,
                fixed_out_cents: m.fixed_out_cents,
                daily_out_cents: m.daily_out_cents,
                real_daily_avg_cents: m.real_daily_avg_cents,
                economia_cents: m.economia_cents,
                savings_rate_bps: m.savings_rate_bps,
            })
            .collect(),
    })
}

// --- Visão anual (spec 019 month-views) ---

/// Todos os eventos do ANO (realizado + projetado), classificados — sem o teto de diário (que só
/// vale para o mês corrente no forecast). Para a visão anual das 4 métricas.
async fn load_year_events(pool: &SqlitePool, year: i32) -> Result<Vec<CashflowEvent>, String> {
    let rows: Vec<(String, i64, String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT t.type, t.amount, t.date, COALESCE(t.payment_method,''), t.is_fixed, t.is_projection, \
                COALESCE(a.liquidity,'') \
         FROM \"transaction\" t LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE substr(t.date, 1, 4) = ?1",
    )
    .bind(format!("{year:04}"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query year events: {e}"))?;

    Ok(rows
        .into_iter()
        .filter_map(|(ttype, amount, date_str, pm, is_fixed, is_proj, liq)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            let pm = (!pm.is_empty()).then_some(pm);
            let to_liq = (!liq.is_empty()).then_some(liq);
            let kind = forecast::classify(&ttype, is_fixed != 0, pm.as_deref(), to_liq.as_deref())?;
            Some(CashflowEvent {
                date,
                kind,
                // Invariante: o evento guarda MAGNITUDE positiva; o sinal vem do `kind` (ver
                // `forecast::signed`). `.abs()` blinda contra um `transfer` gravado negativo.
                amount_cents: amount.abs(),
                realized: is_proj == 0,
            })
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct AnnualMetricsDto {
    pub year: i32,
    pub months: Vec<MonthMetricDto>,
}

async fn annual_metrics(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<AnnualMetricsDto, String> {
    let events = load_year_events(pool, year).await?;
    let months: Vec<(i32, u32)> = (1..=12).map(|m| (year, m)).collect();
    let metrics = forecast::month_metrics_for(today, &events, &months);
    let months = metrics
        .iter()
        .map(|m| MonthMetricDto {
            year: m.year,
            month: m.month,
            income_cents: m.income_cents,
            performance_cents: m.performance_cents,
            cost_of_living_cents: m.cost_of_living_cents,
            fixed_out_cents: m.fixed_out_cents,
            daily_out_cents: m.daily_out_cents,
            real_daily_avg_cents: m.real_daily_avg_cents,
            economia_cents: m.economia_cents,
            savings_rate_bps: m.savings_rate_bps,
        })
        .collect();
    Ok(AnnualMetricsDto { year, months })
}

// --- Grade do mês (visão fiel à planilha: Data | Entrada | Saída | Diário | Saldo) ---

/// Um dia da grade mensal. `balance_cents` é o Saldo encadeado da planilha (None se aquele dia não
/// foi importado). Os fluxos são agregados das transações do dia, separados por tipo.
#[derive(serde::Serialize)]
pub struct MonthGridDayDto {
    pub date: String,
    pub day: u32,
    pub income_cents: i64,
    pub fixed_out_cents: i64,
    pub daily_out_cents: i64,
    pub balance_cents: Option<i64>,
}

/// Grade de TODOS os dias de um mês (1..último), com os fluxos realizados/pré-lançados agregados por
/// dia e o Saldo da planilha (`sheet_daily_balance`). É a visão Data|Entrada|Saída|Diário|Saldo que o
/// usuário tem na planilha, para QUALQUER mês (passado ou futuro) — diferente do `forecast.daily`,
/// que só vai de hoje para frente. Dias sem Saldo importado vêm com `balance_cents = None`.
async fn month_grid(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> Result<Vec<MonthGridDayDto>, String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or("mês inválido")?;
    let last = forecast::last_day_of_month(year, month);
    let first_s = first.format("%Y-%m-%d").to_string();
    let last_s = last.format("%Y-%m-%d").to_string();

    // Fluxos por dia, separados por tipo (Entrada / Saída fixa / Diário variável).
    let flows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        // Crédito (régua 2) entra em Saída como a fatura, não em Diário — espelha forecast::classify.
        "SELECT date, \
                COALESCE(SUM(CASE WHEN type='income' THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND (COALESCE(is_fixed,0)=1 OR payment_method='credit') THEN amount ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN type='expense' AND COALESCE(is_fixed,0)=0 AND COALESCE(payment_method,'')<>'credit' THEN amount ELSE 0 END), 0) \
         FROM \"transaction\" WHERE date BETWEEN ?1 AND ?2 GROUP BY date",
    )
    .bind(&first_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("month flows: {e}"))?;

    // Saldo da planilha por dia.
    let balances: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date, balance_cents FROM sheet_daily_balance WHERE date BETWEEN ?1 AND ?2",
    )
    .bind(&first_s)
    .bind(&last_s)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("month balances: {e}"))?;

    let flow_of = |d: &str| flows.iter().find(|f| f.0 == d).map(|f| (f.1, f.2, f.3));
    let balance_of = |d: &str| balances.iter().find(|b| b.0 == d).map(|b| b.1);

    let n_days = (last - first).num_days() + 1;
    let mut grid = Vec::with_capacity(n_days as usize);
    for offset in 0..n_days {
        let date = first + chrono::Duration::days(offset);
        let date_s = date.format("%Y-%m-%d").to_string();
        let (income, fixed_out, daily_out) = flow_of(&date_s).unwrap_or((0, 0, 0));
        grid.push(MonthGridDayDto {
            day: date.day(),
            income_cents: income,
            fixed_out_cents: fixed_out,
            daily_out_cents: daily_out,
            balance_cents: balance_of(&date_s),
            date: date_s,
        });
    }
    Ok(grid)
}

/// Grade do mês `year-month` (visão fiel à planilha). Ver [`month_grid`].
#[tauri::command]
pub async fn get_month_grid(
    pool: State<'_, SqlitePool>,
    year: i32,
    month: u32,
) -> Result<Vec<MonthGridDayDto>, String> {
    month_grid(pool.inner(), year, month).await
}

#[tauri::command]
pub async fn get_annual_metrics(
    pool: State<'_, SqlitePool>,
    year: i32,
) -> Result<AnnualMetricsDto, String> {
    annual_metrics(pool.inner(), year, chrono::Local::now().date_naive()).await
}

// --- Dashboard query commands ---

#[derive(serde::Serialize)]
pub struct DashboardSummary {
    pub balance: i64,
    pub daily_budget: i64,
    pub daily_spend_today: i64,
    pub credit_spend_month: i64,
    /// Há rastreio de crédito (cartão ou gasto de crédito). `false` → a UI mostra "—" no tile,
    /// não um R$0 estrutural.
    pub has_credit: bool,
    pub reserve_months: f64,
    pub reserve_trend: String,
    pub transaction_count: i64,
}

#[tauri::command]
pub async fn get_dashboard_summary(
    pool: State<'_, SqlitePool>,
) -> Result<DashboardSummary, String> {
    dashboard_summary(pool.inner(), chrono::Local::now().date_naive()).await
}

/// Inner implementation: takes `&SqlitePool` and an injected `today`, so it is deterministic and
/// integration-testable without Tauri `State` or the ambient clock.
async fn dashboard_summary(
    pool: &SqlitePool,
    today_naive: NaiveDate,
) -> Result<DashboardSummary, String> {
    let today = today_naive.format("%Y-%m-%d").to_string();

    // Seed + forward events: shared with `forecast_dto` (single source of event mapping).
    let seed = projection_seed(pool, today_naive).await?;
    let horizon_end = forecast_horizon_end(pool, today_naive).await?;
    let all_events = load_forecast_events(pool, today_naive, horizon_end).await?;

    // `balance` is the projected end-of-current-month figure (the method's hero, spec 003 US8),
    // not the raw current account sum.
    let fc = forecast::project(seed, today_naive, &all_events, horizon_end);
    let projected_balance = fc
        .month_end
        .iter()
        .find(|m| m.year == today_naive.year() && m.month == today_naive.month())
        .map(|m| m.balance_cents)
        .or_else(|| fc.daily.last().map(|p| p.balance_cents))
        .unwrap_or(seed);

    // Teto do diário exibido no tile "Diário de hoje" (`de R$X`): orçamento explícito ativo, senão
    // o Diário médio do mês anterior — mesma fonte do driver de projeção do forecast (consistência).
    let daily_budget = effective_daily_ceiling(pool, today_naive).await?;

    // Diário de HOJE como MAGNITUDE positiva (o card faz `teto - gasto` e `gasto/teto`).
    // - Sinal: por convenção, `amount` é gravado como magnitude positiva (import faz `.abs()`,
    //   `create_transaction` exige `> 0`); o sinal vem do `type`. `ABS()` é defesa-em-profundidade,
    //   espelhando o `amount.abs()` do forecast — robusto caso algum writer grave com sinal.
    // - Fonte única (sem double-count, o achado real): se há transação Diário no dia, ela vence;
    //   o check-in (`daily_checkin`, sem writer em produção hoje) só preenche dias SEM transação.
    //   Invariante: um dia nunca contabiliza check-in E transação Diário ao mesmo tempo (mesmo
    //   dinheiro, Régua 1). Mesma regra no crédito abaixo e no forecast (`load_cashflow_events`).
    let daily_spend: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM \"transaction\" \
                                 WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                                   AND (payment_method IS NULL OR payment_method <> 'credit')) \
                     THEN ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                                        WHERE type='expense' AND is_fixed=0 AND is_projection=0 AND date = ?1 \
                                          AND (payment_method IS NULL OR payment_method <> 'credit')), 0)) \
                     ELSE COALESCE((SELECT SUM(daily_spend) FROM daily_checkin WHERE date = ?1), 0) \
                END",
    )
    .bind(&today)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query daily spend: {e}"))?;

    // Crédito no mês (Régua 2) como MAGNITUDE positiva, mesma regra do Diário: `ABS` por
    // defesa-em-profundidade (amount é positivo por convenção) e fonte única — se há transação de
    // crédito no mês, ela vence; o check-in só preenche meses sem transação de crédito.
    // Janela do MÊS CORRENTE [month_start, month_end]: o método pré-lança o ano inteiro de faturas
    // na planilha; sem o limite superior, crédito datado meses à frente inflava o tile do mês (e o
    // EXISTS sem teto suprimia o fallback de check-in). Escopo análogo ao `date = hoje` do Diário.
    let month_start = format!("{}-01", today_naive.format("%Y-%m"));
    let month_end = forecast::last_day_of_month(today_naive.year(), today_naive.month())
        .format("%Y-%m-%d")
        .to_string();
    let credit_spend: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM \"transaction\" \
                                 WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                                   AND date >= ?1 AND date <= ?2) \
                     THEN ABS(COALESCE((SELECT SUM(amount) FROM \"transaction\" \
                                        WHERE type='expense' AND payment_method='credit' AND is_projection=0 \
                                          AND date >= ?1 AND date <= ?2), 0)) \
                     ELSE COALESCE((SELECT SUM(credit_spend) FROM daily_checkin \
                                    WHERE date >= ?1 AND date <= ?2), 0) \
                END",
    )
    .bind(&month_start)
    .bind(&month_end)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query credit spend: {e}"))?;

    // Há rastreio de crédito? (cartão configurado ou algum gasto de crédito). Sem isso a UI mostra
    // "—" no tile de crédito, em vez de um R$0 estrutural enganoso.
    let has_credit: (i64,) = sqlx::query_as(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM account WHERE type='credit_card') \
                  OR EXISTS(SELECT 1 FROM \"transaction\" WHERE payment_method='credit') \
                  OR COALESCE((SELECT SUM(credit_spend) FROM daily_checkin), 0) > 0 \
                THEN 1 ELSE 0 END",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("query has_credit: {e}"))?;

    // Reserva em MESES de custo de vida (método): saldo das contas de reserva ÷ custo de vida mensal.
    // Custo de vida mensal = mediana das saídas dos últimos meses completos (realized_monthly_baseline
    // = fixas + diário + cartão). A tabela `reserve.current_months` não tem writer de produção (só seed
    // de teste), então derivamos ao vivo dos dados importados — espelha os R$ que o PocketsCard mostra.
    // `trend` permanece da tabela/snapshot (default 'flat' enquanto não há histórico).
    let reserve_balance: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(balance), 0) FROM account WHERE liquidity = 'reserve'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query reserve balance: {e}"))?;
    let reserve_baseline = realized_monthly_baseline(pool, today_naive).await?;
    let reserve_months = if reserve_baseline > 0 {
        reserve_balance.0 as f64 / reserve_baseline as f64
    } else {
        0.0
    };
    let reserve_trend: (String,) = sqlx::query_as(
        "SELECT COALESCE(trend, 'flat') FROM reserve ORDER BY last_calculated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve trend: {e}"))?
    .unwrap_or(("flat".to_string(),));

    // Transações já realizadas: por DATA (≤ hoje), não pelo `is_projection` congelado (stale
    // quando o dono não re-importa por dias — auditoria de robustez a edições).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE date <= ?1")
        .bind(&today)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("query: {e}"))?;

    Ok(DashboardSummary {
        balance: projected_balance,
        daily_budget,
        daily_spend_today: daily_spend.0,
        credit_spend_month: credit_spend.0,
        has_credit: has_credit.0 != 0,
        reserve_months,
        reserve_trend: reserve_trend.0,
        transaction_count: count.0,
    })
}

// --- Pockets & liquidity (spec 007) ---

#[derive(serde::Serialize)]
pub struct PocketAccount {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub liquidity: Option<String>,
    pub balance: i64,
    pub institution: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Pockets {
    pub liquid_cents: i64,
    pub reserve_cents: i64,
    pub restricted_cents: i64,
    pub illiquid_cents: i64,
    /// liquid + reserve + illiquid; restricted (vale) is tracked apart and the
    /// credit-card liability belongs to the invoice slice.
    pub net_worth_cents: i64,
    pub accounts: Vec<PocketAccount>,
}

/// Deterministic liquidity class per account type (spec 007 contract).
fn liquidity_for_type(account_type: &str) -> Option<&'static str> {
    match account_type {
        "bank" | "wallet" | "business" => Some("liquid"),
        "savings" => Some("reserve"),
        "meal_voucher" => Some("restricted"),
        "pension" | "fgts" => Some("illiquid"),
        _ => None, // credit_card: liability, not a pocket
    }
}

/// Pure aggregation over the account list (functional core, unit-tested).
fn aggregate_pockets(accounts: Vec<PocketAccount>) -> Pockets {
    let sum = |class: &str| -> i64 {
        accounts
            .iter()
            .filter(|a| a.liquidity.as_deref() == Some(class))
            .map(|a| a.balance)
            .sum()
    };
    let (liquid, reserve, restricted, illiquid) = (
        sum("liquid"),
        sum("reserve"),
        sum("restricted"),
        sum("illiquid"),
    );
    Pockets {
        liquid_cents: liquid,
        reserve_cents: reserve,
        restricted_cents: restricted,
        illiquid_cents: illiquid,
        net_worth_cents: liquid + reserve + illiquid,
        accounts,
    }
}

#[tauri::command]
pub async fn get_pockets(pool: State<'_, SqlitePool>) -> Result<Pockets, String> {
    pockets(pool.inner()).await
}

async fn pockets(pool: &SqlitePool) -> Result<Pockets, String> {
    type PocketRow = (String, String, String, Option<String>, i64, Option<String>);
    let rows: Vec<PocketRow> = sqlx::query_as(
        "SELECT id, name, type, liquidity, balance, institution FROM account \
         WHERE type != 'credit_card' ORDER BY created_at, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    Ok(aggregate_pockets(
        rows.into_iter()
            .map(
                |(id, name, t, liquidity, balance, institution)| PocketAccount {
                    id,
                    name,
                    r#type: t,
                    liquidity,
                    balance,
                    institution,
                },
            )
            .collect(),
    ))
}

#[tauri::command]
pub async fn create_account(
    pool: State<'_, SqlitePool>,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    create_account_inner(pool.inner(), name, account_type, balance_cents, institution).await
}

async fn create_account_inner(
    pool: &SqlitePool,
    name: String,
    account_type: String,
    balance_cents: i64,
    institution: Option<String>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nome obrigatório".into());
    }
    let liquidity = liquidity_for_type(&account_type)
        .ok_or_else(|| format!("tipo inválido: {account_type}"))?;

    // Pockets exist before any sheet import; ensure the default owner person.
    // Atomic insert-if-empty so concurrent calls cannot both bootstrap an "Eu".
    sqlx::query(
        "INSERT INTO person (id, name) SELECT ?1, 'Eu' WHERE NOT EXISTS (SELECT 1 FROM person)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .map_err(|e| format!("create person: {e}"))?;
    let (owner_id,): (String,) =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, institution, balance, liquidity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&id)
    .bind(name)
    .bind(&account_type)
    .bind(&owner_id)
    .bind(&institution)
    .bind(balance_cents)
    .bind(liquidity)
    .execute(pool)
    .await
    .map_err(|e| format!("create account: {e}"))?;

    Ok(id)
}

/// Tag anexada a um lançamento (para os chips do Livro-razão).
#[derive(serde::Serialize, sqlx::FromRow, Clone)]
pub struct TagOnRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TransactionRow {
    pub id: String,
    pub r#type: String,
    pub amount: i64,
    pub description: String,
    pub date: String,
    pub payment_method: String,
    pub is_projection: bool,
    /// Despesa fixa (veio da coluna Saída da planilha) vs variável (Diário). Distingue Saída × Diário.
    pub is_fixed: bool,
    /// Titulares distintos das parcelas (multi-titular). Vazio = sem split por pessoa.
    pub owners: Vec<String>,
    /// Tags anexadas (diagnóstico). Mostradas como chips no Livro-razão.
    pub tags: Vec<TagOnRow>,
    /// Proveniência: "projetado" (previsto), "importado" (da planilha) ou "manual" (do app).
    pub provenance: String,
}

#[tauri::command]
pub async fn get_recent_transactions(
    pool: State<'_, SqlitePool>,
    limit: i64,
) -> Result<Vec<TransactionRow>, String> {
    recent_transactions(pool.inner(), limit).await
}

#[derive(sqlx::FromRow)]
struct RecentRow {
    id: String,
    r#type: String,
    amount: i64,
    description: String,
    date: String,
    payment_method: String,
    is_projection: i64,
    is_fixed: i64,
    /// Titulares distintos, juntados por '|' no SQL (vazio = sem split por pessoa).
    owners: String,
    /// `source_amount` é NULL quando nunca veio da planilha (lançamento manual no app).
    has_source: i64,
}

async fn recent_transactions(pool: &SqlitePool, limit: i64) -> Result<Vec<TransactionRow>, String> {
    // Titulares vêm de um subquery agregado (GROUP_CONCAT com separador '|') — sem N+1.
    let rows: Vec<RecentRow> = sqlx::query_as(
        "SELECT t.id, t.type, t.amount, COALESCE(t.description,'') AS description, t.date, \
                COALESCE(t.payment_method,'') AS payment_method, t.is_projection, t.is_fixed, \
                COALESCE((SELECT GROUP_CONCAT(name, '|') FROM \
                    (SELECT DISTINCT p.name FROM split s \
                     JOIN person p ON p.id = s.owner_person_id \
                     WHERE s.transaction_id = t.id ORDER BY p.name COLLATE NOCASE)), '') AS owners, \
                (t.source_amount IS NOT NULL) AS has_source \
         FROM \"transaction\" t ORDER BY t.date DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query: {e}"))?;

    // Tags só das transações EFETIVAMENTE retornadas acima (busca pelos IDs reais). Uma janela
    // `ORDER BY date DESC LIMIT ?1` separada não garante o MESMO conjunto quando há empate de data
    // na borda do LIMIT (desempate arbitrário do SQLite), e linhas visíveis perderiam suas tags.
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let tag_rows: Vec<(String, String, String, String, Option<String>)> = if ids.is_empty() {
        Vec::new()
    } else {
        // Placeholders posicionais (só `?`, sem dado interpolado) + binds — seguro com AssertSqlSafe.
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT tt.transaction_id, t.id, t.name, t.color, t.emoji \
             FROM transaction_tag tt JOIN tag t ON t.id = tt.tag_id \
             WHERE tt.transaction_id IN ({placeholders}) \
             ORDER BY t.is_special DESC, t.name COLLATE NOCASE"
        );
        let mut q = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            sqlx::AssertSqlSafe(sql),
        );
        for id in &ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .map_err(|e| format!("tag query: {e}"))?
    };
    let mut tags_by_txn: std::collections::HashMap<String, Vec<TagOnRow>> =
        std::collections::HashMap::new();
    for (txn_id, id, name, color, emoji) in tag_rows {
        tags_by_txn.entry(txn_id).or_default().push(TagOnRow {
            id,
            name,
            color,
            emoji,
        });
    }

    Ok(rows
        .into_iter()
        .map(|r| TransactionRow {
            tags: tags_by_txn.get(&r.id).cloned().unwrap_or_default(),
            id: r.id,
            r#type: r.r#type,
            amount: r.amount,
            description: r.description,
            date: r.date,
            payment_method: r.payment_method,
            is_projection: r.is_projection != 0,
            is_fixed: r.is_fixed != 0,
            owners: if r.owners.is_empty() {
                Vec::new()
            } else {
                // Ordena no Rust (não depende da ordem do GROUP_CONCAT, que não é contratual).
                let mut o: Vec<String> = r.owners.split('|').map(str::to_owned).collect();
                o.sort_by_key(|s| s.to_lowercase());
                o
            },
            provenance: if r.is_projection != 0 {
                "projetado".to_string()
            } else if r.has_source != 0 {
                "importado".to_string()
            } else {
                "manual".to_string()
            },
        })
        .collect())
}

/// Repetição opcional de um lançamento ("Repetir": frequência + nº de ocorrências).
#[derive(serde::Deserialize)]
pub struct RecurrenceInput {
    pub frequency: String,
    pub repetitions: usize,
}

/// Cria um lançamento manual (caminho de escrita do app). `amount_cents` é magnitude positiva;
/// a direção vem de `txn_type` ('income'/'expense'). Com `recurrence`, gera a série projetada
/// inteira em vez de um único realizado. As `tag_ids` são anexadas a toda linha criada.
/// Retorna o id do lançamento (ou da série, quando recorrente).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_transaction(
    pool: State<'_, SqlitePool>,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    date: String,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: Vec<String>,
    recurrence: Option<RecurrenceInput>,
) -> Result<String, String> {
    create_transaction_inner(
        pool.inner(),
        &txn_type,
        amount_cents,
        description,
        &date,
        payment_method,
        is_fixed,
        &tag_ids,
        recurrence,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn create_transaction_inner(
    pool: &SqlitePool,
    txn_type: &str,
    amount_cents: i64,
    description: Option<String>,
    date: &str,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: &[String],
    recurrence: Option<RecurrenceInput>,
) -> Result<String, String> {
    if !matches!(txn_type, "income" | "expense") {
        return Err(format!("tipo inválido: {txn_type}"));
    }
    if amount_cents <= 0 {
        return Err("valor deve ser positivo (magnitude)".into());
    }
    let start = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| format!("data: {e}"))?;

    // Caminho recorrente: delega à série projetada e anexa as tags a cada ocorrência.
    if let Some(rec) = recurrence {
        let freq =
            crate::recurrence::Frequency::parse(&rec.frequency).ok_or("frequência inválida")?;
        let template = crate::recurrence::RecurringTemplate {
            txn_type: txn_type.to_string(),
            amount: amount_cents,
            description,
            start,
            payment_method,
            is_fixed,
        };
        let rec_id =
            crate::recurrence::create_recurring_series(pool, &template, freq, rec.repetitions)
                .await?;
        if !tag_ids.is_empty() {
            for i in 0..rec.repetitions {
                crate::tags::set_transaction_tags(pool, &format!("{rec_id}:{i}"), tag_ids).await?;
            }
        }
        return Ok(rec_id);
    }

    // Lançamento único. Data FUTURA → projeção (igual ao import: `classify_row`); hoje/passado →
    // realizado. Marcar um futuro como realizado distorceria o "já aconteceu" do dashboard.
    let is_projection = start > chrono::Local::now().date_naive();
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO \"transaction\" (id, type, amount, description, date, payment_method, is_fixed, is_projection, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
    )
    .bind(&id)
    .bind(txn_type)
    .bind(amount_cents)
    .bind(&description)
    .bind(date)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(is_projection as i64)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("insert transaction: {e}"))?;

    if !tag_ids.is_empty() {
        crate::tags::set_transaction_tags(pool, &id, tag_ids).await?;
    }
    Ok(id)
}

/// Apaga um lançamento manual (não importado) pelo id. O guarda `source_amount IS NULL` impede
/// remover histórico vindo da planilha pelo app — esses precisam de um fluxo próprio.
#[tauri::command]
pub async fn delete_transaction_cmd(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let affected =
        sqlx::query(r#"DELETE FROM "transaction" WHERE id = ?1 AND source_amount IS NULL"#)
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| format!("delete: {e}"))?
            .rows_affected();
    if affected == 0 {
        return Err(
            "lançamento não encontrado ou importado da planilha (não pode ser apagado pelo app)"
                .into(),
        );
    }
    Ok(())
}

/// Edita um lançamento manual (valor, descrição, método, fixo, data) pelo id. Mesmo guarda de
/// `delete_transaction_cmd`: importados da planilha não são editáveis pelo app.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_transaction_cmd(
    pool: State<'_, SqlitePool>,
    id: String,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    payment_method: Option<String>,
    is_fixed: bool,
    date: String,
) -> Result<(), String> {
    // `type` precisa ser atualizável: trocar entrada↔saída no form muda renda↔despesa, e sem isto
    // o sinal do lançamento no forecast ficaria errado. Mesmo conjunto válido do create.
    if !matches!(txn_type.as_str(), "income" | "expense") {
        return Err(format!("tipo inválido: {txn_type}"));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        r#"UPDATE "transaction"
           SET type = ?2, amount = ?3, description = ?4, payment_method = ?5,
               is_fixed = ?6, date = ?7, updated_at = ?8
           WHERE id = ?1 AND source_amount IS NULL"#,
    )
    .bind(&id)
    .bind(&txn_type)
    .bind(amount_cents)
    .bind(&description)
    .bind(&payment_method)
    .bind(is_fixed as i64)
    .bind(&date)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("update: {e}"))?
    .rows_affected();
    if affected == 0 {
        return Err(
            "lançamento não encontrado ou importado da planilha (não pode ser editado pelo app)"
                .into(),
        );
    }
    Ok(())
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

/// Lê as transações e as converte para candidatas de write-back da grade diária do `year`
/// (magnitude positiva; a coluna vem do tipo). DECISÃO de método (a questão em aberto planilha↔
/// modelo): o CARTÃO **colapsa para um lump em Saída no VENCIMENTO** — formato canônico que o dono
/// edita à mão (a planilha crua não tem coluna Cartão). Por isso o crédito é carregado pela janela
/// de VENCIMENTO, não da compra: uma compra de DEZ/ano-1 vence em JAN/ano e tem que entrar no ano.
/// Sem cartão configurado, o crédito do ano cai na Saída da própria data. `transfer` (Economia) NÃO
/// entra aqui — vai para a aba `Economia` (ver `build_economia_plan`).
async fn load_write_back_txns(pool: &SqlitePool, year: i32) -> Result<Vec<WriteBackTxn>, String> {
    // 1) Entrada + Saída/Diário (expense não-crédito) do ano, cada um na sua data.
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT type, date, amount, is_fixed FROM \"transaction\" \
         WHERE substr(date, 1, 4) = ?1 \
           AND NOT (type='expense' AND payment_method='credit')",
    )
    .bind(format!("{year:04}"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query txns: {e}"))?;

    let mut out = Vec::new();
    for (t, date, amount, is_fixed) in rows {
        let mag = amount.abs();
        match t.as_str() {
            "income" => out.push(WriteBackTxn {
                date,
                kind: import::RowKind::Entrada,
                amount_cents: mag,
            }),
            "expense" => out.push(WriteBackTxn {
                date,
                kind: if is_fixed != 0 {
                    import::RowKind::Saida
                } else {
                    import::RowKind::Diario
                },
                amount_cents: mag,
            }),
            _ => {} // transfer (Economia) → aba Economia
        }
    }

    // 2) Cartão → lump no vencimento. Com cartão configurado, junta as compras cujo VENCIMENTO cai
    //    no ano-alvo (janela DEZ/ano-1 .. DEZ/ano, pois a fatura vence ~1 mês após a compra).
    // Dias do cartão são NULL-áveis: FILTRA no SQL (não LIMIT 1 cego) — senão, se o 1º cartão
    // viesse sem ciclo mas existisse outro completo, o write-back ignoraria o ciclo válido e
    // lançaria crédito pela data da compra. Ordem determinística para escolher sempre o mesmo.
    let card: Option<(i64, i64)> = sqlx::query_as(
        "SELECT closing_day, due_day FROM account \
         WHERE type='credit_card' AND closing_day IS NOT NULL AND due_day IS NOT NULL \
         ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query card: {e}"))?;

    match card {
        Some((closing, due)) => {
            let credit: Vec<(String, i64)> = sqlx::query_as(
                "SELECT date, amount FROM \"transaction\" \
                 WHERE type='expense' AND payment_method='credit' AND date >= ?1 AND date <= ?2",
            )
            .bind(format!("{:04}-12-01", year - 1))
            .bind(format!("{year:04}-12-31"))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("query credit: {e}"))?;

            let mut by_due: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (date, amount) in credit {
                if let Ok(d) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                    let due_date = forecast::cycle_due_date(d, closing as u32, due as u32);
                    if due_date.year() == year {
                        *by_due
                            .entry(due_date.format("%Y-%m-%d").to_string())
                            .or_insert(0) += amount.abs();
                    }
                }
            }
            for (due_date, cents) in by_due {
                out.push(WriteBackTxn {
                    date: due_date,
                    kind: import::RowKind::Saida,
                    amount_cents: cents,
                });
            }
        }
        None => {
            // Sem cartão: não há ciclo para colapsar — crédito do ano cai na Saída da própria data.
            let credit: Vec<(String, i64)> = sqlx::query_as(
                "SELECT date, amount FROM \"transaction\" \
                 WHERE type='expense' AND payment_method='credit' AND substr(date,1,4) = ?1",
            )
            .bind(format!("{year:04}"))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("query credit nocard: {e}"))?;
            for (date, amount) in credit {
                out.push(WriteBackTxn {
                    date,
                    kind: import::RowKind::Saida,
                    amount_cents: amount.abs(),
                });
            }
        }
    }
    Ok(out)
}

/// Núcleo compartilhado por `preview_write_back` (read-only) e `apply_write_back` (escreve): lê a
/// aba, resolve layout+mappings, carrega as transações do ano e planeja o diff célula a célula.
/// Devolve o `SheetsClient` autenticado (para o apply reusar na escrita) + o plano.
async fn build_write_back_plan(
    app_dir: &std::path::Path,
    pool: &SqlitePool,
    spreadsheet_id: &str,
    sheet_name: &str,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<(SheetsClient, Vec<CellWrite>), String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let range = quote_sheet(sheet_name);
    let values = client.get_sheet_values(spreadsheet_id, &range).await?;

    let layout = import::get_layout_for_sheet(pool, sheet_name)
        .await?
        .ok_or("layout não detectado para esta aba — rode a detecção primeiro")?;
    let mappings = import::get_active_mappings_for_sheet(pool, sheet_name).await?;
    // Ano não detectado → nada a planejar (nunca assume 2025).
    let Some(year) = layout.year else {
        return Ok((client, Vec::new()));
    };
    let txns = load_write_back_txns(pool, year).await?;
    let plan = write_back::plan_write_back(&values.values, &layout, &mappings, &txns);
    Ok((client, plan))
}

/// Pré-visualização do write-back: lê a planilha e produz o DIFF (transação → célula) para
/// aprovação. READ-ONLY — não escreve nada, então é seguro mesmo com a flag desligada.
#[tauri::command]
pub async fn preview_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<CellWrite>, String> {
    let (_client, plan) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;
    Ok(plan)
}

/// Estado da flag de write-back (a UI usa para mostrar "desligado" e desabilitar o envio).
#[tauri::command]
pub fn write_back_enabled() -> bool {
    write_back::WRITE_BACK_ENABLED
}

/// Lê uma preferência local (KV). `None` quando a chave nunca foi gravada.
#[tauri::command]
pub async fn get_app_setting(
    pool: State<'_, SqlitePool>,
    key: String,
) -> Result<Option<String>, String> {
    app_setting_get(pool.inner(), &key).await
}

async fn app_setting_get(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_setting WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("get setting: {e}"))?;
    Ok(row.map(|(v,)| v))
}

/// Grava uma preferência local (KV), sobrescrevendo.
#[tauri::command]
pub async fn set_app_setting(
    pool: State<'_, SqlitePool>,
    key: String,
    value: String,
) -> Result<(), String> {
    app_setting_set(pool.inner(), &key, &value).await
}

async fn app_setting_set(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_setting (key, value) VALUES (?1, ?2)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| format!("set setting: {e}"))?;
    Ok(())
}

/// Backup do banco local em `dest_path` (escolhido pelo usuário no save dialog). Usa `VACUUM INTO`,
/// que cria uma cópia CONSISTENTE mesmo com o banco em uso e em modo WAL — diferente de copiar o
/// arquivo `.db` cru, que poderia capturar um estado parcial (WAL não aplicado). Local-first: o dono
/// do dado precisa conseguir levar uma cópia íntegra. Retorna o caminho gravado.
#[tauri::command]
pub async fn backup_database(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    dest_path: String,
) -> Result<String, String> {
    backup_db(pool.inner(), &app_dir.0.join("neko-finance.db"), &dest_path).await
}

async fn backup_db(
    pool: &SqlitePool,
    active_db: &std::path::Path,
    dest_path: &str,
) -> Result<String, String> {
    use std::path::{Path, PathBuf};
    let dest = dest_path.trim();
    if dest.is_empty() {
        return Err("escolha um destino para o backup".into());
    }
    let dest_buf = PathBuf::from(dest);

    // NUNCA fazer backup SOBRE o banco em uso: apagá-lo/escrevê-lo desvincularia o arquivo aberto
    // (Unix) e perderia escritas futuras, ou falharia travado (Windows). Só rejeita quando o destino
    // já existe E é o mesmo arquivo (canonicalize); um destino novo nunca pode ser o banco ativo.
    if let (Ok(d), Ok(a)) = (
        std::fs::canonicalize(&dest_buf),
        std::fs::canonicalize(active_db),
    ) && d == a
    {
        return Err("escolha um destino diferente do banco em uso.".into());
    }

    // Grava num TEMP no MESMO diretório do destino e só então faz `rename` (atômico no mesmo
    // filesystem): o backup ANTERIOR só é substituído quando o novo está completo. Se o VACUUM
    // falhar, o destino antigo permanece intacto. (`VACUUM INTO` recusa arquivo já existente, daí o
    // temp único; e roda como SQL BRUTO via `raw_sql` — um prepared statement o silenciaria.)
    let parent = dest_buf.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".neko-backup-{}.tmp", uuid::Uuid::new_v4()));
    let tmp_sql = tmp.to_string_lossy().replace('\'', "''");
    let stmt = format!("VACUUM INTO '{tmp_sql}'");
    if let Err(e) = sqlx::raw_sql(sqlx::AssertSqlSafe(stmt)).execute(pool).await {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("backup: {e}"));
    }
    std::fs::rename(&tmp, &dest_buf).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("finalizar backup: {e}")
    })?;
    Ok(dest.to_string())
}

/// Aplica o write-back: escreve as células DIVERGENTES de volta na aba. Trava-mestra: enquanto
/// `WRITE_BACK_ENABLED` estiver desligado, falha cedo e NÃO escreve nada. A UI já obteve o diff via
/// `preview_write_back` e o humano aprovou; aqui só replanejamos (a planilha pode ter mudado) e
/// escrevemos as células que ainda diferem. Retorna quantas células foram escritas.
#[tauri::command]
pub async fn apply_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    sheet_name: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    write_back::ensure_write_back_enabled()?;

    let (client, plan) = build_write_back_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        &sheet_name,
        &client_id,
        client_secret,
    )
    .await?;

    // Só as células que MUDARAM; range com nome da aba ('2026'!E3); valor numérico em reais.
    let updates: Vec<(String, f64)> = plan
        .iter()
        .filter(|c| c.changed)
        .map(|c| {
            (
                format!("{}!{}", quote_sheet(&sheet_name), c.a1),
                c.value_cents as f64 / 100.0,
            )
        })
        .collect();

    client.batch_update_values(&spreadsheet_id, &updates).await
}

/// Economia REGISTRADA por mês (1..=12) do ano: soma dos transfers→reserva/ilíquido. É o numerador
/// do Economizado% do método e o que vai para a coluna `Economia` da aba homônima no write-back.
async fn load_economia_by_month(pool: &SqlitePool, year: i32) -> Result<[i64; 12], String> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT substr(t.date, 6, 2), COALESCE(SUM(ABS(t.amount)), 0) FROM \"transaction\" t \
         LEFT JOIN account a ON a.id = t.to_account_id \
         WHERE substr(t.date, 1, 4) = ?1 AND t.type = 'transfer' \
           AND a.liquidity IN ('reserve','illiquid') \
         GROUP BY substr(t.date, 6, 2)",
    )
    .bind(format!("{year:04}"))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("query economia: {e}"))?;
    let mut by = [0i64; 12];
    for (mm, cents) in rows {
        if let Ok(m) = mm.parse::<usize>()
            && (1..=12).contains(&m)
        {
            by[m - 1] = cents;
        }
    }
    Ok(by)
}

/// Núcleo compartilhado do write-back da Economia (aba `Economia`, separada da grade diária).
async fn build_economia_plan(
    app_dir: &std::path::Path,
    pool: &SqlitePool,
    spreadsheet_id: &str,
    year: i32,
    client_id: &str,
    client_secret: Option<String>,
) -> Result<(SheetsClient, Vec<CellWrite>), String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(app_dir, client_id, client_secret.as_deref())
            .await?;
    let client = SheetsClient::new(token);
    let values = client
        .get_sheet_values(spreadsheet_id, "'Economia'")
        .await?;
    let by_month = load_economia_by_month(pool, year).await?;
    let plan = write_back::plan_economia_write_back(&values.values, year, &by_month);
    Ok((client, plan))
}

/// Preview READ-ONLY do write-back da Economia (transfers→reserva → coluna `Economia` por mês).
#[tauri::command]
pub async fn preview_economia_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    year: i32,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<CellWrite>, String> {
    let (_client, plan) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;
    Ok(plan)
}

/// Aplica o write-back da Economia. Atrás da MESMA flag `WRITE_BACK_ENABLED`. Retorna nº de células.
#[tauri::command]
pub async fn apply_economia_write_back(
    app_dir: State<'_, AppDataDir>,
    pool: State<'_, SqlitePool>,
    spreadsheet_id: String,
    year: i32,
    client_id: String,
    client_secret: Option<String>,
) -> Result<usize, String> {
    write_back::ensure_write_back_enabled()?;
    let (client, plan) = build_economia_plan(
        &app_dir.0,
        pool.inner(),
        &spreadsheet_id,
        year,
        &client_id,
        client_secret,
    )
    .await?;
    let updates: Vec<(String, f64)> = plan
        .iter()
        .filter(|c| c.changed)
        .map(|c| (format!("'Economia'!{}", c.a1), c.value_cents as f64 / 100.0))
        .collect();
    client.batch_update_values(&spreadsheet_id, &updates).await
}

/// Conta de RESERVA destino da Economia. Usa a primeira `liquidity='reserve'`; se não houver, cria
/// uma "Reserva" padrão (savings/reserve) do 1º titular — assim a Economia importada tem para onde ir.
async fn ensure_reserve_account(pool: &SqlitePool) -> Result<String, String> {
    if let Some((id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM account WHERE liquidity='reserve' ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("query reserve: {e}"))?
    {
        return Ok(id);
    }
    let owner: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("query person: {e}"))?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
         VALUES (?1, 'Reserva', 'savings', ?2, 0, 'reserve')",
    )
    .bind(&id)
    .bind(owner.map(|(p,)| p))
    .execute(pool)
    .await
    .map_err(|e| format!("create reserve: {e}"))?;
    Ok(id)
}

async fn store_economia_entries(
    pool: &SqlitePool,
    entries: &[(i32, u32, i64)],
) -> Result<usize, String> {
    let today = chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339();

    // A conta de reserva é pré-requisito das linhas com economia > 0 — resolvida ANTES da transação
    // (assim um import só de zeros/deleções não cria uma reserva à toa). Os upserts/deletes correm
    // numa ÚNICA transação: uma falha no meio deixaria o Economizado%/ColchaoCard parcialmente errado.
    let needs_reserve = entries.iter().any(|(_, _, cents)| *cents > 0);
    let reserve_id = if needs_reserve {
        Some(ensure_reserve_account(pool).await?)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(|e| format!("begin: {e}"))?;
    let mut count = 0usize;

    for (year, month, cents) in entries {
        let last = forecast::last_day_of_month(*year, *month);
        let date = last.format("%Y-%m-%d").to_string();
        let id = format!("economia:{year:04}-{month:02}");

        if *cents > 0 {
            let Some(reserve) = reserve_id.as_ref() else {
                return Err("conta de reserva não resolvida para a Economia".into());
            };
            let is_projection = (last > today) as i64;
            sqlx::query(
                "INSERT INTO \"transaction\" (id, type, amount, description, date, to_account_id, is_projection, created_at, updated_at) \
                 VALUES (?1, 'transfer', ?2, 'Economia (importada da aba Economia)', ?3, ?4, ?5, ?6, ?6) \
                 ON CONFLICT(id) DO UPDATE SET amount=excluded.amount, date=excluded.date, \
                   is_projection=excluded.is_projection, updated_at=excluded.updated_at",
            )
            .bind(&id)
            .bind(cents)
            .bind(&date)
            .bind(reserve)
            .bind(is_projection)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("upsert economia: {e}"))?;
        } else {
            sqlx::query("DELETE FROM \"transaction\" WHERE id = ?1")
                .bind(&id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("delete economia: {e}"))?;
        }
        count += 1;
    }

    tx.commit().await.map_err(|e| format!("commit: {e}"))?;
    Ok(count)
}

/// Importa a aba `Economia` da planilha → transações `transfer→reserva` (a representação da Economia
/// registrada do método). Id determinístico `economia:{ano}-{mês}` ⇒ re-import ATUALIZA, não duplica.
/// É o que faltava: sem isso, o Economizado%/ColchaoCard e o write-back da Economia ficam zerados.
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

#[derive(serde::Serialize)]
pub struct UserSpreadsheet {
    pub id: String,
    pub name: String,
    pub modified_time: String,
}

#[tauri::command]
pub async fn list_user_spreadsheets(
    app_dir: State<'_, AppDataDir>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<Vec<UserSpreadsheet>, String> {
    let client_secret = oauth::pkce::resolve_client_secret(client_secret);
    let token =
        oauth::token_store::ensure_valid_token(&app_dir.0, &client_id, client_secret.as_deref())
            .await?;

    let url = "https://www.googleapis.com/drive/v3/files?q=mimeType%3D'application%2Fvnd.google-apps.spreadsheet'&fields=files(id,name,modifiedTime)&orderBy=modifiedTime%20desc&pageSize=50";

    let resp = crate::http::send_with_retry(
        crate::http::client()
            .get(url)
            .bearer_auth(&token.access_token),
    )
    .await
    .map_err(|e| format!("drive request: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Drive API error: {body}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    let files = json["files"].as_array().cloned().unwrap_or_default();

    Ok(files
        .into_iter()
        .filter_map(|f| {
            let id = f["id"].as_str()?.to_string();
            let name = f["name"].as_str().unwrap_or("").to_string();
            let modified = f["modifiedTime"].as_str().unwrap_or("").to_string();
            Some(UserSpreadsheet {
                id,
                name,
                modified_time: modified,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_exposes_version_and_db_path() {
        let info = app_info_for_dir(std::path::Path::new("/tmp/neko-test"));
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.db_path.ends_with("neko-finance.db"));
        assert!(info.db_path.starts_with("/tmp/neko-test"));
    }

    #[test]
    fn test_parse_number_via_import() {
        use google_sheets::import::compute_checksum;
        let rows = vec![google_sheets::import::ImportedRow {
            date: "2025-01-01".into(),
            amount: 10000,
            description: "Test".into(),
            is_projection: false,
            kind: google_sheets::import::RowKind::Entrada,
        }];
        let checksum = compute_checksum(&rows);
        assert!(!checksum.is_empty());
    }

    // Spec 010 slice 0: floats do calamine chegam ao parse_number sem ambiguidade de
    // separador — regressão do bug de 100× (12.34 → R$ 1.234,00).
    #[test]
    fn xlsx_float_cells_parse_to_correct_cents() {
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(12.34)),
            "12.3400"
        );
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::Float(1234.56)),
            "1234.5600"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Int(1370)), "1370");
        assert_eq!(
            xlsx_cell_to_string(&calamine::Data::String(" Entrada ".into())),
            "Entrada"
        );
        assert_eq!(xlsx_cell_to_string(&calamine::Data::Empty), "");

        use google_sheets::import::parse_number;
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(12.34))),
            1234
        );
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(5678.1234))),
            567812
        );
        // float que parece milhar (3 dígitos após o ponto) — o {:.4} blinda o caso.
        assert_eq!(
            parse_number(&xlsx_cell_to_string(&calamine::Data::Float(123.456))),
            12346
        );
    }

    #[test]
    fn local_xlsx_path_validation_rejects_non_xlsx() {
        let path = std::env::temp_dir().join(format!("neko-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not a workbook").unwrap();

        let err = validate_local_xlsx_path(path.to_str().unwrap()).unwrap_err();

        std::fs::remove_file(&path).unwrap();
        assert!(err.contains(".xlsx"));
    }

    #[test]
    fn local_xlsx_path_validation_accepts_regular_xlsx_file() {
        let path = std::env::temp_dir().join(format!("neko-{}.xlsx", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"placeholder").unwrap();

        let got = validate_local_xlsx_path(path.to_str().unwrap()).unwrap();

        std::fs::remove_file(&path).unwrap();
        assert_eq!(got.extension().and_then(|e| e.to_str()), Some("xlsx"));
    }

    // T7.6 — get_dashboard_summary returns the PROJECTED balance, not the raw account sum.
    #[tokio::test]
    async fn dashboard_balance_is_projected_not_raw_sum() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(&pool)
            .await
            .unwrap();

        // Liquid bank account with R$1000.00 → raw SUM(balance) = 100000.
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind("bank")
        .bind(&pid)
        .bind(100_000i64)
        .execute(&pool)
        .await
        .unwrap();

        // A FUTURE projected fixed expense of R$300.00 later this same month.
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) VALUES (?1,'expense',?2,?3,'debit',1,1)"
        )
        .bind(&tid).bind(30_000i64).bind("2026-03-20")
        .execute(&pool).await.unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();

        // Raw sum is 100000; the projected end-of-March is 100000 − 30000 = 70000.
        assert_eq!(summary.balance, 70_000);
    }

    // Bug: tiles do dashboard mostravam R$0 estrutural porque liam só `daily_checkin` (vazia).
    // Agora o Diário de hoje vem das transações realizadas; e `has_credit=false` sem cartão.
    #[tokio::test]
    async fn dashboard_daily_spend_comes_from_transactions_and_credit_flag() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Diário realizado de hoje (expense, is_fixed=0) — sem nenhum check-in.
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await;
        // Despesa de outro dia não conta no "hoje".
        insert_realized(&pool, "expense", 9_999, "2026-06-12").await;

        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "Diário de hoje vem das transações, não R$0 estrutural"
        );
        assert!(!s.has_credit, "sem cartão nem crédito → has_credit false");
    }

    // Defesa-em-profundidade (review adversarial): `amount` é magnitude positiva por convenção, mas
    // `daily_spend_today` usa ABS espelhando o forecast — então um amount negativo (não-canônico)
    // ainda rende magnitude positiva, jamais um "gasto negativo" que quebraria `teto - gasto`.
    #[tokio::test]
    async fn dashboard_daily_spend_is_positive_magnitude_even_if_amount_negative() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized(&pool, "expense", -4_271, "2026-06-13").await; // sinal não-canônico
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "ABS garante magnitude positiva mesmo com amount negativo"
        );
    }

    // Regressão (review adversarial): um dia com check-in E transação Diário não pode somar os dois
    // (mesmo dinheiro, Régua 1). A transação realizada vence; o check-in só preenche dias sem ela.
    #[tokio::test]
    async fn dashboard_daily_spend_no_double_count_checkin_and_txn() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await; // magnitude positiva (canônico)
        // Check-in no mesmo dia com daily_spend 9_999 — NÃO pode ser somado por cima.
        sqlx::query(
            "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&pid)
        .bind("2026-06-13")
        .bind(9_999i64)
        .execute(&pool)
        .await
        .unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "transação Diário vence; check-in não soma por cima (sem double-count)"
        );
    }

    // Regressão (review adversarial): o gasto realizado de HOJE não pode contar ocorrências
    // PROJETADAS (is_projection=1) — ex.: uma recorrência futura cuja ocorrência cai hoje.
    #[tokio::test]
    async fn dashboard_daily_spend_excludes_projected() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        insert_realized(&pool, "expense", 4_271, "2026-06-13").await; // realizado → conta
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_fixed, is_projection) \
             VALUES (?1,'expense',?2,'2026-06-13',0,1)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(9_999i64)
        .execute(&pool)
        .await
        .unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.daily_spend_today, 4_271,
            "projeção não entra no gasto realizado de hoje"
        );
    }

    // Regressão (review adversarial): o método pré-lança o ano inteiro de faturas na planilha. O
    // tile "Crédito no mês" deve contar SÓ o mês corrente — crédito datado meses à frente não pode
    // inflar o número (era over-count silencioso por falta do limite superior de data).
    #[tokio::test]
    async fn dashboard_credit_month_excludes_future_lumps() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let sql = "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) \
                   VALUES (?1,'expense',?2,?3,'credit',0,?4)";
        // Mês corrente → conta.
        sqlx::query(sql)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(30_000i64)
            .bind("2026-06-05")
            .bind(0i64)
            .execute(&pool)
            .await
            .unwrap();
        // Fatura futura pré-lançada (projeção) → NÃO conta.
        sqlx::query(sql)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(99_999i64)
            .bind("2026-08-10")
            .bind(1i64)
            .execute(&pool)
            .await
            .unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(
            s.credit_spend_month, 30_000,
            "só o crédito do mês corrente; fatura futura pré-lançada fica de fora"
        );
    }

    // Visão anual: agrega as 4 métricas por mês a partir das transações do ano (realizado +
    // projetado), independente do horizonte do forecast.
    #[tokio::test]
    async fn annual_metrics_aggregates_each_month() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        // Março realizado: renda 700, diário 250.
        insert_realized(&pool, "income", 700_000, "2026-03-05").await;
        insert_realized(&pool, "expense", 250_000, "2026-03-10").await;
        // Julho projetado: renda 500.
        insert_projection(&pool, "income", 500_000, "2026-07-05", "", 0).await;

        let a = annual_metrics(&pool, 2026, today).await.unwrap();
        assert_eq!(a.year, 2026);
        assert_eq!(a.months.len(), 12);
        let mar = a.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.income_cents, 700_000);
        assert_eq!(mar.cost_of_living_cents, 250_000); // diário realizado
        assert_eq!(mar.performance_cents, 450_000); // 700 − 250
        let jul = a.months.iter().find(|m| m.month == 7).unwrap();
        assert_eq!(jul.income_cents, 500_000);
        let jan = a.months.iter().find(|m| m.month == 1).unwrap();
        assert_eq!(jan.income_cents, 0); // mês vazio
    }

    // --- 005 get_forecast (TDD) ---

    async fn fixture_pool() -> sqlx::SqlitePool {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_liquid_account(pool: &sqlx::SqlitePool, balance: i64) {
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(pool)
            .await
            .unwrap();
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,'bank',?3,?4)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind(&pid)
        .bind(balance)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn economia_import_zero_removes_stale_month() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;

        assert_eq!(
            store_economia_entries(&pool, &[(2026, 1, 100_000)])
                .await
                .unwrap(),
            1
        );
        let (stored,): (i64,) =
            sqlx::query_as("SELECT amount FROM \"transaction\" WHERE id='economia:2026-01'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 100_000);

        assert_eq!(
            store_economia_entries(&pool, &[(2026, 1, 0)])
                .await
                .unwrap(),
            1
        );
        let (remaining,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id='economia:2026-01'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    // Regressão (review): store_economia_entries grava upsert+delete numa única transação. Uma
    // chamada com meses mistos (>0 e =0) deve aplicar TODOS atomicamente, criando a reserva 1×.
    #[tokio::test]
    async fn economia_mixed_entries_commit_in_one_transaction() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;

        let n = store_economia_entries(
            &pool,
            &[(2026, 1, 100_000), (2026, 2, 0), (2026, 3, 50_000)],
        )
        .await
        .unwrap();
        assert_eq!(n, 3);

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM \"transaction\" WHERE id LIKE 'economia:2026-%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2, "jan e mar gravados; fev (0) não cria linha");

        let (reserves,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM account WHERE liquidity='reserve'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reserves, 1, "a conta de reserva é criada uma única vez");
    }

    // M2: backup via VACUUM INTO grava um arquivo SQLite válido e não-vazio. Usa uma fonte em
    // ARQUIVO (como o app real), pois VACUUM INTO a partir de `:memory:` não materializa o arquivo.
    // Checa o cabeçalho mágico do SQLite — prova que VACUUM INTO produziu um DB de verdade.
    #[tokio::test]
    async fn backup_database_writes_valid_sqlite_file() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let src = std::env::temp_dir().join(format!("neko-src-{}.db", uuid::Uuid::new_v4()));
        let src_str = src.to_str().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite:{src_str}"))
                    .unwrap()
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        insert_liquid_account(&pool, 123_456).await;

        let dest = std::env::temp_dir().join(format!("neko-backup-{}.db", uuid::Uuid::new_v4()));
        let dest_str = dest.to_str().unwrap();
        let written = backup_db(&pool, &src, dest_str).await.unwrap();
        assert_eq!(written, dest_str);

        let bytes = std::fs::read(&dest).unwrap();
        assert!(
            bytes.starts_with(b"SQLite format 3\0"),
            "o backup é um arquivo SQLite válido"
        );
        assert!(bytes.len() > 1000, "backup não-vazio");

        // Re-backup sobre um destino EXISTENTE: o swap atômico substitui sem deixar .tmp órfão.
        backup_db(&pool, &src, dest_str).await.unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".neko-backup-"))
            .collect();
        assert!(leftovers.is_empty(), "nenhum .tmp órfão após o rename");

        // Destino vazio é rejeitado; o próprio banco ativo é rejeitado (não desvincular o DB em uso).
        assert!(backup_db(&pool, &src, "   ").await.is_err());
        assert!(
            backup_db(&pool, &src, src_str).await.is_err(),
            "backup sobre o banco ativo é recusado"
        );

        drop(pool);
        std::fs::remove_file(&dest).ok();
        std::fs::remove_file(&src).ok();
    }

    async fn insert_projection(
        pool: &sqlx::SqlitePool,
        ttype: &str,
        amount: i64,
        date: &str,
        payment_method: &str,
        is_fixed: i64,
    ) {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, payment_method, is_fixed, is_projection) VALUES (?1,?2,?3,?4,?5,?6,1)"
        )
        .bind(&tid).bind(ttype).bind(amount).bind(date)
        .bind((!payment_method.is_empty()).then_some(payment_method))
        .bind(is_fixed)
        .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn forecast_dto_chains_daily_flows_and_safe_to_spend() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // R$ 1.000,00
        // Future fixed expense R$ 300,00 on the 20th; future income R$ 200,00 on the 25th.
        insert_projection(&pool, "expense", 30_000, "2026-03-20", "debit", 1).await;
        insert_projection(&pool, "income", 20_000, "2026-03-25", "", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.today, "2026-03-10");
        assert_eq!(fc.horizon_end, "2026-03-31");
        // One row per day, today through month-end inclusive.
        assert_eq!(fc.daily.len(), 22);
        assert_eq!(fc.daily[0].date, "2026-03-10");
        assert_eq!(fc.daily[0].balance_cents, 100_000);

        let d20 = fc.daily.iter().find(|d| d.date == "2026-03-20").unwrap();
        assert_eq!(d20.fixed_out_cents, 30_000);
        assert_eq!(d20.income_cents, 0);
        assert_eq!(d20.balance_cents, 70_000);

        let d25 = fc.daily.iter().find(|d| d.date == "2026-03-25").unwrap();
        assert_eq!(d25.income_cents, 20_000);
        assert_eq!(d25.balance_cents, 90_000);

        // Guardrail duplo: tudo aqui é projetado (nenhum realizado no ano) → a régua de poupança
        // ANUAL está inativa, manda o CAIXA: pode gastar = o vale de R$ 700,00. A régua de
        // poupança anual é exercitada no teste `forecast_dual_guardrail_savings_binds_for_owner`.
        assert_eq!(fc.cash_headroom_cents, 70_000);
        assert_eq!(fc.binding_guardrail, "cash");
        assert_eq!(fc.savings_headroom_cents, None);
        assert_eq!(fc.safe_to_spend_today_cents, 70_000);
        // Positive horizon: deepest point reported, but not negative.
        let trough = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(trough.balance_cents, 70_000);

        // Current month-end matches the dashboard hero.
        assert_eq!(fc.month_end.len(), 1);
        assert_eq!(fc.month_end[0].year, 2026);
        assert_eq!(fc.month_end[0].month, 3);
        assert_eq!(fc.month_end[0].balance_cents, 90_000);
    }

    #[tokio::test]
    async fn forecast_dto_reports_negative_deficit_and_zero_safe_to_spend() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 10_000).await; // R$ 100,00
        insert_projection(&pool, "expense", 50_000, "2026-03-15", "debit", 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        let deficit = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(deficit.balance_cents, -40_000);
        assert_eq!(deficit.date, "2026-03-15");
        assert_eq!(fc.safe_to_spend_today_cents, 0);
    }

    // Spec 011: um `transfer` (magnitude positiva) para um bolso de POUPANÇA (reserve) vira
    // Economia — sai do saldo de gasto E conta no Economizado. Vale-refeição (restricted) NÃO.
    #[tokio::test]
    async fn forecast_dto_transfer_to_reserve_is_economia_and_leaves_balance() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // R$ 1.000,00 em caixa

        // Conta de reserva (poupança real).
        let pid: (String,) = sqlx::query_as("SELECT id FROM person LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Reserva','savings',?2,0,'reserve')",
        )
        .bind(&reserve_id)
        .bind(&pid.0)
        .execute(&pool)
        .await
        .unwrap();

        // Transfer FUTURO de R$ 300,00 (magnitude positiva) para a reserva.
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES (?1,'transfer',30_000,'2026-03-20',?2,1)",
        )
        .bind(&tid)
        .bind(&reserve_id)
        .execute(&pool)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Conta como Economia no mês.
        let mar = fc.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(mar.economia_cents, 30_000, "guardar na reserva = Economia");
        // E sai do saldo de gasto (caixa cai de 100k para 70k).
        let d20 = fc.daily.iter().find(|d| d.date == "2026-03-20").unwrap();
        assert_eq!(d20.balance_cents, 70_000);
    }

    // Regressão (review adversarial): a Economia REGISTRADA anual (transfer→reserva) é distinta do
    // net superávit. O ColchaoCard mostrava "R$ 0" fixo; agora vem do DTO. Só transfer→reserva conta
    // — income/expense e transfer→líquido (net-zero entre contas) não.
    #[tokio::test]
    async fn annual_registered_economia_counts_only_reserve_transfers() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await;
        let pid: (String,) = sqlx::query_as("SELECT id FROM person LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reserve_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Reserva','savings',?2,0,'reserve')")
            .bind(&reserve_id).bind(&pid.0).execute(&pool).await.unwrap();
        let liquid2 = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) VALUES (?1,'Conta2','bank',?2,0,'liquid')")
            .bind(&liquid2).bind(&pid.0).execute(&pool).await.unwrap();
        // Mês COMPLETO (março; hoje = junho).
        insert_realized(&pool, "income", 500_000, "2026-03-05").await;
        insert_realized(&pool, "expense", 100_000, "2026-03-10").await;
        let mk_transfer = |amount: i64, date: &'static str, to: String| {
            let p = pool.clone();
            async move {
                sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, to_account_id, is_projection) VALUES (?1,'transfer',?2,?3,?4,0)")
                    .bind(uuid::Uuid::new_v4().to_string()).bind(amount).bind(date).bind(to)
                    .execute(&p).await.unwrap();
            }
        };
        mk_transfer(30_000, "2026-03-20", reserve_id).await; // → reserva = Economia registrada
        mk_transfer(20_000, "2026-03-15", liquid2).await; // → líquido = net-zero, NÃO conta

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();
        assert_eq!(
            fc.annual_savings.registered_economia_cents, 30_000,
            "só transfer→reserva conta como Economia registrada"
        );
    }

    async fn insert_sheet_balance(pool: &sqlx::SqlitePool, sheet: &str, date: &str, cents: i64) {
        sqlx::query(
            "INSERT INTO sheet_daily_balance (sheet_name, date, balance_cents) VALUES (?1,?2,?3)",
        )
        .bind(sheet)
        .bind(date)
        .bind(cents)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_realized(pool: &sqlx::SqlitePool, ttype: &str, amount: i64, date: &str) {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES (?1,?2,?3,?4,0)",
        )
        .bind(&tid).bind(ttype).bind(amount).bind(date)
        .execute(pool).await.unwrap();
    }

    // KV de preferências: grava e lê; chave ausente → None.
    #[tokio::test]
    async fn app_setting_roundtrip() {
        let pool = fixture_pool().await;
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            None
        );
        app_setting_set(&pool, "onboarding_done", "true")
            .await
            .unwrap();
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            Some("true".to_string())
        );
        // Sobrescreve.
        app_setting_set(&pool, "onboarding_done", "false")
            .await
            .unwrap();
        assert_eq!(
            app_setting_get(&pool, "onboarding_done").await.unwrap(),
            Some("false".to_string())
        );
    }

    // Multi-titular: get_recent_transactions traz os titulares distintos das parcelas (sem N+1),
    // ordenados por nome; transação sem split vem com `owners` vazio.
    #[tokio::test]
    async fn recent_transactions_carry_distinct_owners() {
        let pool = fixture_pool().await;
        for (id, name) in [("bruno", "Bruno"), ("ana", "Ana")] {
            sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
                .bind(id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        // Lançamento dividido entre Bruno e Ana.
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES ('t1','expense',-30000,'2026-06-05',0)")
            .execute(&pool).await.unwrap();
        for (sid, owner, amt) in [("s1", "bruno", -20000), ("s2", "ana", -10000)] {
            sqlx::query("INSERT INTO split (id, transaction_id, amount, owner_person_id) VALUES (?1,'t1',?2,?3)")
                .bind(sid).bind(amt).bind(owner)
                .execute(&pool).await.unwrap();
        }
        // Lançamento sem split → sem titulares.
        insert_realized(&pool, "expense", -5000, "2026-06-04").await;

        let rows = recent_transactions(&pool, 10).await.unwrap();
        let split = rows.iter().find(|r| r.id == "t1").unwrap();
        assert_eq!(split.owners, vec!["Ana".to_string(), "Bruno".to_string()]);
        let solo = rows.iter().find(|r| r.id != "t1").unwrap();
        assert!(solo.owners.is_empty(), "sem split → owners vazio");
    }

    // Os chips de tag do Livro-razão: recent_transactions devolve as tags anexadas.
    #[tokio::test]
    async fn recent_transactions_carry_attached_tags() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Viagem", "var(--cat-sky)", Some("✈"), false)
            .await
            .unwrap();
        sqlx::query("INSERT INTO \"transaction\" (id, type, amount, date, is_projection) VALUES ('t1','expense',-5000,'2026-06-05',0)")
            .execute(&pool).await.unwrap();
        insert_realized(&pool, "expense", -3000, "2026-06-04").await; // sem tag
        crate::tags::set_transaction_tags(&pool, "t1", std::slice::from_ref(&tag))
            .await
            .unwrap();

        let rows = recent_transactions(&pool, 10).await.unwrap();
        let tagged = rows.iter().find(|r| r.id == "t1").unwrap();
        assert_eq!(tagged.tags.len(), 1);
        assert_eq!(tagged.tags[0].name, "Viagem");
        assert_eq!(tagged.tags[0].emoji.as_deref(), Some("✈"));
        let untagged = rows.iter().find(|r| r.id != "t1").unwrap();
        assert!(untagged.tags.is_empty(), "sem tag → tags vazio");
    }

    // Caminho de escrita: lançamento único realizado, com tags anexadas.
    #[tokio::test]
    async fn create_transaction_inserts_realized_with_tags() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Viagem", "#3aa", None, false)
            .await
            .unwrap();

        let id = create_transaction_inner(
            &pool,
            "expense",
            12_345,
            Some("Mercado".into()),
            "2026-06-14",
            Some("debit".into()),
            false,
            std::slice::from_ref(&tag),
            None,
        )
        .await
        .unwrap();

        let (amount, is_proj, desc): (i64, i64, String) = sqlx::query_as(
            "SELECT amount, is_projection, COALESCE(description,'') FROM \"transaction\" WHERE id = ?1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(amount, 12_345);
        assert_eq!(is_proj, 0, "lançamento manual é realizado");
        assert_eq!(desc, "Mercado");

        let (tag_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transaction_tag WHERE transaction_id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(tag_count, 1, "tag anexada ao lançamento");
    }

    // "Repetir": o caminho recorrente gera a série projetada inteira e propaga a tag.
    #[tokio::test]
    async fn create_transaction_with_recurrence_builds_tagged_series() {
        let pool = fixture_pool().await;
        let tag = crate::tags::create_tag(&pool, "Aluguel", "#a33", None, false)
            .await
            .unwrap();

        let rec_id = create_transaction_inner(
            &pool,
            "expense",
            230_000,
            Some("Aluguel".into()),
            "2026-06-15",
            Some("debit".into()),
            true,
            std::slice::from_ref(&tag),
            Some(RecurrenceInput {
                frequency: "mensal".into(),
                repetitions: 3,
            }),
        )
        .await
        .unwrap();

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM \"transaction\" WHERE recurrence_id = ?1 AND is_projection = 1",
        )
        .bind(&rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 3, "série projetada de 3 meses");

        let (tagged,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transaction_tag tt JOIN \"transaction\" t ON t.id = tt.transaction_id WHERE t.recurrence_id = ?1",
        )
        .bind(&rec_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tagged, 3, "tag propagada a cada ocorrência");
    }

    #[tokio::test]
    async fn create_transaction_rejects_bad_input() {
        let pool = fixture_pool().await;
        assert!(
            create_transaction_inner(
                &pool,
                "transfer",
                100,
                None,
                "2026-06-14",
                None,
                false,
                &[],
                None
            )
            .await
            .is_err(),
            "tipo não suportado pelo form é rejeitado"
        );
        assert!(
            create_transaction_inner(
                &pool,
                "expense",
                0,
                None,
                "2026-06-14",
                None,
                false,
                &[],
                None
            )
            .await
            .is_err(),
            "valor zero/negativo é rejeitado"
        );
    }

    // Previsibilidade: meses futuros esparsos (só fixas) são detectados como incompletos, e a
    // poupança realizada (honesta) difere da projetada (otimista). O caso do usuário.
    #[tokio::test]
    async fn forecast_flags_incomplete_future_and_honest_annual_savings() {
        let pool = fixture_pool().await;
        // Meses realizados (jan–mai) com gasto típico R$ 1.000/mês e dissaving (renda 900 < 1000).
        for m in [1, 2, 3, 4, 5] {
            insert_realized(&pool, "income", 90_000, &format!("2026-{m:02}-05")).await;
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        // Junho corrente: realizado parcial.
        insert_realized(&pool, "income", 90_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 100_000, "2026-06-10").await;
        // Julho COMPLETO (salário + gasto típico) e agosto ESPARSO (salário + só fixa) — como na
        // planilha real: o futuro tem salário mas falta a fatura/variável → projeção otimista.
        insert_projection(&pool, "income", 90_000, "2026-07-05", "", 0).await;
        insert_projection(&pool, "expense", 100_000, "2026-07-10", "debit", 0).await;
        insert_projection(&pool, "income", 90_000, "2026-08-05", "", 0).await;
        insert_projection(&pool, "expense", 30_000, "2026-08-10", "debit", 1).await;
        insert_sheet_balance(&pool, "2026", "2026-12-31", 50_000).await; // estende horizonte

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Cobertura: julho completo (~100% do típico), agosto esparso (~30%).
        let jul = fc.coverage.iter().find(|c| c.month == 7).unwrap();
        let ago = fc.coverage.iter().find(|c| c.month == 8).unwrap();
        assert!(jul.is_complete);
        assert!(!ago.is_complete);
        assert!(ago.estimated_missing_cents > 0);
        assert_eq!(fc.trusted_through_month.as_deref(), Some("2026-07")); // confiável até julho
        assert!(fc.total_missing_cents > 0);

        // Poupança realizada (honesta) negativa; a projetada parece melhor (futuro esparso).
        assert!(fc.annual_savings.realized_rate_bps < fc.annual_savings.projected_rate_bps);
        assert_eq!(fc.annual_savings.target_bps, 2500);
    }

    // Staleness: um mês COMPLETO conta na poupança anual mesmo se as transações ainda têm
    // is_projection=1 congelado (dono importou quando era futuro e não re-importou). A janela
    // de DATA é a fonte de verdade, não o flag (auditoria: edições em dias passados).
    #[tokio::test]
    async fn realized_annual_ignores_stale_is_projection_flag() {
        let pool = fixture_pool().await;
        // Maio (mês completo) lançado como PROJEÇÃO (stale) — hoje é junho.
        insert_projection(&pool, "income", 500_000, "2026-05-05", "", 0).await;
        insert_projection(&pool, "expense", 480_000, "2026-05-10", "debit", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let (income, savings) = realized_annual_savings(&pool, today).await.unwrap();
        assert_eq!(income, 500_000); // maio conta apesar do is_projection=1
        assert_eq!(savings, 20_000); // 500.000 − 480.000
    }

    // Semente: o Saldo da planilha vence os Bolsos quando há série importada.
    #[tokio::test]
    async fn projection_seed_prefers_sheet_saldo_over_pockets() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // bolso de R$ 1.000,00
        insert_sheet_balance(&pool, "2026", "2026-06-12", 500_000).await; // Saldo de hoje

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // Saldo de hoje na planilha vence o bolso; sem lacuna (hoje está na série).
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 500_000);
    }

    // Sem planilha importada, a semente continua sendo os Bolsos líquidos (spec 007).
    #[tokio::test]
    async fn projection_seed_falls_back_to_pockets_without_sheet() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 250_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 250_000);
    }

    // Planilha sem o dia de hoje preenchido: semente = último Saldo ≤ hoje + lançamentos
    // realizados no intervalo (não pode perder o que aconteceu entre o último saldo e hoje).
    #[tokio::test]
    async fn projection_seed_folds_realized_gap_up_to_today() {
        let pool = fixture_pool().await;
        insert_sheet_balance(&pool, "2026", "2026-06-10", 500_000).await;
        insert_realized(&pool, "income", 20_000, "2026-06-11").await;
        insert_realized(&pool, "expense", 5_000, "2026-06-12").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        // 500.000 + 20.000 − 5.000 = 515.000
        assert_eq!(projection_seed(&pool, today).await.unwrap(), 515_000);
    }

    // Regressão: sem bolso e com o Saldo da planilha sendo descartado, o saldo de hoje aparecia
    // ZERADO e surgia um déficit ("buraco") falso. Com a série de Saldo da planilha como semente,
    // o saldo de hoje passa a bater com a planilha e o déficit falso some.
    #[tokio::test]
    async fn forecast_seeds_from_sheet_saldo_no_false_deficit() {
        let pool = fixture_pool().await;
        // Saldo de hoje vindo da planilha; uma saída futura pequena no dia 26.
        insert_sheet_balance(&pool, "2026", "2026-06-12", 500_000).await;
        insert_projection(&pool, "expense", 7_890, "2026-06-26", "debit", 0).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        // Antes: daily[0] = 0 (saldo zerado). Agora bate com o Saldo de hoje da planilha.
        assert_eq!(fc.daily[0].date, "2026-06-12");
        assert_eq!(fc.daily[0].balance_cents, 500_000);
        // Antes: deepest_deficit negativo (o "buraco que não existe"). Agora positivo.
        let trough = fc.deepest_deficit.as_ref().unwrap();
        assert_eq!(trough.balance_cents, 500_000 - 7_890);
        assert!(trough.balance_cents > 0);
        assert!(fc.safe_to_spend_today_cents > 0);
    }

    // Guardrail duplo end-to-end: caixa cheio (semente alta), mas o mês corrente com performance
    // negativa → "pode gastar" honesto = 0, limitado pela poupança ANUAL. O horizonte varre até o
    // fim dos dados pré-lançados (dez). Crava o P0 do review: a performance do mês corrente inclui o
    // REALIZADO antes de hoje (não só os eventos futuros), senão o mês aparece com sinal trocado e o
    // guardrail decide errado.
    #[tokio::test]
    async fn forecast_dual_guardrail_savings_binds_for_owner() {
        let pool = fixture_pool().await;
        insert_sheet_balance(&pool, "2026", "2026-06-13", 500_000).await; // Saldo de hoje
        insert_sheet_balance(&pool, "2026", "2026-12-31", 1_500_000).await; // estende horizonte
        // Meses COMPLETOS (jan–mai) abaixo da meta → a poupança ANUAL morde. Sem isto, o mês
        // corrente (junho, em andamento) NÃO conta — evita o falso pânico de meio de mês.
        for m in [1, 2, 3, 4, 5] {
            insert_realized(&pool, "income", 200_000, &format!("2026-{m:02}-05")).await;
            insert_realized(&pool, "expense", 220_000, &format!("2026-{m:02}-10")).await;
        }
        // Junho (corrente) — metade realizada antes de hoje, metade projetada: testa que a
        // PERFORMANCE do mês inclui o realizado (o P0), mesmo o mês não contando na poupança anual.
        insert_realized(&pool, "income", 400_000, "2026-06-05").await;
        insert_realized(&pool, "expense", 700_000, "2026-06-10").await;
        insert_realized(&pool, "income", 600_000, "2026-06-29").await;
        insert_projection(&pool, "expense", 400_000, "2026-06-30", "debit", 1).await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.horizon_end, "2026-12-31");
        // Performance de junho = MÊS INTEIRO (realizado + projetado): inclui os R$ 700k já
        // realizados em 10/jun, que o cálculo antigo (só futuros) ignorava (o P0). Inclui também a
        // PREVISÃO de diário restante: como não há orçamento diário explícito, o teto/dia vem do
        // Diário médio de maio (220.000 ÷ 31 = 7.096/dia), injetado nos dias futuros de junho
        // (14–30 = 17 dias) = 120.632 — junho fica corretamente mais conservador que o antigo −100k.
        let jun = fc.months.iter().find(|m| m.month == 6).unwrap();
        let projected_daily_jun = (220_000 / 31) * 17; // teto = Diário médio de maio × dias 14–30
        assert_eq!(
            jun.performance_cents,
            1_000_000 - 1_100_000 - projected_daily_jun
        ); // −220.632
        // Poupança ANUAL (meses completos jan–mai, abaixo da meta) manda → pode gastar 0.
        assert_eq!(fc.binding_guardrail, "savings");
        assert_eq!(fc.safe_to_spend_today_cents, 0);
        assert!(fc.savings_headroom_cents.unwrap() < 0);
        assert!(fc.cash_headroom_cents > 0); // mas há caixa (é a reserva)
        assert_eq!(fc.savings_target_bps, 2500);
    }

    async fn insert_reserve_account(pool: &sqlx::SqlitePool, balance: i64) {
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(pool)
            .await
            .unwrap();
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, liquidity) \
             VALUES (?1,'Reserva','savings',?2,?3,'reserve')",
        )
        .bind(&aid)
        .bind(&pid)
        .bind(balance)
        .execute(pool)
        .await
        .unwrap();
    }

    // P1.1 (review): reserve_months derivado ao vivo = saldo das contas de reserva ÷ custo de vida
    // mensal (baseline), em vez do `reserve.current_months` que nunca tem writer de produção.
    #[tokio::test]
    async fn dashboard_reserve_months_derived_from_balance_and_baseline() {
        let pool = fixture_pool().await;
        // Custo de vida: meses completos (mar–mai) com saída 100.000/mês → mediana baseline = 100.000.
        for m in [3, 4, 5] {
            insert_realized(&pool, "expense", 100_000, &format!("2026-{m:02}-10")).await;
        }
        insert_reserve_account(&pool, 600_000).await; // 600.000 ÷ 100.000 = 6,0 meses
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert!((s.reserve_months - 6.0).abs() < 1e-9);
    }

    // Sem nenhum mês completo (baseline 0), reserve_months é 0 (não divide por zero).
    #[tokio::test]
    async fn dashboard_reserve_months_zero_without_baseline() {
        let pool = fixture_pool().await;
        insert_reserve_account(&pool, 600_000).await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.reserve_months, 0.0);
    }

    // P1.3 (review): teto do diário cai para o Diário médio do mês anterior quando não há orçamento
    // explícito (antes era 0 fixo → tile "de R$0" e forecast otimista).
    #[tokio::test]
    async fn dashboard_daily_budget_falls_back_to_prior_month_avg() {
        let pool = fixture_pool().await;
        // Maio: diário (is_fixed=0) de 310.000 em 31 dias → média = 10.000/dia.
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await;
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.daily_budget, 310_000 / 31); // 10.000
    }

    // O saldo projetado do dashboard deve usar o mesmo driver de Diário futuro do forecast; senão o
    // tile "Saldo projetado" fica otimista e diverge do fim do mês exibido no herói.
    #[tokio::test]
    async fn dashboard_projected_balance_includes_future_daily_ceiling() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 300_000).await;
        // Maio: diário de 310.000 em 31 dias → teto típico = 10.000/dia.
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await;

        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();
        let forecast = forecast_dto(&pool, today).await.unwrap();

        // Junho tem 17 dias após 13/jun (14..30). O saldo precisa reservar esse Diário restante.
        assert_eq!(summary.balance, 130_000);
        assert_eq!(forecast.month_end[0].balance_cents, summary.balance);
    }

    // Orçamento diário explícito ativo vence o fallback.
    #[tokio::test]
    async fn dashboard_daily_budget_prefers_explicit_active_budget() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "expense", 310_000, "2026-05-10").await; // existiria fallback 10.000
        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, 'Tester')")
            .bind(&pid)
            .execute(&pool)
            .await
            .unwrap();
        let bid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status, free_income) \
             VALUES (?1,?2,?3,'2026-06-01','active',0)",
        )
        .bind(&bid)
        .bind(&pid)
        .bind(15_000_i64)
        .execute(&pool)
        .await
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let s = dashboard_summary(&pool, today).await.unwrap();
        assert_eq!(s.daily_budget, 15_000); // orçamento explícito, não o fallback de 10.000
    }

    // Grade do mês: 31 linhas (maio), fluxos agregados por dia e Saldo vindo da planilha.
    #[tokio::test]
    async fn month_grid_aggregates_flows_and_uses_sheet_saldo() {
        let pool = fixture_pool().await;
        insert_realized(&pool, "income", 700_000, "2026-05-05").await;
        insert_realized(&pool, "expense", 12_000, "2026-05-05").await; // diário (is_fixed NULL → Diário)
        insert_realized(&pool, "expense", 4_000, "2026-05-09").await;
        insert_sheet_balance(&pool, "2026", "2026-05-05", 580_000).await;

        let grid = month_grid(&pool, 2026, 5).await.unwrap();
        assert_eq!(grid.len(), 31); // maio tem 31 dias
        assert_eq!(grid[0].day, 1);
        assert_eq!(grid[0].balance_cents, None); // dia 1 não importado

        let d5 = grid.iter().find(|g| g.day == 5).unwrap();
        assert_eq!(d5.income_cents, 700_000);
        assert_eq!(d5.daily_out_cents, 12_000);
        assert_eq!(d5.fixed_out_cents, 0);
        assert_eq!(d5.balance_cents, Some(580_000)); // Saldo da planilha

        let d9 = grid.iter().find(|g| g.day == 9).unwrap();
        assert_eq!(d9.daily_out_cents, 4_000);
        assert_eq!(d9.balance_cents, None);
    }

    #[tokio::test]
    async fn forecast_dto_empty_db_is_flat_zero() {
        let pool = fixture_pool().await;
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fc = forecast_dto(&pool, today).await.unwrap();

        assert_eq!(fc.daily.len(), 22);
        assert!(fc.daily.iter().all(|d| d.balance_cents == 0));
        assert!(
            fc.daily
                .iter()
                .all(|d| d.income_cents == 0 && d.fixed_out_cents == 0 && d.daily_out_cents == 0)
        );
        assert_eq!(fc.safe_to_spend_today_cents, 0);
    }

    // --- 007 pockets & liquidity (TDD) ---

    #[test]
    fn liquidity_is_derived_deterministically_per_type() {
        assert_eq!(liquidity_for_type("bank"), Some("liquid"));
        assert_eq!(liquidity_for_type("wallet"), Some("liquid"));
        assert_eq!(liquidity_for_type("business"), Some("liquid"));
        assert_eq!(liquidity_for_type("savings"), Some("reserve"));
        assert_eq!(liquidity_for_type("meal_voucher"), Some("restricted"));
        assert_eq!(liquidity_for_type("pension"), Some("illiquid"));
        assert_eq!(liquidity_for_type("fgts"), Some("illiquid"));
        assert_eq!(liquidity_for_type("credit_card"), None);
        assert_eq!(liquidity_for_type("nope"), None);
    }

    #[tokio::test]
    async fn savings_no_longer_inflates_the_projection_seed() {
        let pool = fixture_pool().await;
        insert_liquid_account(&pool, 100_000).await; // bank → liquid
        // Savings R$ 5.000,00 — reserve money, must stay out of the cash seed.
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) \
             SELECT ?1, 'Poupança', 'savings', owner_person_id, 500000 FROM account LIMIT 1",
        )
        .bind(&aid)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(liquid_seed(&pool).await.unwrap(), 100_000);
    }

    #[tokio::test]
    async fn create_account_derives_liquidity_and_default_person() {
        let pool = fixture_pool().await;
        // No person yet — command must bootstrap "Eu".
        let id = create_account_inner(&pool, "Vale".into(), "meal_voucher".into(), 42_000, None)
            .await
            .unwrap();
        let (liq, owner): (String, String) =
            sqlx::query_as("SELECT liquidity, owner_person_id FROM account WHERE id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(liq, "restricted");
        let (owner_name,): (String,) = sqlx::query_as("SELECT name FROM person WHERE id = ?1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(owner_name, "Eu");

        // Invalid type and empty name are rejected at the boundary.
        assert!(
            create_account_inner(&pool, "X".into(), "credit_card".into(), 0, None)
                .await
                .is_err()
        );
        assert!(
            create_account_inner(&pool, "  ".into(), "bank".into(), 0, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn pockets_groups_and_net_worth_follow_the_contract() {
        let pool = fixture_pool().await;
        for (name, t, balance) in [
            ("Conta", "bank", 100_000i64),
            ("Poupança", "savings", 500_000),
            ("Vale", "meal_voucher", 42_000),
            ("Previdência", "pension", 900_000),
            ("FGTS", "fgts", 300_000),
        ] {
            create_account_inner(&pool, name.into(), t.into(), balance, None)
                .await
                .unwrap();
        }
        let p = pockets(&pool).await.unwrap();
        assert_eq!(p.liquid_cents, 100_000);
        assert_eq!(p.reserve_cents, 500_000);
        assert_eq!(p.restricted_cents, 42_000);
        assert_eq!(p.illiquid_cents, 1_200_000);
        // Net worth excludes the restricted vale ledger.
        assert_eq!(p.net_worth_cents, 1_800_000);
        assert_eq!(p.accounts.len(), 5);
    }

    #[tokio::test]
    async fn migration_trigger_backfills_liquidity_on_plain_inserts() {
        let pool = fixture_pool().await;
        // Mirrors legacy import paths that insert without the liquidity column.
        insert_liquid_account(&pool, 1).await;
        let (liq,): (Option<String>,) =
            sqlx::query_as("SELECT liquidity FROM account WHERE type = 'bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(liq.as_deref(), Some("liquid"));
    }

    // T7.2 — credit cycle aggregation: credit_spend from daily_checkin lands as a lump at due_day.
    #[tokio::test]
    async fn dashboard_credit_lump_at_due_day() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let pid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, name) VALUES (?1, ?2)")
            .bind(&pid)
            .bind("Tester")
            .execute(&pool)
            .await
            .unwrap();

        // Liquid bank account with R$2000.00
        let aid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&aid)
        .bind("Conta")
        .bind("bank")
        .bind(&pid)
        .bind(200_000i64)
        .execute(&pool)
        .await
        .unwrap();

        // Credit card: closes on day 20, due on day 10
        let card_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account (id, name, type, owner_person_id, balance, closing_day, due_day) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )
        .bind(&card_id)
        .bind("Cartão")
        .bind("credit_card")
        .bind(&pid)
        .bind(0i64)
        .bind(20i32)
        .bind(10i32)
        .execute(&pool)
        .await
        .unwrap();

        // Checkin on March 15 with credit_spend = R$500.00
        // Cycle closes March 20 → due April 10
        let checkin_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES (?1,?2,?3,?4,?5)",
        )
        .bind(&checkin_id)
        .bind(&pid)
        .bind("2026-03-15")
        .bind(0i64)
        .bind(50_000i64)
        .execute(&pool)
        .await
        .unwrap();

        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let summary = dashboard_summary(&pool, today).await.unwrap();

        // Seed = 200000. Credit lump of 50000 lands on April 10 (outside March horizon).
        // Projected end-of-March = 200000 (no events in March after today).
        assert_eq!(summary.balance, 200_000);
    }
}
