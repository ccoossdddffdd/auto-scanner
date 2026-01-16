use crate::infrastructure::adspower::AdsPowerClient;
use crate::infrastructure::logging::init_logging;
use crate::infrastructure::process::PidManager;
use crate::services::email::tracker::FileTracker;
use crate::services::email::{EmailConfig, EmailMonitor};
use crate::services::processor::{
    process_file, BrowserConfig, FileConfig, ProcessConfig, WorkerConfig,
};
use anyhow::{Context, Result};
use async_channel;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Url;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

const PID_FILE: &str = "auto-scanner-master.pid";

/// Master 上下文 - 包含所有运行时状态
struct MasterContext {
    input_path: PathBuf,
    doned_dir: PathBuf,
    adspower: Option<Arc<AdsPowerClient>>,
    exe_path: PathBuf,
    email_monitor: Option<Arc<EmailMonitor>>,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    processing_files: Arc<std::sync::Mutex<HashSet<PathBuf>>>,
}

impl MasterContext {
    /// 初始化 Master 上下文
    async fn initialize(config: &MasterConfig, input_dir: String) -> Result<Self> {
        let input_path = PathBuf::from(&input_dir);
        if !input_path.exists() {
            fs::create_dir_all(&input_path).context("Failed to create monitoring directory")?;
        }

        let doned_dir =
            PathBuf::from(std::env::var("DONED_DIR").unwrap_or_else(|_| "input/doned".to_string()));
        if !doned_dir.exists() {
            fs::create_dir_all(&doned_dir).context("Failed to create doned directory")?;
        }

        let adspower = if config.backend == "adspower" {
            Some(Arc::new(AdsPowerClient::new()))
        } else {
            None
        };

        let email_monitor = initialize_email_monitor(config).await;

        let (permit_tx, permit_rx) = async_channel::bounded(config.thread_count);
        for i in 0..config.thread_count {
            permit_tx.send(i).await.expect("初始化线程池失败");
        }

        let exe_path = if let Some(path) = config.exe_path.clone() {
            path
        } else {
            env::current_exe().context("获取当前可执行文件路径失败")?
        };

        Ok(Self {
            input_path,
            doned_dir,
            adspower,
            exe_path,
            email_monitor,
            permit_rx,
            permit_tx,
            processing_files: Arc::new(std::sync::Mutex::new(HashSet::new())),
        })
    }
}

/// 文件处理器
struct FileProcessingHandler {
    config: MasterConfig,
    context: Arc<MasterContext>,
}

impl FileProcessingHandler {
    fn new(config: MasterConfig, context: Arc<MasterContext>) -> Self {
        Self { config, context }
    }

    /// 统一获取锁的处理函数
    fn lock_processing_files(&self) -> std::sync::MutexGuard<'_, HashSet<PathBuf>> {
        match self.context.processing_files.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("Mutex poisoned: {:?}", poisoned);
                poisoned.into_inner()
            }
        }
    }

    /// 处理传入的文件
    async fn handle_incoming_file(&self, csv_path: PathBuf) {
        // Remove redundant file existence check.
        // We let process_file attempt to open it. If it fails, it returns an error,
        // and we naturally fall through to the cleanup block.
        // This avoids TOCTOU race conditions and simplifies lock handling.

        info!("正在处理文件: {:?}", csv_path);
        let batch_name = csv_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let process_config = self.build_process_config(batch_name.clone());

        let result = process_file(
            &csv_path,
            &batch_name,
            process_config,
            self.context.permit_rx.clone(),
            self.context.permit_tx.clone(),
            self.context.email_monitor.clone(),
        )
        .await;

        // Cleanup: remove from processing set regardless of success/failure
        {
            let mut processing = self.lock_processing_files();
            processing.remove(&csv_path);
        }

        match result {
            Ok(processed_path) => {
                info!("文件处理完成: {:?}", processed_path);
            }
            Err(e) => {
                error!("处理文件 {:?} 时出错: {}", csv_path, e);
            }
        }
    }

    /// 构建处理配置
    fn build_process_config(&self, batch_name: String) -> ProcessConfig {
        let browser_config = BrowserConfig {
            backend: self.config.backend.clone(),
            remote_url: self.config.remote_url.clone(),
            adspower: self.context.adspower.clone(),
        };

        let worker_config = WorkerConfig {
            exe_path: self.context.exe_path.clone(),
            strategy: self.config.strategy.clone(),
        };

        let file_config = FileConfig {
            doned_dir: self.context.doned_dir.clone(),
        };

        ProcessConfig::new(batch_name, browser_config, worker_config, file_config)
    }
}

