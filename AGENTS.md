# Auto Scanner - 开发人员与 Agent 指南

## 1. 项目概览

Auto Scanner 是一个高性能、异步的 Rust 应用程序，专为自动化浏览器交互而设计。它采用 **Master-Worker 架构** 来高效处理任务，同时支持本地执行（通过 Playwright）和指纹浏览器集成（通过 AdsPower）。

系统基于“文件驱动”和“邮件触发”模式运行：它监控目录中的输入文件（CSV/Excel）或通过电子邮件接收文件，利用浏览器自动化并发处理账号，并输出结果。

## 2. 架构图谱

代码库遵循清晰的分层架构：

- **`src/core/`**: 领域模型和共享工具。
    - `cli.rs`: 命令行接口定义（Master/Worker 模式）。
    - `models.rs`: 核心数据结构（`Account`, `WorkerResult`）。

- **`src/infrastructure/`**: 外部系统交互的实现。
    - `browser/`: 浏览器自动化适配器（`BrowserAdapter` trait），将逻辑与具体驱动（Playwright/Mock）解耦。
    - `adspower.rs`: 健壮的 AdsPower API 客户端（环境管理、浏览器控制）。
    - `imap.rs`: 邮件协议处理。
    - `process.rs` & `daemon.rs`: 系统级进程管理。

- **`src/services/`**: 业务逻辑与编排。
    - `master.rs`: 中枢神经系统。负责文件监控（`notify`）、并发控制（`permit` 通道）和生命周期管理。
    - `worker/`: Worker 进程逻辑（`coordinator.rs`, `runner.rs`）。
    - `email/`: 端到端邮件监控、解析和通知服务。
    - `processor.rs`: 连接 Master 和 Worker，处理文件 I/O 和结果聚合。

- **`src/strategies/`**: 可插拔的自动化策略。
    - `facebook/`: 实现 `LoginStrategy`。包含登录、处理 2FA/验证码以及提取数据的逻辑。
        - `mod.rs`: 主逻辑。
        - `constants.rs`: 集中管理的配置，包含选择器、超时设置和关键词。

## 3. 核心工作流

### 3.1. 文件处理流水线

1.  **检测**: `Master` 检测到 `INPUT_DIR` 中的新文件（通过 `notify`）。
2.  **解析**: `csv_reader` 或 `excel_handler` 将文件解析为 `Vec<Account>`。
3.  **分发**: 根据可用的线程许可，将账号分发给 `WorkerCoordinator`。
4.  **执行**:
    - 启动一个 Worker 进程。
    - 如果使用 AdsPower，创建/检索唯一的浏览器环境。
    - `FacebookLoginStrategy` 执行自动化任务。
5.  **聚合**: 收集结果并回写到文件。
6.  **收尾**: 处理后的文件移动到 `DONED_DIR`。

### 3.2. 邮件自动化流程

1.  **监控**: `EmailMonitor` 通过 IMAP 轮询收件箱。
2.  **触发**: 发现匹配的邮件后，下载附件到 `INPUT_DIR`。
3.  **处理**: 标准文件处理流水线 (3.1) 接管。
4.  **回复**: 处理完成后，`EmailNotification` 将结果文件作为附件回复给发送者。

## 4. 开发指南

### 4.1. 代码风格与规范

- **异步优先**: 项目大量使用异步（`tokio`）。避免在异步上下文中进行阻塞操作。
- **错误处理**: 使用 `anyhow::Result` 处理应用级错误。使用 `.context()` 提供上下文。
    - _规则_: 严禁在生产代码中使用 `.unwrap()`。使用 `match` 或 `?` 优雅地处理错误。
- **配置管理**:
    - 使用 `src/strategies/facebook/constants.rs` 管理策略相关的常量（选择器、关键词）。
    - 使用 `.env` 和 `clap`（CLI 参数）进行系统配置。

### 4.2. 添加新策略

1.  在 `src/strategies/` 中创建一个新模块。
2.  实现 `LoginStrategy` trait。
3.  定义选择器/常量的配置结构体。
4.  在 `src/services/worker/runner.rs` 中注册策略。

### 4.3. 与 AdsPower 协作

- `AdsPowerClient` (`src/infrastructure/adspower.rs`) 设计得非常健壮。
- 它处理速率限制和连接检查。
- **重要**: 添加新的 AdsPower 功能时，确保处理好生命周期（创建 -> 启动 -> 停止 -> 删除），以防止资源泄露（僵尸环境）。

## 5. 配置参考

### 环境变量

- `INPUT_DIR`: 监控输入文件的目录。
- `DONED_DIR`: 处理后文件的存放目录。
- `ADSPOWER_API_URL`: 本地 AdsPower API 的 URL（默认：`http://127.0.0.1:50325`）。
- `ADSPOWER_API_KEY`: AdsPower 的 API Key。
- `ADSPOWER_PROXYID`: 用于创建环境的代理 ID。

### 策略配置

参见 `src/strategies/facebook/constants.rs` 中的可修改值，包括：

- `timeouts`: 登录和页面加载等待时间。
- `selectors`: 登录表单、指示器和错误信息的 CSS 选择器。
- `keywords`: 用于检测验证码、锁定和密码错误的多语言列表。

## 6. 测试

- **单元测试**: 运行 `cargo test` 执行位于源模块内的单元测试。
- **集成测试**: 运行 `cargo test --test integration_test` 执行完整的端到端工作流测试（使用 Mock 后端）。
