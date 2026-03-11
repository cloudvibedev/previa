mod cli;
mod config;
mod envfile;
mod health;
mod logs;
mod output;
mod paths;
mod process;
mod runtime;
mod selectors;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::Parser;
use reqwest::Client;

use crate::cli::{Cli, Commands, DownArgs, LogsArgs, PsArgs, RestartArgs, StatusArgs, UpArgs};
use crate::config::{ResolvedUpConfig, resolve_up_config};
use crate::health::{DerivedState, probe_health, state_from_pid_and_health};
use crate::logs::{follow_logs, print_logs};
use crate::output::{
    ListEntryJson, ProcessJson, StatusJson, StatusProcessJson, print_list_human,
    print_process_rows, print_status_human,
};
use crate::paths::{PreviaPaths, StackPaths};
use crate::process::{
    SpawnedStack, graceful_shutdown_pids, monitor_foreground_stack, spawn_detached_stack,
    spawn_foreground_stack,
};
use crate::runtime::{
    DetachedRuntimeState, LocalRunnerRuntime, MainRuntime, acquire_lock, read_runtime_state,
    remove_runtime_state, write_runtime_state,
};
use crate::selectors::{RunnerSelector, parse_stack_name};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = PreviaPaths::discover()?;
    let http = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .context("failed to build HTTP client")?;

    match cli.command {
        Commands::Up(args) => cmd_up(&paths, &http, args).await,
        Commands::Down(args) => cmd_down(&paths, args).await,
        Commands::Restart(args) => cmd_restart(&paths, &http, args).await,
        Commands::Status(args) => cmd_status(&paths, &http, args).await,
        Commands::List(args) => cmd_list(&paths, &http, args.json).await,
        Commands::Ps(args) => cmd_ps(&paths, &http, args).await,
        Commands::Logs(args) => cmd_logs(&paths, args).await,
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

async fn cmd_up(paths: &PreviaPaths, http: &Client, args: UpArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    let stack_paths = paths.stack(&stack_name);
    let resolved = resolve_up_config(paths, &stack_paths, args).await?;

    if resolved.dry_run {
        print_dry_run(&resolved);
        return Ok(());
    }

    if resolved.detach {
        let _lock = acquire_lock(&stack_paths)?;
        if stack_paths.runtime_file.exists() {
            bail!(
                "detached runtime already exists for stack '{}': {}",
                stack_name,
                stack_paths.runtime_file.display()
            );
        }

        let spawned = spawn_detached_stack(&resolved, http).await?;
        let state = detached_state_from_spawn(&resolved, &spawned)?;
        write_runtime_state(&stack_paths, &state)?;
        println!(
            "stack '{}' started in detached mode (main: {}:{})",
            stack_name, state.main.address, state.main.port
        );
        return Ok(());
    }

    let foreground = spawn_foreground_stack(&resolved, http).await?;
    monitor_foreground_stack(foreground).await
}

async fn cmd_down(paths: &PreviaPaths, args: DownArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    let stack_paths = paths.stack(&stack_name);
    let _lock = acquire_lock(&stack_paths)?;
    let mut state = read_required_state(&stack_paths)?;

    let selectors = parse_runner_selectors(&args.runners)?;
    if selectors.is_empty() {
        let pids = all_runtime_pids(&state);
        graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
        remove_runtime_state(&stack_paths)?;
        println!("stack '{}' stopped", stack_name);
        return Ok(());
    }

    let selected = select_runner_indexes(&state.runners, &selectors)?;
    let remaining_local = state.runners.len().saturating_sub(selected.len());
    if remaining_local == 0 && state.attached_runners.is_empty() {
        bail!("cannot remove the selected runners because the stack would have zero runner sources");
    }

    let selected_pids = selected
        .iter()
        .map(|idx| state.runners[*idx].pid)
        .collect::<Vec<_>>();
    graceful_shutdown_pids(&selected_pids, Duration::from_secs(3)).await?;

    state.runners = state
        .runners
        .into_iter()
        .enumerate()
        .filter_map(|(idx, runner)| (!selected.contains(&idx)).then_some(runner))
        .collect();
    write_runtime_state(&stack_paths, &state)?;
    println!("stack '{}' updated", stack_name);
    Ok(())
}

async fn cmd_restart(paths: &PreviaPaths, http: &Client, args: RestartArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    let stack_paths = paths.stack(&stack_name);
    let _lock = acquire_lock(&stack_paths)?;
    let state = read_required_state(&stack_paths)?;

    let pids = all_runtime_pids(&state);
    graceful_shutdown_pids(&pids, Duration::from_secs(3)).await?;
    remove_runtime_state(&stack_paths)?;

    let resolved = ResolvedUpConfig::from_runtime(paths, &stack_paths, &state).await?;
    let spawned = spawn_detached_stack(&resolved, http).await?;
    let next_state = detached_state_from_spawn(&resolved, &spawned)?;
    write_runtime_state(&stack_paths, &next_state)?;
    println!("stack '{}' restarted", stack_name);
    Ok(())
}

async fn cmd_status(paths: &PreviaPaths, http: &Client, args: StatusArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    if args.main && args.runner.is_some() {
        bail!("--main and --runner are mutually exclusive");
    }

    let stack_paths = paths.stack(&stack_name);
    let state = read_runtime_state(&stack_paths)?;
    let selector = args
        .runner
        .as_deref()
        .map(RunnerSelector::parse)
        .transpose()?;

    let status = build_status_json(&stack_paths, state.as_ref(), http, selector.as_ref(), args.main).await?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).context("failed to serialize status JSON")?
        );
    } else {
        print_status_human(&status, args.main, selector.is_some());
    }
    Ok(())
}