#[derive(Clone, Debug)]
pub struct MasterConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
    pub strategy: String,
    pub stop: bool,
    pub daemon: bool,
    pub status: bool,
    pub enable_email_monitor: bool,
    pub email_poll_interval: u64,
    pub exe_path: Option<PathBuf>,
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
                    info!("收到文件事件: {:?}", event.kind);
                    for path in event.paths {
                        if is_supported_file(&path) {
                            let mut processing = match processing_files.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => {
                                    error!("Mutex poisoned in file watcher: {:?}", poisoned);
                                    poisoned.into_inner()
                                }
                            };
                            if processing.insert(path.clone()) {
                                info!("检测到新文件: {:?}", path);
                                if let Err(e) = tx.blocking_send(path) {
                                    error!("发送文件路径到处理器失败: {}", e);
                                    // If we can't send, we should remove it from processing set
                                    // so it can be picked up again later
                                    // Note: we need to re-acquire the lock or handle this better,
                                    // but since we are in the watcher callback, simple error logging is safer than complex recovery.
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => error!("监控错误: {:?}", e),
        },
        Config::default(),
    )?;

    Ok(watcher)
}

async fn initialize_email_monitor(config: &MasterConfig) -> Option<Arc<EmailMonitor>> {
    if !config.enable_email_monitor {
        return None;
    }

    info!("邮件监控已启用");

    let file_tracker = Arc::new(FileTracker::new());
    let email_config = match EmailConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            warn!("创建邮件配置失败: {}, 禁用邮件监控", e);
            return None;
        }
    };

    match EmailMonitor::new(email_config, file_tracker.clone()) {
        Ok(monitor) => {
            let monitor = Arc::new(monitor);
            let monitor_clone = monitor.clone();
            tokio::spawn(async move {
                info!("邮件监控任务已启动");
                if let Err(e) = monitor_clone.start_monitoring().await {
                    error!("邮件监控失败: {}", e);
                }
            });
            Some(monitor)
        }
        Err(e) => {
            warn!("创建邮件监控失败: {}, 禁用邮件监控", e);
            None
        }
    }
}

async fn ensure_backend_ready(config: &MasterConfig) -> Result<()> {
    info!("正在确保后端就绪: {}", config.backend);

    if config.backend == "mock" {
        info!("跳过 mock 后端的就绪检查");
        return Ok(());
    }

    if config.backend == "adspower" {
        let client = AdsPowerClient::new();
        client.check_connectivity().await?;
        info!("AdsPower API 可达");
    } else {
        let url_str = if config.remote_url.is_empty() {
            "http://127.0.0.1:9222"
        } else {
            &config.remote_url
        };

        let url = Url::parse(url_str).context(format!("解析 remote_url 失败: {}", url_str))?;

        let host = url.host_str().unwrap_or("127.0.0.1");
        let port = url.port_or_known_default().unwrap_or(9222);

        let addr = format!("{}:{}", host, port);
        info!("测试连接到 {}", addr);

        TcpStream::connect(&addr)
            .await
            .with_context(|| format!("连接到浏览器 {} 失败", addr))?;

        info!("成功连接到浏览器 {}", addr);
    }

    Ok(())
}

pub async fn run(config: MasterConfig) -> Result<()> {
    dotenv::dotenv().ok();

    let pid_manager = PidManager::new(PID_FILE);

    if config.status {
        return pid_manager.check_status();
    }

    if config.stop {
        return pid_manager.stop();
    }

    init_logging("auto-scanner", config.daemon)?;

    let input_dir = std::env::var("INPUT_DIR").context("必须设置 INPUT_DIR 环境变量")?;

    info!(
        "Master 已启动。监控目录: {}, 线程数: {}, 策略: {}, 后端: {}, 守护进程: {}",
        input_dir, config.thread_count, config.strategy, config.backend, config.daemon
    );

    if !config.daemon {
        pid_manager.write_pid()?;
    }

    // 确保在检查后端连接性之前加载环境变量
    dotenv::dotenv().ok();
    ensure_backend_ready(&config).await?;

    // 初始化上下文
    let context = Arc::new(MasterContext::initialize(&config, input_dir).await?);

    let (tx, mut rx) = mpsc::channel::<PathBuf>(100);

    // 扫描现有文件
    let entries = fs::read_dir(&context.input_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if is_supported_file(&path) {
            let should_process = {
                let mut processing = match context.processing_files.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => {
                        error!("Mutex poisoned during initial scan: {:?}", poisoned);
                        poisoned.into_inner()
                    }
                };
                processing.insert(path.clone())
            };
            if should_process {
                tx.send(path).await?;
            }
        }
    }

    // 设置文件监控器
    let mut watcher = create_file_watcher(
        &context.input_path,
        tx.clone(),
        context.processing_files.clone(),
    )?;
    watcher.watch(&context.input_path, RecursiveMode::NonRecursive)?;

    // 创建文件处理器
    let handler = FileProcessingHandler::new(config, context);

    info!("等待新文件...");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("收到 SIGTERM，正在关闭...");
                break;
            }
            _ = sigint.recv() => {
                info!("收到 SIGINT，正在关闭...");
                break;
            }
            Some(csv_path) = rx.recv() => {
                handler.handle_incoming_file(csv_path).await;
            }
        }
    }

    pid_manager.remove_pid_file();
    info!("Master 关闭完成");

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
