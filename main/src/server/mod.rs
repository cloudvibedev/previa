use axum::Router;
use axum::http::header;
use axum::middleware::from_fn;
use axum::routing::{get, post, put};
use tower_http::cors::{Any, CorsLayer};

use crate::server::handlers::executions::{cancel_execution, stream_execution};
use crate::server::handlers::health::{get_info, health, openapi_json};
use crate::server::handlers::history_e2e::{
    delete_e2e_history, delete_e2e_test_by_id, get_e2e_test_by_id, list_e2e_history,
};
use crate::server::handlers::history_load::{
    delete_load_history, delete_load_test_by_id, get_load_test_by_id, list_load_history,
};
use crate::server::handlers::pipelines::{
    create_project_pipeline, delete_project_pipeline, get_project_pipeline, list_project_pipelines,
    upsert_project_pipeline,
};
use crate::server::handlers::projects::{
    create_project, delete_project, get_project, list_projects, upsert_project,
};
use crate::server::handlers::proxy::proxy_request;
use crate::server::handlers::specs::{
    create_project_spec, delete_project_spec, get_project_spec, list_project_specs,
    upsert_project_spec, validate_openapi_spec,
};
use crate::server::handlers::tests_e2e::run_e2e_test_for_project;
use crate::server::handlers::tests_load::run_load_test_for_project;
use crate::server::handlers::transfers::{export_project, import_project};
use crate::server::mcp::handlers::{delete_http_session, handle_http};
use crate::server::mcp::models::McpConfig;
use crate::server::middleware::transaction::propagate_transaction_header;
use crate::server::state::AppState;

pub mod db;
pub mod docs;
pub mod errors;
pub mod execution;
pub mod handlers;
pub mod mcp;
pub mod middleware;
pub mod models;
pub mod state;
pub mod utils;
pub mod validation;

pub fn build_app(state: AppState, mcp_config: &McpConfig) -> Router {
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/info", get(get_info))
        .route("/openapi.json", get(openapi_json))
        .route("/proxy", post(proxy_request))
        .route(
            "/api/v1/executions/{executionId}/cancel",
            post(cancel_execution),
        )
        .route(
            "/api/v1/projects/{projectId}/executions/{executionId}",
            get(stream_execution),
        )
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/projects", post(create_project))
        .route("/api/v1/projects/import", post(import_project))
        .route("/api/v1/specs/validate", post(validate_openapi_spec))
        .route("/api/v1/projects/{projectId}", get(get_project))
        .route("/api/v1/projects/{projectId}/export", get(export_project))
        .route(
            "/api/v1/projects/{projectId}/specs",
            get(list_project_specs).post(create_project_spec),
        )
        .route(
            "/api/v1/projects/{projectId}/specs/{specId}",
            get(get_project_spec)
                .put(upsert_project_spec)
                .delete(delete_project_spec),
        )
        .route(
            "/api/v1/projects/{projectId}/pipelines",
            get(list_project_pipelines).post(create_project_pipeline),
        )
        .route(
            "/api/v1/projects/{projectId}/pipelines/{pipelineId}",
            get(get_project_pipeline)
                .put(upsert_project_pipeline)
                .delete(delete_project_pipeline),
        )
        .route("/api/v1/projects/{projectId}", put(upsert_project))
        .route(
            "/api/v1/projects/{projectId}",
            axum::routing::delete(delete_project),
        )
        .route(
            "/api/v1/projects/{projectId}/tests/e2e",
            get(list_e2e_history)
                .post(run_e2e_test_for_project)
                .delete(delete_e2e_history),
        )
        .route(
            "/api/v1/projects/{projectId}/tests/e2e/{test_id}",
            get(get_e2e_test_by_id).delete(delete_e2e_test_by_id),
        )
        .route(
            "/api/v1/projects/{projectId}/tests/load",
            get(list_load_history)
                .post(run_load_test_for_project)
                .delete(delete_load_history),
        )
        .route(
            "/api/v1/projects/{projectId}/tests/load/{test_id}",
            get(get_load_test_by_id).delete(delete_load_test_by_id),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers([header::CONTENT_TYPE]),
        )
        .layer(from_fn(propagate_transaction_header));

    if mcp_config.enabled {
        app = app.route(
            &mcp_config.path,
            post(handle_http).delete(delete_http_session),
        );
    }

    app.with_state(state)
}
