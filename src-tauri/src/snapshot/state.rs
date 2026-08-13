//! Estado LOCAL do lease: quem este aparelho é e até onde já sincronizou. Linha única — o
//! contraponto LOCAL do manifest remoto (`snapshot::manifest`), nunca o mesmo dado.

use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotState {
    pub device_id: String,
    pub base_sequence: i64,
    pub last_checkin_at: Option<String>,
    pub last_checkin_device_id: Option<String>,
    /// Hash (sha256 hex) do ÚLTIMO snapshot exportado por este aparelho — o jeito honesto de
    /// saber se um novo check-in tem algo de fato novo, sem hooks em todo gesto que muda o banco.
    pub last_export_sha256: Option<String>,
    /// Quando este aparelho puxou por último o snapshot remoto (check-out).
    pub last_checkout_at: Option<String>,
    /// `device_id` do manifest remoto BAIXADO no último check-out — de qual aparelho veio o que
    /// este recebeu, nunca a identidade deste aparelho (espelha `last_checkin_device_id`, que é
    /// "por qual aparelho" do lado do check-in).
    pub last_checkout_device_id: Option<String>,
    /// Rótulo fechado do desfecho do ÚLTIMO check-out que mereceu aviso na UI: `None` quando o
    /// check-out foi em dia, restaurou com sucesso, ou nunca rodou — só os dois desfechos que a
    /// UI de Conexão precisa avisar (`"refused_newer_schema"`, `"error"` — os dois de uma tentativa
    /// real de restauração; `"newer_available"` da sonda leve de FOCO, que só avisa sem trocar
    /// arquivo, ver `checkout::probe_newer_snapshot_on_focus`) ficam aqui.
    pub last_checkout_outcome: Option<String>,
    /// Complemento do desfecho acima: versões de schema local/remoto na recusa, ou a mensagem de
    /// erro na falha. Sem significado quando `last_checkout_outcome` é `None`.
    pub last_checkout_outcome_detail: Option<String>,
    /// O hash do export ATUAL difere do último publicado (`last_export_sha256`) — o mesmo sinal
    /// que `drive_checkin_core` calcula a cada tentativa, persistido para a UI de Conexão mostrar
    /// "há mudanças locais ainda não publicadas" sem reexportar o banco a cada render.
    pub pending_local_changes: bool,
    /// Quando uma tentativa de check-in (automática ou manual) descobriu o veredito `Conflict` do
    /// árbitro — `None` quando não há disputa aberta. Gate dos gatilhos automáticos (ADR-0015):
    /// nenhum roda enquanto isto não é `None`, para nunca competir com a escolha do dono na tela
    /// de conflito.
    pub conflict_pending_since: Option<String>,
}

