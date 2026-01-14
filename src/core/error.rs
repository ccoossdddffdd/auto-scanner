use anyhow::Result as AnyhowResult;
use thiserror::Error;

/// 应用错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Email error: {0}")]
    Email(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Browser error: {0}")]
    Browser(String),

    #[error("Worker spawn failed: {0}")]
    WorkerSpawn(String),

    #[error("File processing error: {0}")]
    Processing(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 应用级别通用 Result 类型
pub type AppResult<T> = Result<T, AppError>;

/// Unit Result 简写
pub type UnitResult = AppResult<()>;

/// 兼容性：保留 anyhow Result
pub type AnyhowAppResult<T> = AnyhowResult<T>;
