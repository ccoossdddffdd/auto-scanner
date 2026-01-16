pub mod scheduler;
pub mod server;
pub mod watcher;

use crate::core::config::AppConfig;
use anyhow::Result;
use server::MasterServer;
use std::path::PathBuf;

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

pub async fn run(config: AppConfig) -> Result<()> {
    MasterServer::new(config).run().await
}
