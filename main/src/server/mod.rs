use std::time::Duration;

use axum::Router;
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
use crate::server::handlers::tests_e2e_queue::{
    create_e2e_queue_for_project, delete_e2e_queue_for_project, get_current_e2e_queue_for_project,
    get_e2e_queue_for_project,
};
use crate::server::handlers::tests_load::run_load_test_for_project;
use crate::server::handlers::transfers::{export_project, import_pipelines, import_project};
use crate::server::mcp::handlers::{delete_http_session, get_http, handle_http, preflight};
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
pub mod services;
pub mod state;
pub mod utils;
pub mod validation;

pub fn build_app(state: AppState, mcp_config: &McpConfig) -> Router {
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/info", get(get_info))
        .route("/openapi.json", get(openapi_json))
        .route("/proxy", post(proxy_request).options(preflight))
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
        .route("/api/v1/projects/import/pipelines", post(import_pipelines))
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
            "/api/v1/projects/{projectId}/tests/e2e/queue",
            get(get_current_e2e_queue_for_project).post(create_e2e_queue_for_project),
        )
        .route(
            "/api/v1/projects/{projectId}/tests/e2e/queue/{queueId}",
            get(get_e2e_queue_for_project).delete(delete_e2e_queue_for_project),
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
        );

    if mcp_config.enabled {
        app = app.route(
            &mcp_config.path,
            get(get_http)
                .post(handle_http)
                .delete(delete_http_session)
                .options(preflight),
        );
    }

    app.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_private_network(true)
            .expose_headers(Any)
            .max_age(Duration::from_secs(60 * 60)),
    )
    .layer(from_fn(propagate_transaction_header))
    .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use axum::body::{Body, to_bytes};
    use axum::http::{HeaderValue, Method, Request, StatusCode};
    use reqwest::Client;
    use serde_json::Value;
    use sqlx::sqlite::SqlitePoolOptions;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    use crate::server::execution::ExecutionScheduler;
    use crate::server::mcp::models::McpConfig;
    use crate::server::state::AppState;

    use super::build_app;

    async fn test_app(mcp_enabled: bool) -> axum::Router {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite memory db");
        let state = AppState {
            client: Client::new(),
            db,
            context_name: "default".to_owned(),
            runner_endpoints: Vec::new(),
            runner_auth_key: None,
            rps_per_node: 1000,
            scheduler: ExecutionScheduler::new(Default::default()),
            executions: Arc::new(RwLock::new(HashMap::new())),
            e2e_queues: Arc::new(RwLock::new(HashMap::new())),
            mcp_sessions: Arc::new(RwLock::new(HashMap::new())),
        };

        build_app(
            state,
            &McpConfig {
                enabled: mcp_enabled,
                path: "/mcp".to_owned(),
            },
        )
    }

    #[tokio::test]
    async fn proxy_preflight_allows_private_network_requests() {
        let app = test_app(false).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/proxy")
                    .header("origin", "https://id-preview.example")
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "content-type")
                    .header("access-control-request-private-network", "true")
                    .body(Body::empty())
                    .expect("preflight request"),
            )
            .await
            .expect("preflight response");

        assert!(response.status().is_success());
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-private-network"),
            Some(&HeaderValue::from_static("true"))
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin")
        );
    }

    #[tokio::test]
    async fn mcp_get_returns_method_not_allowed() {
        let app = test_app(true).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/mcp")
                    .body(Body::empty())
                    .expect("mcp get request"),
            )
            .await
            .expect("mcp get response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn mcp_tools_list_requires_session_header_with_http_400() {
        let app = test_app(true).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
                    ))
                    .expect("mcp tools/list request"),
            )
            .await
            .expect("mcp tools/list response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn mcp_initialize_then_tools_list_returns_ok() {
        let app = test_app(true).await;

        let initialize = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"codex-test","version":"1.0"}}}"#,
                    ))
                    .expect("initialize request"),
            )
            .await
            .expect("initialize response");

        assert_eq!(initialize.status(), StatusCode::OK);
        let session_id = initialize
            .headers()
            .get("mcp-session-id")
            .expect("session header")
            .to_str()
            .expect("session header utf8")
            .to_owned();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .header("mcp-session-id", &session_id)
                    .header("mcp-protocol-version", "2025-06-18")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
                    ))
                    .expect("tools/list request"),
            )
            .await
            .expect("tools/list response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read tools/list body");
        let payload: Value = serde_json::from_slice(&body).expect("parse tools/list body");
        assert!(payload
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(Value::as_array)
            .is_some());
    }
}
