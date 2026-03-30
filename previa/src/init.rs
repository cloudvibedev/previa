use std::path::PathBuf;

use anyhow::{Context, Result, bail};

const DEFAULT_COMPOSE_TEMPLATE: &str = r#"version: 1
main:
  address: 0.0.0.0
  port: 5588
runners:
  local:
    address: 127.0.0.1
    count: 1
    port_range:
      start: 55880
      end: 55889
"#;

pub fn init_compose(force: bool) -> Result<PathBuf> {
    let path = std::env::current_dir()
        .context("failed to read current directory")?
        .join("previa-compose.yaml");

    if path.exists() && !force {
        bail!(
            "'{}' already exists; rerun with --force to overwrite it",
            path.display()
        );
    }

    std::fs::write(&path, DEFAULT_COMPOSE_TEMPLATE)
        .with_context(|| format!("failed to write '{}'", path.display()))?;

    Ok(path)
}
