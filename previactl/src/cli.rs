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
    name = "previactl",
    version,
    about = "CLI local para operar contexts do Previa"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Up(UpArgs),
    Pull(PullArgs),
    Down(DownArgs),
    Restart(RestartArgs),
    Status(StatusArgs),
    List(ListArgs),
    Ps(PsArgs),
    Logs(LogsArgs),
    Open(OpenArgs),
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PullTarget {
    Main,
    Runner,
    All,
}

#[derive(Debug, Args)]
pub struct PullArgs {
    #[arg(value_enum, default_value_t = PullTarget::All)]
    pub target: PullTarget,
    #[arg(long, default_value = "latest")]
    pub version: String,
}

#[derive(Debug, Args)]
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
    #[arg(short = 'r', long)]
    pub runners: Option<usize>,
    #[arg(short = 'a', long = "attach-runner")]
    pub attach_runners: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(short = 'd', long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
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
pub struct RestartArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
}

#[derive(Debug, Args)]
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
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
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
pub struct OpenArgs {
    #[arg(
        long = "context",
        value_name = "CONTEXT",
        default_value = "default",
        help = "Context name"
    )]
    pub context: String,
}
