use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

fn parse_tail_lines(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid tail line count '{value}'"))?;
    if parsed == 0 {
        return Err("tail line count must be greater than zero".to_owned());
    }
    Ok(parsed)
}

#[derive(Debug, Parser)]
#[command(
    name = "previa",
    version,
    about = "CLI local para operar contexts do Previa"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub home: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Start a Previa context")]
    Up(UpArgs),
    #[command(about = "Pull published runtime images")]
    Pull(PullArgs),
    #[command(about = "Stop a detached context or selected local runners")]
    Down(DownArgs),
    #[command(about = "Restart a detached context")]
    Restart(RestartArgs),
    #[command(about = "Show the current state of a context")]
    Status(StatusArgs),
    #[command(about = "List known contexts")]
    List(ListArgs),
    #[command(about = "Show recorded processes for a context")]
    Ps(PsArgs),
    #[command(about = "Read logs from a detached context")]
    Logs(LogsArgs),
    #[command(about = "Open the Previa IDE with the current context")]
    Open(OpenArgs),
    #[command(about = "Print the CLI version")]
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PullTarget {
    Main,
    Runner,
    All,
}

#[derive(Debug, Args)]
#[command(about = "Pull published runtime images")]
pub struct PullArgs {
    #[arg(value_enum, default_value_t = PullTarget::All)]
    pub target: PullTarget,
    #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
    pub version: String,
}

#[derive(Debug, Args)]
#[command(about = "Start a Previa context")]
pub struct UpArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    pub source: Option<String>,
    #[arg(long)]
    pub main_address: Option<String>,
    #[arg(short = 'p', long)]
    pub main_port: Option<u16>,
    #[arg(long)]
    pub runner_address: Option<String>,
    #[arg(short = 'P', long = "runner-port-range")]
    pub runner_port_range: Option<String>,
    #[arg(long)]
    pub runners: Option<usize>,
    #[arg(short = 'i', long = "import", value_name = "PATH")]
    pub import_path: Option<String>,
    #[arg(short = 'r', long)]
    pub recursive: bool,
    #[arg(short = 's', long = "stack", value_name = "STACK")]
    pub stack: Option<String>,
    #[arg(short = 'a', long = "attach-runner")]
    pub attach_runners: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(short = 'd', long)]
    pub detach: bool,
    #[arg(long = "bin")]
    pub bin: bool,
    #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
    pub version: String,
}

#[derive(Debug, Args)]
#[command(about = "Stop a detached context or selected local runners")]
pub struct DownArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    #[arg(long = "all-contexts")]
    pub all_context: bool,
    #[arg(long = "runner")]
    pub runners: Vec<String>,
}

#[derive(Debug, Args)]
#[command(about = "Restart a detached context")]
pub struct RestartArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Debug, Args)]
#[command(about = "Show the current state of a context")]
pub struct StatusArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    #[arg(long)]
    pub main: bool,
    #[arg(long)]
    pub runner: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(about = "List known contexts")]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(about = "Show recorded processes for a context")]
pub struct PsArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(about = "Read logs from a detached context")]
pub struct LogsArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
    #[arg(long)]
    pub main: bool,
    #[arg(long)]
    pub runner: Option<String>,
    #[arg(long)]
    pub follow: bool,
    #[arg(
        short = 't',
        long,
        num_args = 0..=1,
        default_missing_value = "10",
        value_parser = parse_tail_lines
    )]
    pub tail: Option<usize>,
}

#[derive(Debug, Args)]
#[command(about = "Open the Previa IDE with the current context")]
pub struct OpenArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
}
