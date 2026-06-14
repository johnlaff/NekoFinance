-- Synthetic fixture for forecast end-to-end testing (T8.1)
-- This fixture creates a known projected curve with a future negative trough.
-- Run with: sqlite3 neko-finance.db < scripts/seed-forecast-fixture.sql

-- Person
INSERT OR REPLACE INTO person (id, name, email) VALUES
  ('person-fixture-1', 'Fixture User', 'fixture@example.com');

-- Profile
INSERT OR REPLACE INTO profile (id, person_id, device_id) VALUES
  ('profile-fixture-1', 'person-fixture-1', 'fixture-device');

-- Liquid accounts (seed = R$3000.00)
INSERT OR REPLACE INTO account (id, name, type, owner_person_id, institution, balance) VALUES
  ('acc-bank-1', 'Conta Corrente', 'bank', 'person-fixture-1', 'Banco Exemplo', 250000),
  ('acc-wallet-1', 'Carteira', 'wallet', 'person-fixture-1', NULL, 50000);

-- Credit card (closes day 20, due day 10)
INSERT OR REPLACE INTO account (id, name, type, owner_person_id, institution, balance, closing_day, due_day) VALUES
  ('acc-card-1', 'Cartão de crédito', 'credit_card', 'person-fixture-1', 'Banco Exemplo', 0, 20, 10);

-- Categories
INSERT OR IGNORE INTO category (id, name, parent_id, nature) VALUES
  ('cat-income', 'Renda', NULL, 'variable'),
  ('cat-salary', 'Salário', 'cat-income', 'variable'),
  ('cat-expense', 'Despesas', NULL, 'fixed'),
  ('cat-rent', 'Aluguel', 'cat-expense', 'fixed'),
  ('cat-food', 'Alimentação', 'cat-expense', 'variable');

-- Transactions (realized + projections)
-- Realized income this month
INSERT OR REPLACE INTO "transaction" (id, type, amount, description, date, payment_method, is_fixed, is_projection) VALUES
  ('txn-income-1', 'income', 500000, 'Salário', '2026-06-05', 'pix', 0, 0);

-- Realized expenses this month
INSERT OR REPLACE INTO "transaction" (id, type, amount, description, date, payment_method, is_fixed, is_projection) VALUES
  ('txn-rent-1', 'expense', 150000, 'Aluguel', '2026-06-10', 'debit', 1, 0),
  ('txn-food-1', 'expense', 30000, 'Mercado', '2026-06-12', 'debit', 0, 0);

-- Future projected expenses (create the negative trough)
INSERT OR REPLACE INTO "transaction" (id, type, amount, description, date, payment_method, is_fixed, is_projection) VALUES
  ('txn-proj-1', 'expense', 200000, 'Viagem projetada', '2026-06-25', 'debit', 0, 1),
  ('txn-proj-2', 'expense', 100000, 'Conserto carro', '2026-06-28', 'debit', 0, 1);

-- Daily checkins (Régua 1: daily_spend, Régua 2: credit_spend)
INSERT OR REPLACE INTO daily_checkin (id, person_id, date, daily_spend, credit_spend) VALUES
  ('checkin-1', 'person-fixture-1', '2026-06-15', 5000, 20000),
  ('checkin-2', 'person-fixture-1', '2026-06-16', 4500, 15000),
  ('checkin-3', 'person-fixture-1', '2026-06-17', 6000, 25000);

-- Daily budget
INSERT OR REPLACE INTO daily_budget (id, person_id, amount, start_date, status, free_income) VALUES
  ('budget-1', 'person-fixture-1', 5000, '2026-06-01', 'active', 150000);

-- Reserve
INSERT OR REPLACE INTO reserve (id, person_id, target_months, current_months, trend) VALUES
  ('reserve-1', 'person-fixture-1', 6, 3.5, 'up');
