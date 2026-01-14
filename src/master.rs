use crate::adspower::AdsPowerClient;
use crate::csv_reader::read_accounts_from_csv;
use crate::excel_handler::{read_accounts_from_excel, write_results_to_excel};
use crate::models::WorkerResult;
use anyhow::{Context, Result};
use chrono::Local;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PID_FILE: &str = "auto-scanner-master.pid";

pub async fn run(
    input_dir: Option<String>,
    backend: String,
    remote_url: String,
    thread_count: usize,
    enable_screenshot: bool,
    stop: bool,
    daemon: bool,
    status: bool,
) -> Result<()> {
    if status {
        return check_status();
    }

    if stop {
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
        input_dir, thread_count, enable_screenshot, backend, daemon
    );

    if !daemon {
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

        let mut pid_file = File::create(PID_FILE).context("Failed to create PID file")?;
        write!(pid_file, "{}", pid)?;
        info!("Written PID {} to {}", pid, PID_FILE);
    }

    let adspower = if backend == "adspower" {
        Some(Arc::new(AdsPowerClient::new()))
    } else {
        None
    };

    let input_path = PathBuf::from(&input_dir);

    if !input_path.exists() {
        fs::create_dir_all(&input_path).context("Failed to create monitoring directory")?;
    }

    let (tx, mut rx) = mpsc::channel(100);
    let processing_files = Arc::new(Mutex::new(HashSet::new()));

    let entries = fs::read_dir(&input_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if is_supported_file(&path) {
            let mut processing = processing_files.lock().unwrap();
            if processing.insert(path.clone()) {
                tx.send(path).await?;
            }
        }
    }

    let tx_clone = tx.clone();
    let processing_files_clone = processing_files.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| match res {
            Ok(event) => match event.kind {
                EventKind::Create(_) => {
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
                _ => {}
            },
            Err(e) => error!("Watch error: {:?}", e),
        },
        Config::default(),
    )?;

    watcher.watch(&input_path, RecursiveMode::NonRecursive)?;

    let (permit_tx, permit_rx) = async_channel::bounded(thread_count);
    for i in 0..thread_count {
        permit_tx
            .send(i)
            .await
            .expect("Failed to initialize thread pool");
    }

    let exe_path = env::current_exe().context("Failed to get current executable path")?;

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

                let result = process_file(
                    &csv_path,
                    &batch_name,
                    adspower.clone(),
                    &exe_path,
                    &backend,
                    &remote_url,
                    permit_rx.clone(),
                    permit_tx.clone(),
                    enable_screenshot,
                )
                .await;

                {
                    let mut processing = processing_files.lock().unwrap();
                    processing.remove(&csv_path);
                }

                match result {
                    Ok(_) => {
                        info!("Finished processing file: {:?}", csv_path);
                        rename_processed_file(&csv_path)?;
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
    path.extension().map_or(false, |ext| {
        let ext = ext.to_string_lossy().to_lowercase();
        ext == "csv" || ext == "xls" || ext == "xlsx"
    })
}

async fn process_file(
    path: &Path,
    batch_name: &str,
    adspower: Option<Arc<AdsPowerClient>>,
    exe_path: &PathBuf,
    backend: &str,
    remote_url: &str,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    enable_screenshot: bool,
) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_excel = extension == "xls" || extension == "xlsx";

    // Read accounts and original records/headers
    let (accounts, records, headers) = if is_excel {
        read_accounts_from_excel(path)?
    } else {
        read_accounts_from_csv(path.to_str().unwrap()).await?
    };

    info!("Read {} accounts from {}", accounts.len(), batch_name);

    let mut handles = Vec::new();

    for (index, account) in accounts.iter().enumerate() {
        let thread_index = permit_rx.recv().await.unwrap();

        let exe_path = exe_path.clone();
        let username = account.username.clone();
        let password = account.password.clone();
        let backend_str = backend.to_string();
        let remote_url_str = remote_url.to_string();

        let adspower = adspower.clone();
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
                        if let Err(e) = client.update_profile_for_account(&id, &username).await {
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
                                error!("Failed to start AdsPower browser for {}: {}", username, e);
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

            if enable_screenshot {
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
        write_results_to_excel(path, &new_headers, &new_records)?;
    } else {
        let mut wtr = csv::Writer::from_path(path)?;
        
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

    info!("Results written back to {:?}", path);

    Ok(())
}

fn rename_processed_file(path: &Path) -> Result<()> {
    let now = Local::now().format("%Y%m%d-%H%M%S");
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid filename")?;

    let parent = path.parent().context("Invalid file path")?;
    let doned_dir = parent.join("doned");

    if !doned_dir.exists() {
        fs::create_dir_all(&doned_dir).context("Failed to create doned directory")?;
    }

    let new_name = format!("{}.done-{}.csv", file_name, now);
    let new_path = doned_dir.join(new_name);

    fs::rename(path, &new_path).context("Failed to move processed file to doned directory")?;
    info!("Moved processed file to {:?}", new_path);
    Ok(())
}
