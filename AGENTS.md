# Auto Scanner - 开发人员与 Agent 指南

## 1. 项目概览

Auto Scanner 是一个高性能、异步的 Rust 应用程序，专为自动化浏览器交互而设计。它采用 **Master-Worker 架构** 来高效处理任务，同时支持本地执行（通过 Playwright）和指纹浏览器集成（通过 AdsPower）。

系统基于"文件驱动"和"邮件触发"模式运行：它监控目录中的输入文件（CSV/Excel）或通过电子邮件接收文件，利用浏览器自动化并发处理账号，并输出结果。

## 2. 构建与测试命令

### 构建命令
```bash
cargo build              # Debug 构建
cargo build --release    # Release 构建（优化）
cargo check              # 快速检查代码
```

### 测试命令
```bash
cargo test                              # 运行所有测试（单元测试 + 集成测试）
cargo test --lib                        # 仅运行单元测试
cargo test --test integration_test      # 运行特定集成测试
cargo test --test outlook_register_test  # 运行 Outlook 注册测试
```

#### 运行单个测试
```bash
cargo test test_account_creation                    # 按名称运行测试
cargo test test_account_creation --exact             # 精确匹配测试名称
cargo test test_cli_master_mode --exact              # 运行特定 CLI 测试
cargo test test_outlook_register_complete_flow --test outlook_register_test  # 在特定测试文件中运行
```

#### 调试测试
```bash
cargo test -- --nocapture          # 显示测试输出
cargo test -- --show-output        # 显示所有测试输出
RUST_LOG=debug cargo test          # 启用调试日志
```

### Lint 与格式化
```bash
cargo clippy                          # 运行 Clippy 检查
cargo clippy --all-targets            # 检查所有目标
cargo clippy --fix                    # 自动修复警告
cargo fmt                             # 格式化代码
cargo fmt --check                     # 检查格式（不修改）
```

### 开发脚本
```bash
./scripts/test.sh    # 运行完整测试套件（单元 + 集成）
./scripts/start.sh   # 构建并启动守护进程
./scripts/stop.sh    # 停止守护进程
./scripts/status.sh   # 检查守护进程状态
```

## 3. 代码风格与规范

### 3.1 导入组织
```rust
// 1. 外部 crate 导入（按字母顺序）
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// 2. 内部 crate 导入（按层级组织）
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;

// 3. 模块本地导入
use super::BaseStrategy;
use constants::FacebookConfig;
```

### 3.2 类型定义与命名约定

**Structs: `PascalCase`**
```rust
pub struct FacebookLoginStrategy { /* ... */ }
pub struct Account { /* ... */ }
```

**Enums: `PascalCase`**
```rust
pub enum AppError { /* ... */ }
pub enum WorkerStrategy { FacebookLogin, OutlookRegister }
```

**Functions: `snake_case`**
```rust
pub async fn spawn_worker(&self, index: usize) -> AppResult<()>
fn perform_login(&self, adapter: &dyn BrowserAdapter) -> Result<()>
```

**Constants: `SCREAMING_SNAKE_CASE`**
```rust
const INPUT_DIR: &str = "input";
const PID_FILE: &str = "auto-scanner.pid";
```

**Type Aliases: `PascalCase` + `Result`**
```rust
pub type AppResult<T> = Result<T, AppError>;
pub type UnitResult = AppResult<()>;
```

### 3.3 错误处理
```rust
// 业务逻辑使用 anyhow::Result
use anyhow::{Context, Result};

async fn perform_login(&self, account: &Account) -> Result<()> {
    adapter.navigate(&url)
        .await
        .context("导航到登录页失败")?;
    Ok(())
}

// 基础设施错误使用自定义 AppError
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(String),
}

// 严禁在生产代码中使用 .unwrap()
// 错误示例：let result = some_operation().unwrap();  // ❌
// 正确示例：let result = some_operation().context("...")?;  // ✅
```

### 3.4 异步模式
```rust
// 所有 I/O 操作使用 async/await
pub async fn run(self) -> AppResult<()> {
    tokio::spawn(async move {
        coord.spawn_worker(index, &account).await
    });

    tokio::select! {
        _ = sigterm.recv() => {
            info!("收到 SIGTERM，正在关闭...");
            break;
        }
        Some(path) = rx.recv() => {
            // 处理文件
        }
    }

    Ok(())
}

// trait 实现使用 #[async_trait]
#[async_trait]
impl BaseStrategy for FacebookLoginStrategy {
    async fn run(&self, adapter: &dyn BrowserAdapter, account: &Account) -> Result<WorkerResult> {
        // ...
    }
}
```

### 3.5 测试约定
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        // Arrange-Act-Assert 模式
        let account = Account::new("test@example.com".to_string(), "pass".to_string());
        assert_eq!(account.username, "test@example.com");
    }

    #[tokio::test]
    async fn test_async_operation() {
        let strategy = OutlookRegisterStrategy::new();
        let result = strategy.run(&adapter, &account).await;
        assert!(result.is_ok(), "策略执行失败: {:?}", result);
    }
}
```

### 3.6 文档与注释
```rust
/// 描述性文档注释
pub async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
    // TODO: Replace fixed sleep with dynamic wait in future refactoring
    tokio::time::sleep(Duration::from_secs(8)).await;
}

// 使用 tracing 而非 println!
use tracing::{info, warn, error, debug};
info!("Master 已启动。监控目录: {}", self.config.input_dir);
```

## 4. 架构图谱

- **`src/core/`**: 领域模型和共享工具（`cli.rs`, `models.rs`, `error.rs`）
- **`src/infrastructure/`**: 外部系统交互（`browser/`, `adspower.rs`, `imap.rs`）
- **`src/services/`**: 业务逻辑与编排（`master/`, `worker/`, `email/`）
- **`src/strategies/`**: 可插拔自动化策略（`facebook_login/`, `outlook_register/`）

## 5. 环境变量

- `INPUT_DIR`: 监控输入文件的目录
- `DONED_DIR`: 处理后文件的存放目录
- `ADSPOWER_API_URL`: AdsPower API URL（默认：`http://127.0.0.1:50325`）
- `ADSPOWER_API_KEY`: AdsPower 认证密钥
- `ADSPOWER_PROXYID`: 代理 ID

## 6. 测试文件结构

**单元测试**（在源模块中）：`src/core/models.rs`, `src/services/email/tracker.rs`
**集成测试**（在 `tests/` 目录）：`tests/integration_test.rs`, `tests/outlook_register_test.rs`

**Mock 实现**：
- `MockBrowserAdapter`: 浏览器自动化模拟
- `MockTimeProvider`: 时间提供者模拟（用于时间依赖逻辑）
