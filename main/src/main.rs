mod server;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use reqwest::Client;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;

use crate::server::build_app;
use crate::server::db::{backfill_project_spec_md5_hashes, cancel_stale_e2e_queues};
use crate::server::execution::{SchedulerConfig, parse_runner_endpoints};
use crate::server::mcp::models::McpConfig;
use crate::server::state::{AppState, DB_SCHEMA_VERSION};
use crate::server::utils::now_iso;

fn should_print_version(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-v")
}

#[tokio::main]
async fn main() {
    if should_print_version(std::env::args()) {
        println!("previa-main {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let runner_endpoints = parse_runner_endpoints();
    let mcp_config = McpConfig::from_env();
    let database_url = std::env::var("ORCHESTRATOR_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://orchestrator.db".to_owned());
    let rps_per_node = std::env::var("RUNNER_RPS_PER_NODE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1000);
    let e2e_per_runner_limit = std::env::var("E2E_EXECUTIONS_PER_RUNNER")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let load_per_runner_limit = std::env::var("LOAD_EXECUTIONS_PER_RUNNER")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5588);
    let context_name = std::env::var("PREVIA_CONTEXT").unwrap_or_else(|_| "default".to_owned());
    let bind_addr = format!("{}:{}", address, port);

    let connect_options = SqliteConnectOptions::from_str(&database_url)
        .expect("invalid ORCHESTRATOR_DATABASE_URL")
        .create_if_missing(true)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .expect("failed to connect orchestrator sqlite database");
    sqlx::migrate!()
        .run(&db)
        .await
        .expect("failed to run orchestrator database migrations");
    let backfilled_spec_hashes = backfill_project_spec_md5_hashes(&db)
        .await
        .expect("failed to backfill OpenAPI spec md5 hashes");
    let cancelled_stale_queues = cancel_stale_e2e_queues(&db, &now_iso())
        .await
        .expect("failed to cancel stale e2e queues");

    let state = AppState {
        client: Client::new(),
        db,
        context_name: context_name.clone(),
        runner_endpoints,
        rps_per_node,
        scheduler: crate::server::execution::ExecutionScheduler::new(SchedulerConfig {
            e2e_per_runner_limit,
            load_per_runner_limit,
        }),
        executions: Arc::new(RwLock::new(HashMap::new())),
        e2e_queues: Arc::new(RwLock::new(HashMap::new())),
        mcp_sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = build_app(state, &mcp_config);

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind orchestrator listener");
    let local_addr = listener
        .local_addr()
        .expect("failed to read local bind address");

    info!(
        "previa-main listening on http://{} (context: {}, database: {}, schema_version: {})",
        local_addr, context_name, database_url, DB_SCHEMA_VERSION
    );
    if mcp_config.enabled {
        info!(
            "mcp server enabled at http://{}{}",
            local_addr, mcp_config.path
        );
    }
    if backfilled_spec_hashes > 0 {
        info!(
            "backfilled {} OpenAPI specs without md5 hash",
            backfilled_spec_hashes
        );
    }
    if cancelled_stale_queues > 0 {
        info!(
            "cancelled {} stale e2e queues from previous startup",
            cancelled_stale_queues
        );
    }
    info!(
        "execution scheduler configured (e2e_per_runner_limit: {}, load_per_runner_limit: {})",
        e2e_per_runner_limit, load_per_runner_limit
    );

    axum::serve(listener, app)
        .await
        .expect("failed to start orchestrator");
}

#[cfg(test)]
mod tests {
    use super::should_print_version;

    #[test]
    fn detects_version_flags() {
        assert!(should_print_version(vec![
            "previa-main".to_owned(),
            "--version".to_owned(),
        ]));
        assert!(should_print_version(vec![
            "previa-main".to_owned(),
            "-v".to_owned(),
        ]));
        assert!(!should_print_version(vec!["previa-main".to_owned()]));
    }
}
