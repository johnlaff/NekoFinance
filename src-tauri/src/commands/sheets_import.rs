use super::*;
use quick_xml::Reader as XmlReader;
use quick_xml::events::{BytesRef, BytesStart, Event};
use std::collections::HashMap;
use std::path::Path;

#[derive(serde::Serialize)]
pub struct SheetInfo {
    pub title: String,
    pub sheet_id: i64,
}

/// Retorno estruturado do import: mantém `count` NUMÉRICO (consumido
/// aritmeticamente pelo frontend, ex. `importAllTabs`/`Acc`) e acrescenta os diagnósticos de
/// precisão (nota não itemizada / item↔célula divergente) sem substituir nada que já existia.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportOutcome {
    pub count: usize,
    pub summary: String,
    pub diagnostics: Vec<import::ImportDiagnostic>,
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
    // JUNHO–DEZEMBRO no preview, como no import.
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
) -> Result<ImportOutcome, String> {
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
) -> Result<ImportOutcome, String> {
    if layout_detect::is_metric_tab(sheet_name) {
        return Ok(ImportOutcome {
            count: 0,
            summary: String::new(),
            diagnostics: Vec::new(),
        });
    }

    // Grade usada inteira: a planilha real tem 12 blocos mensais até a coluna BO (~71
    // colunas) — um range A:Z cortaria JUNHO–DEZEMBRO em silêncio.
    let range = quote_sheet(sheet_name);
    let values = client.get_sheet_values(spreadsheet_id, &range).await?;
    let rows = values.values;

    if rows.len() < 3 {
        return Ok(ImportOutcome {
            count: 0,
            summary: String::new(),
            diagnostics: Vec::new(),
        });
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

    // Diagnósticos de precisão são função do LOTE já parseado (nota crua + total da
    // célula), não da escrita — coletados ANTES do skip de checksum para que uma reimportação
    // idêntica (dedup) continue reportando as mesmas notas que precisam de atenção.
    let diagnostics =
        import::collect_import_diagnostics(sheet_name, &imported_rows, descriptions_trusted);

    // Checksum + checagem de duplicata ANTES de abrir a transação (leitura no pool; dentro da tx
    // daria read-your-writes falso-negativo). Dataset idêntico ao último import → idempotente.
    let checksum = import::compute_import_checksum(&imported_rows, descriptions_trusted);
    if !imported_rows.is_empty()
        && import::check_duplicate_import(pool, sheet_name, &checksum).await?
    {
        return Ok(ImportOutcome {
            count: 0,
            summary: String::new(),
            diagnostics,
        });
    }

    // Captura a coluna Saldo (o saldo corrente do método) → semente da projeção + visão histórica.
    // Sem isto a semente era 0 e o saldo de hoje aparecia zerado. `get_balance_offset_for_sheet` é
    // leitura no pool; pode rodar antes de abrir a tx.
    let balance_offset = import::get_balance_offset_for_sheet(pool, sheet_name).await?;
    let balances = import::parse_balance_series(&rows, &layout, balance_offset)?;
    // Zeros de template anteriores à adoção da planilha não são dado — caem antes do store.
    let first_txn_date = imported_rows.iter().map(|r| r.date.as_str()).min();
    let balances = import::trim_pre_history_balances(balances, first_txn_date);

    // Cerimônia do teto documentada em nota da coluna Diário → proposta (confirmação na UI).
    // Só com notas confiáveis: um ciclo degradado não pode propor de dado ausente.
    let daily_offset = mappings
        .iter()
        .find(|(field, _)| field == "amount_daily")
        .map(|(_, off)| *off as usize)
        .unwrap_or(3);
    let ceremony_note = if descriptions_trusted {
        import::scan_ceiling_ceremony_note(&rows, &notes, &layout, daily_offset)
    } else {
        None
    };

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

    if let Some((source_month, raw_note)) = &ceremony_note {
        import::upsert_ceiling_proposal_in_tx(&mut tx, source_month, raw_note).await?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("commit import: {e}"))?;

    Ok(ImportOutcome {
        count,
        summary: format!("{count} linha(s) importada(s) de '{sheet_name}'."),
        diagnostics,
    })
}

/// Células numéricas do calamine viram string decimal-com-ponto de 4 casas fixas: `123.456`
/// vira `123.4560`, que o `parse_number` não confunde com agrupamento de milhar; `12.34` deve
/// representar R$ 12,34, nunca R$ 1.234,00.
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

// --- Recuperação de notas de célula no import local de .xlsx ---
//
// calamine 0.36.0 expõe valores, mas não notas de célula; `check_comments` apenas controla
// comentários XML do parser interno, sem relação com anotações de planilha (verificado 2026-07).
// As notas existem no arquivo: um `.xlsx` é um zip, e comentários de célula LEGADOS — exatamente o
// que a API do Sheets chama de "nota" (sem autor, sem thread) — vivem em `xl/comments<N>.xml`,
// referenciados pelo `.rels` da aba (`xl/worksheets/_rels/sheet<M>.xml.rels`). As funções abaixo
// leem esse zip em paralelo ao calamine para reconstruir a MESMA grade `Vec<Vec<String>>` que
// `SheetsClient::get_sheet_notes` devolve no caminho da API, para que `parse_rows_with_layout` /
// `cell_raw_note` / `parse_itemized_note` (que já sabem processar notas) recebam dado real.

/// Limites REAIS da grade do Excel (`XFD1048576`): 1.048.576 linhas × 16.384 colunas. Uma `ref`
/// além disso é impossível numa planilha válida — só aparece por corrupção/edição manual/arquivo
/// forjado, e NÃO pode ser propagada para `grid.resize(row+1, ..)` (ver `decode_a1_ref`).
const XLSX_MAX_ROWS: u32 = 1_048_576;
const XLSX_MAX_COLS: u32 = 16_384;

/// Decodifica uma referência A1 (`"A1"`, `"AZ16"`, `"BL32"`) em (linha, coluna) 0-based. Suporta
/// colunas MULTI-LETRA (a planilha real passa de Z — os blocos mensais vão até ~coluna BO).
/// `None` para referências malformadas (sem letras, sem dígitos, linha/coluna < 1) OU fora dos
/// limites da grade do Excel (`XFD1048576`).
fn decode_a1_ref(a1: &str) -> Option<(u32, u32)> {
    let split_at = a1.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = a1.split_at(split_at);
    if letters.is_empty() || digits.is_empty() || !letters.bytes().all(|b| b.is_ascii_alphabetic())
    {
        return None;
    }
    let mut col: u64 = 0;
    for b in letters.bytes() {
        col = col * 26 + u64::from(b.to_ascii_uppercase() - b'A' + 1);
    }
    let col0 = u32::try_from(col.checked_sub(1)?).ok()?;
    let row1: u32 = digits.parse().ok()?;
    let row0 = row1.checked_sub(1)?;
    // Rejeita `ref` fora da grade real (ex.: `A4294967295`, bem-formada mas absurda). Sem isto, o
    // `grid.resize(row+1, ..)` em `read_xlsx_comments` pediria dezenas/centenas de GB e o
    // allocator ABORTA o processo — um abort não é `Err` capturável, então o `.unwrap_or_else`
    // do chamador nunca degradaria. Aqui a `ref` inválida cai no mesmo caminho já testado de
    // "pula esta nota".
    if row0 >= XLSX_MAX_ROWS || col0 >= XLSX_MAX_COLS {
        return None;
    }
    Some((row0, col0))
}

/// Extrai o valor (com entidades já resolvidas) de UM atributo pelo nome LOCAL — ignora o
/// prefixo de namespace (`r:id` casa com `"id"`), espelhando como o próprio calamine resolve o
/// mesmo atributo ao montar seu mapa aba→caminho interno (`read_workbook`, calamine `xlsx/mod.rs`).
fn attr_by_local_name(
    tag: &BytesStart<'_>,
    reader: &XmlReader<&[u8]>,
    local_name: &[u8],
) -> Result<Option<String>, String> {
    for attr in tag.attributes() {
        let attr = attr.map_err(|e| format!("atributo: {e}"))?;
        if attr.key.local_name().as_ref() == local_name {
            let value = attr
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
                .map_err(|e| format!("valor de atributo: {e}"))?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// Resolve UM `Event::GeneralRef` (entidade `&nome;` ou referência numérica `&#NNN;`) para o
/// caractere real. quick-xml 0.41 passou a emitir essas referências como evento SEPARADO do texto
/// ao redor (não mais pré-resolvidas dentro de `Event::Text`) — sem este passo extra, uma nota com
/// `&amp;`/`&#10;` sairia com o nome da entidade literal em vez do caractere.
fn resolve_general_ref(entity: &BytesRef<'_>) -> Result<String, String> {
    let name = entity.decode().map_err(|e| format!("entidade: {e}"))?;
    if let Some(resolved) = quick_xml::escape::resolve_xml_entity(&name) {
        return Ok(resolved.to_string());
    }
    match entity
        .resolve_char_ref()
        .map_err(|e| format!("referência numérica: {e}"))?
    {
        Some(ch) => Ok(ch.to_string()),
        None => Err(format!("entidade XML não reconhecida em nota: &{name};")),
    }
}

/// Lê `xl/comments<N>.xml` → lista `(ref A1, texto)` na ordem do documento. O texto de UM
/// comentário é a concatenação, SEM separador, de todo conteúdo textual dentro de `<t>` — runs
/// (`<r><rPr/><t/></r>`) existem só para formatação; concatenar sem separador reproduz a MESMA
/// string que `SheetsClient::get_sheet_notes` devolve para a mesma nota (o campo `note` da API do
/// Sheets também é string plana, sem marcação de run). Acumular só dentro de `<t>` (e não em
/// `<text>`) evita que espaço/indentação entre `<r>`/`<rPr>` vaze para dentro da nota.
fn parse_comment_notes(xml: &str) -> Result<Vec<(String, String)>, String> {
    let mut reader = XmlReader::from_str(xml);
    let mut results: Vec<(String, String)> = Vec::new();
    let mut current_ref: Option<String> = None;
    let mut t_depth: usize = 0;
    let mut acc = String::new();

    loop {
        match reader
            .read_event()
            .map_err(|e| format!("xl/comments*.xml: {e}"))?
        {
            Event::Start(e) => match e.local_name().as_ref() {
                b"comment" => {
                    current_ref = attr_by_local_name(&e, &reader, b"ref")?;
                    acc.clear();
                }
                b"t" => t_depth += 1,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"comment" => {
                // `<comment ref="A1"/>` sem `<text>` — nota vazia, mas ainda é uma nota presente.
                if let Some(r) = attr_by_local_name(&e, &reader, b"ref")? {
                    results.push((r, String::new()));
                }
            }
            Event::Text(t) if t_depth > 0 => {
                acc.push_str(
                    &t.xml10_content()
                        .map_err(|e| format!("texto da nota: {e}"))?,
                );
            }
            // `<t><![CDATA[..]]></t>` — conteúdo literal (sem escapes). Raro em notas, mas um
            // exportador pode usá-lo; sem este ramo o texto seria SILENCIOSAMENTE descartado.
            Event::CData(c) if t_depth > 0 => {
                acc.push_str(&c.decode().map_err(|e| format!("CDATA da nota: {e}"))?);
            }
            Event::GeneralRef(r) if t_depth > 0 => {
                acc.push_str(&resolve_general_ref(&r)?);
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => t_depth = t_depth.saturating_sub(1),
                b"comment" => {
                    if let Some(r) = current_ref.take() {
                        results.push((r, std::mem::take(&mut acc)));
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(results)
}

/// Lê `xl/workbook.xml` → lista `(nome da aba, r:id)` na ordem do documento — o `r:id` resolve o
/// CAMINHO da aba via `xl/_rels/workbook.xml.rels` no próximo passo.
fn parse_workbook_sheet_rids(xml: &str) -> Result<Vec<(String, String)>, String> {
    let mut reader = XmlReader::from_str(xml);
    let mut sheets = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| format!("xl/workbook.xml: {e}"))?
        {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"sheet" => {
                let name = attr_by_local_name(&e, &reader, b"name")?;
                let rid = attr_by_local_name(&e, &reader, b"id")?;
                if let (Some(name), Some(rid)) = (name, rid) {
                    sheets.push((name, rid));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(sheets)
}

/// Lê um `.rels` (`Relationships` do OOXML) → lista `(Id, Type, Target)` na ordem do documento.
fn parse_relationships(xml: &str) -> Result<Vec<(String, String, String)>, String> {
    let mut reader = XmlReader::from_str(xml);
    let mut rels = Vec::new();
    loop {
        match reader.read_event().map_err(|e| format!("rels: {e}"))? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                let id = attr_by_local_name(&e, &reader, b"Id")?;
                let ty = attr_by_local_name(&e, &reader, b"Type")?;
                let target = attr_by_local_name(&e, &reader, b"Target")?;
                if let (Some(id), Some(ty), Some(target)) = (id, ty, target) {
                    rels.push((id, ty, target));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rels)
}

/// Resolve um `Target` de relationship RELATIVO ao diretório da PARTE de origem (não ao diretório
/// `_rels/` — regra do OOXML: relacionamentos resolvem contra o diretório da parte que os declara,
/// nunca contra a pasta `_rels` que os contém). `..` sobe um nível; alvo já absoluto (`/xl/...`)
/// ignora `base_dir`.
fn resolve_relative_target(base_dir: &str, target: &str) -> String {
    if let Some(stripped) = target.strip_prefix('/') {
        return stripped.to_string();
    }
    let mut segments: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// Normaliza um caminho de parte OPC para lookup no zip: `\` → `/` e caixa baixa. O OOXML admite
/// `\` como separador e trata nomes de parte como case-insensitive; sem normalizar, um `.xlsx`
/// spec-legal com caixa mista/backslash leria valores (calamine normaliza internamente) mas
/// perderia TODAS as notas silenciosamente. Espelha o cache de caminhos do calamine.
fn normalize_zip_path(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

/// Índice caminho-normalizado → nome REAL da entrada, construído uma vez a partir de
/// `zip.file_names()`. Resolve o mismatch de caixa/separador entre os caminhos que MONTAMOS
/// (`xl/comments1.xml`, `xl/worksheets/_rels/sheet1.xml.rels`) e como o arquivo de fato os
/// armazenou.
fn build_zip_name_index(zip: &zip::ZipArchive<std::fs::File>) -> HashMap<String, String> {
    zip.file_names()
        .map(|n| (normalize_zip_path(n), n.to_string()))
        .collect()
}

/// Lê UMA entrada do zip como string UTF-8 via o índice normalizado; `Ok(None)` quando a entrada
/// não existe (relação OPCIONAL — ex.: aba sem nenhum comentário não tem `.rels` de comments) —
/// NUNCA um erro.
fn zip_entry_to_string(
    zip: &mut zip::ZipArchive<std::fs::File>,
    name_index: &HashMap<String, String>,
    name: &str,
) -> Result<Option<String>, String> {
    use std::io::Read;
    let Some(actual) = name_index.get(&normalize_zip_path(name)) else {
        return Ok(None);
    };
    let mut entry = zip
        .by_name(actual)
        .map_err(|e| format!("entrada do zip {name}: {e}"))?;
    let mut buf = String::new();
    entry
        .read_to_string(&mut buf)
        .map_err(|e| format!("ler {name}: {e}"))?;
    Ok(Some(buf))
}

/// Notas de célula do `.xlsx` por aba — chave = nome EXATO da aba (igual a
/// `workbook.sheet_names()` do calamine), valor = grade `[linha][coluna]` 0-based com ORIGEM
/// ABSOLUTA (linha 0 = linha 1 da planilha, coluna 0 = coluna A). O alinhamento com a origem do
/// `range` do calamine (que pode não começar em A1) é responsabilidade do chamador
/// (`align_notes_grid`), já que este cálculo roda UMA vez para o workbook inteiro, antes do
/// calamine abrir cada aba individualmente.
///
/// Erro só para partes OBRIGATÓRIAS ausentes/corrompidas (zip inválido, `xl/workbook.xml` ou
/// `xl/_rels/workbook.xml.rels` ausentes/malformados — sinal de layout inesperado, por exemplo
/// comentários THREADED do Sheets em `xl/threadedComments/` em vez do formato legado
/// `xl/comments*.xml`). O chamador degrada para SEM notas em qualquer erro, então um formato
/// diferente nunca quebra o import — só perde a itemização. Ausência de comentários numa aba
/// ESPECÍFICA (sem `.rels` de comments) é normal e vira grade vazia para aquela aba, sem erro.
fn read_xlsx_comments(path: &Path) -> Result<HashMap<String, Vec<Vec<String>>>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("abrir .xlsx como zip: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("abrir .xlsx como zip: {e}"))?;
    // Índice montado UMA vez (empresta o zip imutavelmente); depois disto todo lookup é por nome
    // real, tolerante a caixa/backslash.
    let name_index = build_zip_name_index(&zip);

    let workbook_xml = zip_entry_to_string(&mut zip, &name_index, "xl/workbook.xml")?
        .ok_or_else(|| "xl/workbook.xml ausente".to_string())?;
    let sheets = parse_workbook_sheet_rids(&workbook_xml)?;

    let rels_xml = zip_entry_to_string(&mut zip, &name_index, "xl/_rels/workbook.xml.rels")?
        .ok_or_else(|| "xl/_rels/workbook.xml.rels ausente".to_string())?;
    let workbook_rels = parse_relationships(&rels_xml)?;
    let rid_to_target: HashMap<&str, &str> = workbook_rels
        .iter()
        .map(|(id, _, target)| (id.as_str(), target.as_str()))
        .collect();

    let mut result = HashMap::new();
    for (sheet_name, rid) in &sheets {
        let Some(target) = rid_to_target.get(rid.as_str()) else {
            continue;
        };
        let sheet_path = resolve_relative_target("xl", target);
        let (sheet_dir, sheet_file) = match sheet_path.rsplit_once('/') {
            Some((dir, file)) => (dir, file),
            None => ("", sheet_path.as_str()),
        };
        let sheet_rels_path = if sheet_dir.is_empty() {
            format!("_rels/{sheet_file}.rels")
        } else {
            format!("{sheet_dir}/_rels/{sheet_file}.rels")
        };

        let Some(sheet_rels_xml) = zip_entry_to_string(&mut zip, &name_index, &sheet_rels_path)?
        else {
            continue; // aba sem .rels próprio → sem comentários (grade ausente = vazia)
        };
        let sheet_rels = parse_relationships(&sheet_rels_xml)?;
        let Some((_, _, comments_target)) = sheet_rels
            .iter()
            .find(|(_, ty, _)| ty.ends_with("/comments"))
        else {
            continue; // aba tem .rels mas nenhuma relação de comments (sem notas)
        };
        let comments_path = resolve_relative_target(sheet_dir, comments_target);
        let Some(comments_xml) = zip_entry_to_string(&mut zip, &name_index, &comments_path)? else {
            continue;
        };

        let mut grid: Vec<Vec<String>> = Vec::new();
        for (a1, text) in parse_comment_notes(&comments_xml)? {
            let Some((row, col)) = decode_a1_ref(&a1) else {
                continue; // ref malformada — ignora esta nota, não aborta a aba
            };
            let (row, col) = (row as usize, col as usize);
            if grid.len() <= row {
                grid.resize(row + 1, Vec::new());
            }
            if grid[row].len() <= col {
                grid[row].resize(col + 1, String::new());
            }
            grid[row][col] = text;
        }
        result.insert(sheet_name.clone(), grid);
    }
    Ok(result)
}

/// Recorta a grade ABSOLUTA (linha 1/coluna A) de `read_xlsx_comments` para a MESMA origem do
/// `range` do calamine (`range.start()`) — sem isto, `notes[r][c]` e `rows[r][c]` apontariam para
/// células diferentes sempre que a aba não começar exatamente em A1 (calamine usa
/// `HeaderRow::FirstNonEmptyRow` por padrão). Notas acima/à esquerda da origem são descartadas.
fn align_notes_grid(absolute: &[Vec<String>], start: (u32, u32)) -> Vec<Vec<String>> {
    let (start_row, start_col) = (start.0 as usize, start.1 as usize);
    absolute
        .iter()
        .skip(start_row)
        .map(|row| row.get(start_col..).map(|s| s.to_vec()).unwrap_or_default())
        .collect()
}

/// `true` quando a grade tem PELO MENOS uma nota não-vazia — decide `descriptions_trusted` e o
/// aviso de degradação: uma grade "presente" mas 100% vazia (aba sem nenhum comentário) deve se
/// comportar como sem notas, não como notas confiáveis vazias.
fn grid_has_any_note(grid: &[Vec<String>]) -> bool {
    grid.iter()
        .any(|row| row.iter().any(|cell| !cell.trim().is_empty()))
}

#[tauri::command]
pub async fn import_local_xlsx(
    pool: State<'_, SqlitePool>,
    guard: State<'_, std::sync::Arc<crate::sync_task::SyncGuard>>,
    file_path: String,
    profile_id: String,
) -> Result<ImportOutcome, String> {
    use calamine::{Reader, Xlsx, open_workbook};

    let workbook_path = validate_local_xlsx_path(&file_path)?;
    let mut workbook: Xlsx<_> =
        open_workbook(&workbook_path).map_err(|e| format!("open error: {e}"))?;

    // calamine não expõe notas de célula (ver banner acima); lemos o zip do .xlsx à parte para
    // recuperá-las. Erro aqui (zip corrompido, layout inesperado) degrada para SEM notas — os
    // valores ainda entram, mas o import não falha por conta de um anexo que este parser
    // auxiliar não conseguiu ler.
    let comments_by_sheet = read_xlsx_comments(&workbook_path).unwrap_or_else(|e| {
        eprintln!("[import] notas de célula do .xlsx indisponíveis: {e} — import sem notas");
        HashMap::new()
    });

    let sheet_names = workbook.sheet_names().to_vec();
    let mut total = 0usize;
    let mut sheets_imported = Vec::new();
    // Sinaliza o aviso de degradação só quando FOR verdade: pelo menos uma aba importada ficou
    // sem notas legíveis. Um import onde toda aba trouxe notas não deve carregar aviso nenhum.
    let mut any_sheet_without_notes = false;
    // Diagnósticos de precisão acumulados por aba (nota não itemizada / item↔célula
    // divergente) — surgem mesmo quando o import não escreve nada (aba deduplicada por checksum).
    let mut all_diagnostics: Vec<import::ImportDiagnostic> = Vec::new();

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

            // Notas de célula desta aba, realinhadas para a MESMA origem da grade de valores
            // (calamine começa no primeiro range não-vazio, não necessariamente em A1) — sem
            // isto `notes[r][c]` e `rows[r][c]` apontariam para células diferentes.
            let sheet_notes: Vec<Vec<String>> = comments_by_sheet
                .get(sheet_name)
                .map(|absolute| align_notes_grid(absolute, range.start().unwrap_or((0, 0))))
                .unwrap_or_default();
            let notes_found = grid_has_any_note(&sheet_notes);
            if !notes_found {
                any_sheet_without_notes = true;
            }

            let imported_rows =
                import::parse_rows_with_layout(&rows, &layout, &mappings, &sheet_notes)?;
            if imported_rows.is_empty() {
                continue;
            }

            // Coletado sobre o lote já parseado, ANTES do skip de checksum — sobrevive
            // ao dedup (uma reimportação idêntica desta aba continua reportando as mesmas notas).
            all_diagnostics.extend(import::collect_import_diagnostics(
                sheet_name,
                &imported_rows,
                notes_found,
            ));

            let options = import::ImportRowsOptions {
                descriptions_trusted: notes_found,
            };
            let checksum = import::compute_import_checksum(&imported_rows, notes_found);
            if import::check_duplicate_import(&pool, sheet_name, &checksum).await? {
                continue;
            }

            // Série de Saldo da aba (semente da projeção + visão histórica do livro-razão).
            let balance_offset = import::get_balance_offset_for_sheet(&pool, sheet_name).await?;
            let balances = import::parse_balance_series(&rows, &layout, balance_offset)?;
            // Zeros de template anteriores à adoção da planilha não são dado — caem antes do store.
            let first_txn_date = imported_rows.iter().map(|r| r.date.as_str()).min();
            let balances = import::trim_pre_history_balances(balances, first_txn_date);

            // Cerimônia do teto em nota da coluna Diário → proposta (confirmação na UI).
            let daily_offset = mappings
                .iter()
                .find(|(field, _)| field == "amount_daily")
                .map(|(_, off)| *off as usize)
                .unwrap_or(3);
            let ceremony_note = if notes_found {
                import::scan_ceiling_ceremony_note(&rows, &sheet_notes, &layout, daily_offset)
            } else {
                None
            };

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

            if let Some((source_month, raw_note)) = &ceremony_note {
                import::upsert_ceiling_proposal_in_tx(&mut tx, source_month, raw_note).await?;
            }

            tx.commit()
                .await
                .map_err(|e| format!("commit import: {e}"))?;

            total += count;
            sheets_imported.push(format!("{sheet_name} ({count} rows)"));
        }
    }

    // Aviso de degradação CONDICIONAL: só aparece quando alguma aba importada de
    // fato ficou sem notas de célula legíveis — sem elas o classificador de 5 tipos não roda
    // nessa aba (Cartão/Economia/Patrimônio caem em Saída sem itemização). Um import onde toda
    // aba trouxe notas não carrega aviso nenhum; a classificação de imports anteriores é
    // preservada (gate de confiança no `import_rows_core`) mesmo quando o aviso aparece.
    let warning = if any_sheet_without_notes {
        " Aviso: uma ou mais abas não carregaram notas de célula — a classificação por seção \
         (Cartão/Economia/Patrimônio) dessas abas exige o import ao vivo do Google Sheets ou um \
         .xlsx com anotações legíveis; itens já classificados foram preservados."
    } else {
        ""
    };

    Ok(ImportOutcome {
        count: total,
        summary: format!(
            "Imported {} total rows from: {}.{}",
            total,
            sheets_imported.join(", "),
            warning
        ),
        diagnostics: all_diagnostics,
    })
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

    // Grade inteira: A1:Z10 pode cortar a linha de dados/cabeçalhos da detecção.
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

/// Importa a aba `Economia` da planilha → ANOTAÇÃO de métrica em `economia_annotation`:
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

/// Timestamp (UTC, formato "YYYY-MM-DD HH:MM:SS" de `datetime('now')`) do evento
/// de sincronização com a planilha mais recente — import ou write-back. `None`
/// quando ainda não houve nenhuma sincronização. Semântica "última MUDANÇA":
/// o `sync_log` só ganha linha quando algo entra/sai da planilha.
#[tauri::command]
pub async fn last_sync_at(pool: State<'_, SqlitePool>) -> Result<Option<String>, String> {
    last_sync_at_query(pool.inner()).await
}

async fn last_sync_at_query(pool: &SqlitePool) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT MAX(timestamp) FROM sync_log WHERE event_type IN ('import', 'write_back')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("last_sync_at: {e}"))
}

#[cfg(test)]
mod xlsx_comment_notes_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    // Fixture .xlsx: aba "2026", 2 blocos mensais (JANEIRO/FEVEREIRO, block_size 6) — o mínimo
    // que `find_month_names_row` aceita (exige ≥2 nomes de mês na linha) — dia 1 com uma Saída de
    // R$150,00 em C3. Mesma geometria de `real_geometry_rows` (google_sheets::import), reduzida.

    const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/comments1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/>
</Types>"#;

    const ROOT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;

    const WORKBOOK_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="2026" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#;

    const WORKBOOK_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;

    const SHEET1_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>JANEIRO</t></is></c>
      <c r="G1" t="inlineStr"><is><t>FEVEREIRO</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Data</t></is></c>
      <c r="B2" t="inlineStr"><is><t>Entrada</t></is></c>
      <c r="C2" t="inlineStr"><is><t>Saída</t></is></c>
      <c r="D2" t="inlineStr"><is><t>Diário</t></is></c>
      <c r="E2" t="inlineStr"><is><t>Saldo</t></is></c>
      <c r="G2" t="inlineStr"><is><t>Data</t></is></c>
      <c r="H2" t="inlineStr"><is><t>Entrada</t></is></c>
      <c r="I2" t="inlineStr"><is><t>Saída</t></is></c>
      <c r="J2" t="inlineStr"><is><t>Diário</t></is></c>
      <c r="K2" t="inlineStr"><is><t>Saldo</t></is></c>
    </row>
    <row r="3">
      <c r="A3"><v>1</v></c>
      <c r="C3"><v>150</v></c>
    </row>
  </sheetData>
</worksheet>"#;

    const SHEET1_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/>
</Relationships>"#;

    // Malformada de propósito: `<t>` fechado por `</text>` (mismatch) — `check_end_names` (default
    // do quick-xml) deve rejeitar isto com erro, exercitando o fallback sem falhar o import.
    const MALFORMED_COMMENTS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <commentList>
    <comment ref="C3"><text><r><t>oops</text></r></comment>
  </commentList>
</comments>"#;

    /// Nome real da entrada do zip para `xl/comments1.xml` no caso padrão. A `SHEET1_RELS_XML`
    /// aponta o Target lowercase (`../comments1.xml`), então um nome de entrada em caixa diferente
    /// exercita a normalização de caminho (`normalize_zip_path`).
    const DEFAULT_COMMENTS_PART: &str = "xl/comments1.xml";

    fn comments_xml_with_ref(cell: &str, note: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors><author></author></authors>
  <commentList>
    <comment ref="{cell}" authorId="0"><text><r><t xml:space="preserve">{note}</t></r></text></comment>
  </commentList>
</comments>"#
        )
    }

    enum FixtureComments<'a> {
        /// Aba sem NENHUMA relação de comments (regressão: import sem notas, como hoje).
        None,
        /// Nota válida na célula C3 (a Saída de R$150,00 do dia 1/JANEIRO).
        Notes(&'a str),
        /// Nota numa célula ARBITRÁRIA (para `ref` fora dos limites da grade, etc.).
        NoteAtRef { cell: &'a str, note: &'a str },
        /// `xl/comments1.xml` deliberadamente malformado para exercitar o fallback não fatal.
        Malformed,
    }

    /// `comments_part` = nome REAL da entrada do zip para o arquivo de comentários (normalmente
    /// `DEFAULT_COMMENTS_PART`; um nome em caixa diferente prova a normalização de caminho).
    fn build_fixture_xlsx(comments: FixtureComments<'_>, comments_part: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

            zip.start_file("[Content_Types].xml", opts).unwrap();
            zip.write_all(CONTENT_TYPES_XML.as_bytes()).unwrap();

            zip.start_file("_rels/.rels", opts).unwrap();
            zip.write_all(ROOT_RELS_XML.as_bytes()).unwrap();

            zip.start_file("xl/workbook.xml", opts).unwrap();
            zip.write_all(WORKBOOK_XML.as_bytes()).unwrap();

            zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
            zip.write_all(WORKBOOK_RELS_XML.as_bytes()).unwrap();

            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all(SHEET1_XML.as_bytes()).unwrap();

            if !matches!(comments, FixtureComments::None) {
                zip.start_file("xl/worksheets/_rels/sheet1.xml.rels", opts)
                    .unwrap();
                zip.write_all(SHEET1_RELS_XML.as_bytes()).unwrap();

                zip.start_file(comments_part, opts).unwrap();
                let comments_xml = match comments {
                    FixtureComments::Notes(note) => comments_xml_with_ref("C3", note),
                    FixtureComments::NoteAtRef { cell, note } => comments_xml_with_ref(cell, note),
                    FixtureComments::Malformed => MALFORMED_COMMENTS_XML.to_string(),
                    FixtureComments::None => unreachable!(),
                };
                zip.write_all(comments_xml.as_bytes()).unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    fn write_fixture_to_temp(comments: FixtureComments<'_>) -> std::path::PathBuf {
        write_fixture_to_temp_at(comments, DEFAULT_COMMENTS_PART)
    }

    fn write_fixture_to_temp_at(
        comments: FixtureComments<'_>,
        comments_part: &str,
    ) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("neko-notes-{}.xlsx", uuid::Uuid::new_v4()));
        std::fs::write(&path, build_fixture_xlsx(comments, comments_part)).unwrap();
        path
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn count_line_items(pool: &SqlitePool, txn_id: &str) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM line_item WHERE transaction_id = ?1")
            .bind(txn_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_transactions(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM \"transaction\"")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Reproduz o que `import_local_xlsx` faz por aba, sem o `tauri::State` (que o teste não
    /// consegue construir): `read_xlsx_comments` → calamine `worksheet_range` → `align_notes_grid`
    /// → detecção de layout → `parse_rows_with_layout`. Mesma sequência, pool-level.
    fn parse_fixture_rows(path: &Path) -> (Vec<import::ImportedRow>, bool) {
        use calamine::{Reader, Xlsx, open_workbook};

        let comments_by_sheet = read_xlsx_comments(path).unwrap_or_default();
        let mut workbook: Xlsx<_> = open_workbook(path).unwrap();
        let range = workbook.worksheet_range("2026").unwrap();
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(xlsx_cell_to_string).collect())
            .collect();

        let layout = layout_detect::detect_layout(&rows, "2026").unwrap();
        let mappings: Vec<(String, i32)> = layout_detect::generate_mappings(&layout)
            .into_iter()
            .filter(|m| m.is_active != 0)
            .map(|m| (m.target_field, m.block_offset))
            .collect();

        let sheet_notes: Vec<Vec<String>> = comments_by_sheet
            .get("2026")
            .map(|absolute| align_notes_grid(absolute, range.start().unwrap_or((0, 0))))
            .unwrap_or_default();
        let notes_found = grid_has_any_note(&sheet_notes);

        (
            import::parse_rows_with_layout(&rows, &layout, &mappings, &sheet_notes).unwrap(),
            notes_found,
        )
    }

    // --- decode_a1_ref: coluna multi-letra (a planilha real passa de Z) ---

    #[test]
    fn decode_a1_ref_single_and_multi_letter_columns() {
        assert_eq!(decode_a1_ref("A1"), Some((0, 0)));
        assert_eq!(decode_a1_ref("C3"), Some((2, 2)));
        assert_eq!(decode_a1_ref("Z1"), Some((0, 25)));
        assert_eq!(decode_a1_ref("AA1"), Some((0, 26)));
        assert_eq!(decode_a1_ref("AZ16"), Some((15, 51)));
        assert_eq!(decode_a1_ref("BL32"), Some((31, 63)));
        assert_eq!(decode_a1_ref("BP32"), Some((31, 67)));
    }

    #[test]
    fn decode_a1_ref_rejects_malformed_refs() {
        assert_eq!(decode_a1_ref(""), None);
        assert_eq!(decode_a1_ref("1A"), None);
        assert_eq!(decode_a1_ref("A0"), None);
        assert_eq!(decode_a1_ref("A"), None);
        assert_eq!(decode_a1_ref("3"), None);
    }

    // Bem-formada mas ABSURDA: além da grade real do Excel. Sem o bound, `grid.resize(row+1, ..)`
    // pediria dezenas de GB e o allocator ABORTA o processo (não é `Err` capturável).
    #[test]
    fn decode_a1_ref_rejects_refs_outside_excel_grid() {
        // Última célula válida da grade real (XFD1048576) → aceita.
        assert_eq!(
            decode_a1_ref("XFD1048576"),
            Some((XLSX_MAX_ROWS - 1, XLSX_MAX_COLS - 1))
        );
        // Uma linha além do limite → None.
        assert_eq!(decode_a1_ref("A1048577"), None);
        // Uma coluna além do limite (XFE = 16.385) → None.
        assert_eq!(decode_a1_ref("XFE1"), None);
        // Linha bem-formada mas gigantesca (u32::MAX) → None (o cenário do crash).
        assert_eq!(decode_a1_ref("A4294967295"), None);
    }

    // --- parse_comment_notes: entidades (quick-xml 0.41 separa `&nome;` do texto ao redor) ---

    #[test]
    fn parse_comment_notes_resolves_named_and_numeric_entities() {
        let xml = r#"<?xml version="1.0"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <commentList>
    <comment ref="A1"><text><r><t>Mercado &amp; Cia&#10;Total</t></r></text></comment>
  </commentList>
</comments>"#;
        let notes = parse_comment_notes(xml).unwrap();
        assert_eq!(
            notes,
            vec![("A1".to_string(), "Mercado & Cia\nTotal".to_string())]
        );
    }

    #[test]
    fn parse_comment_notes_empty_comment_without_text_element() {
        let xml = r#"<?xml version="1.0"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <commentList>
    <comment ref="B2"/>
  </commentList>
</comments>"#;
        let notes = parse_comment_notes(xml).unwrap();
        assert_eq!(notes, vec![("B2".to_string(), String::new())]);
    }

    // CDATA dentro de `<t>` não pode ser descartado silenciosamente.
    #[test]
    fn parse_comment_notes_reads_cdata_text() {
        let xml = r#"<?xml version="1.0"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <commentList>
    <comment ref="A1"><text><r><t><![CDATA[R$ 10 & taxa]]></t></r></text></comment>
  </commentList>
</comments>"#;
        let notes = parse_comment_notes(xml).unwrap();
        assert_eq!(notes, vec![("A1".to_string(), "R$ 10 & taxa".to_string())]);
    }

    // --- INVARIANTE do plano: byte-identical com o caminho da API (`get_sheet_notes`) ---

    // Uma nota canônica com quebra de linha, `&` (precisa virar `&amp;`) e não-ASCII.
    const CANONICAL_NOTE: &str = "Café & Pão\nR$ 100,00 - Total";

    // A MESMA nota, como um `.xlsx` a codifica em `xl/comments*.xml` (`&`→`&amp;`, `\n`→`&#10;`).
    const CANONICAL_NOTE_XLSX_ENCODED: &str = "Café &amp; Pão&#10;R$ 100,00 - Total";

    /// Extração no estilo do caminho da API do Sheets (`get_sheet_notes`, mod.rs:88-134): a nota é
    /// o campo `c["note"]` puxado como string plana. Modelamos a MESMA forma de JSON e a lemos
    /// EXATAMENTE como o caminho da API faz.
    fn note_from_api_json(json: &serde_json::Value) -> String {
        json["sheets"][0]["data"][0]["rowData"][0]["values"][0]["note"]
            .as_str()
            .unwrap_or("")
            .to_string()
    }

    // Para a MESMA nota, a string do caminho `.xlsx` precisa ser
    // byte-a-byte igual à do caminho da API — senão um reimport cruzado dispara `note_changed`
    // (import.rs:561)/checksum espúrio. Este teste prova a igualdade diretamente.
    #[test]
    fn xlsx_note_string_is_byte_identical_to_get_sheet_notes() {
        // (a) caminho da API: `c["note"]` chega já decodificado (JSON desescapa), verbatim.
        let api_json = serde_json::json!({
            "sheets": [{ "data": [{ "rowData": [
                { "values": [ { "note": CANONICAL_NOTE } ] }
            ]}]}]
        });
        let from_api = note_from_api_json(&api_json);
        assert_eq!(from_api, CANONICAL_NOTE, "sanidade do modelo da API");

        // (b) caminho .xlsx: a MESMA nota codificada em `xl/comments*.xml`, achatada pelo parser.
        let comments_xml = comments_xml_with_ref("C3", CANONICAL_NOTE_XLSX_ENCODED);
        let notes = parse_comment_notes(&comments_xml).unwrap();
        let from_xlsx = &notes[0].1;
        assert_eq!(*from_xlsx, CANONICAL_NOTE, "sanidade do modelo do .xlsx");

        // A igualdade que importa: os dois caminhos produzem os MESMOS bytes.
        assert_eq!(from_api, *from_xlsx);
    }

    // --- align_notes_grid / grid_has_any_note ---

    #[test]
    fn align_notes_grid_shifts_origin_and_drops_cells_outside_range() {
        let absolute = vec![
            vec!["".to_string(), "".to_string(), "".to_string()],
            vec!["".to_string(), "".to_string(), "".to_string()],
            vec!["".to_string(), "".to_string(), "nota".to_string()],
        ];
        // range começa em (1,1) — a nota em (2,2) absoluto deve virar (1,1) alinhado.
        let aligned = align_notes_grid(&absolute, (1, 1));
        assert_eq!(aligned.len(), 2);
        assert_eq!(aligned[1][1], "nota");
    }

    #[test]
    fn grid_has_any_note_detects_non_empty_and_empty() {
        assert!(!grid_has_any_note(&[]));
        assert!(!grid_has_any_note(&[vec![
            "".to_string(),
            "  ".to_string()
        ]]));
        assert!(grid_has_any_note(&[vec!["".to_string(), "x".to_string()]]));
    }

    // --- read_xlsx_comments: fixture .xlsx real (zip) com UMA nota conhecida ---

    #[test]
    fn read_xlsx_comments_finds_known_note_at_right_cell() {
        let path = write_fixture_to_temp(FixtureComments::Notes(
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
        ));

        let result = read_xlsx_comments(&path);
        std::fs::remove_file(&path).unwrap();

        let by_sheet = result.unwrap();
        let grid = by_sheet.get("2026").expect("aba 2026 sem notas");
        // C3 → (row0=2, col0=2) — igual à indexação usada por `cell_raw_note`.
        assert_eq!(grid[2][2], "R$ 100,00 - Parte A\nR$ 50,00 - Parte B");
    }

    #[test]
    fn read_xlsx_comments_sheet_without_rels_has_no_entry() {
        let path = write_fixture_to_temp(FixtureComments::None);

        let result = read_xlsx_comments(&path);
        std::fs::remove_file(&path).unwrap();

        let by_sheet = result.unwrap();
        assert!(!by_sheet.contains_key("2026"));
    }

    #[test]
    fn read_xlsx_comments_errors_on_malformed_comments_xml() {
        let path = write_fixture_to_temp(FixtureComments::Malformed);

        let result = read_xlsx_comments(&path);
        std::fs::remove_file(&path).unwrap();

        assert!(
            result.is_err(),
            "XML malformado deveria propagar erro (caller degrada p/ sem notas)"
        );
    }

    // Ref bem-formada porém fora da grade do Excel: `read_xlsx_comments` NÃO pode alocar GB nem
    // abortar — devolve Ok e simplesmente pula a nota (grade da aba sem nota).
    #[test]
    fn read_xlsx_comments_skips_out_of_grid_ref_without_crash() {
        let path = write_fixture_to_temp(FixtureComments::NoteAtRef {
            cell: "A4294967295",
            note: "boom",
        });

        let result = read_xlsx_comments(&path);
        std::fs::remove_file(&path).unwrap();

        let by_sheet = result.unwrap();
        let grid = by_sheet.get("2026").expect("aba 2026 deveria existir");
        assert!(
            !grid_has_any_note(grid),
            "a nota fora da grade deve ser pulada, não alocada"
        );
    }

    // OPC admite caixa mista no nome da parte: a entrada real do zip é `xl/Comments1.xml` mas o
    // Target do rels é `../comments1.xml` (lowercase). A normalização de caminho tem que casar os
    // dois — senão a nota some silenciosamente.
    #[test]
    fn read_xlsx_comments_resolves_mixed_case_part_name() {
        let path = write_fixture_to_temp_at(
            FixtureComments::Notes("R$ 100,00 - Parte A\nR$ 50,00 - Parte B"),
            "xl/Comments1.xml",
        );

        let result = read_xlsx_comments(&path);
        std::fs::remove_file(&path).unwrap();

        let by_sheet = result.unwrap();
        let grid = by_sheet.get("2026").expect("aba 2026 sem notas");
        assert_eq!(grid[2][2], "R$ 100,00 - Parte A\nR$ 50,00 - Parte B");
    }

    // --- pipeline completo (pool-level, mirror de line_items_stored_when_note_sums_match_total) ---

    #[tokio::test]
    async fn xlsx_with_notes_recovers_description_and_line_items() {
        let path = write_fixture_to_temp(FixtureComments::Notes(
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
        ));
        let (rows, notes_found) = parse_fixture_rows(&path);
        std::fs::remove_file(&path).unwrap();

        assert!(notes_found, "fixture COM notas deveria marcar notes_found");
        let saida = rows
            .iter()
            .find(|r| r.kind == import::RowKind::Saida)
            .expect("linha de Saída não encontrada");
        assert_eq!(saida.date, "2026-01-01");
        assert_eq!(saida.amount, -15_000);
        // Descrição real da nota — NÃO o fallback genérico "Saída {data}".
        assert_eq!(
            saida.description,
            "R$ 100,00 - Parte A · R$ 50,00 - Parte B"
        );
        assert_eq!(saida.raw_note, "R$ 100,00 - Parte A\nR$ 50,00 - Parte B");

        let pool = test_pool().await;
        let options = import::ImportRowsOptions {
            descriptions_trusted: notes_found,
        };
        let count = import::import_rows_with_options(&pool, "2026", &rows, "p1", options)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let txn_id = import::row_id("2026", "2026-01-01", import::RowKind::Saida, 0);
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            2,
            "os 2 itens da nota devem virar line_item"
        );
    }

    #[tokio::test]
    async fn xlsx_without_notes_imports_generic_description_and_no_items() {
        let path = write_fixture_to_temp(FixtureComments::None);
        let (rows, notes_found) = parse_fixture_rows(&path);
        std::fs::remove_file(&path).unwrap();

        assert!(
            !notes_found,
            "fixture SEM notas não deveria marcar notes_found"
        );
        let saida = rows
            .iter()
            .find(|r| r.kind == import::RowKind::Saida)
            .expect("linha de Saída não encontrada");
        // Sem notas, a descrição usa o fallback estrutural determinístico.
        assert_eq!(saida.description, "Saída 2026-01-01");
        assert_eq!(saida.raw_note, "");

        let pool = test_pool().await;
        let options = import::ImportRowsOptions {
            descriptions_trusted: notes_found,
        };
        import::import_rows_with_options(&pool, "2026", &rows, "p1", options)
            .await
            .unwrap();

        let txn_id = import::row_id("2026", "2026-01-01", import::RowKind::Saida, 0);
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            0,
            "sem notas, nenhum line_item deve ser criado"
        );
    }

    #[tokio::test]
    async fn xlsx_with_notes_reimport_is_idempotent() {
        let path = write_fixture_to_temp(FixtureComments::Notes(
            "R$ 100,00 - Parte A\nR$ 50,00 - Parte B",
        ));
        // DUAS leituras INDEPENDENTES do MESMO arquivo em disco: cada uma re-executa
        // `read_xlsx_comments` → `align_notes_grid` → `parse_rows_with_layout`. Assim a igualdade
        // de checksum de fato prova que o CAMINHO NOVO é determinístico (não é SHA sobre um único
        // buffer em memória reusado).
        let (rows_a, notes_a) = parse_fixture_rows(&path);
        let (rows_b, notes_b) = parse_fixture_rows(&path);
        std::fs::remove_file(&path).unwrap();

        let pool = test_pool().await;
        let options = import::ImportRowsOptions {
            descriptions_trusted: notes_a,
        };
        let checksum_a = import::compute_import_checksum(&rows_a, notes_a);
        let checksum_b = import::compute_import_checksum(&rows_b, notes_b);
        assert_eq!(
            checksum_a, checksum_b,
            "duas leituras do disco → mesmo checksum (caminho novo é determinístico)"
        );

        // Reimport = importar o resultado da SEGUNDA leitura de disco por cima da primeira.
        let first = import::import_rows_with_options(&pool, "2026", &rows_a, "p1", options)
            .await
            .unwrap();
        let second = import::import_rows_with_options(&pool, "2026", &rows_b, "p1", options)
            .await
            .unwrap();
        assert_eq!(first, 1, "primeira importação grava a transação");
        assert_eq!(second, 0, "reimport idêntico é no-op (checksum igual)");

        assert_eq!(
            count_transactions(&pool).await,
            1,
            "sem duplicar linha no reimport"
        );
        let txn_id = import::row_id("2026", "2026-01-01", import::RowKind::Saida, 0);
        assert_eq!(
            count_line_items(&pool, &txn_id).await,
            2,
            "itens da nota seguem 2 (sem duplicar)"
        );
    }
}

#[cfg(test)]
mod last_sync_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        // Este teste exercita apenas a query MAX(); dispensa a cadeia de FK
        // person→profile→sync_log (sqlx liga foreign_keys por padrão).
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    async fn insert_sync(pool: &SqlitePool, id: &str, event_type: &str, ts: &str) {
        sqlx::query(
            "INSERT INTO sync_log (id, event_type, entity_type, entity_id, profile_id, timestamp, source_sheet) \
             VALUES (?1, ?2, 'transaction', ?1, 'pr-1', ?3, '2026')",
        )
        .bind(id)
        .bind(event_type)
        .bind(ts)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn returns_most_recent_sync_timestamp() {
        let pool = test_pool().await;
        insert_sync(&pool, "a", "import", "2026-07-04 10:00:00").await;
        insert_sync(&pool, "b", "write_back", "2026-07-04 10:18:00").await;
        // Um evento não-sync não deve mascarar o MAX dos eventos de planilha.
        insert_sync(&pool, "c", "local_edit", "2026-07-04 11:00:00").await;

        let got = last_sync_at_query(&pool).await.unwrap();
        assert_eq!(got.as_deref(), Some("2026-07-04 10:18:00"));
    }

    #[tokio::test]
    async fn returns_none_without_history() {
        let pool = test_pool().await;
        let got = last_sync_at_query(&pool).await.unwrap();
        assert_eq!(got, None);
    }
}
