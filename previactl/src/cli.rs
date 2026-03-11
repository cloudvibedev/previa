use clap::{Args, CommandFactory, Parser, Subcommand};

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
    disable_version_flag = true,
    about = "CLI local para operar stacks do Previa"
)]
pub struct Cli {
    #[arg(short = 'v', long = "version")]
    pub version: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn parse_or_exit() -> Self {
        let cli = Self::parse();
        if cli.version || cli.command.is_some() {
            return cli;
        }
        Self::command()
            .error(clap::error::ErrorKind::MissingSubcommand, "a subcommand is required")
            .exit();
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Up(UpArgs),
    Down(DownArgs),
    Restart(RestartArgs),
    Status(StatusArgs),
    List(ListArgs),
    Ps(PsArgs),
    Logs(LogsArgs),
    Version,
}

#[derive(Debug, Args)]
pub struct UpArgs {
    #[arg(long, default_value = "default")]
    pub name: String,
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
    #[arg(long, default_value = "default")]
    pub name: String,
    #[arg(long = "runner")]
    pub runners: Vec<String>,
}

#[derive(Debug, Args)]
pub struct RestartArgs {
    #[arg(long, default_value = "default")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long, default_value = "default")]
    pub name: String,
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
    #[arg(long, default_value = "default")]
    pub name: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(long, default_value = "default")]
    pub name: String,
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
