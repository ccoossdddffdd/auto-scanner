use crate::infrastructure::browser_manager::BrowserEnvironmentManager;
use crate::services::email::monitor::EmailMonitor;
use crate::services::file::get_account_source;
use crate::services::file::operation::{ensure_csv_format, write_results_and_rename};
use crate::services::worker::coordinator::WorkerCoordinator;
use crate::services::worker::orchestrator::WorkerOrchestrator;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

/// 浏览器配置
#[derive(Clone)]
pub struct BrowserConfig {
    pub backend: String,
    pub remote_url: String,
    pub adspower: Option<Arc<dyn BrowserEnvironmentManager>>,
}

/// Worker 配置
#[derive(Clone)]
pub struct WorkerConfig {
    pub exe_path: PathBuf,
    pub strategy: String,
}

/// 文件配置
#[derive(Clone)]
pub struct FileConfig {
    pub doned_dir: PathBuf,
}

/// 处理配置
#[derive(Clone)]
pub struct ProcessConfig {
    pub batch_name: String,
    pub browser: BrowserConfig,
    pub worker: WorkerConfig,
    pub file: FileConfig,
}

impl ProcessConfig {
    pub fn new(
        batch_name: String,
        browser: BrowserConfig,
        worker: WorkerConfig,
        file: FileConfig,
    ) -> Self {
        Self {
            batch_name,
            browser,
            worker,
            file,
        }
    }
}

async fn handle_email_notification(
    email_monitor: &Option<Arc<EmailMonitor>>,
    email_id: &Option<String>,
    result: &Result<PathBuf>,
) {
    if let (Some(monitor), Some(id)) = (email_monitor, email_id) {
        let metadata = monitor.get_file_tracker().get_email_metadata(id);
        let from = metadata.map(|m| m.from).unwrap_or_default();

        match result {
            Ok(final_path) => {
                info!("发送成功通知给 {}", from);
                if let Err(e) = monitor
                    .send_success_notification(&from, final_path.clone())
                    .await
                {
                    error!("发送成功通知失败: {}", e);
                }
                if let Err(e) = monitor
                    .get_file_tracker()
                    .mark_success(id, final_path.clone())
                {
                    warn!("在追踪器中标记成功失败: {}", e);
                }
            }
            Err(e) => {
                info!("发送失败通知给 {}", from);
                if let Err(e) = monitor
                    .send_failure_notification(&from, &e.to_string(), None)
                    .await
                {
                    error!("发送失败通知失败: {}", e);
                }
                if let Err(e) = monitor
                    .get_file_tracker()
                    .mark_failed(id, e.to_string(), None)
                {
                    warn!("在追踪器中标记失败失败: {}", e);
                }
            }
        }
    }
}

pub async fn process_file(
    path: &Path,
    batch_name: &str,
    config: ProcessConfig,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    email_monitor: Option<Arc<EmailMonitor>>,
) -> Result<PathBuf> {
    let (path_to_process, converted) = ensure_csv_format(path).await?;

    if converted {
        if let Some(monitor) = &email_monitor {
            if let Err(e) = monitor
                .get_file_tracker()
                .update_file_path(path, &path_to_process)
            {
                warn!("更新文件追踪器路径失败: {}", e);
            }
        }
    }

    let email_id = extract_email_id(&path_to_process, &email_monitor);

    let processing_result =
        process_accounts(&path_to_process, batch_name, config, permit_rx, permit_tx).await;

    handle_email_notification(&email_monitor, &email_id, &processing_result).await;

    processing_result
}

/// 提取邮件 ID
fn extract_email_id(path: &Path, email_monitor: &Option<Arc<EmailMonitor>>) -> Option<String> {
    email_monitor.as_ref().and_then(|monitor| {
        let filename = path.file_name()?.to_str().unwrap_or("unknown");
        monitor.get_file_tracker().find_email_by_file(filename)
    })
}

/// 处理账号列表
async fn process_accounts(
    path: &Path,
    batch_name: &str,
    config: ProcessConfig,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
) -> Result<PathBuf> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let source = get_account_source(path);
    let (accounts, records, headers) = source.read(path).await?;

    info!("从 {} 读取了 {} 个账号", batch_name, accounts.len());

    let coordinator = WorkerCoordinator::new(
        permit_rx,
        permit_tx,
        config.browser.adspower.clone(),
        config.worker.exe_path.clone(),
        config.browser.backend.clone(),
        config.browser.remote_url.clone(),
        config.worker.strategy.clone(),
    );

    let results = coordinator.spawn_batch(&accounts).await;

    write_results_and_rename(
        path,
        &extension,
        results,
        records,
        headers,
        &config.file.doned_dir,
    )
    .await
}
