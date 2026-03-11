use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::StackPaths;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainRuntime {
    pub pid: u32,
    pub address: String,
    pub port: u16,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRunnerRuntime {
    pub pid: u32,
    pub address: String,
    pub port: u16,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedRuntimeState {
    pub name: String,
    pub mode: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub main: MainRuntime,
    pub runner_port_range: PortRange,
    pub attached_runners: Vec<String>,
    pub runners: Vec<LocalRunnerRuntime>,
}

pub fn read_runtime_state(stack_paths: &StackPaths) -> Result<Option<DetachedRuntimeState>> {
    if !stack_paths.runtime_file.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&stack_paths.runtime_file)
        .with_context(|| format!("failed to read '{}'", stack_paths.runtime_file.display()))?;
    let state = serde_json::from_str::<DetachedRuntimeState>(&contents)
        .with_context(|| format!("failed to parse '{}'", stack_paths.runtime_file.display()))?;
    Ok(Some(state))
}
