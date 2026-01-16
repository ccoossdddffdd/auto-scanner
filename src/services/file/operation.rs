use crate::core::models::WorkerResult;
use crate::services::email::monitor::EmailMonitor;
use crate::services::file::get_account_source;
use crate::services::file_policy::FilePolicyService;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

pub async fn convert_txt_to_csv(path: &Path) -> Result<PathBuf> {
    info!("正在将 TXT 转换为 CSV: {:?}", path);
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
    if !doned_dir.exists() {
        fs::create_dir_all(doned_dir).context("创建完成目录失败")?;
    }

    let new_path = FilePolicyService::generate_processed_path(path, doned_dir)?;

    fs::rename(path, &new_path).context("移动处理后的文件到完成目录失败")?;
    info!("已将处理后的文件移动到 {:?}", new_path);
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
            warn!("更新文件追踪器路径失败: {}", e);
        }
    }

    fs::remove_file(path).context("删除原始 TXT 文件失败")?;
    info!("已将 {:?} 转换为 CSV 并删除原文件", path);

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
    new_headers.push("信息".to_string());

    // Collect all dynamic keys from results
    let mut dynamic_keys = std::collections::BTreeSet::new();
    for (_, worker_res_opt) in &results {
        if let Some(res) = worker_res_opt {
            if let Some(data) = &res.data {
                for key in data.keys() {
                    dynamic_keys.insert(key.clone());
                }
            }
        }
    }

    // Add dynamic keys to headers
    for key in &dynamic_keys {
        new_headers.push(key.clone());
    }

    let mut new_records = Vec::new();
    for (idx, worker_res_opt) in results {
        if let Some(record) = records.get(idx) {
            let mut new_record = record.clone();
            if let Some(res) = worker_res_opt {
                new_record.push(res.status.clone());
                new_record.push(res.message.clone());

                // Add values for dynamic keys
                for key in &dynamic_keys {
                    let value = if let Some(data) = &res.data {
                        data.get(key)
                            .map(|v| match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Null => "无".to_string(),
                                _ => v.to_string(),
                            })
                            .unwrap_or_else(|| "未知".to_string())
                    } else {
                        "未知".to_string()
                    };
                    new_record.push(value);
                }
            } else {
                new_record.push("系统错误".to_string());
                new_record.push("Worker 执行失败".to_string());
                // Fill dynamic keys with "未知"
                for _ in &dynamic_keys {
                    new_record.push("未知".to_string());
                }
            }
            new_records.push(new_record);
        }
    }

    source.write(path, &new_headers, &new_records).await?;
    info!("结果已写回 {:?}", path);

    if extension == "xls" {
        let new_xlsx_path = path.with_extension("xlsx");
        fs::rename(path, &new_xlsx_path).context("重命名 .xls 到 .xlsx 失败")?;
        info!("已将处理后的 .xls 重命名为 .xlsx: {:?}", new_xlsx_path);
        rename_processed_file(&new_xlsx_path, doned_dir)
    } else {
        rename_processed_file(path, doned_dir)
    }
}
