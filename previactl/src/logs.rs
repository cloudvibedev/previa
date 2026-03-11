use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::time::sleep;

pub async fn print_logs(files: Vec<(String, PathBuf)>, tail: Option<usize>) -> Result<()> {
    for (idx, (label, path)) in files.iter().enumerate() {
        let contents = read_log_output(path, tail).await?;
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

pub async fn follow_logs(files: Vec<(String, PathBuf)>, tail: Option<usize>) -> Result<()> {
    let mut offsets = BTreeMap::new();
    for (idx, (label, path)) in files.iter().enumerate() {
        let contents = read_log_contents(path).await?;
        let display = render_tail(&contents, tail);
        if files.len() > 1 {
            if idx > 0 {
                println!();
            }
            println!("== {} ==", label);
        }
        print!("{display}");
        offsets.insert(path.clone(), contents.len() as u64);
    }

    loop {
        for (label, path) in &files {
            let next = read_log_contents(path).await?;
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

async fn read_log_contents(path: &PathBuf) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read '{}'", path.display()))
}

async fn read_log_output(path: &PathBuf, tail: Option<usize>) -> Result<String> {
    let contents = read_log_contents(path).await?;
    Ok(render_tail(&contents, tail))
}

fn render_tail(contents: &str, tail: Option<usize>) -> String {
    let Some(limit) = tail else {
        return contents.to_owned();
    };
    let mut lines = contents.lines().collect::<Vec<_>>();
    if lines.len() > limit {
        lines.drain(..lines.len() - limit);
    }
    let mut out = lines.join("\n");
    if contents.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out
}
