use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use utoipa::OpenApi;

use crate::server::docs::ApiDoc;
use crate::server::execution::collect_runner_statuses;
use crate::server::models::OrchestratorInfoResponse;
use crate::server::state::AppState;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Orchestrator saudável")
    )
)]
pub async fn health() -> StatusCode {
    StatusCode::OK
}

#[utoipa::path(
    get,
    path = "/info",
    responses(
        (status = 200, description = "Runners cadastrados e status de atividade", body = OrchestratorInfoResponse)
    )
)]
pub async fn get_info(State(state): State<AppState>) -> Json<OrchestratorInfoResponse> {
    let runners = collect_runner_statuses(&state.client, &state.runner_endpoints).await;
    let active_runners = runners.iter().filter(|runner| runner.active).count();

    Json(OrchestratorInfoResponse {
        total_runners: runners.len(),
        active_runners,
        runners,
    })
}

pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    let mut openapi = ApiDoc::openapi();
    openapi.info.title = env!("CARGO_PKG_NAME").to_owned();
    openapi.info.version = env!("CARGO_PKG_VERSION").to_owned();
    let package_description = env!("CARGO_PKG_DESCRIPTION").trim();
    let package_authors = env!("CARGO_PKG_AUTHORS")
        .split(':')
        .map(str::trim)
        .filter(|author| !author.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let mut description_parts = Vec::new();
    if !package_description.is_empty() {
        description_parts.push(package_description.to_owned());
    }
    if !package_authors.is_empty() {
        description_parts.push(format!("Authors: {}", package_authors));
    }
    openapi.info.description = if description_parts.is_empty() {
        None
    } else {
        Some(description_parts.join("\n\n"))
    };
    Json(openapi)
}