/// Garante que a linha singleton existe, gerando `device_id` (UUID v4) na primeira leitura DESTE
/// aparelho. Idempotente: chamadas seguintes só leem a linha já criada — nunca reemitem o id.
pub async fn load_or_init(pool: &SqlitePool) -> Result<SnapshotState, String> {
    if let Some(row) = sqlx::query_as::<
        _,
        (
            String,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            Option<String>,
        ),
    >(
        "SELECT device_id, base_sequence, last_checkin_at, last_checkin_device_id, \
         last_export_sha256, last_checkout_at, last_checkout_device_id, \
         last_checkout_outcome, last_checkout_outcome_detail, \
         pending_local_changes, conflict_pending_since \
         FROM snapshot_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("ler snapshot_state: {e}"))?
    {
        return Ok(SnapshotState {
            device_id: row.0,
            base_sequence: row.1,
            last_checkin_at: row.2,
            last_checkin_device_id: row.3,
            last_export_sha256: row.4,
            last_checkout_at: row.5,
            last_checkout_device_id: row.6,
            last_checkout_outcome: row.7,
            last_checkout_outcome_detail: row.8,
            pending_local_changes: row.9 != 0,
            conflict_pending_since: row.10,
        });
    }

    let device_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO snapshot_state (id, device_id, base_sequence) VALUES (1, ?1, 0)")
        .bind(&device_id)
        .execute(pool)
        .await
        .map_err(|e| format!("inicializar snapshot_state: {e}"))?;

    Ok(SnapshotState {
        device_id,
        base_sequence: 0,
        last_checkin_at: None,
        last_checkin_device_id: None,
        last_export_sha256: None,
        last_checkout_at: None,
        last_checkout_device_id: None,
        last_checkout_outcome: None,
        last_checkout_outcome_detail: None,
        pending_local_changes: false,
        conflict_pending_since: None,
    })
}

/// Grava o resultado de um check-in bem-sucedido: a NOVA sequência-base (o que acabamos de
/// publicar, já confirmada no manifest remoto), quando/por qual aparelho, e o hash do export
/// publicado (para o PRÓXIMO check-in saber se algo mudou de verdade). Uma publicação bem-sucedida
/// também limpa `pending_local_changes` (o que era pendente acabou de subir) e
/// `conflict_pending_since` (chegar até aqui exige o veredito `Push`, nunca `Conflict` — qualquer
/// disputa anterior já não se sustenta).
pub async fn record_checkin(
    pool: &SqlitePool,
    new_base_sequence: i64,
    checked_in_at: &str,
    device_id: &str,
    export_sha256: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE snapshot_state \
         SET base_sequence = ?1, last_checkin_at = ?2, last_checkin_device_id = ?3, \
         last_export_sha256 = ?4, pending_local_changes = 0, conflict_pending_since = NULL \
         WHERE id = 1",
    )
    .bind(new_base_sequence)
    .bind(checked_in_at)
    .bind(device_id)
    .bind(export_sha256)
    .execute(pool)
    .await
    .map_err(|e| format!("gravar check-in: {e}"))?;
    Ok(())
}

/// Grava se o hash do export ATUAL difere do último publicado — chamado a cada tentativa de
/// check-in (automática ou manual), sucesso ou falha, ANTES de a tentativa decidir se publica. Uma
/// falha ao publicar (rede fora do ar, `Pull`, `Conflict`) deixa a flag em `true`: a UI de Conexão
/// precisa mostrar "não publicado" mesmo quando a tentativa não deu certo — só uma publicação de
/// fato limpa isto (`record_checkin`).
pub async fn record_pending_local_changes(pool: &SqlitePool, pending: bool) -> Result<(), String> {
    sqlx::query("UPDATE snapshot_state SET pending_local_changes = ?1 WHERE id = 1")
        .bind(pending as i64)
        .execute(pool)
        .await
        .map_err(|e| format!("gravar mudanças pendentes: {e}"))?;
    Ok(())
}

/// Grava (ou limpa, com `None`) o carimbo de quando uma disputa `Conflict` foi descoberta. Gate
/// dos gatilhos automáticos (ADR-0015): eles leem este campo via
/// [`SnapshotState::conflict_pending_since`] e não tentam nada enquanto não é `None`.
pub async fn record_conflict_pending(pool: &SqlitePool, since: Option<&str>) -> Result<(), String> {
    sqlx::query("UPDATE snapshot_state SET conflict_pending_since = ?1 WHERE id = 1")
        .bind(since)
        .execute(pool)
        .await
        .map_err(|e| format!("gravar conflito pendente: {e}"))?;
    Ok(())
}

