CREATE TABLE IF NOT EXISTS integration_history (
    id TEXT PRIMARY KEY NOT NULL,
    execution_id TEXT NOT NULL UNIQUE,
    transaction_id TEXT,
    project_id TEXT,
    pipeline_index BIGINT,
    pipeline_id TEXT,
    pipeline_name TEXT NOT NULL,
    selected_base_url_key TEXT,
    status TEXT NOT NULL,
    started_at_ms BIGINT NOT NULL,
    finished_at_ms BIGINT NOT NULL,
    duration_ms BIGINT NOT NULL,
    summary_json TEXT,
    steps_json TEXT NOT NULL,
    errors_json TEXT NOT NULL,
    request_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_integration_history_project_pipeline
    ON integration_history(project_id, pipeline_index);

CREATE INDEX IF NOT EXISTS idx_integration_history_finished_at
    ON integration_history(finished_at_ms DESC);

CREATE TABLE IF NOT EXISTS load_history (
    id TEXT PRIMARY KEY NOT NULL,
    execution_id TEXT NOT NULL UNIQUE,
    transaction_id TEXT,
    project_id TEXT,
    pipeline_index BIGINT,
    pipeline_id TEXT,
    pipeline_name TEXT NOT NULL,
    selected_base_url_key TEXT,
    status TEXT NOT NULL,
    started_at_ms BIGINT NOT NULL,
    finished_at_ms BIGINT NOT NULL,
    duration_ms BIGINT NOT NULL,
    requested_config_json TEXT NOT NULL,
    final_consolidated_json TEXT,
    final_lines_json TEXT NOT NULL,
    errors_json TEXT NOT NULL,
    request_json TEXT NOT NULL,
    context_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_load_history_project_pipeline
    ON load_history(project_id, pipeline_index);

CREATE INDEX IF NOT EXISTS idx_load_history_finished_at
    ON load_history(finished_at_ms DESC);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    spec_json TEXT,
    execution_backend_url TEXT
);

CREATE INDEX IF NOT EXISTS idx_projects_updated_at_ms
    ON projects(updated_at_ms DESC);

CREATE TABLE IF NOT EXISTS pipelines (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    position BIGINT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    pipeline_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pipelines_project_position
    ON pipelines(project_id, position ASC);

CREATE INDEX IF NOT EXISTS idx_pipelines_project
    ON pipelines(project_id);

CREATE TABLE IF NOT EXISTS project_openapi_specs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    url TEXT,
    sync BIGINT NOT NULL DEFAULT 0,
    slug TEXT,
    urls_json TEXT NOT NULL DEFAULT '[]',
    spec_md5 TEXT NOT NULL DEFAULT '',
    live BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_project_openapi_specs_project
    ON project_openapi_specs(project_id);

CREATE INDEX IF NOT EXISTS idx_project_openapi_specs_updated_at_ms
    ON project_openapi_specs(updated_at_ms DESC);

CREATE INDEX IF NOT EXISTS idx_project_openapi_specs_slug
    ON project_openapi_specs(slug);

CREATE UNIQUE INDEX IF NOT EXISTS idx_project_openapi_specs_project_slug_unique
    ON project_openapi_specs(project_id, slug)
    WHERE slug IS NOT NULL;
