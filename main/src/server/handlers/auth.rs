use axum::extract::{Extension, Json, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;

use crate::server::auth::config::AuthMode;
use crate::server::auth::passwords::{hash_password, verify_password};
use crate::server::auth::permissions::Role;
use crate::server::auth::{Principal, PrincipalSource};
use crate::server::db::{
    ApiTokenInsert, UserUpdate, insert_api_token_record, load_user_auth_record_by_id,
    load_user_auth_record_by_username, load_user_record, update_user_record,
};
use crate::server::models::{
    AuthClientKind, AuthLoginRequest, AuthLoginResponse, AuthMeUpdateRequest, AuthPrincipalSource,
    AuthTokenKind, AuthUserResponse, ErrorResponse,
};
use crate::server::state::AppState;
use crate::server::utils::new_uuid_v7;

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = AuthLoginRequest,
    responses(
        (status = 200, description = "Authenticated session or API token", body = AuthLoginResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 409, description = "Auth disabled in anonymous mode", body = ErrorResponse)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<AuthLoginRequest>,
) -> Response {
    if state.auth.config.mode() == AuthMode::AnonymousFullAccess {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "auth_disabled".to_owned(),
                message: "login is not required while anonymous access is enabled".to_owned(),
            }),
        )
            .into_response();
    }

    let Some(authenticated) = authenticate_password(&state, &payload).await else {
        return unauthorized();
    };

    match payload.client_kind {
        AuthClientKind::App => {
            let Some(jwt) = state.auth.jwt.as_ref() else {
                return unauthorized();
            };
            match jwt.issue(
                &authenticated.id,
                &authenticated.username,
                authenticated.role,
                authenticated.jwt_source,
            ) {
                Ok(token) => Json(AuthLoginResponse {
                    token_kind: AuthTokenKind::Jwt,
                    token,
                    expires_at: None,
                    user: Some(authenticated.user_response()),
                    record: None,
                })
                .into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "auth_error".to_owned(),
                        message: err.to_string(),
                    }),
                )
                    .into_response(),
            }
        }
        AuthClientKind::ApiToken => {
            let Some(api_tokens) = state.auth.api_tokens.as_ref() else {
                return unauthorized();
            };
            let issued = match api_tokens.issue() {
                Ok(issued) => issued,
                Err(err) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "auth_error".to_owned(),
                            message: err.to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            let name = payload
                .token_name
                .unwrap_or_else(|| "previa-cli".to_owned());
            let record = match insert_api_token_record(
                &state.db,
                ApiTokenInsert {
                    id: new_uuid_v7(),
                    name,
                    token_prefix: issued.prefix,
                    token_hash: issued.hash,
                    role: authenticated.role,
                    created_by_user_id: Some(authenticated.id),
                    created_by_username: authenticated.username,
                    expires_at: None,
                },
            )
            .await
            {
                Ok(record) => record,
                Err(err) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "auth_error".to_owned(),
                            message: err.to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            Json(AuthLoginResponse {
                token_kind: AuthTokenKind::ApiToken,
                token: issued.raw,
                expires_at: None,
                user: None,
                record: Some(record),
            })
            .into_response()
        }
    }
}

struct AuthenticatedUser {
    id: String,
    username: String,
    name: Option<String>,
    email: Option<String>,
    role: Role,
    source: AuthPrincipalSource,
    jwt_source: &'static str,
}

impl AuthenticatedUser {
    fn user_response(&self) -> AuthUserResponse {
        AuthUserResponse {
            id: self.id.clone(),
            username: self.username.clone(),
            name: self.name.clone(),
            email: self.email.clone(),
            role: self.role,
            source: self.source.clone(),
        }
    }
}

