use std::net::TcpListener;

use anyhow::{Context, Result, bail};

use crate::config::ResolvedUpConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingConflictKind {
    Main,
    Runner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingConflict {
    pub kind: BindingConflictKind,
    pub address: String,
    pub port: u16,
}

pub fn validate_startup_bindings(config: &ResolvedUpConfig) -> Result<()> {
    for conflict in startup_binding_conflicts(config) {
        bail!("{}", conflict_message(&conflict));
    }
    Ok(())
}

pub fn startup_binding_conflicts(config: &ResolvedUpConfig) -> Vec<BindingConflict> {
    let mut held_listeners = Vec::new();
    let mut conflicts = Vec::new();

    if bind_target(&config.main.address, config.main.port)
        .map(|listener| held_listeners.push(listener))
        .is_err()
    {
        conflicts.push(BindingConflict {
            kind: BindingConflictKind::Main,
            address: config.main.address.clone(),
            port: config.main.port,
        });
    }

    for runner in &config.local_runners {
        if bind_target(&runner.address, runner.port)
            .map(|listener| held_listeners.push(listener))
            .is_err()
        {
            conflicts.push(BindingConflict {
                kind: BindingConflictKind::Runner,
                address: runner.address.clone(),
                port: runner.port,
            });
        }
    }

    conflicts
}

fn bind_target(address: &str, port: u16) -> Result<TcpListener> {
    TcpListener::bind(format_bind_address(address, port)).context("bind target unavailable")
}

pub fn conflict_message(conflict: &BindingConflict) -> String {
    let bind_address = format_bind_address(&conflict.address, conflict.port);
    let role = match conflict.kind {
        BindingConflictKind::Main => "main",
        BindingConflictKind::Runner => "runner",
    };
    format!("Requested {role} bind target '{bind_address}' is already in use or unavailable")
}

fn format_bind_address(address: &str, port: u16) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}
