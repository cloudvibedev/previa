use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub struct AppState {
    pub executions: Arc<RwLock<HashMap<String, CancellationToken>>>,
}
