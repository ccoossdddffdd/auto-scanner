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
    pub enable_screenshot: bool,
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
            "Spawning worker for {} on thread {}",
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
                    error!(
                        "AdsPower session preparation failed for {}, aborting worker",
                        account.username
                    );
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
            .map_err(|e| anyhow::anyhow!("Failed to acquire thread: {}", e))
    }

    /// 准备 AdsPower 会话
    async fn prepare_adspower_session(
        &self,
        thread_index: usize,
        username: &str,
    ) -> Option<AdsPowerSession> {
        let client = self.adspower.as_ref()?;

        let profile_id = match client.ensure_profile_for_thread(thread_index).await {
            Ok(id) => id,
            Err(e) => {
                error!(
                    "Failed to ensure AdsPower profile for thread {}: {}",
                    thread_index, e
                );
                return None;
            }
        };

        info!(
            "Using AdsPower profile {} for account {} on thread {}",
            profile_id, username, thread_index
        );

        match client.start_browser(&profile_id).await {
            Ok(ws_url) => Some(AdsPowerSession { profile_id, ws_url }),
            Err(e) => {
                error!("Failed to start AdsPower browser for {}: {}", username, e);
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
            .arg(&self.backend);

        if self.enable_screenshot {
            cmd.arg("--enable-screenshot");
        }

        cmd
    }

    /// 执行 Worker 进程
    async fn execute_worker(&self, mut cmd: Command, username: &str) -> Result<WorkerResult> {
        // Set a timeout for the worker process to prevent hanging indefinitely
        // Default to 5 minutes (300 seconds) which should be enough for a login attempt
        let timeout_duration = std::time::Duration::from_secs(300);

        let output = match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => anyhow::bail!("Failed to run worker process: {}", e),
            Err(_) => anyhow::bail!(
                "Worker timed out after {} seconds",
                timeout_duration.as_secs()
            ),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(json_str) = line.strip_prefix("RESULT_JSON:") {
                if let Ok(result) = serde_json::from_str::<WorkerResult>(json_str) {
                    return Ok(result);
                }
            }
        }

        anyhow::bail!("Worker for {} did not return valid JSON result", username)
    }

    /// 清理会话资源
    async fn cleanup_session(&self, session: Option<AdsPowerSession>, thread_index: usize) {
        if let (Some(client), Some(sess)) = (&self.adspower, session) {
            // Stop the browser first
            if let Err(e) = client.stop_browser(&sess.profile_id).await {
                error!("Failed to stop AdsPower browser: {}", e);
            }

            // Delete the profile to ensure clean state for next run
            if let Err(e) = client.delete_profile(&sess.profile_id).await {
                error!(
                    "Failed to delete AdsPower profile {}: {}",
                    sess.profile_id, e
                );
            }
        }

        let _ = self.permit_tx.send(thread_index).await;
    }
}
