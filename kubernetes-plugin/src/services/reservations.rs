use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{ReservationCreateRequest, ReservationRunner, ReservationStatus};

#[derive(Clone)]
pub struct ReservationStore {
    inner: Arc<RwLock<HashMap<String, ReservationStatus>>>,
    reservation_ttl_seconds: i64,
    static_runner_endpoints: Vec<String>,
}

impl Default for ReservationStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            reservation_ttl_seconds: 300,
            static_runner_endpoints: Vec::new(),
        }
    }
}

impl ReservationStore {
    pub fn from_env() -> Self {
        let reservation_ttl_seconds = std::env::var("PREVIA_RESERVATION_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(300);
        let static_runner_endpoints = std::env::var("PREVIA_STATIC_RUNNER_ENDPOINTS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            reservation_ttl_seconds,
            static_runner_endpoints,
        }
    }

    pub async fn create(&self, request: ReservationCreateRequest) -> ReservationStatus {
        let reservation_id = format!("rr_{}", Uuid::new_v4());
        let mut status = ReservationStatus {
            reservation_id: reservation_id.clone(),
            status: "provisioning".to_owned(),
            requested_runners: request.count,
            ready_runners: 0,
            reservation_token: None,
            expires_at: None,
            runners: Vec::new(),
        };
        if self.static_runner_endpoints.len() >= request.count {
            status.status = "ready".to_owned();
            status.ready_runners = request.count;
            status.reservation_token = Some(format!("rt_{}", Uuid::new_v4()));
            status.expires_at =
                Some((Utc::now() + Duration::seconds(self.reservation_ttl_seconds)).to_rfc3339());
            status.runners = self
                .static_runner_endpoints
                .iter()
                .take(request.count)
                .enumerate()
                .map(|(index, endpoint)| ReservationRunner {
                    id: format!("runner-{}", index + 1),
                    endpoint: endpoint.clone(),
                })
                .collect();
        }
        self.inner
            .write()
            .await
            .insert(reservation_id, status.clone());
        status
    }

    pub async fn get(&self, reservation_id: &str) -> Option<ReservationStatus> {
        self.inner.read().await.get(reservation_id).cloned()
    }

    #[cfg(test)]
    pub async fn mark_ready_for_test(
        &self,
        reservation_id: &str,
        endpoints: Vec<String>,
    ) -> Option<ReservationStatus> {
        let mut lock = self.inner.write().await;
        let status = lock.get_mut(reservation_id)?;
        status.status = "ready".to_owned();
        status.ready_runners = endpoints.len();
        status.reservation_token = Some(format!("rt_{}", Uuid::new_v4()));
        status.expires_at =
            Some((Utc::now() + Duration::seconds(self.reservation_ttl_seconds)).to_rfc3339());
        status.runners = endpoints
            .into_iter()
            .enumerate()
            .map(|(index, endpoint)| ReservationRunner {
                id: format!("runner-{}", index + 1),
                endpoint,
            })
            .collect();
        Some(status.clone())
    }

    pub async fn cancel(&self, reservation_id: &str) -> bool {
        self.inner.write().await.remove(reservation_id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::ReservationStore;
    use crate::models::ReservationCreateRequest;

    #[tokio::test]
    async fn create_reservation_starts_in_provisioning() {
        let store = ReservationStore::default();
        let status = store
            .create(ReservationCreateRequest {
                execution_id: "exec-1".to_owned(),
                pipeline_id: "pipe-1".to_owned(),
                count: 3,
            })
            .await;

        assert_eq!(status.status, "provisioning");
        assert_eq!(status.requested_runners, 3);
        assert_eq!(status.ready_runners, 0);
        assert!(status.reservation_token.is_none());
    }

    #[tokio::test]
    async fn ready_reservation_gets_token_expiry_and_runners() {
        let store = ReservationStore::default();
        let status = store
            .create(ReservationCreateRequest {
                execution_id: "exec-1".to_owned(),
                pipeline_id: "pipe-1".to_owned(),
                count: 1,
            })
            .await;

        let ready = store
            .mark_ready_for_test(
                &status.reservation_id,
                vec!["http://10.0.0.1:55880".to_owned()],
            )
            .await
            .expect("ready reservation");

        assert_eq!(ready.status, "ready");
        assert_eq!(ready.ready_runners, 1);
        assert!(ready.reservation_token.is_some());
        assert!(ready.expires_at.is_some());
        assert_eq!(ready.runners[0].endpoint, "http://10.0.0.1:55880");
    }
}
