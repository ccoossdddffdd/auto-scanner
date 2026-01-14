# 代码重构计划 - 第四轮

**创建时间**: 2026-01-14  
**目标**: 降低认知复杂度，提升代码可读性和可测性，优化错误处理

---

## 重构优先级

### P0: 拆分 master::run 主事件循环 (认知复杂度 29/25) 🔴
**问题**: 
- 函数认知复杂度 29 超过阈值 (25)
- 168 行函数包含多个职责：初始化、信号处理、文件处理、配置构建
- 循环内部 select! 分支逻辑复杂

**当前代码**:
```rust
// src/services/master.rs:106-272
pub async fn run(input_dir: Option<String>, config: MasterConfig) -> Result<()> {
    // 106-189: 初始化逻辑 (83 行)
    // - PID 管理、日志、目录创建
    // - 文件扫描、监控器设置
    // - 邮件监控、线程池、信号处理
    
    // 195-266: 主事件循环 (71 行)
    loop {
        tokio::select! {
            _ = sigterm.recv() => { ... }
            _ = sigint.recv() => { ... }
            Some(csv_path) = rx.recv() => {
                // 205-263: 文件处理逻辑 (58 行)
                // - 路径验证
                // - 配置构建 (3 个 config 对象)
                // - process_file 调用
                // - 结果处理
            }
        }
    }
}
```

**重构方案**:
```rust
// 1. 提取初始化逻辑
struct MasterContext {
    input_path: PathBuf,
    doned_dir: PathBuf,
    adspower: Option<Arc<AdsPowerClient>>,
    exe_path: PathBuf,
    email_monitor: Option<Arc<EmailMonitor>>,
    permit_rx: async_channel::Receiver<usize>,
    permit_tx: async_channel::Sender<usize>,
    processing_files: Arc<std::sync::Mutex<HashSet<PathBuf>>>,
}

impl MasterContext {
    async fn initialize(config: &MasterConfig, input_dir: String) -> Result<Self>
}

// 2. 提取文件处理器
struct FileProcessingHandler {
    config: MasterConfig,
    context: Arc<MasterContext>,
}

impl FileProcessingHandler {
    async fn handle_incoming_file(&self, path: PathBuf) -> Result<PathBuf>
    
    fn build_process_config(&self, batch_name: String) -> ProcessConfig
}

// 3. 简化主循环
pub async fn run(input_dir: Option<String>, config: MasterConfig) -> Result<()> {
    // 初始化
    let context = MasterContext::initialize(&config, input_dir?).await?;
    let handler = FileProcessingHandler::new(config, Arc::new(context));
    
    // 主循环 (< 30 行)
    loop {
        tokio::select! {
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
            Some(path) = rx.recv() => {
                handler.handle_incoming_file(path).await;
            }
        }
    }
}
```

**收益**:
- 主循环从 168 行降至 < 40 行 (-76%)
- 认知复杂度从 29 降至 < 15
- 初始化逻辑可单元测试
- 文件处理逻辑可独立测试

**风险**: 中等 - 需要重新组织多个依赖关系

---

### P1: 重构 WorkerCoordinator::spawn_worker (认知复杂度 26/25) 🔴
**问题**:
- 函数认知复杂度 26 超过阈值
- 94 行函数包含嵌套 if-let 和多个错误路径
- AdsPower 集成逻辑与 Worker 启动逻辑耦合

**当前代码**:
```rust
// src/services/worker/coordinator.rs:20-114
pub async fn spawn_worker(&self, index: usize, account: &Account) 
    -> (usize, Option<WorkerResult>) 
{
    let thread_index = self.permit_rx.recv().await.unwrap();
    
    // 25-68: AdsPower 集成 (43 行)
    let mut adspower_id = None;
    let mut active_remote_url = self.remote_url.clone();
    
    if let Some(client) = &self.adspower {
        match client.ensure_profile_for_thread(thread_index).await {
            Ok(id) => {
                // 嵌套错误处理
                if let Err(e) = client.update_profile_for_account(&id, &username).await {
                    // 错误返回
                }
                match client.start_browser(&id).await {
                    // 更多嵌套
                }
            }
            Err(e) => { ... }
        }
    }
    
    // 70-114: Worker 进程启动 (44 行)
    let mut cmd = Command::new(&self.exe_path);
    // ...命令构建和执行
}
```

