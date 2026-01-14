use anyhow::Result;
use auto_scanner::cli::{Cli, Commands};
use auto_scanner::master;
use auto_scanner::worker;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Master {
            input,
            backend,
            remote_url,
            thread_count,
            enable_screenshot,
            stop,
            daemon,
        } => {
            // Initialize logging here for Master, as it might need to happen after daemonization
            master::run(
                input,
                backend,
                remote_url,
                thread_count,
                enable_screenshot,
                stop,
                daemon,
            )
            .await?;
        }
        Commands::Worker {
            username,
            password,
            remote_url,
            backend,
            enable_screenshot,
        } => {
            // Initialize logging for Worker
            let file_appender = tracing_appender::rolling::daily("logs", "auto-scanner-worker.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false),
                )
                .init();

            worker::run(username, password, remote_url, backend, enable_screenshot).await?;
        }
    }

    Ok(())
}
