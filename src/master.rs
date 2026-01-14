use crate::adspower::AdsPowerClient;
use crate::csv_reader::read_accounts_from_csv;
use crate::database::Database;
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

const PID_FILE: &str = "auto-scanner-master.pid";

pub async fn run(
    input_dir: Option<String>,
    backend: String,
    remote_url: String,
    thread_count: usize,
    enable_screenshot: bool,
    stop: bool,
) -> Result<()> {
    if stop {
        return stop_master();
    }

    let input_dir = input_dir.expect("Input directory is required unless --stop is specified");

    info!(
        "Master started. Monitoring directory: {}, Threads: {}, Screenshots: {}, Backend: {}",
        input_dir, thread_count, enable_screenshot, backend
    );

    // Write PID file
    let pid = std::process::id();
    if Path::new(PID_FILE).exists() {
        // Check if process is actually running
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

    let db = Arc::new(Database::new("auto-scanner.db").await?);
    let adspower = if backend == "adspower" {
        Some(Arc::new(AdsPowerClient::new()))
    } else {
        None
    };

    let input_path = PathBuf::from(&input_dir);

    if !input_path.exists() {
        fs::create_dir_all(&input_path).context("Failed to create monitoring directory")?;
    }

    // Channel for file events
    let (tx, mut rx) = mpsc::channel(100);
    let processing_files = Arc::new(Mutex::new(HashSet::new()));

    // Initial scan
    let entries = fs::read_dir(&input_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if is_csv_file(&path) {
            let mut processing = processing_files.lock().unwrap();
            if processing.insert(path.clone()) {
                tx.send(path).await?;
            }
        }
    }

    // Setup watcher
    let tx_clone = tx.clone();
    let processing_files_clone = processing_files.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    info!("Received file event: {:?}", event.kind);
                    // Match Create, Modify (for moves/overwrites), or Rename (for moves into dir)
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                            for path in event.paths {
                                if is_csv_file(&path) {
                                    let mut processing = processing_files_clone.lock().unwrap();
                                    if processing.insert(path.clone()) {
                                        info!("Detected new CSV file: {:?}", path);
                                        let _ = tx_clone.try_send(path);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        },
        Config::default(),
    )?;

    watcher.watch(&input_path, RecursiveMode::NonRecursive)?;

    // We use a custom semaphore implementation to track which thread ID is available
    // Instead of just counting permits, we need to know WHICH specific permit (0..thread_count-1) we got.
    let (permit_tx, permit_rx) = async_channel::bounded(thread_count);
    for i in 0..thread_count {
        permit_tx
            .send(i)
            .await
            .expect("Failed to initialize thread pool");
    }

    let exe_path = env::current_exe().context("Failed to get current executable path")?;

    info!("Waiting for new CSV files...");

    // Setup signal handler for graceful shutdown
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
                    db.clone(),
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

    // Cleanup PID file
    let _ = fs::remove_file(PID_FILE);
    info!("Master shutdown complete");

    Ok(())
}

fn check_process_running(pid: i32) -> bool {
    // 0 signal checks if process exists and we have permission
    signal::kill(Pid::from_raw(pid), None).is_ok()
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

    // Clean up PID file
    let _ = fs::remove_file(PID_FILE);

    Ok(())
}

fn is_csv_file(path: &Path) -> bool {
    path.is_file()
        && path.extension().map_or(false, |ext| ext == "csv")
        && !path
            .file_name()
            .map_or(false, |n| n.to_string_lossy().contains(".done-"))
}

async fn process_file(
    path: &Path,
    batch_name: &str,
    db: Arc<Database>,
    adspower: Option<Arc<AdsPowerClient>>,
    exe_path: &PathBuf,
    backend: &str,
    remote_url: &str,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    enable_screenshot: bool,
) -> Result<()> {
    let accounts = read_accounts_from_csv(path.to_str().unwrap()).await?;
    info!("Read {} accounts from {}", accounts.len(), batch_name);

    db.insert_accounts(&accounts, Some(batch_name)).await?;

    let mut handles = Vec::new();

    for account in accounts {
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

            // Handle AdsPower lifecycle if enabled
            let mut adspower_id = None;
            let mut active_remote_url = remote_url_str.clone();

            if let Some(client) = &adspower {
                match client.ensure_profile_for_thread(thread_index).await {
                    Ok(id) => {
                        info!(
                            "Using AdsPower profile {} (thread {}) for {}",
                            id, thread_index, username
                        );

                        // Update profile settings (clear cookies/cache logic inside)
                        if let Err(e) = client.update_profile_for_account(&id, &username).await {
                            error!("Failed to update AdsPower profile for {}: {}", username, e);
                            let _ = permit_tx.send(thread_index).await;
                            return;
                        }

                        match client.start_browser(&id).await {
                            Ok(ws_url) => {
                                adspower_id = Some(id);
                                active_remote_url = ws_url;
                            }
                            Err(e) => {
                                error!("Failed to start AdsPower browser for {}: {}", username, e);
                                let _ = permit_tx.send(thread_index).await;
                                return; // Skip this worker
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to check/create AdsPower profile for thread {}: {}",
                            thread_index, e
                        );
                        let _ = permit_tx.send(thread_index).await;
                        return; // Skip this worker
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

            let status = cmd.status().await;

            // Cleanup AdsPower
            if let Some(client) = &adspower {
                if let Some(id) = adspower_id {
                    let _ = client.stop_browser(&id).await;
                }
            }

            match status {
                Ok(s) if s.success() => {
                    info!("Worker for {} completed successfully", username);
                }
                Ok(s) => {
                    error!("Worker for {} exited with error status: {}", username, s);
                }
                Err(e) => {
                    error!("Failed to spawn worker for {}: {}", username, e);
                }
            }

            let _ = permit_tx.send(thread_index).await;
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

fn rename_processed_file(path: &Path) -> Result<()> {
    let now = Local::now().format("%Y%m%d-%H%M%S");
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid filename")?;

    let new_name = format!("{}.done-{}.csv", file_name, now);
    let new_path = path.with_file_name(new_name);

    fs::rename(path, &new_path).context("Failed to rename processed file")?;
    info!("Renamed file to {:?}", new_path);
    Ok(())
}
