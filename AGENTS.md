# Auto Scanner - 开发人员与 Agent 指南

## 1. 项目概览

Auto Scanner 是一个高性能、异步的 Rust 应用程序，专为自动化浏览器交互而设计。它采用 **Master-Worker 架构** 来高效处理任务，支持三种浏览器适配器：
- **Mock 模式**：用于测试，无需真实浏览器
- **Playwright 模式**：本地浏览器自动化（通过 Playwright）
- **AdsPower/BitBrowser 模式**：指纹浏览器集成（反检测）

系统基于"文件驱动"和"邮件触发"模式运行：它监控目录中的输入文件（CSV）或通过电子邮件接收文件，利用浏览器自动化并发处理账号，并输出结果到 `doned` 目录。

## 2. 构建与测试命令

### 构建命令
```bash
cargo build              # Debug 构建
cargo build --release    # Release 构建（优化）
cargo check              # 快速检查代码
```

### 测试命令
```bash
cargo test                                   # 运行所有测试（单元测试 + 集成测试）
cargo test --lib                             # 仅运行单元测试
cargo test --test integration_test           # 运行特定集成测试
cargo test --test outlook_register_test      # 运行 Outlook 注册测试
cargo test --test bitbrowser_integration_test # 运行 BitBrowser 集成测试
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
./scripts/test.sh     # 运行完整测试套件（等效于 cargo test 顺序执行）
./scripts/start.sh    # 构建并启动守护进程（后台运行 master 模式）
./scripts/stop.sh     # 停止守护进程（通过 PID 文件）
./scripts/status.sh   # 检查守护进程状态
```

**注意**：脚本需要执行权限，首次使用运行 `chmod +x scripts/*.sh`

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
use crate::strategies::facebook_login::constants::FacebookConfig;
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

**规则**：
1. **业务逻辑（strategies/）**：使用 `anyhow::Result` 提供灵活的错误上下文
2. **基础设施层（services/、infrastructure/）**：使用 `AppResult<T>` 保证类型安全
3. **混合场景**：`AppError` 实现了 `From<anyhow::Error>`，可以互相转换

```rust
// ✅ 策略实现：使用 anyhow::Result
use anyhow::{Context, Result};

#[async_trait]
impl BaseStrategy for FacebookLoginStrategy {
    async fn run(&self, adapter: &dyn BrowserAdapter, account: &Account) -> Result<WorkerResult> {
        adapter.navigate(&url)
            .await
            .context("导航到登录页失败")?;
        Ok(WorkerResult::success())
    }
}

// ✅ 服务层：使用 AppResult
use crate::core::error::{AppError, AppResult};

pub async fn spawn_worker(&self, index: usize) -> AppResult<()> {
    let result = some_operation()
        .await
        .map_err(|e| AppError::WorkerSpawn(e.to_string()))?;
    Ok(())
}

// ✅ 自定义 AppError（在 src/core/error.rs 中定义）
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Browser error: {0}")]
    Browser(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    // 兼容 anyhow
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// ❌ 严禁在生产代码中使用 .unwrap()
// 错误示例：let result = some_operation().unwrap();
// 正确示例：let result = some_operation().context("操作描述")?;
```

### 3.4 异步模式
```rust
// 所有 I/O 操作使用 async/await
use tokio::spawn;

pub async fn run(self) -> AppResult<()> {
    // 生成并发任务
    for (index, account) in accounts.into_iter().enumerate() {
        let coordinator = self.coordinator.clone();
        spawn(async move {
            coordinator.spawn_worker(index, &account).await
        });
    }

    // 使用 tokio::select! 处理多路事件
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
        // 异步操作
        adapter.navigate("https://facebook.com").await?;
        Ok(WorkerResult::success())
    }
}
```

### 3.5 测试约定
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::browser::mock_adapter::MockBrowserAdapter;

    #[test]
    fn test_account_creation() {
        // Arrange-Act-Assert 模式
        let account = Account::new("test@example.com".to_string(), "pass".to_string());
        assert_eq!(account.username, "test@example.com");
    }

    #[tokio::test]
    async fn test_async_operation() {
        // 使用 Mock Adapter 进行测试
        let adapter = MockBrowserAdapter::new();
        let strategy = OutlookRegisterStrategy::new();
        let account = Account::new("test@outlook.com".to_string(), "Pass123!".to_string());
        
        let result = strategy.run(&adapter, &account).await;
        assert!(result.is_ok(), "策略执行失败: {:?}", result);
    }
}
```

**Mock 使用示例**：
```rust
// MockBrowserAdapter：模拟浏览器操作（无需真实浏览器）
use crate::infrastructure::browser::mock_adapter::MockBrowserAdapter;
let adapter = MockBrowserAdapter::new();

