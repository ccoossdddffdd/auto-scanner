use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::Result;
use async_trait::async_trait;

pub mod facebook_login;

#[async_trait]
pub trait BaseStrategy: Send + Sync {
    async fn run(
        &self,
        adapter: &dyn BrowserAdapter,
        account: &Account,
    ) -> Result<WorkerResult>;
}