/// Semeia a linha singleton no banco RECÉM-RESTAURADO (o arquivo baixado do Drive, cujo
/// `snapshot_state` veio VAZIO — `strip_from_export_copy` apaga a linha antes de qualquer
/// publicação). `device_id` é o identificador que ESTE aparelho já tinha ANTES da troca de
/// arquivo — precisa ser capturado pelo chamador antes do swap e passado aqui, nunca gerado de
/// novo: a identidade do aparelho é bookkeeping local, não dado que viaja no snapshot.
///
/// `last_checkin_at`/`last_checkin_device_id` são o MESMO tipo de bookkeeping local: o histórico
/// de check-in deste aparelho não veio no arquivo baixado (foi apagado do lado de quem publicou),
/// então precisa ser capturado e re-semeado junto do `device_id` — sem isso, um aparelho que já
/// publicou perde o próprio histórico a cada check-out e a UI volta a dizer "nenhum check-in
/// ainda" para quem já fez um.
///
/// `restored_export_sha256` é o hash do conteúdo RECÉM-restaurado (os bytes baixados, já sem
/// `snapshot_state` — a mesma forma que um export produziria) e grava-se em `last_export_sha256`
/// nos DOIS ramos (ao contrário de `last_checkin_at`/`last_checkin_device_id`, que só valem para
/// semear uma linha nova): o conteúdo ativo acabou de virar o do remoto, então o PRÓXIMO check-in
/// precisa comparar contra ELE, nunca contra o hash de uma publicação anterior deste aparelho.
/// Sem isto, `drive_checkin_core` sempre lia "mudou" logo depois de toda restauração (o hash local
/// ficava `NULL`) e republicava um conteúdo idêntico ao que acabou de baixar.
///
/// `ON CONFLICT` (em vez de `INSERT` cru) porque o arquivo baixado, embora normalmente stripped,
/// não é sob controle deste aparelho — um upsert idempotente é mais seguro que assumir a tabela
/// vazia. Quando a linha JÁ existe (a troca não encontrou uma tabela vazia), o `SET` não toca
/// `last_checkin_at`/`last_checkin_device_id`: os valores capturados só valem para SEMEAR uma
/// linha nova, nunca para sobrescrever um histórico de check-in que já estava ali.
///
/// `pending_local_changes`/`conflict_pending_since` SEMPRE voltam ao estado limpo (0/`NULL`) nos
/// dois ramos: o conteúdo ativo acabou de ser TROCADO pelo do remoto, então qualquer diff ou
/// disputa registrada antes da troca se refere a um conteúdo que não existe mais.
#[allow(clippy::too_many_arguments)]
pub async fn adopt_after_restore(
    pool: &SqlitePool,
    device_id: &str,
    base_sequence: i64,
    checked_out_at: &str,
    remote_device_id: &str,
    last_checkin_at: Option<&str>,
    last_checkin_device_id: Option<&str>,
    restored_export_sha256: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO snapshot_state \
            (id, device_id, base_sequence, last_checkin_at, last_checkin_device_id, \
             last_export_sha256, last_checkout_at, last_checkout_device_id, pending_local_changes, \
             conflict_pending_since) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL) \
         ON CONFLICT (id) DO UPDATE SET \
            device_id = excluded.device_id, \
            base_sequence = excluded.base_sequence, \
            last_export_sha256 = excluded.last_export_sha256, \
            last_checkout_at = excluded.last_checkout_at, \
            last_checkout_device_id = excluded.last_checkout_device_id, \
            pending_local_changes = 0, \
            conflict_pending_since = NULL",
    )
    .bind(device_id)
    .bind(base_sequence)
    .bind(last_checkin_at)
    .bind(last_checkin_device_id)
    .bind(restored_export_sha256)
    .bind(checked_out_at)
    .bind(remote_device_id)
    .execute(pool)
    .await
    .map_err(|e| format!("adotar estado pós-restauração: {e}"))?;
    Ok(())
}

/// Avança só a sequência-base local, sem tocar em mais nada — o caso em que o manifest remoto
/// carrega o NOSSO PRÓPRIO `device_id` (ADR-0015: um check-in que morreu entre o upload
/// confirmado e a gravação do estado local deixa o remoto um passo à frente da nossa base, com a
/// NOSSA identidade). O conteúdo já é nosso — só a base local está atrasada;
/// restaurar de verdade descartaria qualquer trabalho feito depois do upload. Nunca mexe em
/// `last_checkout_at`/`last_checkout_device_id`: nada foi de fato lido de outro aparelho.
pub async fn adopt_own_sequence(pool: &SqlitePool, base_sequence: i64) -> Result<(), String> {
    sqlx::query("UPDATE snapshot_state SET base_sequence = ?1 WHERE id = 1")
        .bind(base_sequence)
        .execute(pool)
        .await
        .map_err(|e| format!("adotar sequência do próprio aparelho: {e}"))?;
    Ok(())
}