**重构方案**:
```rust
// 1. 提取 AdsPower 准备逻辑
struct AdsPowerSession {
    profile_id: String,
    ws_url: String,
}

impl WorkerCoordinator {
    async fn prepare_adspower_session(
        &self, 
        thread_index: usize, 
        username: &str
    ) -> Result<Option<AdsPowerSession>>
    
    // 2. 提取 Worker 命令构建
    fn build_worker_command(
        &self,
        username: &str,
        password: &str,
        remote_url: &str,
    ) -> Command
    
    // 3. 提取执行逻辑
    async fn execute_worker(
        &self,
        cmd: Command,
        username: &str,
    ) -> Result<WorkerResult>
    
    // 4. 简化主函数
    pub async fn spawn_worker(&self, index: usize, account: &Account) 
        -> (usize, Option<WorkerResult>) 
    {
        let thread_index = self.acquire_thread().await;
        let session = self.prepare_adspower_session(thread_index, &account.username).await;
        
        let remote_url = session.as_ref()
            .map(|s| s.ws_url.as_str())
            .unwrap_or(&self.remote_url);
        
        let cmd = self.build_worker_command(&account.username, &account.password, remote_url);
        let result = self.execute_worker(cmd, &account.username).await;
        
        self.cleanup(session, thread_index).await;
        (index, result.ok())
    }
}
```

**收益**:
- 主函数从 94 行降至 < 25 行 (-73%)
- 认知复杂度从 26 降至 < 10
- AdsPower 逻辑可独立测试
- Worker 执行可模拟测试

---

### P2: 统一错误处理模式 - Result Type Alias 🟡
**问题**:
- 整个项目中使用 `anyhow::Result` 缺乏类型安全
- 多处 `Result<()>` 重复出现
- 无法区分不同领域的错误类型

**当前状态**:
```rust
// 分散在各处
pub async fn process_file(...) -> Result<PathBuf> { ... }
pub async fn run(...) -> Result<()> { ... }
pub fn new(...) -> Result<EmailMonitor> { ... }
```

**重构方案**:
```rust
// 创建 src/core/error.rs
use anyhow::Result as AnyhowResult;

/// 应用级别通用 Result 类型
pub type AppResult<T> = AnyhowResult<T>;

/// Unit Result 简写
pub type UnitResult = AnyhowResult<()>;

/// 各领域特定 Result
pub type EmailResult<T> = AppResult<T>;
pub type ProcessResult<T> = AppResult<T>;
pub type BrowserResult<T> = AppResult<T>;

// 应用到整个项目
pub async fn process_file(...) -> ProcessResult<PathBuf> { ... }
pub async fn run(...) -> UnitResult { ... }
pub fn new(...) -> EmailResult<EmailMonitor> { ... }
```

**收益**:
- 提高类型语义清晰度
- 为未来迁移到自定义错误类型做准备
- 简化函数签名可读性

**影响**: 低 - 纯类型别名，无运行时开销

---

### P3: 拆分 EmailMonitor::check_and_process_emails 的 IMAP 会话管理 🟡
**问题**:
- 函数混合 IMAP 连接管理和邮件处理逻辑
- 52 行函数包含连接、搜索、循环处理、登出
- 错误处理导致会话可能未正确关闭

**当前代码**:
```rust
// src/services/email/monitor.rs:162-211
async fn check_and_process_emails(&self) -> Result<()> {
    let imap_client = ImapClient::new(...);
    let mut session = imap_client.connect().await?;
    
    let inbox = session.select("INBOX").await?;
    let search_result = session.search("UNSEEN").await?;
    let uid_set: Vec<u32> = search_result.iter().copied().collect();
    
    if uid_set.is_empty() {
        session.logout().await?;  // 早返回需要手动登出
        return Ok(());
    }
    
    for uid in &uid_set {
        if let Err(e) = self.fetch_and_process_email(*uid, &mut session).await {
            error!("Failed to process email UID {}: {}", uid, e);
        }
    }
    
    session.logout().await?;  // 重复的登出逻辑
    Ok(())
}
```

