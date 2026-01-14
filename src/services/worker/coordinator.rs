use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::adspower::AdsPowerClient;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{error, info};

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
        let thread_index = self.permit_rx.recv().await.unwrap();

        let username = account.username.clone();
        let password = account.password.clone();

        info!(
            "Spawning worker for {} on thread {}",
            username, thread_index
        );

        let mut adspower_id = None;
        let mut active_remote_url = self.remote_url.clone();

        if let Some(client) = &self.adspower {
            match client.ensure_profile_for_thread(thread_index).await {
                Ok(id) => {
                    if let Err(e) = client.update_profile_for_account(&id, &username).await {
                        error!("Failed to update AdsPower profile for {}: {}", username, e);
                        let _ = self.permit_tx.send(thread_index).await;
                        return (index, None);
                    }

                    match client.start_browser(&id).await {
                        Ok(ws_url) => {
                            adspower_id = Some(id);
                            active_remote_url = ws_url;
                        }
                        Err(e) => {
                            error!("Failed to start AdsPower browser for {}: {}", username, e);
                            let _ = self.permit_tx.send(thread_index).await;
                            return (index, None);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to check/create AdsPower profile for thread {}: {}",
                        thread_index, e
                    );
                    let _ = self.permit_tx.send(thread_index).await;
                    return (index, None);
                }
            }
        }

        let mut cmd = Command::new(&self.exe_path);
        cmd.arg("worker")
            .arg("--username")
            .arg(&username)
            .arg("--password")
            .arg(&password)
            .arg("--remote-url")
            .arg(&active_remote_url)
            .arg("--backend")
            .arg(&self.backend);

        if self.enable_screenshot {
            cmd.arg("--enable-screenshot");
        }

        let output = cmd.output().await;

        if let Some(client) = &self.adspower {
            if let Some(id) = adspower_id {
                let _ = client.stop_browser(&id).await;
            }
        }

        let _ = self.permit_tx.send(thread_index).await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
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
    }
}
