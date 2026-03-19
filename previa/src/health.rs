use reqwest::Client;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedState {
    Running,
    Degraded,
    Stopped,
}

impl DerivedState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Degraded => "degraded",
            Self::Stopped => "stopped",
        }
    }

    pub fn from_value(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "degraded" => Self::Degraded,
            _ => Self::Stopped,
        }
    }

    pub fn collapse(states: &[Self]) -> Self {
        if states.is_empty() {
            return Self::Stopped;
        }
        if states.iter().any(|state| matches!(state, Self::Degraded)) {
            return Self::Degraded;
        }
        if states.iter().all(|state| matches!(state, Self::Stopped)) {
            return Self::Stopped;
        }
        if states.iter().any(|state| matches!(state, Self::Stopped)) {
            return Self::Degraded;
        }
        Self::Running
    }
}

pub async fn probe_health(http: &Client, url: &str, authorization: Option<&str>) -> bool {
    let mut request = http.get(url);
    if let Some(authorization) = authorization
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header(reqwest::header::AUTHORIZATION, authorization);
    }

    match request.send().await {
        Ok(response) => response.status() == reqwest::StatusCode::OK,
        Err(_) => false,
    }
}

pub fn state_from_running_and_health(running: bool, healthy: bool) -> DerivedState {
    if !running {
        DerivedState::Stopped
    } else if healthy {
        DerivedState::Running
    } else {
        DerivedState::Degraded
    }
}

pub fn state_from_pid_and_health(pid: u32, healthy: bool) -> DerivedState {
    state_from_running_and_health(pid > 0, healthy)
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::http::header::AUTHORIZATION;
    use axum::routing::get;
    use axum::{Router, http::HeaderMap};
    use reqwest::Client;
    use tokio::net::TcpListener;

    use super::probe_health;

    async fn health(State(expected): State<Option<String>>, headers: HeaderMap) -> StatusCode {
        match expected.as_deref() {
            Some(expected) => {
                let provided = headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok());
                if provided == Some(expected) {
                    StatusCode::OK
                } else {
                    StatusCode::UNAUTHORIZED
                }
            }
            None => StatusCode::OK,
        }
    }

    async fn spawn_test_server(expected_auth: Option<&str>) -> String {
        let app = Router::new()
            .route("/health", get(health))
            .with_state(expected_auth.map(ToOwned::to_owned));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });
        format!("http://{address}/health")
    }

    #[tokio::test]
    async fn probe_health_accepts_unprotected_endpoint() {
        let client = Client::new();
        let url = spawn_test_server(None).await;

        assert!(probe_health(&client, &url, None).await);
    }

    #[tokio::test]
    async fn probe_health_includes_authorization_when_present() {
        let client = Client::new();
        let url = spawn_test_server(Some("secret")).await;

        assert!(!probe_health(&client, &url, None).await);
        assert!(probe_health(&client, &url, Some("secret")).await);
    }
}
