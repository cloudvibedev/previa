ALTER TABLE users ADD COLUMN name TEXT;
ALTER TABLE users ADD COLUMN email TEXT;

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
