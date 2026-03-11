use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::cli::UpArgs;
use crate::envfile::{
    default_main_env_map, default_runner_env_map, ensure_default_env_files, read_env_file,
};
use crate::paths::{PreviaPaths, StackPaths, sqlite_database_url};
use crate::runtime::{DetachedRuntimeState, PortRange};
use crate::selectors::normalize_attach_runner;

#[derive(Debug, Clone)]
pub struct ResolvedUpConfig {
    pub previa_paths: PreviaPaths,
    pub stack_paths: StackPaths,
    pub source: Option<PathBuf>,
    pub main: MainResolvedConfig,
    pub main_env: BTreeMap<String, String>,
    pub local_runner_count: usize,
    pub runner_port_range: PortRange,
    pub local_runners: Vec<RunnerLaunch>,
    pub local_runner_ports: Vec<(String, u16)>,
    pub attached_runners: Vec<String>,
    pub dry_run: bool,
    pub detach: bool,
}

#[derive(Debug, Clone)]
pub struct MainResolvedConfig {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct RunnerLaunch {
    pub address: String,
    pub port: u16,
    pub env: BTreeMap<String, String>,
}

impl RunnerLaunch {
    pub fn health_url(&self) -> String {
        format!("http://{}:{}/health", self.address, self.port)
    }
}

impl ResolvedUpConfig {
    pub async fn from_runtime(
        paths: &PreviaPaths,
        stack_paths: &StackPaths,
        state: &DetachedRuntimeState,
    ) -> Result<Self> {
        let main_env = read_env_file(&stack_paths.main_env)?;
        let runner_env = read_env_file(&stack_paths.runner_env)?;
        let local_runner_count = state.runners.len();
        let local_runners = state
            .runners
            .iter()
            .map(|runner| {
                let mut env = runner_env.clone();
                env.insert("ADDRESS".to_owned(), runner.address.clone());
                env.insert("PORT".to_owned(), runner.port.to_string());
                RunnerLaunch {
                    address: runner.address.clone(),
                    port: runner.port,
                    env,
                }
            })
            .collect::<Vec<_>>();

        let mut main_env = merge_env(default_main_env_map(stack_paths), main_env);
        main_env.insert("ADDRESS".to_owned(), state.main.address.clone());
        main_env.insert("PORT".to_owned(), state.main.port.to_string());
        main_env.insert("PREVIA_CONTEXT".to_owned(), stack_paths.name.clone());
        main_env.insert(
            "RUNNER_ENDPOINTS".to_owned(),
            state
                .runners
                .iter()
                .map(|runner| format!("http://{}:{}", runner.address, runner.port))
                .chain(state.attached_runners.clone())
                .collect::<Vec<_>>()
                .join(","),
        );

        Ok(Self {
            previa_paths: paths.clone(),
            stack_paths: stack_paths.clone(),
            source: state.source.as_ref().map(PathBuf::from),
            main: MainResolvedConfig {
                address: state.main.address.clone(),
                port: state.main.port,
            },
            main_env,
            local_runner_count,
            runner_port_range: state.runner_port_range,
            local_runners: local_runners.clone(),
            local_runner_ports: local_runners
                .iter()
                .map(|runner| (runner.address.clone(), runner.port))
                .collect(),
            attached_runners: state.attached_runners.clone(),
            dry_run: false,
            detach: true,
        })
    }

