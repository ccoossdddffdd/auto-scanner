use anyhow::Result as AnyhowResult;

/// 应用级别通用 Result 类型
pub type AppResult<T> = AnyhowResult<T>;

/// Unit Result 简写
pub type UnitResult = AnyhowResult<()>;
