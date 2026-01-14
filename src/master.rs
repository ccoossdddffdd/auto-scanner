use crate::infrastructure::adspower::AdsPowerClient;
use crate::processor::{process_file, ProcessConfig};
use crate::services::email::monitor::{EmailConfig, EmailMonitor};
use crate::services::email::tracker::FileTracker;
use anyhow::{Context, Result};
use async_channel;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PID_FILE: &str = "auto-scanner-master.pid";

#[derive(Clone, Debug)]
pub struct MasterConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
    pub enable_screenshot: bool,
    pub stop: bool,
    pub daemon: bool,
    pub status: bool,
    pub enable_email_monitor: bool,
    pub email_poll_interval: u64,
    pub exe_path: Option<PathBuf>,
}

pub async fn run(input_dir: Option<String>, config: MasterConfig) -> Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    if config.status {
        return check_status();
    }

    if config.stop {
        return stop_master();
    }

    // Initialize logging
    let file_appender = tracing_appender::rolling::daily("logs", "auto-scanner.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .init();

    let input_dir = input_dir.expect("Input directory is required unless --stop is specified");

    info!(
        "Master started. Monitoring directory: {}, Threads: {}, Screenshots: {}, Backend: {}, Daemon: {}",
        input_dir, config.thread_count, config.enable_screenshot, config.backend, config.daemon
    );

    if !config.daemon {
        let pid = std::process::id();
        if Path::new(PID_FILE).exists() {
            if let Ok(content) = fs::read_to_string(PID_FILE) {
                if let Ok(old_pid) = content.trim().parse::<i32>() {
                    if check_process_running(old_pid) {
                        anyhow::bail!("Master process is already running (PID: {})", old_pid);
                    }
                }
            }
        }
        fs::write(PID_FILE, pid.to_string()).context("Failed to write PID file")?;
        info!("Written PID {} to {}", pid, PID_FILE);
    }

    let adspower = if config.backend == "adspower" {
        Some(Arc::new(AdsPowerClient::new()))
    } else {
        None
    };

    let input_path = PathBuf::from(&input_dir);

    if !input_path.exists() {
        fs::create_dir_all(&input_path).context("Failed to create monitoring directory")?;
    }

    let doned_dir =
        PathBuf::from(std::env::var("DONED_DIR").unwrap_or_else(|_| "input/doned".to_string()));
    if !doned_dir.exists() {
        fs::create_dir_all(&doned_dir).context("Failed to create doned directory")?;
    }

    let (tx, mut rx) = mpsc::channel::<PathBuf>(100);
    let processing_files = Arc::new(std::sync::Mutex::new(HashSet::new()));

    let entries = fs::read_dir(&input_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if is_supported_file(&path) {
            let should_process = {
                let mut processing = processing_files.lock().unwrap();
                processing.insert(path.clone())
            };
            if should_process {
                tx.send(path).await?;
            }
        }
    }

    let tx_clone = tx.clone();
    let processing_files_clone = processing_files.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                if let EventKind::Create(_) = event.kind {
                    info!("Received file event: {:?}", event.kind);
                    for path in event.paths {
                        if is_supported_file(&path) {
                            let mut processing = processing_files_clone.lock().unwrap();
                            if processing.insert(path.clone()) {
                                info!("Detected new file: {:?}", path);
                                let _ = tx_clone.try_send(path);
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Watch error: {:?}", e),
        },
        Config::default(),
    )?;

    watcher.watch(&input_path, RecursiveMode::NonRecursive)?;

    // 启动邮件监控（如果启用）
    let email_monitor_instance = if config.enable_email_monitor {
        info!("Email monitoring enabled");

        let file_tracker = Arc::new(FileTracker::new());
        let email_config = match EmailConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                warn!(
                    "Failed to create email config: {}, disabling email monitoring",
                    e
                );
                return Ok(());
            }
        };

        match EmailMonitor::new(email_config, file_tracker.clone()) {
            Ok(monitor) => {
                let monitor = Arc::new(monitor);
                // 启动邮件监控任务
                let monitor_clone = monitor.clone();
                let monitor_handle = tokio::spawn(async move {
                    info!("Email monitor task started");
                    if let Err(e) = monitor_clone.start_monitoring().await {
                        error!("Email monitor failed: {}", e);
                    }
                });

                info!(
                    "Email monitor task spawned (handle: {:?})",
                    monitor_handle.id()
                );
                Some(monitor)
            }
            Err(e) => {
                warn!(
                    "Failed to create email monitor: {}, disabling email monitoring",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let (permit_tx, permit_rx) = async_channel::bounded(config.thread_count);
    for i in 0..config.thread_count {
        permit_tx
            .send(i)
            .await
            .expect("Failed to initialize thread pool");
    }

    let exe_path = if let Some(path) = config.exe_path.clone() {
        path
    } else {
        env::current_exe().context("Failed to get current executable path")?
    };

    info!("Waiting for new files...");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down...");
                break;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down...");
                break;
            }
            Some(csv_path) = rx.recv() => {
                 if !csv_path.exists() {
                    let mut processing = processing_files.lock().unwrap();
                    processing.remove(&csv_path);
                    continue;
                }

                info!("Processing file: {:?}", csv_path);
                let batch_name = csv_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let process_config = ProcessConfig {
                    batch_name: batch_name.clone(),
                    adspower: adspower.clone(),
                    backend: config.backend.clone(),
                    remote_url: config.remote_url.clone(),
                    exe_path: exe_path.clone(),
                    enable_screenshot: config.enable_screenshot,
                    doned_dir: doned_dir.clone(),
                };

                let result = process_file(
                    &csv_path,
                    &batch_name,
                    process_config,
                    permit_rx.clone(),
                    permit_tx.clone(),
                    email_monitor_instance.clone(),
                )
                .await;

                {
                    let mut processing = processing_files.lock().unwrap();
                    processing.remove(&csv_path);
                }

                match result {
                    Ok(processed_path) => {
                        info!("Finished processing file: {:?}", processed_path);
                    }
                    Err(e) => {
                        error!("Error processing file {:?}: {}", csv_path, e);
                    }
                }
            }
        }
    }

    let _ = tokio::fs::remove_file(PID_FILE).await;
    info!("Master shutdown complete");

    Ok(())
}

fn check_process_running(pid: i32) -> bool {
    signal::kill(Pid::from_raw(pid), None).is_ok()
}

fn check_status() -> Result<()> {
    let pid_path = Path::new(PID_FILE);
    if !pid_path.exists() {
        println!("Not running");
        return Ok(());
    }

    let pid_str = fs::read_to_string(pid_path).context("Failed to read PID file")?;
    let pid: i32 = pid_str.trim().parse().context("Failed to parse PID")?;

    match signal::kill(Pid::from_raw(pid), None) {
        Ok(_) => {
            println!("Running (PID: {})", pid);
        }
        Err(_) => {
            println!("Not running (Stale PID file found)");
        }
    }

    Ok(())
}

fn stop_master() -> Result<()> {
    if !Path::new(PID_FILE).exists() {
        info!("No PID file found. Master process might not be running.");
        return Ok(());
    }

    let content = fs::read_to_string(PID_FILE).context("Failed to read PID file")?;
    let pid = content
        .trim()
        .parse::<i32>()
        .context("Invalid PID in file")?;

    info!("Stopping master process with PID {}", pid);

    if check_process_running(pid) {
        signal::kill(Pid::from_raw(pid), Signal::SIGTERM).context("Failed to send SIGTERM")?;
        info!("Sent SIGTERM to process {}", pid);
    } else {
        warn!("Process {} not found", pid);
    }

    let _ = fs::remove_file(PID_FILE);

    Ok(())
}

fn is_supported_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    // Check for ignored files
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.contains(".done-") || name.contains(".result.") {
            return false;
        }
        if name.starts_with("~$") {
            // Ignore Excel temp files
            return false;
        }
    }

    // Check extension
    path.extension().is_some_and(|ext| {
        let ext = ext.to_string_lossy().to_lowercase();
        ext == "csv" || ext == "xls" || ext == "xlsx" || ext == "txt"
    })
}
