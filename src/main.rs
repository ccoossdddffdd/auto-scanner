mod cli;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .init();

    let cli = Cli::parse();
    
    info!("Starting auto-scanner");
    info!("Input file: {}", cli.input);

    Ok(())
}
