-- Estado LOCAL do lease de convergência: quem este aparelho é, até onde já sincronizou, e
-- quando/por qual aparelho foi o último check-in. Linha única (singleton) — o manifest REMOTO
-- (o que cada aparelho publica) não mora aqui, mora só no `appDataFolder`.
--
-- `last_export_sha256` é o hash do ÚLTIMO snapshot publicado por este aparelho: sem hooks em
-- todo gesto que muda o banco (fora do escopo deste corte), é o jeito honesto de saber se um novo
-- check-in tem algo de fato novo para subir — comparar o hash do export atual contra o último
-- publicado, em vez de sempre assumir que houve mudança.
CREATE TABLE IF NOT EXISTS snapshot_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    device_id TEXT NOT NULL,
    base_sequence INTEGER NOT NULL DEFAULT 0,
    last_checkin_at TEXT,
    last_checkin_device_id TEXT,
    last_export_sha256 TEXT
);
