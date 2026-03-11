mod cli;
mod health;
mod output;
mod paths;
mod runtime;
mod selectors;

use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use reqwest::Client;

use crate::cli::{Cli, Commands, PsArgs, StatusArgs};
use crate::health::{DerivedState, probe_health, state_from_pid_and_health};
use crate::output::{
    ListEntryJson, ProcessJson, StatusJson, print_list_human, print_process_rows,
    print_status_human,
};
use crate::paths::{PreviaPaths, StackPaths};
use crate::runtime::{DetachedRuntimeState, LocalRunnerRuntime, MainRuntime, read_runtime_state};
use crate::selectors::{RunnerSelector, parse_stack_name};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = PreviaPaths::discover()?;
    let http = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .context("failed to build HTTP client")?;

    match cli.command {
        Commands::Status(args) => cmd_status(&paths, &http, args).await,
        Commands::List(args) => cmd_list(&paths, &http, args.json).await,
        Commands::Ps(args) => cmd_ps(&paths, &http, args).await,
    }
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

    let status =
        build_status_json(&stack_paths, state.as_ref(), http, selector.as_ref(), args.main).await?;
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
        Some(process_json_from_main(&state.main, http).await?)
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

    let runners = collect_runner_json(&runners, http).await?;
    let state_name =
        derive_overall_state(main.as_ref(), &runners, main_only, runner_selector.is_some());

    Ok(StatusJson {
        name: state.name.clone(),
        state: state_name.as_str().to_owned(),
        runtime_file,
        main,
        runners,
        attached_runners: state.attached_runners.clone(),
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

async fn overall_stack_state(
    state: Option<&DetachedRuntimeState>,
    http: &Client,
) -> Result<DerivedState> {
    let Some(state) = state else {
        return Ok(DerivedState::Stopped);
    };
    let main = process_json_from_main(&state.main, http).await?;
    let runners = collect_runner_json(&state.runners, http).await?;
    Ok(derive_overall_state(
        Some(&main),
        &runners,
        false,
        false,
    ))
}

fn derive_overall_state(
    main: Option<&ProcessJson>,
    runners: &[ProcessJson],
    main_only: bool,
    runner_only: bool,
) -> DerivedState {
    let mut states = Vec::new();
    if !runner_only && let Some(main) = main {
        states.push(DerivedState::from_value(&main.state));
    }
    if !main_only {
        states.extend(
            runners
                .iter()
                .map(|runner| DerivedState::from_value(&runner.state)),
        );
    }
    DerivedState::collapse(&states)
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
