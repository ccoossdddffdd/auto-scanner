use anyhow::Result;
use async_trait::async_trait;
use std::process::Output;
use tokio::process::Command;

#[async_trait]
pub trait ProcessExecutor: Send + Sync {
    async fn execute(&self, cmd: Command) -> Result<Output>;
}

pub struct TokioProcessExecutor;

#[async_trait]
impl ProcessExecutor for TokioProcessExecutor {
    async fn execute(&self, mut cmd: Command) -> Result<Output> {
        // Set a default timeout for the process execution
        let timeout_duration = std::time::Duration::from_secs(300);

        match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(anyhow::anyhow!("Process execution failed: {}", e)),
            Err(_) => Err(anyhow::anyhow!(
                "Process timed out after {}s",
                timeout_duration.as_secs()
            )),
        }
    }
}
