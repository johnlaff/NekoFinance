-- Spec 007: pockets & liquidity. SQLite cannot alter a CHECK constraint, so the
-- table is rebuilt in place (no indexes exist on account; child tables keep
-- referencing "account" by name). FK enforcement is ON (sqlx enables PRAGMA
-- foreign_keys by default; the app pool now also sets it explicitly): the rebuild
-- runs inside the migration transaction and the DROP/RENAME swaps the table so
-- child FKs resolve by name to the renamed table.
CREATE TABLE account_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('bank','credit_card','wallet','savings','business','meal_voucher','pension','fgts')),
    owner_person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    institution TEXT,
    balance INTEGER NOT NULL DEFAULT 0,
    credit_limit INTEGER,
    closing_day INTEGER,
    due_day INTEGER,
    linked_account_id TEXT REFERENCES account(id),
    provider TEXT,
    liquidity TEXT CHECK(liquidity IN ('liquid','reserve','restricted','illiquid')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO account_new (id, name, type, owner_person_id, institution, balance, credit_limit, closing_day, due_day, linked_account_id, provider, created_at, liquidity)
SELECT id, name, type, owner_person_id, institution, balance, credit_limit, closing_day, due_day, linked_account_id, provider, created_at,
    CASE type
        WHEN 'savings' THEN 'reserve'
        WHEN 'credit_card' THEN NULL
        ELSE 'liquid'
    END
FROM account;

DROP TABLE account;
ALTER TABLE account_new RENAME TO account;

-- Boundary default: inserts that do not classify liquidity get it derived from
-- the type, so no account ever sits unclassified (credit_card stays NULL — the
-- invoice is a liability, not a pocket).
CREATE TRIGGER account_liquidity_default AFTER INSERT ON account
WHEN NEW.liquidity IS NULL AND NEW.type != 'credit_card'
BEGIN
    UPDATE account SET liquidity = CASE NEW.type
        WHEN 'savings' THEN 'reserve'
        WHEN 'meal_voucher' THEN 'restricted'
        WHEN 'pension' THEN 'illiquid'
        WHEN 'fgts' THEN 'illiquid'
        ELSE 'liquid'
    END
    WHERE id = NEW.id;
END;
