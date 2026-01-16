use crate::core::error::AppResult;
use crate::infrastructure::adspower::ProfileConfig;
use async_trait::async_trait;

#[async_trait]
pub trait BrowserEnvironmentManager: Send + Sync {
    async fn check_connectivity(&self) -> AppResult<()>;
    async fn ensure_profiles_for_workers(
        &self,
        worker_count: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<()>;
    async fn ensure_profile_for_thread(
        &self,
        thread_index: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<String>;
    async fn start_browser(&self, user_id: &str) -> AppResult<String>;
    async fn stop_browser(&self, user_id: &str) -> AppResult<()>;
    async fn delete_profile(&self, user_id: &str) -> AppResult<()>;
}
