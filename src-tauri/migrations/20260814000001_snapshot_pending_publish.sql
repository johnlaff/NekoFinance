-- Sequência PRETENDIDA de uma publicação em andamento (ADR-0015, issue #446 item 11): gravada
-- ANTES do upload para o Drive e limpa quando `record_checkin` confirma que a gravação local
-- terminou. Cobre o "check-in morto" (upload confirmado, gravação local que morreu antes de
-- terminar) para QUALQUER publicação — check-in normal (candidato sempre `base + 1`) e resolução
-- de conflito mantendo este aparelho (`resolve_conflict_keep_local_core`, candidato
-- `max(base + 1, remote + 1)`, que pode passar de `base + 1`). A guarda do próprio `device_id` em
-- `checkout::checkout_on_open` compara o manifest remoto contra ESTE valor em vez de inferir a
-- janela por aritmética (`base + 1`), que só cobria a primeira publicação.
ALTER TABLE snapshot_state ADD COLUMN pending_publish_sequence INTEGER;
