-- Estado LOCAL que os gatilhos automáticos de check-in/check-out precisam para decidir sem rede
-- (ADR-0015, spec 043 US13/AC #427):
--
-- `pending_local_changes` reflete o MESMO sinal que `drive_checkin_core` já calcula a cada
-- tentativa (o hash do export atual difere do último publicado, `last_export_sha256`) — persistido
-- para a UI mostrar "há mudanças locais ainda não publicadas" sem reexportar o banco a cada render.
-- 0/1 (SQLite não tem BOOLEAN nativo); 0 é o estado inicial (nada publicado, nada pendente).
--
-- `conflict_pending_since` grava QUANDO uma tentativa de check-in (automática ou manual) descobriu
-- o veredito `Conflict` do árbitro — NULL quando não há disputa aberta. Gate dos gatilhos
-- automáticos (foco/gesto material/fechar): nenhum deles tenta de novo enquanto isto não é NULL,
-- para não competir com a escolha do dono na tela de conflito. Limpo por qualquer resolução
-- (`resolve_drive_conflict`) ou por um check-in que não bate mais em Conflict.
ALTER TABLE snapshot_state ADD COLUMN pending_local_changes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE snapshot_state ADD COLUMN conflict_pending_since TEXT;
