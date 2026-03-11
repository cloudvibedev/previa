use reqwest::Client;

use crate::process::pid_exists;

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

pub async fn probe_health(http: &Client, url: &str) -> bool {
    match http.get(url).send().await {
        Ok(response) => response.status() == reqwest::StatusCode::OK,
        Err(_) => false,
    }
}

pub fn state_from_pid_and_health(pid: u32, healthy: bool) -> DerivedState {
    if !pid_exists(pid) {
        DerivedState::Stopped
    } else if healthy {
        DerivedState::Running
    } else {
        DerivedState::Degraded
    }
}
