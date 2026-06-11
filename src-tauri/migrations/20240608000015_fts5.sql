CREATE VIRTUAL TABLE IF NOT EXISTS transaction_fts USING fts5(description, content='"transaction"', content_rowid='rowid');
CREATE VIRTUAL TABLE IF NOT EXISTS category_fts USING fts5(name, content='category', content_rowid='rowid');
