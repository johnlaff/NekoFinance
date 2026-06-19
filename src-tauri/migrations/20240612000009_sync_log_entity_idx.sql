-- Índice para o diff-delete do import (import.rs): o SELECT de ids existentes por aba
-- (source_sheet + entity_type) e o DELETE por (entity_id + source_sheet + entity_type) faziam
-- full-scan da sync_log a cada re-import. Cobre ambos com um único índice composto.
CREATE INDEX IF NOT EXISTS idx_sync_log_entity
  ON sync_log (source_sheet, entity_type, entity_id);
