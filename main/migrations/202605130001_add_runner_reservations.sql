CREATE TABLE IF NOT EXISTS runner_reservations (
    execution_id TEXT PRIMARY KEY NOT NULL,
    pipeline_id TEXT,
    capacity_mode TEXT NOT NULL,
    requested_runner_count BIGINT NOT NULL,
    ready_runner_count BIGINT NOT NULL DEFAULT 0,
    target_rps BIGINT NOT NULL,
    node_profile TEXT,
    reservation_id TEXT,
    reservation_token TEXT,
    reservation_expires_at TEXT,
    reservation_status TEXT NOT NULL,
    runner_endpoints_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runner_reservations_pipeline
    ON runner_reservations(pipeline_id);

CREATE INDEX IF NOT EXISTS idx_runner_reservations_reservation
    ON runner_reservations(reservation_id);
