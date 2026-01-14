use crate::infrastructure::adspower::AdsPowerClient;
use crate::services::email::monitor::EmailMonitor;
use crate::services::file::get_account_source;
use crate::services::file::operation::{prepare_input_file, write_results_and_rename};
use crate::services::worker::coordinator::WorkerCoordinator;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

/// 浏览器配置
#[derive(Clone)]
pub struct BrowserConfig {
    pub backend: String,
    pub remote_url: String,
    pub adspower: Option<Arc<AdsPowerClient>>,
}

/// Worker 配置
#[derive(Clone)]
pub struct WorkerConfig {
    pub exe_path: PathBuf,
    pub enable_screenshot: bool,
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
                info!("Sending success notification to {}", from);
                if let Err(e) = monitor
                    .send_success_notification(&from, final_path.clone())
                    .await
                {
                    error!("Failed to send success notification: {}", e);
                }
                if let Err(e) = monitor
                    .get_file_tracker()
                    .mark_success(id, final_path.clone())
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
                    .mark_failed(id, e.to_string(), None)
                {
                    warn!("Failed to mark failed in tracker: {}", e);
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
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let path_to_process = prepare_input_file(path, &email_monitor).await?;

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
        let source = get_account_source(&path_to_process);
        let (accounts, records, headers) = source.read(&path_to_process).await?;

        info!("Read {} accounts from {}", accounts.len(), batch_name);

        let coordinator = WorkerCoordinator {
            permit_rx,
            permit_tx,
            adspower: config.browser.adspower.clone(),
            exe_path: config.worker.exe_path.clone(),
            backend: config.browser.backend.clone(),
            remote_url: config.browser.remote_url.clone(),
            enable_screenshot: config.worker.enable_screenshot,
        };

        let mut handles = Vec::new();
        for (index, account) in accounts.iter().enumerate() {
            let coord = coordinator.clone();
            let account = account.clone();
            let handle = tokio::spawn(async move { coord.spawn_worker(index, &account).await });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(res) = handle.await {
                results.push(res);
            }
        }

        results.sort_by_key(|k| k.0);

        write_results_and_rename(
            &path_to_process,
            &extension,
            results,
            records,
            headers,
            &config.file.doned_dir,
        )
        .await
    }
    .await;

    handle_email_notification(&email_monitor, &email_id, &processing_result).await;

    processing_result
}
