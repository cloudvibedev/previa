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
use crate::server::db::backfill_project_spec_md5_hashes;
use crate::server::execution::parse_runner_endpoints;
use crate::server::state::{AppState, DB_SCHEMA_VERSION};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let runner_endpoints = parse_runner_endpoints();
    let database_url = std::env::var("ORCHESTRATOR_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://orchestrator.db".to_owned());
    let rps_per_node = std::env::var("RUNNER_RPS_PER_NODE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1000);
    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5588);
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

    let state = AppState {
        client: Client::new(),
        db,
        runner_endpoints,
        rps_per_node,
        executions: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = build_app(state);

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind orchestrator listener");
    let local_addr = listener
        .local_addr()
        .expect("failed to read local bind address");

    info!(
        "previa-main listening on http://{} (database: {}, schema_version: {})",
        local_addr, database_url, DB_SCHEMA_VERSION
    );
    if backfilled_spec_hashes > 0 {
        info!(
            "backfilled {} OpenAPI specs without md5 hash",
            backfilled_spec_hashes
        );
    }

    axum::serve(listener, app)
        .await
        .expect("failed to start orchestrator");
}