async fn cmd_list(paths: &PreviaPaths, http: &Client, json: bool) -> Result<()> {
    let mut entries = Vec::new();
    for stack_paths in paths.stack_roots()? {
        let state = read_runtime_state(&stack_paths)?;
        let overall = overall_stack_state(state.as_ref(), http).await?;
        entries.push(ListEntryJson {
            name: stack_paths.name.clone(),
            state: overall.as_str().to_owned(),
            runtime_file: stack_paths.runtime_file.display().to_string(),
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).context("failed to serialize list JSON")?
        );
    } else {
        print_list_human(&entries);
    }
    Ok(())
}

async fn cmd_ps(paths: &PreviaPaths, http: &Client, args: PsArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    let stack_paths = paths.stack(&stack_name);
    let state = read_runtime_state(&stack_paths)?;
    let rows = process_rows(state.as_ref(), http).await?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).context("failed to serialize ps JSON")?
        );
    } else {
        print_process_rows(&rows);
    }
    Ok(())
}

async fn cmd_logs(paths: &PreviaPaths, args: LogsArgs) -> Result<()> {
    let stack_name = parse_stack_name(&args.name)?;
    if args.main && args.runner.is_some() {
        bail!("--main and --runner are mutually exclusive");
    }

    let stack_paths = paths.stack(&stack_name);
    let state = read_required_state(&stack_paths)?;
    let logs = if args.main {
        vec![("main".to_owned(), PathBuf::from(&state.main.log_path))]
    } else if let Some(selector) = args.runner.as_deref() {
        let selector = RunnerSelector::parse(selector)?;
        let indexes = select_runner_indexes(&state.runners, &[selector])?;
        indexes
            .into_iter()
            .map(|idx| {
                let runner = &state.runners[idx];
                (
                    format!("runner:{}:{}", runner.address, runner.port),
                    PathBuf::from(&runner.log_path),
                )
            })
            .collect()
    } else {
        let mut files = vec![("main".to_owned(), PathBuf::from(&state.main.log_path))];
        let mut runners = state.runners.clone();
        runners.sort_by_key(|runner| runner.port);
        files.extend(runners.into_iter().map(|runner| {
            (
                format!("runner:{}:{}", runner.address, runner.port),
                PathBuf::from(runner.log_path),
            )
        }));
        files
    };

    if args.follow {
        follow_logs(logs, args.tail).await
    } else {
        print_logs(logs, args.tail).await
    }
}

fn print_dry_run(resolved: &ResolvedUpConfig) {
    println!("stack: {}", resolved.stack_paths.name);
    println!(
        "main: {}:{}",
        resolved.main.address,
        resolved.main.port
    );
    println!(
        "local runners: {} ({:?}-{:?})",
        resolved.local_runner_count,
        resolved.runner_port_range.start,
        resolved.runner_port_range.end
    );
    println!("attached runners: {}", resolved.attached_runners.join(", "));
    if let Some(source) = &resolved.source {
        println!("source: {}", source.display());
    }
}