// MockTimeProvider：模拟时间（用于测试时间敏感逻辑）
use crate::core::time::{MockTimeProvider, TimeProvider};
let time_provider = MockTimeProvider::with_fixed_time(
    chrono::Utc::now() - chrono::Duration::hours(2)
);
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

### 目录结构与职责
- **`src/core/`**：领域核心
  - `cli.rs`：命令行参数解析（master/worker/daemon 模式）
  - `models.rs`：核心数据模型（Account, WorkerResult）
  - `error.rs`：统一错误类型（AppError, AppResult）
  - `time.rs`：时间提供者（生产 + Mock）
  - `config.rs`：全局配置管理

- **`src/infrastructure/`**：基础设施层（外部依赖交互）
  - `browser/`：浏览器适配器（Playwright, Mock）
  - `adspower.rs`：AdsPower API 客户端
  - `bitbrowser.rs`：BitBrowser API 客户端
  - `browser_manager.rs`：多浏览器适配器管理
  - `imap.rs`：邮件接收（IMAP 协议）
  - `proxy_pool.rs`：代理池管理
  - `daemon.rs`：守护进程化工具
  - `logging.rs`：日志初始化

- **`src/services/`**：业务服务层
  - `master/`：主控模块
    - `server.rs`：Master 主服务器
    - `watcher.rs`：文件监控器
    - `scheduler.rs`：任务调度器
  - `worker/`：工作器模块
    - `coordinator.rs`：Worker 协调器
    - `runner.rs`：Worker 执行器
    - `strategy.rs`：策略枚举
    - `factory.rs`：策略工厂
  - `email/`：邮件服务
    - `config.rs`：邮件配置
    - `tracker.rs`：邮件追踪器
  - `processor.rs`：文件处理器（CSV 解析）

- **`src/strategies/`**：可插拔自动化策略
  - `mod.rs`：BaseStrategy trait 定义
  - `facebook_login/`：Facebook 登录策略
  - `outlook_register/`：Outlook 注册策略

- **`src/config/`**：配置模块
  - `logging.rs`：日志配置

### 数据流
```
输入源（CSV文件/邮件）
  ↓
Master Server (watcher/email tracker)
  ↓
Scheduler（任务分发）
  ↓
Worker Coordinator（并发管理）
  ↓
Strategy（业务逻辑） + BrowserAdapter（浏览器操作）
  ↓
输出（results.csv → doned/目录）
```

## 5. 环境变量

### 必需配置
- **`INPUT_DIR`**：监控输入文件的目录（默认：`input`）
- **`DONED_DIR`**：处理后文件的存放目录（默认：`input/doned`）

### 浏览器适配器配置

#### AdsPower（指纹浏览器）
- **`ADSPOWER_API_URL`**：API 地址（默认：`http://127.0.0.1:50325`）
- **`ADSPOWER_API_KEY`**：API 鉴权密钥（示例：`8d2b9c4c3cc37c78e9a91debad28f910`）
- **`ADSPOWER_PROXYID`**：代理配置 ID（数字，如 `3`）
  - 用途：创建新浏览器环境时指定代理
  - 获取方式：AdsPower 管理面板 → 代理配置 → 复制 ID

#### BitBrowser（指纹浏览器）
- **`BITBROWSER_API_URL`**：API 地址（默认：`http://127.0.0.1:54345`）
- **`BITBROWSER_API_KEY`**：API 鉴权密钥

#### 代理配置
- **`DYNAMIC_IP_URL`**：动态 IP 提取 API（用于代理池轮换）
- **`NO_PROXY`**：免代理域名列表（示例：`127.0.0.1,localhost`）

### 邮件监控配置（可选）
- **`EMAIL_USERNAME`**：邮箱账号
- **`EMAIL_PASSWORD`**：邮箱密码（建议使用应用专用密码）
- **`EMAIL_IMAP_SERVER`**：IMAP 服务器（如 `outlook.office365.com`）
- **`EMAIL_IMAP_PORT`**：IMAP 端口（默认：`993`）
- **`EMAIL_POLL_INTERVAL`**：邮件轮询间隔（秒，默认：`60`）
- **`EMAIL_SUBJECT_FILTER`**：邮件标题过滤关键词（如 `FB账号`）
- **`EMAIL_PROCESSED_FOLDER`**：已处理邮件文件夹名称

### 配置示例
参考项目根目录的 `.env` 文件获取完整配置示例。

## 6. 测试文件结构

### 单元测试（在源模块中）
- `src/core/models.rs`：Account、WorkerResult 模型测试
- `src/services/email/tracker.rs`：邮件追踪逻辑测试
- `src/core/time.rs`：时间提供者测试（含 Mock）

### 集成测试（`tests/` 目录）
- **`integration_test.rs`**：完整流程测试（文件监控 + Worker 执行）
- **`outlook_register_test.rs`**：Outlook 注册策略测试
- **`bitbrowser_integration_test.rs`**：BitBrowser 集成测试

### Mock 实现与使用