    pub fn main_health_url(&self) -> String {
        format!("http://{}:{}/health", self.main.address, self.main.port)
    }
}

fn validate_port(port: u16, label: &str) -> Result<u16> {
    if port == 0 {
        bail!("invalid {label} '0'");
    }
    Ok(port)
}

fn validate_port_range(range: PortRange) -> Result<PortRange> {
    validate_port(range.start, "runner port range")?;
    validate_port(range.end, "runner port range")?;
    if range.start > range.end {
        bail!("invalid runner port range");
    }
    Ok(range)
}

pub async fn resolve_up_config(
    paths: &PreviaPaths,
    stack_paths: &StackPaths,
    args: UpArgs,
) -> Result<ResolvedUpConfig> {
    if args.dry_run && args.detach {
        bail!("--dry-run cannot be combined with --detach");
    }
    if !paths.main_binary.exists() {
        bail!("missing '{}'", paths.main_binary.display());
    }

    stack_paths.ensure_parent_dirs()?;
    if !args.dry_run {
        ensure_default_env_files(stack_paths)?;
    }

    let source = resolve_compose_source(args.source.as_deref())?;
    let compose = if let Some(source) = &source {
        Some(read_compose_file(source)?)
    } else {
        None
    };

    let main_env_file = if stack_paths.main_env.exists() {
        read_env_file(&stack_paths.main_env)?
    } else {
        default_main_env_map(stack_paths)
    };
    let runner_env_file = if stack_paths.runner_env.exists() {
        read_env_file(&stack_paths.runner_env)?
    } else {
        default_runner_env_map()
    };

    let main_address = args
        .main_address
        .clone()
        .or_else(|| compose.as_ref().and_then(|compose| compose.main.as_ref()?.address.clone()))
        .or_else(|| main_env_file.get("ADDRESS").cloned())
        .unwrap_or_else(|| "0.0.0.0".to_owned());
    validate_address(&main_address)?;

    let main_port = args
        .main_port
        .or_else(|| compose.as_ref().and_then(|compose| compose.main.as_ref()?.port))
        .or_else(|| main_env_file.get("PORT").and_then(|value| value.parse::<u16>().ok()))
        .unwrap_or(5588);
    let main_port = validate_port(main_port, "main port")?;

    let runner_address = args
        .runner_address
        .clone()
        .or_else(|| compose.as_ref().and_then(|compose| {
            compose.runners.as_ref()?.local.as_ref()?.address.clone()
        }))
        .or_else(|| runner_env_file.get("ADDRESS").cloned())
        .unwrap_or_else(|| "127.0.0.1".to_owned());
    validate_address(&runner_address)?;

    let runner_port_range = if let Some(raw) = args.runner_port_range.as_deref() {
        parse_port_range(raw)?
    } else if let Some(compose) = compose.as_ref() {
        if let Some(local) = compose.runners.as_ref().and_then(|runners| runners.local.as_ref()) {
            PortRange {
                start: local.port_range.as_ref().and_then(|value| value.start).unwrap_or(55880),
                end: local.port_range.as_ref().and_then(|value| value.end).unwrap_or(55979),
            }
        } else {
            PortRange {
                start: 55880,
                end: 55979,
            }
        }
    } else {
        PortRange {
            start: 55880,
            end: 55979,
        }
    };
    let runner_port_range = validate_port_range(runner_port_range)?;

    let local_runner_count = args
        .runners
        .or_else(|| {
            compose
                .as_ref()
                .and_then(|compose| compose.runners.as_ref()?.local.as_ref()?.count)
        })
        .unwrap_or(1);

    let attached_raw = if !args.attach_runners.is_empty() {
        args.attach_runners.clone()
    } else {
        compose
            .as_ref()
            .and_then(|compose| compose.runners.as_ref()?.attach.clone())
            .unwrap_or_default()
    };
    let attached_runners = attached_raw
        .iter()
        .map(|value| normalize_attach_runner(value))
        .collect::<Result<Vec<_>>>()?;

    if local_runner_count == 0 && attached_runners.is_empty() {
        bail!("up requires at least one local or attached runner");
    }
    let capacity = (runner_port_range.end - runner_port_range.start + 1) as usize;
    if local_runner_count > capacity {
        bail!("requested local runner count exceeds the configured port range");
    }
    if local_runner_count > 0 && !paths.runner_binary.exists() {
        bail!("missing '{}'", paths.runner_binary.display());
    }

    let mut main_env = merge_env(default_main_env_map(stack_paths), main_env_file);
    if let Some(compose_main) = compose.as_ref().and_then(|compose| compose.main.as_ref()) {
        if let Some(extra_env) = &compose_main.env {
            main_env = merge_env(main_env, extra_env.clone());
        }
    }
    main_env.insert("ADDRESS".to_owned(), main_address.clone());
    main_env.insert("PORT".to_owned(), main_port.to_string());
    main_env.insert("PREVIA_CONTEXT".to_owned(), stack_paths.name.clone());
    main_env
        .entry("ORCHESTRATOR_DATABASE_URL".to_owned())
        .or_insert_with(|| sqlite_database_url(&stack_paths.orchestrator_db));

    let mut local_runners = Vec::with_capacity(local_runner_count);
    let mut local_runner_ports = Vec::with_capacity(local_runner_count);
    let compose_runner_env = compose
        .as_ref()
        .and_then(|compose| compose.runners.as_ref()?.local.as_ref()?.env.clone())
        .unwrap_or_default();

    for offset in 0..local_runner_count {
        let port = runner_port_range.start + offset as u16;
        let mut env = merge_env(default_runner_env_map(), runner_env_file.clone());
        env = merge_env(env, compose_runner_env.clone());
        env.insert("ADDRESS".to_owned(), runner_address.clone());
        env.insert("PORT".to_owned(), port.to_string());
        local_runners.push(RunnerLaunch {
            address: runner_address.clone(),
            port,
            env,
        });
        local_runner_ports.push((runner_address.clone(), port));
    }

    let runner_endpoints = local_runners
        .iter()
        .map(|runner| format!("http://{}:{}", runner.address, runner.port))
        .chain(attached_runners.iter().cloned())
        .collect::<Vec<_>>();
    main_env.insert("RUNNER_ENDPOINTS".to_owned(), runner_endpoints.join(","));

    Ok(ResolvedUpConfig {
        previa_paths: paths.clone(),
        stack_paths: stack_paths.clone(),
        source,
        main: MainResolvedConfig {
            address: main_address,
            port: main_port,
        },
        main_env,
        local_runner_count,
        runner_port_range,
        local_runners,
        local_runner_ports,
        attached_runners,
        dry_run: args.dry_run,
        detach: args.detach,
    })
}

#[derive(Debug, Deserialize)]
struct ComposeFile {
    version: i64,
    main: Option<ComposeMain>,
    runners: Option<ComposeRunners>,
}

#[derive(Debug, Deserialize)]
struct ComposeMain {
    address: Option<String>,
    port: Option<u16>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct ComposeRunners {
    local: Option<ComposeLocalRunners>,
    attach: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ComposeLocalRunners {
    address: Option<String>,
    count: Option<usize>,
    port_range: Option<ComposePortRange>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct ComposePortRange {
    start: Option<u16>,
    end: Option<u16>,
}

fn resolve_compose_source(source: Option<&str>) -> Result<Option<PathBuf>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let path = PathBuf::from(source);
    if source == "." || path.is_dir() {
        let dir = if source == "." {
            std::env::current_dir().context("failed to read current directory")?
        } else {
            path.canonicalize()
                .with_context(|| format!("failed to access '{}'", path.display()))?
        };
        for candidate in [
            dir.join("previa-compose.yaml"),
            dir.join("previa-compose.yml"),
            dir.join("previa-compose.json"),
        ] {
            if candidate.exists() {
                return Ok(Some(candidate));
            }
        }
        bail!("missing compose file in '{}'", dir.display());
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("unsupported compose file extension"))?;
    if !matches!(extension, "json" | "yaml" | "yml") {
        bail!("unsupported compose file extension '{}'", extension);
    }
    Ok(Some(
        path.canonicalize()
            .with_context(|| format!("failed to access '{}'", path.display()))?,
    ))
}

fn read_compose_file(path: &Path) -> Result<ComposeFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read '{}'", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("unsupported compose file extension"))?;
    let compose = match extension {
        "json" => serde_json::from_str::<ComposeFile>(&contents)
            .with_context(|| format!("invalid JSON compose file '{}'", path.display()))?,
        "yaml" | "yml" => serde_yaml::from_str::<ComposeFile>(&contents)
            .with_context(|| format!("invalid YAML compose file '{}'", path.display()))?,
        _ => bail!("unsupported compose file extension '{}'", extension),
    };
    if compose.version != 1 {
        bail!("unsupported compose version '{}'", compose.version);
    }
    Ok(compose)
}

fn parse_port_range(raw: &str) -> Result<PortRange> {
    let (start, end) = raw
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid runner port range '{}'", raw))?;
    let start = start
        .parse::<u16>()
        .with_context(|| format!("invalid runner port range '{}'", raw))?;
    let end = end
        .parse::<u16>()
        .with_context(|| format!("invalid runner port range '{}'", raw))?;
    Ok(PortRange { start, end })
}

fn validate_address(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("address cannot be empty");
    }
    if value.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    {
        return Ok(());
    }
    bail!("invalid address '{}'", value)
}

fn merge_env(
    base: BTreeMap<String, String>,
    override_values: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged = base;
    for (key, value) in override_values {
        merged.insert(key, value);
    }
    merged
}