**重构方案**:
```rust
// 1. 创建 RAII 风格的会话包装器
struct ImapSessionGuard {
    session: ImapSession,
}

impl ImapSessionGuard {
    async fn new(config: &EmailConfig) -> Result<Self> {
        let client = ImapClient::new(...);
        let session = client.connect().await?;
        Ok(Self { session })
    }
    
    fn as_mut(&mut self) -> &mut ImapSession {
        &mut self.session
    }
}

impl Drop for ImapSessionGuard {
    fn drop(&mut self) {
        // 确保会话总是被关闭
        // 注意: 需要使用 tokio::spawn 或其他机制处理 async
    }
}

// 2. 提取邮件搜索逻辑
async fn search_unread_emails(session: &mut ImapSession) -> Result<Vec<u32>>

// 3. 简化主函数
async fn check_and_process_emails(&self) -> Result<()> {
    let mut session = ImapSessionGuard::new(&self.config).await?;
    let uid_set = search_unread_emails(session.as_mut()).await?;
    
    if uid_set.is_empty() {
        info!("No new unread emails found");
        return Ok(());
    }
    
    info!("Found {} unread emails", uid_set.len());
    self.process_email_batch(&uid_set, session.as_mut()).await
}

async fn process_email_batch(&self, uids: &[u32], session: &mut ImapSession) -> Result<()> {
    for uid in uids {
        if let Err(e) = self.fetch_and_process_email(*uid, session).await {
            error!("Failed to process email UID {}: {}", uid, e);
        }
    }
    Ok(())
}
```

**收益**:
- 会话生命周期管理更安全
- 消除重复的 logout 调用
- 函数从 52 行降至 < 20 行 (-62%)
- 更清晰的错误传播路径

**风险**: 低 - RAII 模式在 Rust 中是标准实践

---

### P4: 重构 EmailConfig 配置验证逻辑 🟢
**问题**:
- `from_env()` 方法缺少配置验证
- 无效配置（如端口 0、空字符串）可能在运行时才发现
- 缺少配置完整性检查

**当前代码**:
```rust
// src/services/email/monitor.rs:31-46
pub fn from_env() -> Result<Self> {
    dotenv::dotenv().ok();
    
    Ok(Self {
        imap_server: Self::env_or("EMAIL_IMAP_SERVER", "outlook.office365.com"),
        imap_port: Self::env_parse("EMAIL_IMAP_PORT", 993)?,
        // ... 直接构造，无验证
    })
}
```

**重构方案**:
```rust
// 1. 添加验证方法
impl EmailConfig {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();
        
        let config = Self {
            imap_server: Self::env_or("EMAIL_IMAP_SERVER", "outlook.office365.com"),
            imap_port: Self::env_parse("EMAIL_IMAP_PORT", 993)?,
            smtp_server: Self::env_or("EMAIL_SMTP_SERVER", "smtp.office365.com"),
            smtp_port: Self::env_parse("EMAIL_SMTP_PORT", 587)?,
            username: Self::env_required("EMAIL_USERNAME")?,
            password: Self::env_required("EMAIL_PASSWORD")?,
            poll_interval: Self::env_parse("EMAIL_POLL_INTERVAL", 60)?,
            processed_folder: Self::env_or("EMAIL_PROCESSED_FOLDER", "已处理"),
            subject_filter: Self::env_or("EMAIL_SUBJECT_FILTER", "FB账号"),
            input_dir: Self::env_or("INPUT_DIR", "input").into(),
            doned_dir: Self::env_or("DONED_DIR", "input/doned").into(),
        };
        
        config.validate()?;
        Ok(config)
    }
    
    fn validate(&self) -> Result<()> {
        // 验证端口范围
        if self.imap_port == 0 || self.imap_port > 65535 {
            anyhow::bail!("Invalid IMAP port: {}", self.imap_port);
        }
        if self.smtp_port == 0 || self.smtp_port > 65535 {
            anyhow::bail!("Invalid SMTP port: {}", self.smtp_port);
        }
        
        // 验证服务器地址
        if self.imap_server.is_empty() {
            anyhow::bail!("IMAP server cannot be empty");
        }
        if self.smtp_server.is_empty() {
            anyhow::bail!("SMTP server cannot be empty");
        }
        
        // 验证轮询间隔
        if self.poll_interval == 0 {
            anyhow::bail!("Poll interval must be greater than 0");
        }
        if self.poll_interval > 3600 {
            warn!("Poll interval {} is very long (>1 hour), is this intended?", self.poll_interval);
        }
        
        // 验证目录路径
        if self.input_dir.to_str().map_or(true, |s| s.is_empty()) {
            anyhow::bail!("Input directory path is invalid");
        }
        if self.doned_dir.to_str().map_or(true, |s| s.is_empty()) {
            anyhow::bail!("Doned directory path is invalid");
        }
        
        Ok(())
    }
}
```

