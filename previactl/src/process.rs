use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::config::ResolvedUpConfig;
use crate::health::probe_health;

pub struct SpawnedStack {
    pub main: Child,
    pub runners: Vec<Child>,
}

pub struct ForegroundStack {
    pub main: Child,
    pub runners: Vec<Child>,
    pub _log_tasks: Vec<JoinHandle<()>>,
}

pub async fn spawn_detached_stack(
    config: &ResolvedUpConfig,
    http: &reqwest::Client,
) -> Result<SpawnedStack> {
    let mut runners = Vec::new();
    for launch in &config.local_runners {
        let log_path = config.stack_paths.runner_log(launch.port);
        let child = match spawn_detached_process(
            &config.previa_paths.runner_binary,
            &launch.env,
            &log_path,
        ) {
            Ok(child) => child,
            Err(err) => {
                cleanup_started_children(&mut runners).await?;
                return Err(err);
            }
        };
        match wait_for_startup(child, &launch.health_url(), http).await {
            Ok(child) => runners.push(child),
            Err(err) => {
                cleanup_started_children(&mut runners).await?;
                return Err(err);
            }
        }
    }

    let main = match spawn_detached_process(
        &config.previa_paths.main_binary,
        &config.main_env,
        &config.stack_paths.main_log,
    ) {
        Ok(child) => child,
        Err(err) => {
            cleanup_started_children(&mut runners).await?;
            return Err(err);
        }
    };
    let main = match wait_for_startup(main, &config.main_health_url(), http).await {
        Ok(child) => child,
        Err(err) => {
            cleanup_started_children(&mut runners).await?;
            return Err(err);
        }
    };
    Ok(SpawnedStack { main, runners })
}

pub async fn spawn_foreground_stack(
    config: &ResolvedUpConfig,
    http: &reqwest::Client,
) -> Result<ForegroundStack> {
    let mut runners = Vec::new();
    let mut tasks = Vec::new();
    for launch in &config.local_runners {
        let (child, mut child_tasks) = match spawn_foreground_process(
            &config.previa_paths.runner_binary,
            &launch.env,
            "runner",
        ) {
            Ok(value) => value,
            Err(err) => {
                cleanup_started_children(&mut runners).await?;
                return Err(err);
            }
        };
        tasks.append(&mut child_tasks);
        match wait_for_startup(child, &launch.health_url(), http).await {
            Ok(child) => runners.push(child),
            Err(err) => {
                cleanup_started_children(&mut runners).await?;
                return Err(err);
            }
        }
    }

    let (main, mut child_tasks) = match spawn_foreground_process(
        &config.previa_paths.main_binary,
        &config.main_env,
        "main",
    ) {
        Ok(value) => value,
        Err(err) => {
            cleanup_started_children(&mut runners).await?;
            return Err(err);
        }
    };
    tasks.append(&mut child_tasks);
    let main = match wait_for_startup(main, &config.main_health_url(), http).await {
        Ok(child) => child,
        Err(err) => {
            cleanup_started_children(&mut runners).await?;
            return Err(err);
        }
    };
    Ok(ForegroundStack {
        main,
        runners,
        _log_tasks: tasks,
    })
}

pub async fn monitor_foreground_stack(mut stack: ForegroundStack) -> Result<()> {
    loop {
        if let Some(status) = stack.main.try_wait()? {
            let pids = child_ids(&stack.runners);
            graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
            bail!("previa-main exited unexpectedly with status {status}");
        }

        for runner in &mut stack.runners {
            if let Some(status) = runner.try_wait()? {
                let mut pids = child_ids(&stack.runners);
                if let Some(main_pid) = stack.main.id() {
                    pids.push(main_pid);
                }
                graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
                bail!("previa-runner exited unexpectedly with status {status}");
            }
        }

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let mut pids = child_ids(&stack.runners);
                if let Some(main_pid) = stack.main.id() {
                    pids.push(main_pid);
                }
                graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
                return Ok(());
            }
            _ = sigterm() => {
                let mut pids = child_ids(&stack.runners);
                if let Some(main_pid) = stack.main.id() {
                    pids.push(main_pid);
                }
                graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
                return Ok(());
            }
            _ = sleep(Duration::from_millis(200)) => {}
        }
    }
}

pub async fn graceful_shutdown_pids(pids: &[u32], timeout: Duration) -> Result<()> {
    for pid in pids {
        if pid_exists(*pid) {
            let _ = kill(Pid::from_raw(*pid as i32), Some(Signal::SIGTERM));
        }
    }

    let start = Instant::now();
    while start.elapsed() < timeout {
        if pids.iter().all(|pid| !pid_exists(*pid)) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    for pid in pids {
        if pid_exists(*pid) {
            let _ = kill(Pid::from_raw(*pid as i32), Some(Signal::SIGKILL));
        }
    }
    Ok(())
}

pub fn pid_exists(pid: u32) -> bool {
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

fn spawn_detached_process(
    binary: &std::path::Path,
    env: &BTreeMap<String, String>,
    log_path: &std::path::Path,
) -> Result<Child> {
    if !binary.exists() {
        bail!("missing binary '{}'", binary.display());
    }
    let stdout = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .with_context(|| format!("failed to open '{}'", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("failed to clone '{}'", log_path.display()))?;

    let mut command = Command::new(binary);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    command.envs(env);
    command
        .spawn()
        .with_context(|| format!("failed to spawn '{}'", binary.display()))
}

fn spawn_foreground_process(
    binary: &std::path::Path,
    env: &BTreeMap<String, String>,
    label: &str,
) -> Result<(Child, Vec<JoinHandle<()>>)> {
    if !binary.exists() {
        bail!("missing binary '{}'", binary.display());
    }
    let mut command = Command::new(binary);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.envs(env);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn '{}'", binary.display()))?;

    let mut tasks = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let prefix = format!("[{label}]");
        tasks.push(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("{prefix} {line}");
            }
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        let prefix = format!("[{label}]");
        tasks.push(tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("{prefix} {line}");
            }
        }));
    }
    Ok((child, tasks))
}

async fn wait_for_startup(mut child: Child, health_url: &str, http: &reqwest::Client) -> Result<Child> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child
            .try_wait()
            .context("failed to inspect child process startup state")?
        {
            bail!("process exited during startup with status {status}");
        }
        if probe_health(http, health_url).await {
            return Ok(child);
        }
        if Instant::now() >= deadline {
            if let Some(pid) = child.id() {
                graceful_shutdown_pids(&[pid], Duration::from_secs(1)).await?;
            }
            bail!("process did not become healthy at {health_url}");
        }
        sleep(Duration::from_millis(100)).await;
    }
}

fn child_ids(children: &[Child]) -> Vec<u32> {
    children.iter().filter_map(Child::id).collect()
}

async fn cleanup_started_children(children: &mut [Child]) -> Result<()> {
    let pids = child_ids(children);
    if pids.is_empty() {
        return Ok(());
    }
    graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
    for child in children {
        let _ = child.wait().await;
    }
    Ok(())
}

async fn sigterm() {
    #[cfg(unix)]
    {
        let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM");
        let _ = stream.recv().await;
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await;
}
