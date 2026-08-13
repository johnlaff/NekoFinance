-- Desfecho do ÚLTIMO check-out (ADR-0015, spec 043 US11): quando a restauração é recusada por
-- schema remoto mais novo, ou falha por rede/integridade, o dono precisa de um aviso na tela de
-- Conexão em vez de silêncio (o `eprintln!` de antes só chegava ao log). `last_checkout_outcome`
-- é um rótulo fechado ("refused_newer_schema" | "error"); NULL significa "nada a avisar" (o
-- check-out foi em dia, restaurou com sucesso, ou nunca rodou). `last_checkout_outcome_detail`
-- carrega o complemento (versões de schema, ou a mensagem de erro) que a copy usa.
ALTER TABLE snapshot_state ADD COLUMN last_checkout_outcome TEXT;
ALTER TABLE snapshot_state ADD COLUMN last_checkout_outcome_detail TEXT;
