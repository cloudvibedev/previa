CREATE TABLE IF NOT EXISTS e2e_queues (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    selected_base_url_key TEXT,
    request_json TEXT NOT NULL,
    active_execution_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_e2e_queues_project_updated_at
    ON e2e_queues(project_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_e2e_queues_project_status
    ON e2e_queues(project_id, status);

CREATE TABLE IF NOT EXISTS e2e_queue_items (
    id TEXT PRIMARY KEY NOT NULL,
    queue_id TEXT NOT NULL REFERENCES e2e_queues(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    position BIGINT NOT NULL,
    pipeline_id TEXT NOT NULL,
    status TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    execution_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_e2e_queue_items_queue_position
    ON e2e_queue_items(queue_id, position ASC);

CREATE INDEX IF NOT EXISTS idx_e2e_queue_items_project_status
    ON e2e_queue_items(project_id, status);

CREATE UNIQUE INDEX IF NOT EXISTS idx_e2e_queue_items_queue_position_unique
    ON e2e_queue_items(queue_id, position);