**收益**:
- 在启动时而非运行时发现配置错误
- 提供清晰的错误消息
- 防止无效配置导致的运行时失败
- 添加 15 行验证代码，节省潜在的数小时调试时间

**影响**: 低 - 仅添加验证逻辑，不改变现有行为

---

### P5: 优化 MasterConfig 结构过大问题 🟢
**问题**:
- `MasterConfig` 包含 11 个字段，职责不清晰
- 同时包含运行模式控制 (stop/status/daemon) 和业务配置
- 构造和传递时容易出错

**当前代码**:
```rust
// src/services/master.rs:22-34
#[derive(Clone, Debug)]
pub struct MasterConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
    pub enable_screenshot: bool,
    pub stop: bool,              // 控制字段
    pub daemon: bool,            // 控制字段
    pub status: bool,            // 控制字段
    pub enable_email_monitor: bool,
    pub email_poll_interval: u64,
    pub exe_path: Option<PathBuf>,
}
```

**重构方案**:
```rust
// 1. 分离控制模式和业务配置
#[derive(Clone, Debug)]
pub enum MasterMode {
    Run(RuntimeConfig),
    Stop,
    Status,
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
    pub enable_screenshot: bool,
    pub daemon: bool,
    pub enable_email_monitor: bool,
    pub email_poll_interval: u64,
    pub exe_path: Option<PathBuf>,
}

// 2. 简化主函数签名
pub async fn run(input_dir: Option<String>, mode: MasterMode) -> Result<()> {
    let config = match mode {
        MasterMode::Stop => return PidManager::new(PID_FILE).stop(),
        MasterMode::Status => return PidManager::new(PID_FILE).check_status(),
        MasterMode::Run(cfg) => cfg,
    };
    
    // 现在只处理运行逻辑，无需内部分支
    // ...
}

// 3. 从 CLI 构建
impl From<Cli> for MasterMode {
    fn from(cli: Cli) -> Self {
        if cli.stop {
            return MasterMode::Stop;
        }
        if cli.status {
            return MasterMode::Status;
        }
        MasterMode::Run(RuntimeConfig { ... })
    }
}
```

**收益**:
- 类型系统强制正确使用模式
- 消除主函数内部的模式判断
- 减少无效配置组合（如 stop=true 同时传入 thread_count）
- 提高代码可读性和类型安全性

**影响**: 中等 - 需要更新 CLI 参数构建逻辑

---

## 重构顺序建议

1. **第一批** (低风险): P2, P4
   - Result Type Alias 纯类型改动
   - EmailConfig 验证逻辑独立

2. **第二批** (中风险): P5, P3
   - MasterConfig 重构影响主入口
   - EmailMonitor 会话管理改进

3. **第三批** (高风险): P0, P1
   - master::run 主循环重构
   - WorkerCoordinator 复杂函数拆分

---

## 预期改进

### 代码度量
- 认知复杂度警告: 2 个 → 0 个 (-100%)
- 最长函数: 168 行 → < 40 行 (-76%)
- 平均函数长度: 降低 30%

### 质量提升
- 配置验证: 运行时 → 启动时
- 错误处理: 更一致的模式
- 类型安全: 模式匹配替代布尔标志

### 可测试性
- 新增可测试单元: 8 个
- 复杂逻辑隔离度: +40%

---

## 风险评估

| 任务 | 风险等级 | 测试覆盖要求 | 回滚难度 |
|------|---------|-------------|---------|
| P0   | 高      | 集成测试必需 | 中      |
| P1   | 高      | 单元+集成   | 中      |
| P2   | 低      | 现有测试足够 | 容易    |
| P3   | 低      | 现有测试足够 | 容易    |
| P4   | 低      | 单元测试    | 容易    |
| P5   | 中      | 集成测试必需 | 中      |

---

## 总结

第四轮重构聚焦于：
1. **降低认知复杂度** - 解决 Clippy 警告
2. **改善错误处理** - 统一模式和验证
3. **提升类型安全** - 使用枚举替代布尔标志
4. **增强可测试性** - 拆分复杂函数为小单元

预计完成后，项目将达到生产级代码质量标准。
