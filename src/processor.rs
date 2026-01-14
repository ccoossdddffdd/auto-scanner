use crate::core::models::WorkerResult;
use crate::infrastructure::adspower::AdsPowerClient;
use crate::services::email::monitor::EmailMonitor;
use crate::services::file::get_account_source;
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{error, info, warn};

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

pub async fn process_file(
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
        let source = get_account_source(&path_to_process);
        let (accounts, records, headers) = source.read(&path_to_process).await?;

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
        source
            .write(&path_to_process, &new_headers, &new_records)
            .await?;

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