fn detached_state_from_spawn(
    resolved: &ResolvedUpConfig,
    spawned: &SpawnedStack,
) -> Result<DetachedRuntimeState> {
    Ok(DetachedRuntimeState {
        name: resolved.stack_paths.name.clone(),
        mode: "detached".to_owned(),
        started_at: Utc::now().to_rfc3339(),
        source: resolved
            .source
            .as_ref()
            .map(|path| path.display().to_string()),
        main: MainRuntime {
            pid: child_id(&spawned.main)?,
            address: resolved.main.address.clone(),
            port: resolved.main.port,
            log_path: resolved.stack_paths.main_log.display().to_string(),
        },
        runner_port_range: resolved.runner_port_range,
        attached_runners: resolved.attached_runners.clone(),
        runners: resolved
            .local_runner_ports
            .iter()
            .zip(spawned.runners.iter())
            .map(|((address, port), child)| {
                Ok(LocalRunnerRuntime {
                    pid: child_id(child)?,
                    address: address.clone(),
                    port: *port,
                    log_path: resolved
                        .stack_paths
                        .runner_log(*port)
                        .display()
                        .to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn child_id(child: &tokio::process::Child) -> Result<u32> {
    child
        .id()
        .ok_or_else(|| anyhow!("spawned process has no pid"))
}

async fn build_status_json(
    stack_paths: &StackPaths,
    state: Option<&DetachedRuntimeState>,
    http: &Client,
    runner_selector: Option<&RunnerSelector>,
    main_only: bool,
) -> Result<StatusJson> {
    let runtime_file = stack_paths.runtime_file.display().to_string();
    let Some(state) = state else {
        if runner_selector.is_some() {
            bail!("no detached runtime exists for stack '{}'", stack_paths.name);
        }
        return Ok(StatusJson {
            name: stack_paths.name.clone(),
            state: "stopped".to_owned(),
            runtime_file,
            main: None,
            runners: Vec::new(),
            attached_runners: Vec::new(),
        });
    };

    let main = if runner_selector.is_none() {
        Some(status_process_json_from_main(&state.main, http).await?)
    } else {
        None
    };

    let runners = if main_only {
        Vec::new()
    } else if let Some(selector) = runner_selector {
        let indexes = select_runner_indexes(&state.runners, std::slice::from_ref(selector))?;
        indexes
            .into_iter()
            .map(|idx| state.runners[idx].clone())
            .collect::<Vec<_>>()
    } else {
        state.runners.clone()
    };

    let runners = collect_status_runner_json(&runners, http).await?;
    let runner_states = runners.iter().map(|runner| runner.state.clone()).collect::<Vec<_>>();
    let state_name = derive_overall_state(
        main.as_ref().map(|main| main.state.as_str()),
        &runner_states,
        state,
        main_only,
        runner_selector.is_some(),
    );

    Ok(StatusJson {
        name: state.name.clone(),
        state: state_name.as_str().to_owned(),
        runtime_file,
        main,
        runners,
        attached_runners: if runner_selector.is_some() || main_only {
            state.attached_runners.clone()
        } else {
            state.attached_runners.clone()
        },
    })
}

async fn process_rows(
    state: Option<&DetachedRuntimeState>,
    http: &Client,
) -> Result<Vec<ProcessJson>> {
    let Some(state) = state else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    rows.push(process_json_from_main(&state.main, http).await?);
    rows.extend(collect_runner_json(&state.runners, http).await?);
    Ok(rows)
}

async fn process_json_from_main(main: &MainRuntime, http: &Client) -> Result<ProcessJson> {
    let health_url = format!("http://{}:{}/health", main.address, main.port);
    let status = state_from_pid_and_health(main.pid, probe_health(http, &health_url).await);
    Ok(ProcessJson {
        role: "main".to_owned(),
        state: status.as_str().to_owned(),
        pid: main.pid,
        address: main.address.clone(),
        port: main.port,
        health_url,
        log_path: main.log_path.clone(),
    })
}

async fn status_process_json_from_main(
    main: &MainRuntime,
    http: &Client,
) -> Result<StatusProcessJson> {
    let health_url = format!("http://{}:{}/health", main.address, main.port);
    let status = state_from_pid_and_health(main.pid, probe_health(http, &health_url).await);
    Ok(StatusProcessJson {
        state: status.as_str().to_owned(),
        pid: main.pid,
        address: main.address.clone(),
        port: main.port,
        health_url,
        log_path: main.log_path.clone(),
    })
}

async fn collect_runner_json(
    runners: &[LocalRunnerRuntime],
    http: &Client,
) -> Result<Vec<ProcessJson>> {
    let mut out = Vec::with_capacity(runners.len());
    for runner in runners {
        let health_url = format!("http://{}:{}/health", runner.address, runner.port);
        let status = state_from_pid_and_health(runner.pid, probe_health(http, &health_url).await);
        out.push(ProcessJson {
            role: "runner".to_owned(),
            state: status.as_str().to_owned(),
            pid: runner.pid,
            address: runner.address.clone(),
            port: runner.port,
            health_url,
            log_path: runner.log_path.clone(),
        });
    }
    Ok(out)
}

async fn collect_status_runner_json(
    runners: &[LocalRunnerRuntime],
    http: &Client,
) -> Result<Vec<StatusProcessJson>> {
    let mut out = Vec::with_capacity(runners.len());
    for runner in runners {
        let health_url = format!("http://{}:{}/health", runner.address, runner.port);
        let status = state_from_pid_and_health(runner.pid, probe_health(http, &health_url).await);
        out.push(StatusProcessJson {
            state: status.as_str().to_owned(),
            pid: runner.pid,
            address: runner.address.clone(),
            port: runner.port,
            health_url,
            log_path: runner.log_path.clone(),
        });
    }
    Ok(out)
}

async fn overall_stack_state(
    state: Option<&DetachedRuntimeState>,
    http: &Client,
) -> Result<DerivedState> {
    let Some(state) = state else {
        return Ok(DerivedState::Stopped);
    };
    let main = process_json_from_main(&state.main, http).await?;
    let runners = collect_runner_json(&state.runners, http).await?;
    let runner_states = runners.iter().map(|runner| runner.state.clone()).collect::<Vec<_>>();
    Ok(derive_overall_state(
        Some(main.state.as_str()),
        &runner_states,
        state,
        false,
        false,
    ))
}

fn derive_overall_state(
    main_state: Option<&str>,
    runner_states: &[String],
    _runtime: &DetachedRuntimeState,
    main_only: bool,
    runner_only: bool,
) -> DerivedState {
    let mut states = Vec::new();
    if !runner_only {
        if let Some(main_state) = main_state {
            states.push(DerivedState::from_value(main_state));
        }
    }
    if !main_only {
        states.extend(
            runner_states
                .iter()
                .map(|runner_state| DerivedState::from_value(runner_state)),
        );
    }
    DerivedState::collapse(&states)
}

fn all_runtime_pids(state: &DetachedRuntimeState) -> Vec<u32> {
    let mut pids = vec![state.main.pid];
    pids.extend(state.runners.iter().map(|runner| runner.pid));
    pids
}

fn select_runner_indexes(
    runners: &[LocalRunnerRuntime],
    selectors: &[RunnerSelector],
) -> Result<Vec<usize>> {
    let mut matches = Vec::new();
    for selector in selectors {
        let mut found = false;
        for (idx, runner) in runners.iter().enumerate() {
            if selector.matches(&runner.address, runner.port) && !matches.contains(&idx) {
                matches.push(idx);
                found = true;
            }
        }
        if !found {
            bail!("runner selector '{}' did not match any local runner", selector.raw());
        }
    }
    Ok(matches)
}

fn parse_runner_selectors(values: &[String]) -> Result<Vec<RunnerSelector>> {
    values.iter().map(|value| RunnerSelector::parse(value)).collect()
}

fn read_required_state(stack_paths: &StackPaths) -> Result<DetachedRuntimeState> {
    read_runtime_state(stack_paths)?
        .ok_or_else(|| anyhow!("no detached runtime exists for stack '{}'", stack_paths.name))
}

#[cfg(test)]
mod tests {
    use crate::selectors::{RunnerSelector, normalize_attach_runner};

    #[test]
    fn selector_matching_by_port_address_and_host() {
        let port = RunnerSelector::parse("55880").expect("port selector");
        assert!(port.matches("127.0.0.1", 55880));
        assert!(!port.matches("127.0.0.1", 55881));

        let addr_port = RunnerSelector::parse("10.0.0.8:55880").expect("addr:port");
        assert!(addr_port.matches("10.0.0.8", 55880));
        assert!(!addr_port.matches("10.0.0.8", 55881));

        let addr = RunnerSelector::parse("10.0.0.8").expect("addr");
        assert!(addr.matches("10.0.0.8", 55880));
        assert!(addr.matches("10.0.0.8", 55881));
    }

    #[test]
    fn attach_runner_normalization() {
        assert_eq!(
            normalize_attach_runner("55880").expect("normalize port"),
            "http://127.0.0.1:55880"
        );
        assert_eq!(
            normalize_attach_runner("10.0.0.8").expect("normalize host"),
            "http://10.0.0.8:55880"
        );
        assert_eq!(
            normalize_attach_runner("10.0.0.8:56000").expect("normalize host:port"),
            "http://10.0.0.8:56000"
        );
    }
}
