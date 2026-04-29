CREATE TABLE IF NOT EXISTS runners (
    id TEXT PRIMARY KEY NOT NULL,
    endpoint TEXT NOT NULL UNIQUE,
    name TEXT,
    source TEXT NOT NULL,
    enabled BIGINT NOT NULL DEFAULT 1,
    health_status TEXT NOT NULL DEFAULT 'unknown',
    last_seen_at TEXT,
    last_error TEXT,
    runtime_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runners_enabled
    ON runners(enabled);

CREATE INDEX IF NOT EXISTS idx_runners_source
    ON runners(source);
