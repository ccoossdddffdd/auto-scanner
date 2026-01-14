use anyhow::Result;
use auto_scanner::cli::{Cli, Commands};
use auto_scanner::master;
use auto_scanner::worker;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Master { input, backend, remote_url, thread_count, enable_screenshot } => {
            master::run(input, backend, remote_url, thread_count, enable_screenshot).await?;
        }
        Commands::Worker { username, password, remote_url, backend, enable_screenshot } => {
            worker::run(username, password, remote_url, backend, enable_screenshot).await?;
        }
    }

    Ok(())
}
