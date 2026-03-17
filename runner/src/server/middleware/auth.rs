use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, body::Body, http::Request};

use crate::server::models::ErrorResponse;
use crate::server::state::AppState;

pub async fn require_runner_authorization(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(expected_key) = state
        .runner_auth_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return next.run(request).await;
    };

    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if provided == Some(expected_key) {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "unauthorized".to_owned(),
            message: "missing or invalid authorization".to_owned(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    use crate::server::build_app;
    use crate::server::state::AppState;

    #[tokio::test]
    async fn passes_through_when_auth_key_is_unset() {
        let app = build_app(AppState::default());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_missing_or_invalid_authorization() {
        let state = AppState {
            runner_auth_key: Some("secret".to_owned()),
            ..AppState::default()
        };
        let app = build_app(state);

        let missing = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("missing response");
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let wrong = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("authorization", "wrong")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("wrong response");
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_matching_authorization() {
        let state = AppState {
            runner_auth_key: Some("secret".to_owned()),
            ..AppState::default()
        };
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("authorization", "secret")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
