use crate::core::models::{Account, WorkerResult};
use async_trait::async_trait;

#[async_trait]
pub trait WorkerOrchestrator: Send + Sync {
    async fn spawn_batch(&self, accounts: &[Account]) -> Vec<(usize, Option<WorkerResult>)>;
}
