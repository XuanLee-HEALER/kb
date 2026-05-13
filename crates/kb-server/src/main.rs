use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use kb_server::{run_server, Config};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "kb-server", about = "KB delta knowledge service")]
struct Cli {
    /// Path to sqlite database file.
    #[arg(long, env = "KB_DB", default_value = "./data/kb.sqlite")]
    db: std::path::PathBuf,

    /// Bind address.
    #[arg(long, env = "KB_BIND", default_value = "127.0.0.1:7890")]
    bind: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let cfg = Config {
        db_path: cli.db,
        bind: cli.bind,
    };

    match real_main(cfg).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("fatal: {e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn real_main(cfg: Config) -> Result<()> {
    run_server(cfg).await
}
