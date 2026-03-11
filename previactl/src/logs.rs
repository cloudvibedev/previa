use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::time::sleep;

pub async fn print_logs(files: Vec<(String, PathBuf)>) -> Result<()> {
    for (idx, (label, path)) in files.iter().enumerate() {
        let contents = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read '{}'", path.display()))?;
        if files.len() > 1 {
            if idx > 0 {
                println!();
            }
            println!("== {} ==", label);
        }
        print!("{contents}");
    }
    Ok(())
}

pub async fn follow_logs(files: Vec<(String, PathBuf)>) -> Result<()> {
    let mut offsets = BTreeMap::new();
    for (_, path) in &files {
        let contents = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read '{}'", path.display()))?;
        print!("{contents}");
        offsets.insert(path.clone(), contents.len() as u64);
    }

    loop {
        for (label, path) in &files {
            let next = tokio::fs::read_to_string(path)
                .await
                .with_context(|| format!("failed to read '{}'", path.display()))?;
            let previous = offsets.get(path).copied().unwrap_or_default() as usize;
            if next.len() > previous {
                let delta = &next[previous..];
                if files.len() > 1 {
                    for line in delta.lines() {
                        println!("[{label}] {line}");
                    }
                } else {
                    print!("{delta}");
                }
                offsets.insert(path.clone(), next.len() as u64);
            }
        }
        sleep(Duration::from_millis(250)).await;
    }
}
