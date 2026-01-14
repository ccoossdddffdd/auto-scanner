use crate::adspower::AdsPowerClient;
use crate::csv_reader::read_accounts_from_csv;
use crate::email_monitor::EmailConfig;
use crate::email_monitor::EmailMonitor;
use crate::excel_handler::{read_accounts_from_excel, write_results_to_excel};
use crate::file_tracker::FileTracker;
use crate::models::WorkerResult;
use anyhow::{Context, Result};
use async_channel;
use chrono::Local;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PID_FILE: &str = "auto-scanner-master.pid";

#[derive(Clone)]
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

#[derive(Clone)]
pub struct ProcessConfig {
    pub batch_name: String,
    pub adspower: Option<Arc<AdsPowerClient>>,
    pub backend: String,
    pub remote_url: String,
    pub exe_path: PathBuf,
    pub enable_screenshot: bool,
    pub doned_dir: PathBuf,
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

    let _ = fs::remove_file(PID_FILE);
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

async fn process_file(
    path: &Path,
    batch_name: &str,
    config: ProcessConfig,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    email_monitor: Option<Arc<EmailMonitor>>,
) -> Result<PathBuf> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_excel = extension == "xls" || extension == "xlsx";
    let is_txt = extension == "txt";

    // Handle TXT to CSV conversion
    let path_to_process = if is_txt {
        let new_path = convert_txt_to_csv(path).await?;

        // Update file tracker mapping
        if let Some(monitor) = &email_monitor {
            if let Err(e) = monitor.get_file_tracker().update_file_path(path, &new_path) {
                warn!("Failed to update file tracker path: {}", e);
            }
        }

        // We delete the original txt file as requested
        fs::remove_file(path).context("Failed to remove original TXT file")?;
        info!("Converted {:?} to CSV and removed original", path);
        new_path
    } else {
        path.to_path_buf()
    };

    // Lookup Email ID
    let email_id = if let Some(monitor) = &email_monitor {
        let filename = path_to_process
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        monitor.get_file_tracker().find_email_by_file(filename)
    } else {
        None
    };

    let processing_result = async {
        // Read accounts and original records/headers
        let (accounts, records, headers) = if is_excel {
            read_accounts_from_excel(&path_to_process)?
        } else {
            read_accounts_from_csv(path_to_process.to_str().unwrap()).await?
        };

        info!("Read {} accounts from {}", accounts.len(), batch_name);

        let mut handles = Vec::new();

        for (index, account) in accounts.iter().enumerate() {
            let thread_index = permit_rx.recv().await.unwrap();

            let exe_path = config.exe_path.clone();
            let username = account.username.clone();
            let password = account.password.clone();
            let backend_str = config.backend.clone();
            let remote_url_str = config.remote_url.clone();

            let adspower = config.adspower.clone();
            let permit_tx = permit_tx.clone();

            let handle = tokio::spawn(async move {
                info!(
                    "Spawning worker for {} on thread {}",
                    username, thread_index
                );

                let mut adspower_id = None;
                let mut active_remote_url = remote_url_str.clone();

                if let Some(client) = &adspower {
                    match client.ensure_profile_for_thread(thread_index).await {
                        Ok(id) => {
                            if let Err(e) = client.update_profile_for_account(&id, &username).await
                            {
                                error!("Failed to update AdsPower profile for {}: {}", username, e);
                                let _ = permit_tx.send(thread_index).await;
                                return (index, None);
                            }

                            match client.start_browser(&id).await {
                                Ok(ws_url) => {
                                    adspower_id = Some(id);
                                    active_remote_url = ws_url;
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to start AdsPower browser for {}: {}",
                                        username, e
                                    );
                                    let _ = permit_tx.send(thread_index).await;
                                    return (index, None);
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to check/create AdsPower profile for thread {}: {}",
                                thread_index, e
                            );
                            let _ = permit_tx.send(thread_index).await;
                            return (index, None);
                        }
                    }
                }

                let mut cmd = Command::new(exe_path);
                cmd.arg("worker")
                    .arg("--username")
                    .arg(&username)
                    .arg("--password")
                    .arg(&password)
                    .arg("--remote-url")
                    .arg(&active_remote_url)
                    .arg("--backend")
                    .arg(&backend_str);

                if config.enable_screenshot {
                    cmd.arg("--enable-screenshot");
                }

                // Capture output
                let output = cmd.output().await;

                if let Some(client) = &adspower {
                    if let Some(id) = adspower_id {
                        let _ = client.stop_browser(&id).await;
                    }
                }

                let _ = permit_tx.send(thread_index).await;

                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        // Find JSON result
                        for line in stdout.lines() {
                            if let Some(json_str) = line.strip_prefix("RESULT_JSON:") {
                                if let Ok(result) = serde_json::from_str::<WorkerResult>(json_str) {
                                    return (index, Some(result));
                                }
                            }
                        }
                        error!("Worker for {} did not return valid JSON result", username);
                        (index, None)
                    }
                    Err(e) => {
                        error!("Failed to run worker for {}: {}", username, e);
                        (index, None)
                    }
                }
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(res) = handle.await {
                results.push(res);
            }
        }

        // Sort by index to maintain original order
        results.sort_by_key(|k| k.0);

        // Overwrite the original file
        if is_excel {
            // Write Headers
            let mut new_headers = headers.clone();
            new_headers.push("状态".to_string());
            new_headers.push("验证码".to_string());
            new_headers.push("2FA".to_string());
            new_headers.push("信息".to_string());

            let mut new_records = Vec::new();
            for (idx, worker_res_opt) in results {
                if let Some(record) = records.get(idx) {
                    let mut new_record = record.clone();
                    if let Some(res) = worker_res_opt {
                        new_record.push(res.status);
                        new_record.push(res.captcha);
                        new_record.push(res.two_fa);
                        new_record.push(res.message);
                    } else {
                        new_record.push("系统错误".to_string());
                        new_record.push("未知".to_string());
                        new_record.push("未知".to_string());
                        new_record.push("Worker 执行失败".to_string());
                    }
                    new_records.push(new_record);
                }
            }
            write_results_to_excel(&path_to_process, &new_headers, &new_records)?;
        } else {
            let mut wtr = csv::Writer::from_path(&path_to_process)?;

            // Write Headers
            let mut new_headers = headers.clone();
            new_headers.push("状态".to_string());
            new_headers.push("验证码".to_string());
            new_headers.push("2FA".to_string());
            new_headers.push("信息".to_string());
            wtr.write_record(&new_headers)?;

            for (idx, worker_res_opt) in results {
                if let Some(record) = records.get(idx) {
                    let mut new_record = record.clone();

                    if let Some(res) = worker_res_opt {
                        new_record.push(res.status);
                        new_record.push(res.captcha);
                        new_record.push(res.two_fa);
                        new_record.push(res.message);
                    } else {
                        // Default failure if no result returned
                        new_record.push("系统错误".to_string());
                        new_record.push("未知".to_string());
                        new_record.push("未知".to_string());
                        new_record.push("Worker 执行失败".to_string());
                    }
                    wtr.write_record(&new_record)?;
                }
            }
            wtr.flush()?;
        }

        info!("Results written back to {:?}", path_to_process);

        if extension == "xls" {
            // The write_results_to_excel function writes XLSX format.
            // If we passed a .xls path, the file content is now XLSX but extension is .xls.
            // We should rename it to .xlsx
            let new_xlsx_path = path_to_process.with_extension("xlsx");
            fs::rename(&path_to_process, &new_xlsx_path)
                .context("Failed to rename .xls to .xlsx")?;
            info!("Renamed processed .xls to .xlsx: {:?}", new_xlsx_path);
            // Original .xls is effectively "deleted" (replaced/renamed)
            rename_processed_file(&new_xlsx_path, &config.doned_dir)
        } else {
            rename_processed_file(&path_to_process, &config.doned_dir)
        }
    }
    .await;

