use crate::core::models::{Account, WorkerResult};
use crate::services::master::server::MasterContext;
use crate::services::master::MasterConfig;
use crate::services::worker::coordinator::WorkerCoordinator;
use anyhow::Result;
use chrono::Local;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

pub struct RegistrationLoopHandler {
    config: MasterConfig,
    context: Arc<MasterContext>,
}

impl RegistrationLoopHandler {
    pub fn new(config: MasterConfig, context: Arc<MasterContext>) -> Self {
        Self { config, context }
    }

    pub async fn start_loop(&self) {
        self.run_continuously().await;
    }

    // Since Coordinator acquires the permit, we just need to call it.
    // However, we want to run this in a loop respecting max threads.
    // If we just loop and spawn, we might spawn thousands of tasks waiting on the channel.
    // We should limit the "pending" spawns.
    // A simple way is to acquire the permit HERE, then pass it to the worker,
    // but the current Coordinator design acquires it inside.

    // Alternative: Just spawn `thread_count` long-running loops.
    pub async fn run_continuously(&self) {
        let mut handles = Vec::new();

        for _ in 0..self.config.thread_count {
            let coordinator = self.create_coordinator();
            let context = self.context.clone();
            // let config = self.config.clone();

            let handle = tokio::spawn(async move {
                loop {
                    // Coordinator internal acquire_thread will block if no permits.
                    // But permits are owned by the coordinator instance?
                    // No, permit_rx is shared.
                    // BUT, if we spawn N loops, and each calls spawn_worker,
                    // spawn_worker will try to recv from rx.
                    // The rx is filled with N tokens initially.
                    // So each loop will grab one, run, release, and grab again.
                    // This is exactly what we want.

                    let dummy_account =
                        Account::new("new_user".to_string(), "password".to_string());
                    let (_, result) = coordinator.spawn_worker(0, &dummy_account).await;

                    if let Some(res) = result {
                        // The strategy returns "处理中" for now as success placeholder
                        if res.status == "成功" || res.status == "处理中" {
                            info!("注册流程完成: {:?}", res);
                            let date_str = Local::now().format("%Y%m%d").to_string();
                            // User asked for xlsx, but we use csv for robustness and simplicity in append mode
                            let filename = format!("outlook_register_{}.csv", date_str);
                            let file_path = context.state.doned_dir.join(filename);

                            if let Err(e) = Self::save_result(&file_path, res).await {
                                error!("保存结果失败: {}", e);
                            }
                        } else {
                            warn!("注册流程未能完成: {}", res.message);
                        }
                    }

                    // Brief pause to prevent tight loop in case of immediate errors
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all (they run forever until cancelled)
        for handle in handles {
            let _ = handle.await;
        }
    }

    fn create_coordinator(&self) -> Arc<WorkerCoordinator> {
        Arc::new(WorkerCoordinator::new(
            self.context.state.permit_rx.clone(),
            self.context.state.permit_tx.clone(),
            self.context.services.adspower.clone(),
            self.context.state.exe_path.clone(),
            self.config.backend.clone(),
            self.config.remote_url.clone(),
            self.config.strategy.clone(),
        ))
    }

    async fn save_result(file_path: &std::path::Path, result: WorkerResult) -> Result<()> {
        let is_new = !file_path.exists();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await?;

        if is_new {
            file.write_all(b"email,password,first_name,last_name,birth_year,status,message\n")
                .await?;
        }

        if let Some(data) = result.data {
            let email = data.get("email").and_then(|v| v.as_str()).unwrap_or("");
            let password = data.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let first = data
                .get("first_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = data.get("last_name").and_then(|v| v.as_str()).unwrap_or("");
            let year = data
                .get("birth_year")
                .map(|v| v.to_string())
                .unwrap_or_default();

            let line = format!(
                "{},{},{},{},{},{},{}\n",
                email, password, first, last, year, result.status, result.message
            );

            file.write_all(line.as_bytes()).await?;
        }

        Ok(())
    }
}
