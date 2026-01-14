use crate::csv_reader::read_accounts_from_csv;
use crate::database::Database;
use anyhow::{Context, Result};
use chrono::Local;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info};

pub async fn run(
    input_dir: String,
    backend: String,
    remote_url: String,
    thread_count: usize,
    enable_screenshot: bool,
) -> Result<()> {
    info!(
        "Master started. Monitoring directory: {}, Threads: {}, Screenshots: {}",
        input_dir, thread_count, enable_screenshot
    );

    let db = Arc::new(Database::new("auto-scanner.db").await?);
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

    let exe_path = env::current_exe().context("Failed to get current executable path")?;
    let semaphore = Arc::new(Semaphore::new(thread_count));

    info!("Waiting for new CSV files...");

    while let Some(csv_path) = rx.recv().await {
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
            &exe_path,
            &backend,
            &remote_url,
            semaphore.clone(),
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
    exe_path: &PathBuf,
    backend: &str,
    remote_url: &str,
    semaphore: Arc<Semaphore>,
    enable_screenshot: bool,
) -> Result<()> {
    let accounts = read_accounts_from_csv(path.to_str().unwrap()).await?;
    info!("Read {} accounts from {}", accounts.len(), batch_name);

    db.insert_accounts(&accounts, Some(batch_name)).await?;

    let mut handles = Vec::new();

    for account in accounts {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let exe_path = exe_path.clone();
        let username = account.username.clone();
        let password = account.password.clone();
        let backend_str = backend.to_string();
        let remote_url_str = remote_url.to_string();

        let handle = tokio::spawn(async move {
            info!("Spawning worker for {}", username);

            let mut cmd = Command::new(exe_path);
            cmd.arg("worker")
                .arg("--username")
                .arg(&username)
                .arg("--password")
                .arg(&password)
                .arg("--remote-url")
                .arg(&remote_url_str)
                .arg("--backend")
                .arg(&backend_str);

            if enable_screenshot {
                cmd.arg("--enable-screenshot");
            }

            let status = cmd.status().await;

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

            drop(permit);
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
