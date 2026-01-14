use crate::infrastructure::adspower::AdsPowerClient;
use crate::infrastructure::logging::init_logging;
use crate::infrastructure::process::PidManager;
use crate::services::email::monitor::{EmailConfig, EmailMonitor};
use crate::services::email::tracker::FileTracker;
use crate::services::processor::{process_file, ProcessConfig};
use anyhow::{Context, Result};
use async_channel;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

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

fn create_file_watcher(
    _input_path: &Path,
    tx: mpsc::Sender<PathBuf>,
    processing_files: Arc<std::sync::Mutex<HashSet<PathBuf>>>,
) -> Result<RecommendedWatcher> {
    let watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                if let EventKind::Create(_) = event.kind {
                    info!("Received file event: {:?}", event.kind);
                    for path in event.paths {
                        if is_supported_file(&path) {
                            let mut processing = processing_files.lock().unwrap();
                            if processing.insert(path.clone()) {
                                info!("Detected new file: {:?}", path);
                                let _ = tx.try_send(path);
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Watch error: {:?}", e),
        },
        Config::default(),
    )?;

    Ok(watcher)
}

async fn initialize_email_monitor(config: &MasterConfig) -> Option<Arc<EmailMonitor>> {
    if !config.enable_email_monitor {
        return None;
    }

    info!("Email monitoring enabled");

    let file_tracker = Arc::new(FileTracker::new());
    let email_config = match EmailConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            warn!(
                "Failed to create email config: {}, disabling email monitoring",
                e
            );
            return None;
        }
    };

    match EmailMonitor::new(email_config, file_tracker.clone()) {
        Ok(monitor) => {
            let monitor = Arc::new(monitor);
            let monitor_clone = monitor.clone();
            tokio::spawn(async move {
                info!("Email monitor task started");
                if let Err(e) = monitor_clone.start_monitoring().await {
                    error!("Email monitor failed: {}", e);
                }
            });
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
}

pub async fn run(input_dir: Option<String>, config: MasterConfig) -> Result<()> {
    dotenv::dotenv().ok();

    let pid_manager = PidManager::new(PID_FILE);

    if config.status {
        return pid_manager.check_status();
    }

    if config.stop {
        return pid_manager.stop();
    }

    init_logging("auto-scanner", config.daemon)?;

    let input_dir = input_dir.expect("Input directory is required unless --stop is specified");

    info!(
        "Master started. Monitoring directory: {}, Threads: {}, Screenshots: {}, Backend: {}, Daemon: {}",
        input_dir, config.thread_count, config.enable_screenshot, config.backend, config.daemon
    );

    if !config.daemon {
        pid_manager.write_pid()?;
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

    // Scan for existing files
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

    // Setup file watcher
    let mut watcher = create_file_watcher(&input_path, tx.clone(), processing_files.clone())?;
    watcher.watch(&input_path, RecursiveMode::NonRecursive)?;

    // Initialize email monitor
    let email_monitor_instance = initialize_email_monitor(&config).await;

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

    pid_manager.remove_pid_file();
    info!("Master shutdown complete");

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
