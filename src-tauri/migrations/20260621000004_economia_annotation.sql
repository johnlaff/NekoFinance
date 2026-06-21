-- Plan 052: Economia-tab values are a metric annotation (% = Economia/Entradas),
-- not a money movement. Storing them in `transaction` caused a Saldo double-count
-- when the same savings was already a grid Saída. This table holds the raw
-- annotation values; it does NOT affect the Saldo chain.
CREATE TABLE IF NOT EXISTS economia_annotation (
    profile_id   TEXT    NOT NULL DEFAULT '',
    year         INTEGER NOT NULL,
    month        INTEGER NOT NULL CHECK (month BETWEEN 1 AND 12),
    amount_cents INTEGER NOT NULL CHECK (amount_cents >= 0),
    updated_at   TEXT    NOT NULL,
    PRIMARY KEY (profile_id, year, month)
);
