use anyhow::{Context, Result};
use auto_scanner::cli::{Cli, Commands};
use auto_scanner::master;
use auto_scanner::worker;
use clap::Parser;
use daemonize::Daemonize;
use std::fs::File;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PID_FILE: &str = "auto-scanner-master.pid";

struct PidTime;

impl tracing_subscriber::fmt::time::FormatTime for PidTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{} [{}]",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6fZ"),
            std::process::id()
        )
    }
}

fn main() -> Result<()> {
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
            status,
            enable_email_monitor,
            email_poll_interval,
        } => {
            if daemon && !stop && !status {
                let stdout = File::create("logs/auto-scanner.out")
                    .context("Failed to create stdout file")?;
                let stderr = File::create("logs/auto-scanner.err")
                    .context("Failed to create stderr file")?;

                let daemonize = Daemonize::new()
                    .pid_file(PID_FILE) // Use daemonize to handle PID file creation
                    .chown_pid_file(true)
                    .working_directory(".")
                    .stdout(stdout)
                    .stderr(stderr);

                match daemonize.start() {
                    Ok(_) => {
                        // We are now in the daemon process
                    }
                    Err(e) => {
                        eprintln!("Error, {}", e);
                        anyhow::bail!("Failed to daemonize: {}", e);
                    }
                }
            }

            // Create runtime and run master
            let config = master::MasterConfig {
                backend,
                remote_url,
                thread_count,
                enable_screenshot,
                stop,
                daemon,
                status,
                enable_email_monitor,
                email_poll_interval,
                exe_path: None,
            };
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async { master::run(input, config).await })?;
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
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::stdout)
                        .with_timer(PidTime),
                )
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_timer(PidTime),
                )
                .init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                worker::run(username, password, remote_url, backend, enable_screenshot).await
            })?;
        }
    }

    Ok(())
}
