use crate::core::models::WorkerResult;
use crate::services::email::monitor::EmailMonitor;
use crate::services::file::get_account_source;
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

pub async fn convert_txt_to_csv(path: &Path) -> Result<PathBuf> {
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

pub fn rename_processed_file(path: &Path, doned_dir: &Path) -> Result<PathBuf> {
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

pub async fn prepare_input_file(
    path: &Path,
    email_monitor: &Option<Arc<EmailMonitor>>,
) -> Result<PathBuf> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension != "txt" {
        return Ok(path.to_path_buf());
    }

    let new_path = convert_txt_to_csv(path).await?;

    if let Some(monitor) = email_monitor {
        if let Err(e) = monitor.get_file_tracker().update_file_path(path, &new_path) {
            warn!("Failed to update file tracker path: {}", e);
        }
    }

    fs::remove_file(path).context("Failed to remove original TXT file")?;
    info!("Converted {:?} to CSV and removed original", path);

    Ok(new_path)
}

pub async fn write_results_and_rename(
    path: &Path,
    extension: &str,
    results: Vec<(usize, Option<WorkerResult>)>,
    records: Vec<Vec<String>>,
    headers: Vec<String>,
    doned_dir: &Path,
) -> Result<PathBuf> {
    let source = get_account_source(path);

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

    source.write(path, &new_headers, &new_records).await?;
    info!("Results written back to {:?}", path);

    if extension == "xls" {
        let new_xlsx_path = path.with_extension("xlsx");
        fs::rename(path, &new_xlsx_path).context("Failed to rename .xls to .xlsx")?;
        info!("Renamed processed .xls to .xlsx: {:?}", new_xlsx_path);
        rename_processed_file(&new_xlsx_path, doned_dir)
    } else {
        rename_processed_file(path, doned_dir)
    }
}
