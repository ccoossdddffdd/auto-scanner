use crate::core::models::{Account, WorkerResult};
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait WorkerOrchestrator: Send + Sync {
    async fn spawn_batch(&self, accounts: &[Account]) -> Vec<(usize, Option<WorkerResult>)>;
    async fn spawn_batch_stream(
        &self,
        accounts: &[Account],
        tx: mpsc::Sender<(usize, Option<WorkerResult>)>,
    );
}