#### MockBrowserAdapter
模拟浏览器操作，无需真实浏览器环境。

**定义位置**：`src/infrastructure/browser/mock_adapter.rs`

**使用示例**：
```rust
use crate::infrastructure::browser::mock_adapter::MockBrowserAdapter;
use crate::strategies::outlook_register::OutlookRegisterStrategy;

#[tokio::test]
async fn test_strategy_with_mock() {
    let adapter = MockBrowserAdapter::new();
    let strategy = OutlookRegisterStrategy::new();
    let account = Account::new("test@outlook.com".to_string(), "Pass123!".to_string());
    
    let result = strategy.run(&adapter, &account).await;
    assert!(result.is_ok());
}
```

#### MockTimeProvider
模拟时间，用于测试时间敏感逻辑（如邮件轮询间隔、会话超时）。

**定义位置**：`src/core/time.rs`

**使用示例**：
```rust
use crate::core::time::{MockTimeProvider, TimeProvider};
use chrono::Duration;

#[test]
fn test_time_sensitive_logic() {
    // 固定时间为 2 小时前
    let fixed_time = chrono::Utc::now() - Duration::hours(2);
    let time_provider = MockTimeProvider::with_fixed_time(fixed_time);
    
    assert_eq!(time_provider.now(), fixed_time);
}
```

### 运行 Mock 测试
Mock 适配器可以在任何环境运行，无需外部依赖：
```bash
cargo test --lib               # 运行所有单元测试（包含 Mock 测试）
cargo test mock                # 仅运行包含 "mock" 的测试
```

## 7. 关键设计决策

### 为何支持多种浏览器适配器？
- **Mock**：快速测试，CI/CD 友好，无需真实浏览器
- **Playwright**：本地开发调试，查看真实浏览器行为
- **AdsPower/BitBrowser**：生产环境，防止指纹检测和封号

### Master-Worker 并发控制
- **并发数配置**：由 CSV 文件行数和系统资源决定
- **Worker 数量**：每个账号独立 Worker，最大并发数 = CPU 核心数 × 2（可调整）
- **任务调度**：Master 负责分发，Worker 独立执行，失败自动重试

### 错误处理策略
- **业务逻辑层**（strategies）：使用 `anyhow::Result` 灵活上下文
- **基础设施层**（services/infrastructure）：使用 `AppResult<T>` 类型安全
- **互操作性**：`AppError::Other(#[from] anyhow::Error)` 支持双向转换

### 代理池管理
- **轮换策略**：每个 Worker 使用独立代理，失败自动切换
- **动态 IP**：支持通过 API 获取新代理（`DYNAMIC_IP_URL`）
- **回退机制**：代理池耗尽时使用 `ADSPOWER_PROXYID` 静态代理

## 8. 常见问题与排查

### 浏览器启动失败
```bash
# 检查 AdsPower/BitBrowser 是否运行
curl http://127.0.0.1:50325/api/v1/browser/active  # AdsPower
curl http://127.0.0.1:54345/api/v1/browser/active  # BitBrowser

# 检查 API Key 是否正确
echo $ADSPOWER_API_KEY

# 查看详细错误日志
RUST_LOG=debug cargo run -- master
```

### 文件监控不工作
```bash
# 检查目录权限
ls -la input/

# 检查环境变量
echo $INPUT_DIR
echo $DONED_DIR

# 手动触发文件处理（绕过监控）
cp test.csv input/
# 观察日志输出
```

### Worker 执行超时
```bash
# 检查代理连接
curl --proxy socks5://your-proxy:port https://www.google.com

# 增加超时时间（修改策略代码）
tokio::time::timeout(Duration::from_secs(120), operation).await

# 查看 Worker 日志
tail -f logs/auto-scanner.log
```

### 测试失败排查
```bash
# 显示完整测试输出
cargo test -- --nocapture

# 运行单个失败测试
cargo test test_name --exact -- --show-output

# 启用调试日志
RUST_LOG=debug cargo test
```

## 9. 开发工作流

### 添加新策略
1. 在 `src/strategies/` 创建新目录（如 `instagram_login/`）
2. 实现 `BaseStrategy` trait
3. 在 `src/services/worker/strategy.rs` 添加枚举变体
4. 在 `src/services/worker/factory.rs` 注册策略工厂
5. 编写测试（使用 `MockBrowserAdapter`）

### 修改现有策略
1. 修改策略代码（`src/strategies/*/mod.rs`）
2. 运行相关测试：`cargo test outlook_register`
3. 使用 Mock 验证逻辑：`cargo test --lib`
4. 本地验证：`cargo run -- worker --strategy outlook_register --account test.csv`

### 提交前检查清单
```bash
cargo fmt --check          # 代码格式
cargo clippy --all-targets # Lint 检查
cargo test                 # 所有测试
./scripts/test.sh          # 完整测试套件
```
