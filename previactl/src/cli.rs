use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "previactl", version, about = "CLI local para operar stacks do Previa")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Status(StatusArgs),
    List(ListArgs),
    Ps(PsArgs),
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
