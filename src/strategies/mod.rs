use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::Result;
use async_trait::async_trait;

pub mod facebook;

#[async_trait]
pub trait LoginStrategy: Send + Sync {
    async fn login(
        &self,
        adapter: &dyn BrowserAdapter,
        account: &Account,
        enable_screenshot: bool,
    ) -> Result<WorkerResult>;
}
