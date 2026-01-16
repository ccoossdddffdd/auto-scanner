use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::adspower::AdsPowerClient;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{error, info};

/// AdsPower 会话信息
struct AdsPowerSession {
    profile_id: String,
    ws_url: String,
}

#[derive(Clone)]
pub struct WorkerCoordinator {
    pub permit_rx: async_channel::Receiver<usize>,
    pub permit_tx: async_channel::Sender<usize>,
    pub adspower: Option<Arc<AdsPowerClient>>,
    pub exe_path: PathBuf,
    pub backend: String,
    pub remote_url: String,
    pub strategy: String,
}

impl WorkerCoordinator {
    pub async fn spawn_worker(
        &self,
        index: usize,
        account: &Account,
    ) -> (usize, Option<WorkerResult>) {
        let thread_index = match self.acquire_thread().await {
            Ok(idx) => idx,
            Err(_) => return (index, None),
        };

        info!(
            "正在线程 {} 上为 {} 启动 Worker",
            account.username, thread_index
        );

        let mut session = None;

        // AdsPower Mode: If session preparation fails, we MUST fail the task.
        // Fallback to default remote_url is dangerous if AdsPower was explicitly requested.
        let remote_url = if self.adspower.is_some() {
            match self
                .prepare_adspower_session(thread_index, &account.username)
                .await
            {
                Some(s) => {
                    let url = s.ws_url.clone();
                    session = Some(s);
                    url
                }
                None => {
                    error!("{} 的 AdsPower 会话准备失败，终止 Worker", account.username);
                    self.cleanup_session(None, thread_index).await;
                    return (index, None);
                }
            }
        } else {
            self.remote_url.clone()
        };

        let cmd = self.build_worker_command(&account.username, &account.password, &remote_url);
        let result = self.execute_worker(cmd, &account.username).await;

        self.cleanup_session(session, thread_index).await;

        (index, result.ok())
    }

    /// 获取线程槽位
    async fn acquire_thread(&self) -> Result<usize> {
        self.permit_rx
            .recv()
            .await
            .map_err(|e| anyhow::anyhow!("获取线程槽位失败: {}", e))
    }

    /// 准备 AdsPower 会话
    async fn prepare_adspower_session(
        &self,
        thread_index: usize,
        username: &str,
    ) -> Option<AdsPowerSession> {
        let client = self.adspower.as_ref()?;

        // Get strategy-specific profile config
        let profile_config = if self.strategy == "facebook_login" {
            Some(crate::strategies::facebook_login::get_profile_config())
        } else {
            None
        };

        let profile_id = match client.ensure_profile_for_thread(thread_index, profile_config.as_ref()).await {
            Ok(id) => id,
            Err(e) => {
                error!("确保线程 {} 的 AdsPower 配置文件失败: {}", thread_index, e);
                return None;
            }
        };

        info!(
            "线程 {}: 账号 {} 使用 AdsPower 配置文件 {}",
            thread_index, username, profile_id
        );

        match client.start_browser(&profile_id).await {
            Ok(ws_url) => Some(AdsPowerSession { profile_id, ws_url }),
            Err(e) => {
                error!("启动 {} 的 AdsPower 浏览器失败: {}", username, e);
                None
            }
        }
    }

    /// 构建 Worker 命令
    fn build_worker_command(&self, username: &str, password: &str, remote_url: &str) -> Command {
        let mut cmd = Command::new(&self.exe_path);
        cmd.arg("worker")
            .arg("--username")
            .arg(username)
            .arg("--password")
            .arg(password)
            .arg("--remote-url")
            .arg(remote_url)
            .arg("--backend")
            .arg(&self.backend)
            .arg("--strategy")
            .arg(&self.strategy);

        cmd
    }

    /// 执行 Worker 进程
    async fn execute_worker(&self, mut cmd: Command, username: &str) -> Result<WorkerResult> {
        // Set a timeout for the worker process to prevent hanging indefinitely
        // Default to 5 minutes (300 seconds) which should be enough for a login attempt
        let timeout_duration = std::time::Duration::from_secs(300);

        let output = match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => anyhow::bail!("运行 Worker 进程失败: {}", e),
            Err(_) => anyhow::bail!("Worker 在 {} 秒后超时", timeout_duration.as_secs()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(json_str) = line.strip_prefix("RESULT_JSON:") {
                if let Ok(result) = serde_json::from_str::<WorkerResult>(json_str) {
                    return Ok(result);
                }
            }
        }

        anyhow::bail!("{} 的 Worker 未返回有效的 JSON 结果", username)
    }

    /// 清理会话资源
    async fn cleanup_session(&self, session: Option<AdsPowerSession>, thread_index: usize) {
        if let (Some(client), Some(sess)) = (&self.adspower, session) {
            // Stop the browser first
            if let Err(e) = client.stop_browser(&sess.profile_id).await {
                error!("停止 AdsPower 浏览器失败: {}", e);
            }

            // Delete the profile to ensure clean state for next run
            if let Err(e) = client.delete_profile(&sess.profile_id).await {
                error!("删除 AdsPower 配置文件 {} 失败: {}", sess.profile_id, e);
            }
        }

        let _ = self.permit_tx.send(thread_index).await;
    }
}
