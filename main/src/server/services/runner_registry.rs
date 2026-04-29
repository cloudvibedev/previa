use reqwest::Client;

use crate::server::db::{DbPool, list_enabled_runner_endpoints, mark_runner_observed};
use crate::server::execution::collect_runner_statuses;
use crate::server::models::RunnerInfo;

pub async fn collect_registered_runner_statuses(
    db: &DbPool,
    client: &Client,
    runner_auth_key: Option<&str>,
) -> Result<Vec<RunnerInfo>, sqlx::Error> {
    let endpoints = list_enabled_runner_endpoints(db).await?;
    let runners = collect_runner_statuses(client, &endpoints, runner_auth_key).await;
    for runner in &runners {
        mark_runner_observed(
            db,
            &runner.endpoint,
            runner.active,
            runner.runtime_error.as_deref(),
            runner.runtime.as_ref(),
        )
        .await?;
    }
    Ok(runners)
}

pub async fn collect_active_registered_runner_endpoints(
    db: &DbPool,
    client: &Client,
    runner_auth_key: Option<&str>,
) -> Result<Vec<String>, sqlx::Error> {
    Ok(
        collect_registered_runner_statuses(db, client, runner_auth_key)
            .await?
            .into_iter()
            .filter(|runner| runner.active)
            .map(|runner| runner.endpoint)
            .collect(),
    )
}