/// Grava o desfecho do último check-out que merece aviso na UI (ADR-0015):
/// `outcome: None` limpa qualquer aviso pendente (o check-out mais recente foi em dia, restaurou
/// com sucesso, ou apenas adotou a própria sequência) — um aviso de uma tentativa ANTERIOR não
/// pode sobreviver a um check-out bem-sucedido subsequente.
pub async fn record_checkout_outcome(
    pool: &SqlitePool,
    outcome: Option<&str>,
    detail: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE snapshot_state SET last_checkout_outcome = ?1, last_checkout_outcome_detail = ?2 \
         WHERE id = 1",
    )
    .bind(outcome)
    .bind(detail)
    .execute(pool)
    .await
    .map_err(|e| format!("gravar desfecho do check-out: {e}"))?;
    Ok(())
}

/// Apaga a linha do `snapshot_state` de uma CÓPIA já exportada (nunca do banco ativo) antes de a
/// cópia virar o snapshot publicado no Drive. Duas razões, uma decisão: `device_id`/`base_sequence`
/// são identidade e progresso DESTE aparelho — se viajassem no snapshot, um restore futuro
/// sobrescreveria a identidade de quem restaura com a de quem publicou; e como esta é a única
/// linha que muda a cada check-in bem-sucedido, deixá-la dentro faria o hash do export nunca se
/// repetir, e "em dia" (nenhuma mudança de domínio) nunca seria alcançável na prática.
///
/// O `VACUUM` depois do `DELETE` não é cosmético: sem ele, o layout físico de página da cópia
/// ainda carrega o tamanho que a linha tinha no banco ATIVO (um hash de 64 caracteres ocupa mais
/// espaço que a linha recém-criada com `base_sequence=0`), e dois exports do mesmo conteúdo
/// lógico sairiam com bytes diferentes — o hash nunca bateria, e "em dia" nunca seria alcançado.
pub(crate) async fn strip_from_export_copy(db_path: &std::path::Path) -> Result<(), String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
        .map_err(|e| format!("abrir cópia exportada: {e}"))?;
    let copy_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| format!("conectar à cópia exportada: {e}"))?;
    let result: Result<(), String> = async {
        sqlx::query("DELETE FROM snapshot_state")
            .execute(&copy_pool)
            .await
            .map_err(|e| format!("limpar estado local da cópia exportada: {e}"))?;
        sqlx::raw_sql(sqlx::AssertSqlSafe("VACUUM".to_string()))
            .execute(&copy_pool)
            .await
            .map_err(|e| format!("normalizar layout da cópia exportada: {e}"))?;
        Ok(())
    }
    .await;
    copy_pool.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::time::Duration;

    /// Pool de UMA conexão — o MESMO perfil de produção (`lib.rs`). Um pool default (múltiplas
    /// conexões) nunca pegaria a classe de regressão de deadlock testada abaixo: com mais de uma
    /// conexão livre, a leitura simplesmente usaria outra e o bug ficaria invisível no teste.
    async fn single_connection_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool SQLite em memória");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações");
        pool
    }

    #[tokio::test]
    async fn load_or_init_creates_the_singleton_row_once_and_is_idempotent() {
        let pool = single_connection_pool().await;

        let first = load_or_init(&pool).await.expect("primeira leitura");
        assert_eq!(first.base_sequence, 0);
        assert!(first.last_checkin_at.is_none());
        assert!(!first.device_id.is_empty());

        let second = load_or_init(&pool).await.expect("segunda leitura");
        assert_eq!(
            second.device_id, first.device_id,
            "device_id não pode trocar entre leituras"
        );
    }

    #[tokio::test]
    async fn record_checkin_updates_base_sequence_who_when_and_export_hash() {
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        assert!(initial.last_export_sha256.is_none());

        record_checkin(
            &pool,
            3,
            "2026-08-11 12:00:00",
            &initial.device_id,
            "deadbeef",
        )
        .await
        .expect("record_checkin");

        let after = load_or_init(&pool).await.expect("releitura");
        assert_eq!(after.base_sequence, 3);
        assert_eq!(
            after.last_checkin_at.as_deref(),
            Some("2026-08-11 12:00:00")
        );
        assert_eq!(
            after.last_checkin_device_id.as_deref(),
            Some(initial.device_id.as_str())
        );
        assert_eq!(after.last_export_sha256.as_deref(), Some("deadbeef"));
    }

    #[tokio::test]
    async fn adopt_after_restore_leaves_preexisting_checkin_fields_untouched() {
        // O upsert de `adopt_after_restore` escreve device_id/base_sequence/check-out
        // incondicionalmente, mas quando a linha JÁ existe (a restauração aconteceu num banco com
        // histórico de check-in próprio, não recém-criado), o `ON CONFLICT` NÃO tem
        // `last_checkin_at`/`last_checkin_device_id` no `SET` — os argumentos capturados abaixo
        // são propositalmente valores que NÃO deveriam aparecer no resultado, para provar que são
        // ignorados neste ramo (só valem para semear uma linha nova, nunca para sobrescrever um
        // histórico de check-in que já estava ali).
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        record_checkin(
            &pool,
            1,
            "2026-08-11 09:00:00",
            &initial.device_id,
            "seed-hash",
        )
        .await
        .expect("record_checkin");

        adopt_after_restore(
            &pool,
            &initial.device_id,
            4,
            "2026-08-12 08:00:00",
            "outro-aparelho",
            Some("captured-mas-deve-ser-ignorado"),
            Some("captured-device-mas-deve-ser-ignorado"),
            "hash-do-conteudo-recem-restaurado",
        )
        .await
        .expect("adopt_after_restore");

        let after = load_or_init(&pool).await.expect("releitura");
        assert_eq!(
            after.base_sequence, 4,
            "base avança para a sequência puxada"
        );
        assert_eq!(
            after.last_checkout_at.as_deref(),
            Some("2026-08-12 08:00:00")
        );
        assert_eq!(
            after.last_checkout_device_id.as_deref(),
            Some("outro-aparelho")
        );
        // Check-in não muda: check-out é um eixo independente do mesmo estado, e o histórico
        // PRÉ-EXISTENTE vence os argumentos capturados passados acima.
        assert_eq!(after.device_id, initial.device_id);
        assert_eq!(
            after.last_checkin_at.as_deref(),
            Some("2026-08-11 09:00:00")
        );
        assert_eq!(
            after.last_checkin_device_id.as_deref(),
            Some(initial.device_id.as_str())
        );
        // Ao contrário do check-in, o hash do export MUDA mesmo neste ramo: o conteúdo ativo
        // acabou de virar o do remoto, então "seed-hash" (de uma publicação de ANTES da troca) não
        // descreve mais o que está no disco.
        assert_eq!(
            after.last_export_sha256.as_deref(),
            Some("hash-do-conteudo-recem-restaurado")
        );
    }

    #[tokio::test]
    async fn adopt_after_restore_preserves_this_devices_own_checkin_history_across_the_swap() {
        // O arquivo baixado do Drive chega com `snapshot_state` VAZIO (stripped do lado de quem
        // publicou) — mas o histórico de check-in que se perde não é do arquivo baixado, é DESTE
        // aparelho, capturado ANTES da troca. Simula exatamente isso: semeia um check-in real
        // numa "pool de origem" (o aparelho antes da troca), lê de volta (o que o chamador real
        // faz em `checkout.rs` antes de fechar o pool antigo), e semeia a linha singleton no
        // destino (a pool vazia, o arquivo recém-baixado) com os valores capturados.
        let source_pool = single_connection_pool().await;
        let source_state = load_or_init(&source_pool).await.expect("init da origem");
        record_checkin(
            &source_pool,
            2,
            "2026-08-10 09:00:00",
            &source_state.device_id,
            "hash-antes-da-troca",
        )
        .await
        .expect("record_checkin na origem");
        let captured = load_or_init(&source_pool)
            .await
            .expect("captura antes de fechar a pool antiga");

        let destination_pool = single_connection_pool().await;
        // Simula o banco RECÉM-RESTAURADO: `snapshot_state` chega vazio.
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snapshot_state")
            .fetch_one(&destination_pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        adopt_after_restore(
            &destination_pool,
            &captured.device_id,
            7,
            "2026-08-12 08:00:00",
            "device-que-publicou",
            captured.last_checkin_at.as_deref(),
            captured.last_checkin_device_id.as_deref(),
            "hash-do-conteudo-baixado",
        )
        .await
        .expect("adopt_after_restore");

        let after = load_or_init(&destination_pool).await.expect("releitura");
        assert_eq!(
            after.device_id, captured.device_id,
            "a identidade deste aparelho sobrevive à troca de arquivo — nunca é regerada"
        );
        assert_eq!(after.base_sequence, 7);
        assert_eq!(
            after.last_checkout_at.as_deref(),
            Some("2026-08-12 08:00:00")
        );
        assert_eq!(
            after.last_checkout_device_id.as_deref(),
            Some("device-que-publicou")
        );
        // O próximo check-in precisa comparar contra o hash do conteúdo QUE ACABOU DE CHEGAR, não
        // contra "hash-antes-da-troca" (a publicação deste aparelho de ANTES da restauração) — senão
        // republicaria à toa um conteúdo idêntico ao que acabou de baixar.
        assert_eq!(
            after.last_export_sha256.as_deref(),
            Some("hash-do-conteudo-baixado")
        );
        // O histórico de check-in DESTE aparelho, capturado antes da troca, sobrevive — a tela
        // não pode voltar a dizer "nenhum check-in ainda" para um aparelho que já publicou.
        assert_eq!(
            after.last_checkin_at.as_deref(),
            Some("2026-08-10 09:00:00"),
            "o check-out apagava o histórico de check-in deste aparelho — precisa sobreviver"
        );
        assert_eq!(
            after.last_checkin_device_id.as_deref(),
            Some(captured.device_id.as_str())
        );
    }

    #[tokio::test]
    async fn adopt_after_restore_seeds_the_singleton_row_with_the_captured_device_id() {
        let pool = single_connection_pool().await;
        // Simula o banco RECÉM-RESTAURADO: `snapshot_state` chega vazio (stripped antes da
        // publicação pelo aparelho de origem) — nenhuma linha ainda.
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snapshot_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        let captured_device_id = "device-que-ja-existia-antes-da-troca";
        adopt_after_restore(
            &pool,
            captured_device_id,
            7,
            "2026-08-12 08:00:00",
            "device-que-publicou",
            None,
            None,
            "hash-do-conteudo-baixado",
        )
        .await
        .expect("adopt_after_restore");

        let after = load_or_init(&pool).await.expect("releitura");
        assert_eq!(
            after.device_id, captured_device_id,
            "a identidade deste aparelho sobrevive à troca de arquivo — nunca é regerada"
        );
        assert_eq!(after.base_sequence, 7);
        assert_eq!(
            after.last_checkout_at.as_deref(),
            Some("2026-08-12 08:00:00")
        );
        assert_eq!(
            after.last_checkout_device_id.as_deref(),
            Some("device-que-publicou")
        );
        // Sem check-in prévio capturado (aparelho novo, nunca publicou): não há o que preservar.
        assert!(after.last_checkin_at.is_none());
        assert_eq!(
            after.last_export_sha256.as_deref(),
            Some("hash-do-conteudo-baixado")
        );
    }

    #[tokio::test]
    async fn adopt_after_restore_is_idempotent_when_a_row_already_exists() {
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");

        adopt_after_restore(
            &pool,
            &initial.device_id,
            2,
            "2026-08-12 08:00:00",
            "device-que-publicou",
            None,
            None,
            "hash-primeira-adocao",
        )
        .await
        .expect("primeira adoção");
        adopt_after_restore(
            &pool,
            &initial.device_id,
            5,
            "2026-08-12 09:00:00",
            "device-que-publicou-de-novo",
            None,
            None,
            "hash-segunda-adocao",
        )
        .await
        .expect("segunda adoção não deve falhar por PK duplicada");

        let after = load_or_init(&pool).await.expect("releitura");
        assert_eq!(after.base_sequence, 5);
        assert_eq!(
            after.last_checkout_device_id.as_deref(),
            Some("device-que-publicou-de-novo")
        );
        assert_eq!(
            after.last_export_sha256.as_deref(),
            Some("hash-segunda-adocao"),
            "cada adoção sobrescreve o hash com o do conteúdo QUE ELA restaurou"
        );
    }

    #[tokio::test]
    async fn adopt_own_sequence_advances_base_without_touching_checkout_bookkeeping() {
        // O manifest remoto tem o NOSSO device_id (um check-in que morreu entre o upload e a
        // gravação local) — a base local avança para alcançar o remoto, mas nada foi de fato
        // LIDO de outro aparelho: `last_checkout_at`/`last_checkout_device_id` continuam como
        // estavam antes.
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        record_checkin(
            &pool,
            1,
            "2026-08-11 09:00:00",
            &initial.device_id,
            "hash-1",
        )
        .await
        .expect("record_checkin");

        adopt_own_sequence(&pool, 2)
            .await
            .expect("adopt_own_sequence");

        let after = load_or_init(&pool).await.expect("releitura");
        assert_eq!(after.base_sequence, 2);
        assert!(
            after.last_checkout_at.is_none(),
            "nada foi lido de outro aparelho — o eixo de check-out não deve mudar"
        );
        // Check-in preservado: eixo independente.
        assert_eq!(
            after.last_checkin_at.as_deref(),
            Some("2026-08-11 09:00:00")
        );
    }

    #[tokio::test]
    async fn record_pending_local_changes_round_trips_and_defaults_to_false() {
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        assert!(
            !initial.pending_local_changes,
            "estado inicial: nada pendente"
        );

        record_pending_local_changes(&pool, true)
            .await
            .expect("gravar pendente");
        assert!(load_or_init(&pool).await.unwrap().pending_local_changes);

        record_pending_local_changes(&pool, false)
            .await
            .expect("limpar pendente");
        assert!(!load_or_init(&pool).await.unwrap().pending_local_changes);
    }

    #[tokio::test]
    async fn record_conflict_pending_round_trips_and_defaults_to_none() {
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        assert!(initial.conflict_pending_since.is_none());

        record_conflict_pending(&pool, Some("2026-08-13T10:00:00Z"))
            .await
            .expect("gravar conflito pendente");
        assert_eq!(
            load_or_init(&pool)
                .await
                .unwrap()
                .conflict_pending_since
                .as_deref(),
            Some("2026-08-13T10:00:00Z")
        );

        record_conflict_pending(&pool, None)
            .await
            .expect("limpar conflito pendente");
        assert!(
            load_or_init(&pool)
                .await
                .unwrap()
                .conflict_pending_since
                .is_none()
        );
    }

    #[tokio::test]
    async fn record_checkin_clears_pending_local_changes_and_conflict_pending() {
        let pool = single_connection_pool().await;
        let initial = load_or_init(&pool).await.expect("init");
        record_pending_local_changes(&pool, true).await.unwrap();
        record_conflict_pending(&pool, Some("2026-08-13T10:00:00Z"))
            .await
            .unwrap();

        record_checkin(
            &pool,
            1,
            "2026-08-13T11:00:00Z",
            &initial.device_id,
            "hash-publicado",
        )
        .await
        .expect("record_checkin");

        let after = load_or_init(&pool).await.unwrap();
        assert!(
            !after.pending_local_changes,
            "publicar com sucesso limpa a flag — o que era pendente acabou de subir"
        );
        assert!(
            after.conflict_pending_since.is_none(),
            "chegar a um check-in publicado exige veredito Push, não Conflict"
        );
    }

    #[tokio::test]
    async fn adopt_after_restore_clears_pending_local_changes_and_conflict_pending_on_both_branches()
     {
        // Ramo 1: linha ainda não existe (INSERT puro) — semear já nasce limpo.
        let pool_insert = single_connection_pool().await;
        adopt_after_restore(
            &pool_insert,
            "device-a",
            3,
            "2026-08-13T09:00:00Z",
            "device-b",
            None,
            None,
            "hash-insert",
        )
        .await
        .expect("adopt_after_restore (INSERT)");
        let after_insert = load_or_init(&pool_insert).await.unwrap();
        assert!(!after_insert.pending_local_changes);
        assert!(after_insert.conflict_pending_since.is_none());

        // Ramo 2: linha já existe com pendências de ANTES da troca — o conteúdo ativo acabou de
        // ser substituído pelo remoto, então nada do que valia antes se sustenta.
        let pool_update = single_connection_pool().await;
        load_or_init(&pool_update).await.unwrap();
        record_pending_local_changes(&pool_update, true)
            .await
            .unwrap();
        record_conflict_pending(&pool_update, Some("2026-08-13T08:00:00Z"))
            .await
            .unwrap();

        adopt_after_restore(
            &pool_update,
            "device-a",
            4,
            "2026-08-13T09:30:00Z",
            "device-b",
            None,
            None,
            "hash-update",
        )
        .await
        .expect("adopt_after_restore (UPDATE)");
        let after_update = load_or_init(&pool_update).await.unwrap();
        assert!(
            !after_update.pending_local_changes,
            "o conteúdo ativo virou o do remoto — o diff de antes não existe mais"
        );
        assert!(
            after_update.conflict_pending_since.is_none(),
            "a disputa era sobre um conteúdo local que acabou de ser substituído"
        );
    }

    #[tokio::test]
    async fn record_checkout_outcome_writes_and_then_clears_the_pending_warning() {
        let pool = single_connection_pool().await;
        load_or_init(&pool).await.expect("init");

        record_checkout_outcome(&pool, Some("refused_newer_schema"), Some("1:2"))
            .await
            .expect("gravar desfecho");
        let after_warning = load_or_init(&pool).await.expect("releitura");
        assert_eq!(
            after_warning.last_checkout_outcome.as_deref(),
            Some("refused_newer_schema")
        );
        assert_eq!(
            after_warning.last_checkout_outcome_detail.as_deref(),
            Some("1:2")
        );

        // Um check-out bem-sucedido subsequente limpa o aviso — ele não pode sobreviver.
        record_checkout_outcome(&pool, None, None)
            .await
            .expect("limpar desfecho");
        let after_clear = load_or_init(&pool).await.expect("releitura");
        assert!(after_clear.last_checkout_outcome.is_none());
        assert!(after_clear.last_checkout_outcome_detail.is_none());
    }

    #[tokio::test]
    async fn strip_from_export_copy_empties_the_table_without_touching_the_live_db() {
        use std::str::FromStr;
        let dir = std::env::temp_dir().join(format!("neko-strip-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("copy.db");

        // Exporta uma cópia de um pool com a linha singleton já criada (device_id real).
        let src_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::from_str(&format!(
                    "sqlite:{}",
                    dir.join("src.db").display()
                ))
                .unwrap()
                .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&src_pool).await.unwrap();
        load_or_init(&src_pool)
            .await
            .expect("cria a linha singleton");
        crate::commands::db_export::vacuum_into_atomic(&src_pool, &db_path)
            .await
            .expect("exportar cópia");

        strip_from_export_copy(&db_path)
            .await
            .expect("limpar a cópia");

        let copy_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path.display()))
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshot_state")
            .fetch_one(&copy_pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "a CÓPIA deve ficar sem a linha de estado local");

        // O banco ATIVO continua com a linha intacta — só a cópia foi limpa.
        let live_state = load_or_init(&src_pool)
            .await
            .expect("releitura do banco ativo");
        assert!(!live_state.device_id.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn load_or_init_queues_behind_an_open_write_transaction_instead_of_deadlocking() {
        // Reproduz a classe de deadlock já documentada no repositório: com pool de 1 conexão,
        // ler enquanto uma escrita mantém uma transação aberta precisa ENFILEIRAR (e completar
        // assim que a tx solta a conexão) — nunca travar para sempre.
        let pool = single_connection_pool().await;
        load_or_init(&pool)
            .await
            .expect("garante a linha singleton antes do teste");

        let mut tx = pool.begin().await.expect("abrir transação de escrita");
        sqlx::query("UPDATE snapshot_state SET base_sequence = base_sequence WHERE id = 1")
            .execute(&mut *tx)
            .await
            .expect("escrita dentro da transação");

        let pool_for_read = pool.clone();
        let read = tokio::spawn(async move { load_or_init(&pool_for_read).await });

        // Dá tempo da leitura tentar adquirir a conexão (e ficar na fila do pool) ANTES de soltar
        // a transação — sem essa espera o teste não exercitaria a contenção de verdade.
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.commit().await.expect("commit da transação");

        let result = tokio::time::timeout(Duration::from_secs(5), read)
            .await
            .expect("a leitura NÃO pode travar para sempre esperando a única conexão")
            .expect("a task de leitura não deve entrar em panic");
        assert!(result.is_ok());
    }
}
