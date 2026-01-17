use crate::core::config::AppConfig;
use crate::infrastructure::adspower::AdsPowerClient;
use crate::infrastructure::browser_manager::BrowserEnvironmentManager;
use crate::infrastructure::process::PidManager;
use crate::services::email::tracker::FileTracker;
use crate::services::email::EmailMonitor;
use crate::services::file_policy::FilePolicyService;
use crate::services::master::registration_loop::RegistrationLoopHandler;
use crate::services::master::scheduler::JobScheduler;
use crate::services::master::watcher::InputWatcher;
use crate::services::master::MasterConfig;
use crate::services::processor::{
    process_file, BrowserConfig, FileConfig, ProcessConfig, WorkerConfig,
};
use crate::services::worker::strategy::WorkerStrategy;
use anyhow::{Context, Result};
use reqwest::Url;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

const PID_FILE: &str = "auto-scanner-master.pid";

pub struct RuntimeState {
    pub input_path: PathBuf,
    pub doned_dir: PathBuf,
    pub exe_path: PathBuf,
    pub permit_rx: async_channel::Receiver<usize>,
    pub permit_tx: async_channel::Sender<usize>,
    pub scheduler: JobScheduler,
}

pub struct ServiceContainer {
    pub adspower: Option<Arc<dyn BrowserEnvironmentManager>>,
    pub email_monitor: Option<Arc<EmailMonitor>>,
}

pub struct MasterContext {
    pub state: RuntimeState,
    pub services: ServiceContainer,
}

impl MasterContext {
    async fn initialize(config: &AppConfig) -> Result<Self> {
        let input_path = Self::ensure_dir(&config.input_dir, "monitoring")?;

        let doned_dir_str =
            std::env::var("DONED_DIR").unwrap_or_else(|_| "input/doned".to_string());
        let doned_dir = Self::ensure_dir(&doned_dir_str, "doned")?;

        let adspower = Self::create_adspower_client(config)?;
        let email_monitor = Self::initialize_email_monitor(config).await;

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
}

struct FileProcessingHandler {
    config: MasterConfig,
    context: Arc<MasterContext>,
}

impl FileProcessingHandler {
    fn new(config: MasterConfig, context: Arc<MasterContext>) -> Self {
        Self { config, context }
    }

    async fn handle_incoming_file(&self, csv_path: PathBuf) {
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

/// 跨平台信号处理器
/// 在 Unix 上监听 SIGTERM 和 SIGINT
/// 在 Windows 上监听 Ctrl+C 和 Ctrl+Break
struct ShutdownSignal {
    #[cfg(unix)]
    sigterm: tokio::signal::unix::Signal,
    #[cfg(unix)]
    sigint: tokio::signal::unix::Signal,
    #[cfg(windows)]
    ctrl_c: tokio::signal::windows::CtrlC,
    #[cfg(windows)]
    ctrl_break: tokio::signal::windows::CtrlBreak,
}

impl ShutdownSignal {
    #[cfg(unix)]
    fn new() -> Result<Self> {
        Ok(Self {
            sigterm: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
            sigint: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?,
        })
    }

    #[cfg(windows)]
    fn new() -> Result<Self> {
        Ok(Self {
            ctrl_c: tokio::signal::windows::ctrl_c()?,
            ctrl_break: tokio::signal::windows::ctrl_break()?,
        })
    }

    #[cfg(unix)]
    async fn recv(&mut self) {
        tokio::select! {
            _ = self.sigterm.recv() => {
                info!("收到 SIGTERM 信号");
            }
            _ = self.sigint.recv() => {
                info!("收到 SIGINT 信号");
            }
        }
    }

    #[cfg(windows)]
    async fn recv(&mut self) {
        tokio::select! {
            _ = self.ctrl_c.recv() => {
                info!("收到 Ctrl+C 信号");
            }
            _ = self.ctrl_break.recv() => {
                info!("收到 Ctrl+Break 信号");
            }
        }
    }
}

pub struct MasterServer {
    config: AppConfig,
}

impl MasterServer {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub async fn run(self) -> Result<()> {
        let pid_manager = PidManager::new(PID_FILE);

        if self.config.master.status {
            return pid_manager.check_status();
        }

        if self.config.master.stop {
            return pid_manager.stop();
        }

        info!(
            "Master 已启动。监控目录: {}, 线程数: {}, 策略: {}, 后端: {}, 守护进程: {}",
            self.config.input_dir,
            self.config.master.thread_count,
            self.config.master.strategy,
            self.config.master.backend,
            self.config.master.daemon
        );

        if !self.config.master.daemon {
            pid_manager.write_pid()?;
        }

        self.ensure_backend_ready().await?;

        let context = Arc::new(MasterContext::initialize(&self.config).await?);

        // Check strategy type
        let strategy = WorkerStrategy::from_str(&self.config.master.strategy)?;

        match strategy {
            WorkerStrategy::OutlookRegister => {
                info!("检测到 Outlook 注册策略，启动无限注册循环...");
                let loop_handler =
                    RegistrationLoopHandler::new(self.config.master.clone(), context.clone());

                // Run continuously until interrupted
                tokio::select! {
                    _ = loop_handler.run_continuously() => {},
                    _ = tokio::signal::ctrl_c() => {
                        info!("收到终止信号，正在停止...");
                    }
                }
            }
            WorkerStrategy::FacebookLogin => {
                info!("检测到 Facebook 登录策略，启动文件监控模式...");
                self.run_file_watcher_mode(context).await?;
            }
        }

        pid_manager.remove_pid_file();
        info!("Master 关闭完成");

        Ok(())
    }

    async fn run_file_watcher_mode(&self, context: Arc<MasterContext>) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<PathBuf>(100);

        // 初始扫描
        let entries = fs::read_dir(&context.state.input_path)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if FilePolicyService::is_supported_file(&path) {
                tx.send(path).await?;
            }
        }

        let _watcher = InputWatcher::new(context.state.input_path.clone(), tx.clone())?;
        let handler = FileProcessingHandler::new(self.config.master.clone(), context.clone());

        info!("等待新文件...");

        // 创建跨平台的关闭信号处理器
        let mut shutdown = ShutdownSignal::new()?;

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("正在关闭 Master 服务...");
                    break;
                }
                Some(path) = rx.recv() => {
                    // Check if file still exists before processing
                    // This prevents processing files that were just moved/deleted
                    if !path.exists() {
                         continue;
                    }

                    if context.state.scheduler.try_schedule(path.clone()) {
                        handler.handle_incoming_file(path).await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn ensure_backend_ready(&self) -> Result<()> {
        info!("正在确保后端就绪: {}", self.config.master.backend);

        if self.config.master.backend == "mock" {
            info!("跳过 mock 后端的就绪检查");
            return Ok(());
        }

        if self.config.master.backend == "adspower" {
            let adspower_config = self.config.adspower.clone().context("AdsPower 配置缺失")?;
            let client = AdsPowerClient::new(adspower_config);
            client.check_connectivity().await?;
            info!("AdsPower API 可达");
        } else {
            let url_str = if self.config.master.remote_url.is_empty() {
                "http://127.0.0.1:9222"
            } else {
                &self.config.master.remote_url
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
}
