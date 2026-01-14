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

/// 文件处理器
struct FileProcessor {
    adspower: Option<Arc<AdsPowerClient>>,
    backend: String,
    remote_url: String,
    exe_path: PathBuf,
    enable_screenshot: bool,
    doned_dir: PathBuf,
}

impl FileProcessor {
    fn new(
        adspower: Option<Arc<AdsPowerClient>>,
        backend: String,
        remote_url: String,
        exe_path: PathBuf,
        enable_screenshot: bool,
        doned_dir: PathBuf,
    ) -> Self {
        Self {
            adspower,
            backend,
            remote_url,
            exe_path,
            enable_screenshot,
            doned_dir,
        }
    }

    async fn process_incoming_file(
        &self,
        path: PathBuf,
        permit_rx: async_channel::Receiver<usize>,
        permit_tx: async_channel::Sender<usize>,
        email_monitor: Option<Arc<EmailMonitor>>,
    ) -> Result<PathBuf> {
        if !path.exists() {
            anyhow::bail!("File no longer exists: {:?}", path);
        }

        info!("Processing file: {:?}", path);

        let batch_name = self.extract_batch_name(&path);
        let config = self.build_process_config(batch_name.clone());

        process_file(
            &path,
            &batch_name,
            config,
            permit_rx,
            permit_tx,
            email_monitor,
        )
        .await
    }

    fn extract_batch_name(&self, path: &Path) -> String {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn build_process_config(&self, batch_name: String) -> ProcessConfig {
        ProcessConfig {
            batch_name,
            adspower: self.adspower.clone(),
            backend: self.backend.clone(),
            remote_url: self.remote_url.clone(),
            exe_path: self.exe_path.clone(),
            enable_screenshot: self.enable_screenshot,
            doned_dir: self.doned_dir.clone(),
        }
    }
}

fn initialize_logging() -> Result<()> {
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

    Ok(())
}

fn setup_pid_management(daemon: bool) -> Result<()> {
    if daemon {
        return Ok(());
    }

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

    Ok(())
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

    if config.status {
        return check_status();
    }

    if config.stop {
        return stop_master();
    }

    initialize_logging()?;

    let input_dir = input_dir.expect("Input directory is required unless --stop is specified");

    info!(
        "Master started. Monitoring directory: {}, Threads: {}, Screenshots: {}, Backend: {}, Daemon: {}",
        input_dir, config.thread_count, config.enable_screenshot, config.backend, config.daemon
    );

    setup_pid_management(config.daemon)?;

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

    let file_processor = FileProcessor::new(
        adspower.clone(),
        config.backend.clone(),
        config.remote_url.clone(),
        exe_path,
        config.enable_screenshot,
        doned_dir.clone(),
    );

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

                let result = file_processor
                    .process_incoming_file(
                        csv_path.clone(),
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
