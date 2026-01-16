use crate::core::config::AppConfig;
use crate::infrastructure::adspower::AdsPowerClient;
use crate::infrastructure::browser_manager::BrowserEnvironmentManager;
use crate::infrastructure::process::PidManager;
use crate::services::email::tracker::FileTracker;
use crate::services::email::EmailMonitor;
use crate::services::file_policy::FilePolicyService;
use crate::services::processor::{
    process_file, BrowserConfig, FileConfig, ProcessConfig, WorkerConfig,
};
use anyhow::{Context, Result};
use async_channel;
use notify::Watcher;
use reqwest::Url;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub mod scheduler;
pub mod watcher;

use scheduler::JobScheduler;
use watcher::InputWatcher;

const PID_FILE: &str = "auto-scanner-master.pid";

struct RuntimeState {
    input_path: PathBuf,
    doned_dir: PathBuf,
    exe_path: PathBuf,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    scheduler: JobScheduler,
}

struct ServiceContainer {
    adspower: Option<Arc<dyn BrowserEnvironmentManager>>,
    email_monitor: Option<Arc<EmailMonitor>>,
}

/// Master 上下文 - 包含所有运行时状态
struct MasterContext {
    state: RuntimeState,
    services: ServiceContainer,
}

impl MasterContext {
    /// 初始化 Master 上下文
    async fn initialize(config: &AppConfig) -> Result<Self> {
        let input_path = Self::ensure_dir(&config.input_dir, "monitoring")?;

        let doned_dir_str =
            std::env::var("DONED_DIR").unwrap_or_else(|_| "input/doned".to_string());
        let doned_dir = Self::ensure_dir(&doned_dir_str, "doned")?;

        let adspower = Self::create_adspower_client(config)?;
        let email_monitor = initialize_email_monitor(config).await;

        let (permit_tx, permit_rx) = async_channel::bounded(config.master.thread_count);
        for i in 0..config.master.thread_count {
            permit_tx.send(i).await.expect("初始化线程池失败");
        }

        let exe_path = if let Some(path) = config.master.exe_path.clone() {
            path
        } else {
            env::current_exe().context("获取当前可执行文件路径失败")?
        };

        Ok(Self {
            state: RuntimeState {
                input_path,
                doned_dir,
                exe_path,
                permit_rx,
                permit_tx,
                scheduler: JobScheduler::new(),
            },
            services: ServiceContainer {
                adspower,
                email_monitor,
            },
        })
    }

    fn ensure_dir(path_str: &str, name: &str) -> Result<PathBuf> {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            fs::create_dir_all(&path).context(format!("创建 {} 目录失败", name))?;
        }
        Ok(path)
    }

    fn create_adspower_client(
        config: &AppConfig,
    ) -> Result<Option<Arc<dyn BrowserEnvironmentManager>>> {
        if config.master.backend == "adspower" {
            let adspower_config = config.adspower.clone().context("AdsPower 配置缺失")?;
            Ok(Some(Arc::new(AdsPowerClient::new(adspower_config))))
        } else {
            Ok(None)
        }
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
            self.context.state.permit_rx.clone(),
            self.context.state.permit_tx.clone(),
            self.context.services.email_monitor.clone(),
        )
        .await;

        // Cleanup: remove from processing set regardless of success/failure
        self.context.state.scheduler.mark_completed(&csv_path);

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
            adspower: self.context.services.adspower.clone(),
        };

        let worker_config = WorkerConfig {
            exe_path: self.context.state.exe_path.clone(),
            strategy: self.config.strategy.clone(),
        };

        let file_config = FileConfig {
            doned_dir: self.context.state.doned_dir.clone(),
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


async fn initialize_email_monitor(config: &AppConfig) -> Option<Arc<EmailMonitor>> {
    if !config.master.enable_email_monitor {
        return None;
    }

    info!("邮件监控已启用");

    let file_tracker = Arc::new(FileTracker::new());
    let email_config = match &config.email {
        Some(c) => c.clone(),
        None => {
            warn!("邮件监控已启用但配置缺失，禁用邮件监控");
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

async fn ensure_backend_ready(config: &AppConfig) -> Result<()> {
    info!("正在确保后端就绪: {}", config.master.backend);

    if config.master.backend == "mock" {
        info!("跳过 mock 后端的就绪检查");
        return Ok(());
    }

    if config.master.backend == "adspower" {
        let adspower_config = config.adspower.clone().context("AdsPower 配置缺失")?;
        let client = AdsPowerClient::new(adspower_config);
        client.check_connectivity().await?;
        info!("AdsPower API 可达");
    } else {
        let url_str = if config.master.remote_url.is_empty() {
            "http://127.0.0.1:9222"
        } else {
            &config.master.remote_url
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

pub async fn run(config: AppConfig) -> Result<()> {
    let pid_manager = PidManager::new(PID_FILE);

    if config.master.status {
        return pid_manager.check_status();
    }

    if config.master.stop {
        return pid_manager.stop();
    }

    info!(
        "Master 已启动。监控目录: {}, 线程数: {}, 策略: {}, 后端: {}, 守护进程: {}",
        config.input_dir,
        config.master.thread_count,
        config.master.strategy,
        config.master.backend,
        config.master.daemon
    );

    if !config.master.daemon {
        pid_manager.write_pid()?;
    }

    ensure_backend_ready(&config).await?;

    // 初始化上下文
    let context = Arc::new(MasterContext::initialize(&config).await?);

    let (tx, mut rx) = mpsc::channel::<PathBuf>(100);

    // 扫描现有文件
    let entries = fs::read_dir(&context.state.input_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if FilePolicyService::is_supported_file(&path) {
            // Initial scan - send to channel to be scheduled
            tx.send(path).await?;
        }
    }

    // 设置文件监控器
    let _watcher = InputWatcher::new(context.state.input_path.clone(), tx.clone())?;

    // 创建文件处理器
    let handler = FileProcessingHandler::new(config.master.clone(), context.clone());

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
            Some(path) = rx.recv() => {
                if context.state.scheduler.try_schedule(path.clone()) {
                    handler.handle_incoming_file(path).await;
                }
            }
        }
    }

    pid_manager.remove_pid_file();
    info!("Master 关闭完成");

    Ok(())
}
