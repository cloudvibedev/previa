use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Client;
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::sync::{RwLock, broadcast};
use tokio_util::sync::CancellationToken;

use crate::server::mcp::models::McpSession;
use crate::server::models::SseMessage;

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub db: SqlitePool,
    pub runner_endpoints: Vec<String>,
    pub rps_per_node: u64,
    pub executions: Arc<RwLock<HashMap<String, Arc<ExecutionCtx>>>>,
    pub mcp_sessions: Arc<RwLock<HashMap<String, McpSession>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionKind {
    E2e,
    Load,
}

#[derive(Debug, Clone)]
pub struct ExecutionCtx {
    pub cancel: CancellationToken,
    pub project_id: String,
    pub kind: ExecutionKind,
    pub sse_tx: broadcast::Sender<SseMessage>,
    pub init_payload: Value,
}

pub const TRANSACTION_ID_HEADER: &str = "x-transaction-id";
pub const LOAD_BATCH_WINDOW_MS: u64 = 50;
pub const DB_SCHEMA_VERSION: u32 = 1;
pub const EXECUTION_SSE_BUFFER_SIZE: usize = 1024;
