use anyhow::Result;
use auto_scanner::core::cli::{Cli, Commands};
use auto_scanner::infrastructure::daemon::start_daemon;
use auto_scanner::infrastructure::logging::init_logging;
use auto_scanner::services::{master, worker};
use clap::Parser;

const PID_FILE: &str = "auto-scanner-master.pid";

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Master {
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
                start_daemon(PID_FILE, "logs/auto-scanner.out", "logs/auto-scanner.err")?;
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
            rt.block_on(async { master::run(config).await })?;
        }
        Commands::Worker {
            username,
            password,
            remote_url,
            backend,
            enable_screenshot,
        } => {
            // Initialize logging for Worker
            init_logging("auto-scanner-worker", false)?;

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                worker::run(username, password, remote_url, backend, enable_screenshot).await
            })?;
        }
    }

    Ok(())
}
