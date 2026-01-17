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
        let register_count = self.config.register_count;

        // 判断运行模式
        if register_count == 0 {
            info!("无限循环模式：将持续注册账号");
        } else {
            info!("限定数量模式：将注册 {} 个账号后退出", register_count);
        }

        let mut handles = Vec::new();

        for _ in 0..self.config.thread_count {
            let coordinator = self.create_coordinator();
            let context = self.context.clone();
            let count = register_count;

            let handle = tokio::spawn(async move {
                let mut registered = 0;

                // 初始延迟，避免 AdsPower API 速率限制
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                loop {
                    let dummy_account =
                        Account::new("new_user".to_string(), "password".to_string());

                    // No global lock needed; coordinator handles concurrency via permits
                    let (_, result) = coordinator.spawn_worker(registered, &dummy_account).await;

                    if let Some(res) = result {
                        // The strategy returns "处理中" for now as success placeholder
                        if res.status == "成功" || res.status == "处理中" {
                            registered += 1;
                            info!(
                                "注册流程完成 ({}/{}): {:?}",
                                registered,
                                if count == 0 {
                                    "∞".to_string()
                                } else {
                                    count.to_string()
                                },
                                res
                            );

                            let date_str = Local::now().format("%Y%m%d").to_string();
                            let filename = format!("outlook_register_{}.csv", date_str);
                            let file_path = context.state.doned_dir.join(filename);

                            if let Err(e) = Self::save_result(&file_path, res).await {
                                error!("保存结果失败: {}", e);
                            }

                            // 检查是否达到目标数量
                            if count > 0 && registered >= count {
                                info!("已完成 {} 个账号注册，程序将退出", registered);
                                std::process::exit(0);
                            }
                        } else {
                            warn!("注册流程未能完成: {}", res.message);
                        }
                    }

                    // 如果是无限循环模式，继续下一次
                    if count == 0 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }

                    // 限定数量模式，每次循环都延迟
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all (they run forever until cancelled or reach count)
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