    // Handle Notifications
    if let Some(monitor) = &email_monitor {
        if let Some(id) = email_id {
            let metadata = monitor.get_file_tracker().get_email_metadata(&id);
            let from = metadata.map(|m| m.from).unwrap_or_default();

            match &processing_result {
                Ok(final_path) => {
                    info!("Sending success notification to {}", from);
                    if let Err(e) = monitor
                        .send_success_notification(&from, final_path.clone())
                        .await
                    {
                        error!("Failed to send success notification: {}", e);
                    }
                    if let Err(e) = monitor
                        .get_file_tracker()
                        .mark_success(&id, final_path.clone())
                    {
                        warn!("Failed to mark success in tracker: {}", e);
                    }
                }
                Err(e) => {
                    info!("Sending failure notification to {}", from);
                    if let Err(e) = monitor
                        .send_failure_notification(&from, &e.to_string(), None)
                        .await
                    {
                        error!("Failed to send failure notification: {}", e);
                    }
                    if let Err(e) = monitor
                        .get_file_tracker()
                        .mark_failed(&id, e.to_string(), None)
                    {
                        warn!("Failed to mark failed in tracker: {}", e);
                    }
                }
            }
        }
    }

    processing_result
}

async fn convert_txt_to_csv(path: &Path) -> Result<PathBuf> {
    info!("Converting TXT to CSV: {:?}", path);
    let content = tokio::fs::read_to_string(path).await?;
    let mut csv_content = String::from("username,password\n");

    for line in content.lines() {
        if let Some((user, pass)) = line.split_once(':') {
            let user = user.trim();
            let pass = pass.trim();
            if !user.is_empty() && !pass.is_empty() {
                csv_content.push_str(&format!("{},{}\n", user, pass));
            }
        }
    }

    let csv_path = path.with_extension("csv");
    tokio::fs::write(&csv_path, csv_content).await?;

    Ok(csv_path)
}

fn rename_processed_file(path: &Path, doned_dir: &Path) -> Result<PathBuf> {
    let now = Local::now().format("%Y%m%d-%H%M%S");
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid filename")?;

    // We use the provided doned_dir instead of assuming it's in the parent directory
    if !doned_dir.exists() {
        fs::create_dir_all(doned_dir).context("Failed to create doned directory")?;
    }

    let new_name = format!("{}.done-{}.csv", file_name, now);
    let new_path = doned_dir.join(new_name);

    fs::rename(path, &new_path).context("Failed to move processed file to doned directory")?;
    info!("Moved processed file to {:?}", new_path);
    Ok(new_path)
}
