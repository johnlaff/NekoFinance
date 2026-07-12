-- Cada alvo pode ter no máximo um override por cenário. A deduplicação preserva a primeira
-- linha inserida no SQLite; os índices tornam a invariável segura também sob concorrência.
DELETE FROM scenario_override
WHERE obligation_id IS NOT NULL
  AND rowid NOT IN (
    SELECT MIN(rowid)
    FROM scenario_override
    WHERE obligation_id IS NOT NULL
    GROUP BY scenario_id, obligation_id
  );

DELETE FROM scenario_override
WHERE recurrence_id IS NOT NULL
  AND rowid NOT IN (
    SELECT MIN(rowid)
    FROM scenario_override
    WHERE recurrence_id IS NOT NULL
    GROUP BY scenario_id, recurrence_id
  );

CREATE UNIQUE INDEX ux_scenario_override_scenario_obligation
ON scenario_override (scenario_id, obligation_id)
WHERE obligation_id IS NOT NULL;

CREATE UNIQUE INDEX ux_scenario_override_scenario_recurrence
ON scenario_override (scenario_id, recurrence_id)
WHERE recurrence_id IS NOT NULL;
