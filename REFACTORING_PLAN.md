# 代码重构计划

**创建时间**: 2026-01-14  
**目标**: 降低函数复杂度，提高代码可读性、可测性，优化文件管理结构

## 重构优先级

### P0: 拆分 `master.rs` 的 `run` 函数
**问题**: 276 行巨型函数，包含日志初始化、PID 管理、文件监控、邮件监控、信号处理、主事件循环

**拆分方案**:
- `initialize_logging()` - 日志系统初始化
- `setup_pid_management(daemon: bool)` - PID 文件管理
- `create_file_watcher(path, tx)` - 文件监控器设置
- `initialize_email_monitor(config)` - 邮件监控初始化
- `run_event_loop(...)` - 主事件循环

**预期收益**:
- 单一职责，每个函数 < 50 行
- 可独立单元测试
- 降低认知负担

---

### P1: 重构 `processor.rs` 的 `process_file` 函数
**问题**: 327 行函数混合多个职责，Worker 调度闭包嵌套过深

**拆分方案**:
```rust
// 文件预处理
async fn prepare_input_file(path: &Path, email_monitor: Option<&EmailMonitor>) -> Result<PathBuf>

// Worker 协调器
struct WorkerCoordinator {
    permit_rx: Receiver<usize>,
    permit_tx: Sender<usize>,
    adspower: Option<Arc<AdsPowerClient>>,
}

impl WorkerCoordinator {
    async fn spawn_worker(&self, account: &Account, config: &WorkerConfig) -> Result<(usize, Option<WorkerResult>)>
}

// 结果写回
async fn write_results_and_rename(path: &Path, results: Vec<(usize, Option<WorkerResult>)>, ...) -> Result<PathBuf>

// 通知处理
async fn handle_email_notification(monitor: &EmailMonitor, email_id: &str, result: Result<PathBuf>)
```

**预期收益**:
- 提高可测试性
- 清晰的职责边界
- 降低圈复杂度

---

### P2: 提取 `EmailMonitor` 的邮件处理逻辑
**问题**: `fetch_and_process_email` 包含邮件解析、附件提取、文件保存、IMAP 操作

**拆分方案**:
```rust
// 邮件解析器
struct EmailParser;
impl EmailParser {
    fn parse_message(raw: &[u8]) -> Result<ParsedEmail>
    fn extract_attachments(msg: &Message) -> Result<Vec<Attachment>>
}

// 附件处理器
struct AttachmentHandler {
    input_dir: PathBuf,
    file_tracker: Arc<FileTracker>,
}
impl AttachmentHandler {
    async fn save_attachment(&self, uid: u32, attachment: &Attachment) -> Result<PathBuf>
    fn generate_filename(&self, uid: u32, original_name: &str) -> String
}
```

**预期收益**:
- 单一职责原则
- 可独立测试解析逻辑
- 更清晰的附件文件名生成

---

### P3: 优化文件管理结构
**问题**: `services/file/` 和 `infrastructure/` 职责不清晰

**重组方案**:
```
src/
├── domain/               # 领域模型（核心业务实体）
│   ├── account.rs       
│   └── worker_result.rs 
├── application/          # 应用服务（高层编排）
│   ├── processor.rs     # 文件处理编排
│   └── master.rs        # Master 进程
├── infrastructure/       # 基础设施（外部依赖）
│   ├── browser/
│   ├── adspower.rs
│   ├── imap.rs
│   └── file_storage/    # 统一文件操作
│       ├── csv_handler.rs
│       ├── excel_handler.rs
│       └── file_manager.rs
└── services/             # 领域服务
    └── email/
```

**预期收益**:
- 遵循依赖倒置原则
- 清晰的分层架构
- 易于扩展

---

### P4: 引入配置对象减少参数传递
**问题**: 函数参数过多（5-6 个），闭包内局部变量 10+

**优化方案**:
```rust
#[derive(Clone)]
pub struct WorkerConfig {
    pub account: Account,
    pub backend: String,
    pub remote_url: String,
    pub exe_path: PathBuf,
    pub enable_screenshot: bool,
    pub adspower: Option<Arc<AdsPowerClient>>,
    pub thread_index: usize,
}

pub async fn run_worker(config: WorkerConfig) -> Result<WorkerResult>

pub struct ProcessConfig {
    pub batch_name: String,
    pub worker_pool: Arc<WorkerPool>,
    pub file_config: FileConfig,
    pub email_monitor: Option<Arc<EmailMonitor>>,
}
```

**预期收益**:
- 减少参数数量（1-2 个）
- 提高可读性
- 便于添加新配置项

---

## 实施策略

1. **逐个处理**: 按 P0 → P4 顺序
2. **测试保障**: 每次重构后运行 `cargo test && cargo clippy`
3. **增量提交**: 每完成一个优先级提交一次
4. **保持行为**: 重构不改变外部行为

## 当前状态
- [ ] P0: 拆分 master::run
- [ ] P1: 重构 processor::process_file
- [ ] P2: 提取 EmailMonitor 逻辑
- [ ] P3: 重组文件结构
- [ ] P4: 引入配置对象
