-- Key-value de preferências locais do app (ex.: onboarding_done). Local-first, sem PII.
CREATE TABLE IF NOT EXISTS app_setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
