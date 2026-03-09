use crate::server::docs::build_openapi_document;
use crate::server::execution::collect_runner_statuses;
use crate::server::models::OrchestratorInfoResponse;
use crate::server::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

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
    Json(build_openapi_document())
}