async fn authenticate_password(
    state: &AppState,
    payload: &AuthLoginRequest,
) -> Option<AuthenticatedUser> {
    if let (Some(root_username), Some(root_password)) = (
        state.auth.config.root_username.as_deref(),
        state.auth.config.root_password.as_deref(),
    ) {
        if payload.username == root_username && constant_eq(&payload.password, root_password) {
            return Some(AuthenticatedUser {
                id: "root".to_owned(),
                username: root_username.to_owned(),
                name: None,
                email: None,
                role: Role::Root,
                source: AuthPrincipalSource::Env,
                jwt_source: "env",
            });
        }
    }

    let record = load_user_auth_record_by_username(&state.db, &payload.username)
        .await
        .ok()
        .flatten()?;
    if !record.active || !verify_password(&payload.password, &record.password_hash) {
        return None;
    }
    Some(AuthenticatedUser {
        id: record.id,
        username: record.username,
        name: record.name,
        email: record.email,
        role: record.role,
        source: AuthPrincipalSource::Database,
        jwt_source: "database",
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current authenticated principal", body = AuthUserResponse)
    )
)]
pub async fn me(
    State(state): State<AppState>,
    principal: Option<Extension<Principal>>,
) -> Response {
    if state.auth.config.mode() == AuthMode::AnonymousFullAccess {
        return Json(AuthUserResponse {
            id: "anonymous".to_owned(),
            username: "anonymous".to_owned(),
            name: None,
            email: None,
            role: Role::Anonymous,
            source: AuthPrincipalSource::Anonymous,
        })
        .into_response();
    }

    let Some(Extension(principal)) = principal else {
        return unauthorized();
    };
    Json(auth_user_response(&state, principal).await).into_response()
}

#[utoipa::path(
    patch,
    path = "/api/v1/auth/me",
    request_body = AuthMeUpdateRequest,
    responses(
        (status = 200, description = "Updated authenticated principal", body = AuthUserResponse),
        (status = 400, description = "Invalid account update", body = ErrorResponse),
        (status = 403, description = "Current principal cannot update account settings", body = ErrorResponse)
    )
)]
pub async fn update_me(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(payload): Json<AuthMeUpdateRequest>,
) -> Response {
    if !matches!(principal.source, PrincipalSource::Database) {
        return forbidden("environment and token principals cannot update account settings");
    }

    let auth_record = match load_user_auth_record_by_id(&state.db, &principal.subject).await {
        Ok(Some(record)) if record.active => record,
        Ok(_) => return unauthorized(),
        Err(err) => return auth_error(err.to_string()),
    };

    let password_hash = if let Some(new_password) = payload
        .new_password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let Some(current_password) = payload.current_password.as_deref() else {
            return bad_request("currentPassword is required to change password");
        };
        if !verify_password(current_password, &auth_record.password_hash) {
            return unauthorized();
        }
        match hash_password(new_password) {
            Ok(hash) => Some(hash),
            Err(err) => return auth_error(err.to_string()),
        }
    } else {
        None
    };

    let username = match payload.username {
        Some(value) => {
            let username = value.trim().to_owned();
            if username.is_empty() {
                return bad_request("username is required");
            }
            Some(username)
        }
        None => None,
    };

    let updated = match update_user_record(
        &state.db,
        &principal.subject,
        UserUpdate {
            username,
            name: normalize_optional(payload.name),
            email: normalize_optional(payload.email),
            password_hash,
            role: None,
            active: None,
        },
    )
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => return unauthorized(),
        Err(err) => return auth_error(err.to_string()),
    };

    Json(AuthUserResponse {
        id: updated.id,
        username: updated.username,
        name: updated.name,
        email: updated.email,
        role: updated.role,
        source: AuthPrincipalSource::Database,
    })
    .into_response()
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "unauthorized".to_owned(),
            message: "invalid username or password".to_owned(),
        }),
    )
        .into_response()
}

async fn auth_user_response(state: &AppState, principal: Principal) -> AuthUserResponse {
    let source = match principal.source {
        PrincipalSource::Env => AuthPrincipalSource::Env,
        PrincipalSource::Database => AuthPrincipalSource::Database,
        PrincipalSource::ApiToken => AuthPrincipalSource::ApiToken,
        PrincipalSource::Anonymous => AuthPrincipalSource::Anonymous,
    };

    if matches!(principal.source, PrincipalSource::Database) {
        if let Ok(Some(user)) = load_user_record(&state.db, &principal.subject).await {
            return AuthUserResponse {
                id: user.id,
                username: user.username,
                name: user.name,
                email: user.email,
                role: user.role,
                source,
            };
        }
    }

    AuthUserResponse {
        id: principal.subject,
        username: principal.username,
        name: None,
        email: None,
        role: principal.role,
        source,
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.map(|item| item.trim().to_owned())
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "bad_request".to_owned(),
            message: message.to_owned(),
        }),
    )
        .into_response()
}

fn forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "forbidden".to_owned(),
            message: message.to_owned(),
        }),
    )
        .into_response()
}

fn auth_error(message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "auth_error".to_owned(),
            message,
        }),
    )
        .into_response()
}

fn constant_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).unwrap_u8() == 1
}
