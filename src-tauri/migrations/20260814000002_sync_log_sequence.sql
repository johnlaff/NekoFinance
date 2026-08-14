-- Âncora de corte por SEQUÊNCIA para a lista de gestos do conflito (ADR-0015, issue #446 D3 do
-- PR #447): o corte anterior comparava `sync_log.timestamp` do OUTRO aparelho contra uma âncora
-- derivada do relógio DESTE — um relógio remoto atrasado escondia gestos recentes dele, e a lista
-- ficava mais estreita que a verdade. `seq` é um contador monotônico gravado NA LINHA (nunca o
-- rowid implícito do SQLite, que `VACUUM INTO` pode renumerar para tabelas sem `INTEGER PRIMARY
-- KEY` — `sync_log.id` é `TEXT`, então o rowid NÃO é estável através do export) — sobrevive
-- intacto ao `VACUUM INTO`/download porque é um valor de COLUNA, não posição física de
-- armazenamento. Os dois aparelhos eram bytes idênticos no momento do último sync (o snapshot
-- inteiro viaja), então `MAX(seq)` naquele instante tem o MESMO significado nos dois lados —
-- comparar contra ele nunca depende de qual relógio está certo.
--
-- Trigger em vez de tocar os ~10 pontos de INSERT espalhados pelo código: nenhum call site precisa
-- saber que `seq` existe, e nenhum pode esquecer de preenchê-lo. `WHEN NEW.seq IS NULL` deixa um
-- valor explícito (só usado em teste) passar sem ser sobrescrito.
ALTER TABLE sync_log ADD COLUMN seq INTEGER;

CREATE TRIGGER IF NOT EXISTS sync_log_assign_seq
AFTER INSERT ON sync_log
WHEN NEW.seq IS NULL
BEGIN
    UPDATE sync_log
    SET seq = (SELECT COALESCE(MAX(seq), 0) + 1 FROM sync_log)
    WHERE rowid = NEW.rowid;
END;

-- Backfill: linhas que já existiam antes desta migration nunca passaram pelo trigger acima.
-- Ordem cronológica (a mesma que o corte por `timestamp` já usava) é a melhor aproximação
-- disponível para dar a elas um `seq` coerente com a ordem real dos gestos passados.
WITH ordered AS (
    SELECT rowid AS rid, ROW_NUMBER() OVER (ORDER BY datetime(timestamp), rowid) AS rn
    FROM sync_log
)
UPDATE sync_log
SET seq = (SELECT rn FROM ordered WHERE ordered.rid = sync_log.rowid)
WHERE seq IS NULL;

-- Sequência-base local (ADR-0015): o `MAX(seq)` do `sync_log` no momento em que `base_sequence`
-- avançou por último (check-in, check-out real, ou adoção da própria sequência) — a âncora única
-- que substitui a aproximação por `MAX(last_checkin_at, last_checkout_at)` (`conflict::base_anchor`,
-- removida). `NULL` = nenhuma base ainda ou `sync_log` vazio naquele momento (nada a excluir).
ALTER TABLE snapshot_state ADD COLUMN base_sync_log_seq INTEGER;
