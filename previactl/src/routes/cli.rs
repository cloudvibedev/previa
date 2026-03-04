use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::services::manager::PreviaManager;

#[derive(Debug, Parser)]
#[command(
    name = "previactl",
    version,
    about = "CLI do Previa para instalar, atualizar e remover binarios (Linux)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Instala a ultima versao disponivel sem sobrescrever a versao atual ativa.
    Install,
    /// Compara versao atual com a ultima disponivel e oferece atualizacao.
    Update,
    /// Remove os binarios gerenciados do previa-main e previa-runner.
    Uninstall,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let manager = PreviaManager::new()?;

    match cli.command {
        Commands::Install => manager.install_latest().await,
        Commands::Update => manager.update().await,
        Commands::Uninstall => manager.uninstall(),
    }
}
