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

-- `sync_log` TEM deleções em produção (diff-delete do re-import, `google_sheets/import/mod.rs`;
-- delete manual de transação, `commands/transactions.rs`) — um gerador que lesse
-- `MAX(seq) FROM sync_log` a cada insert recuaria quando as linhas de maior `seq` fossem
-- apagadas, e o próximo insert reusaria um número já emitido (invisível para quem ancorou a base
-- num `seq` mais alto que já não existe mais). `sync_log_seq` é só um contador: nunca guarda
-- histórico (o trigger abaixo apaga a única linha antes de cada novo insert), e o que garante que
-- o próximo valor NUNCA repete um já emitido é o `AUTOINCREMENT` do SQLite — o dono real da
-- monotonicidade mora em `sqlite_sequence`, uma tabela de sistema que sobrevive a `DELETE` e
-- viaja junto no `VACUUM INTO` (verificado: um `INTEGER PRIMARY KEY AUTOINCREMENT` esvaziado e
-- exportado continua, no arquivo restaurado, do maior valor já emitido na origem — nunca do que
-- resta nas linhas).
CREATE TABLE IF NOT EXISTS sync_log_seq (
    n INTEGER PRIMARY KEY AUTOINCREMENT
);

CREATE TRIGGER IF NOT EXISTS sync_log_assign_seq
AFTER INSERT ON sync_log
WHEN NEW.seq IS NULL
BEGIN
    DELETE FROM sync_log_seq;
    -- `INSERT ... DEFAULT VALUES` não é aceito dentro de corpo de trigger pelo parser do SQLite
    -- (só fora de trigger); `VALUES (NULL)` produz o MESMO efeito num `INTEGER PRIMARY KEY
    -- AUTOINCREMENT` — o `NULL` explícito ainda aciona a atribuição automática.
    INSERT INTO sync_log_seq (n) VALUES (NULL);
    UPDATE sync_log
    SET seq = (SELECT n FROM sync_log_seq)
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

-- Ancora o gerador no maior `seq` já emitido pelo backfill acima — um INSERT com valor EXPLÍCITO
-- também vira o novo watermark em `sqlite_sequence` (o mesmo mecanismo do trigger), então o
-- PRÓXIMO gesto inserido continua a sequência em vez de recomeçar do 1. `COALESCE(..., 0)` cobre
-- o `sync_log` vazio: o primeiro gesto real, então, começa em 1.
INSERT INTO sync_log_seq (n) SELECT COALESCE(MAX(seq), 0) FROM sync_log;

-- Sequência-base local (ADR-0015): o `MAX(seq)` do `sync_log` no momento em que `base_sequence`
-- avançou por último (check-in, check-out real, ou adoção da própria sequência) — a âncora única
-- que substitui a aproximação por `MAX(last_checkin_at, last_checkout_at)` (`conflict::base_anchor`,
-- removida). `NULL` = nenhuma base ainda ou `sync_log` vazio naquele momento (nada a excluir).
ALTER TABLE snapshot_state ADD COLUMN base_sync_log_seq INTEGER;
