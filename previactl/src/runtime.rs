use std::fs::{File, OpenOptions};
use anyhow::{Context, Result, anyhow};
use fs2::FileExt;
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

pub struct StackLock {
    _file: File,
}

pub fn acquire_lock(stack_paths: &StackPaths) -> Result<StackLock> {
    stack_paths.ensure_parent_dirs()?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&stack_paths.lock_file)
        .with_context(|| format!("failed to open '{}'", stack_paths.lock_file.display()))?;
    file.try_lock_exclusive().map_err(|_| {
        anyhow!(
            "stack '{}' is locked by another mutating operation",
            stack_paths.name
        )
    })?;
    Ok(StackLock { _file: file })
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

pub fn write_runtime_state(stack_paths: &StackPaths, state: &DetachedRuntimeState) -> Result<()> {
    stack_paths.ensure_parent_dirs()?;
    let tmp = stack_paths.run_dir.join("state.json.tmp");
    let contents = serde_json::to_vec_pretty(state).context("failed to encode runtime state")?;
    std::fs::write(&tmp, contents)
        .with_context(|| format!("failed to write '{}'", tmp.display()))?;
    std::fs::rename(&tmp, &stack_paths.runtime_file).with_context(|| {
        format!(
            "failed to move '{}' to '{}'",
            tmp.display(),
            stack_paths.runtime_file.display()
        )
    })?;
    Ok(())
}

pub fn remove_runtime_state(stack_paths: &StackPaths) -> Result<()> {
    if stack_paths.runtime_file.exists() {
        std::fs::remove_file(&stack_paths.runtime_file).with_context(|| {
            format!("failed to remove '{}'", stack_paths.runtime_file.display())
        })?;
    }
    Ok(())
}
