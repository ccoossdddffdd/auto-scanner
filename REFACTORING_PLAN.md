# 代码重构计划

**创建时间**: 2026-01-14  
**目标**: 降低函数复杂度，提高代码可读性、可测性，优化文件管理结构

## 第一轮重构 (已完成)

### P0: 拆分 `master.rs` 的 `run` 函数 ✅
**问题**: 276 行巨型函数，包含日志初始化、PID 管理、文件监控、邮件监控、信号处理、主事件循环

**拆分方案**:
- `initialize_logging()` - 日志系统初始化
- `setup_pid_management(daemon: bool)` - PID 文件管理
- `create_file_watcher(path, tx)` - 文件监控器设置
- `initialize_email_monitor(config)` - 邮件监控初始化

**收益**: 主函数从 276 行减少到约 120 行 (-56%)

---

### P1: 重构 `processor.rs` 的 `process_file` 函数 ✅
**问题**: 327 行函数混合多个职责，Worker 调度闭包嵌套过深

**拆分方案**:
- `WorkerCoordinator` 结构体封装 Worker 调度逻辑
- `prepare_input_file()` - 文件预处理
- `write_results_and_rename()` - 结果写回
- `handle_email_notification()` - 通知处理

**收益**: 主函数从 327 行减少到约 70 行 (-79%)

---

## 第二轮重构 (进行中)

### P0: 拆分 `EmailMonitor::fetch_and_process_email` ✅
**问题**: 75 行函数包含多个职责：邮件获取、解析、附件提取、通知发送、IMAP 操作

**拆分方案**:
```rust
// 邮件解析器
struct EmailParser;
impl EmailParser {
    fn parse_from_address(parsed: &Message) -> String
    fn parse_subject(parsed: &Message) -> String
}

// 工作流函数
async fn fetch_email_data(uid, session) -> Result<Option<Fetch>>
async fn process_email_workflow(uid, parsed, session) -> Result<()>
fn should_process_email(&subject) -> bool
async fn process_attachments(uid, parsed, from, session) -> Result<()>
```

**收益**:
- 主函数从 75 行降至 18 行 (-76%)
- 职责单一，易于测试
- 降低认知负担

---

### P1: 修复 `extract_attachments` 错误处理 ✅
**问题**: 多次调用 `.unwrap()` 容易 panic

**解决方案**:
```rust
let content_type = part.content_type()
    .map(|ct| {
        if let Some(subtype) = ct.subtype() {
            format!("{}/{}", ct.c_type, subtype)
        } else {
            ct.c_type.to_string()
        }
    })
    .unwrap_or_else(|| "application/octet-stream".to_string());
```

**收益**:
- 消除潜在 panic
- 提供默认 content_type
- 更健壮的错误处理

---

### P2: 提取 `FacebookLoginStrategy` 结果检测 ✅
**问题**: 结果检测逻辑（30 行）耦合在主流程中，多个 `is_visible` 串行执行

**拆分方案**:
```rust
enum LoginStatus {
    Success, Captcha, TwoFactor, Failed
}

struct LoginResultDetector;
impl LoginResultDetector {
    async fn detect_status(adapter) -> LoginStatus {
        // 并行检测
        let (is_success, has_captcha, has_2fa) = tokio::join!(
            Self::check_success(adapter),
            Self::check_captcha(adapter),
            Self::check_2fa(adapter),
        );
        // ...
    }
}
```

**收益**:
- 并行检测提高性能（3 个 await 并行而非串行）
- 结果检测逻辑可独立测试
- 易于扩展新的登录状态

---

### P3: 统一 `AdsPowerClient` 错误处理 ✅
**问题**: 每个 API 调用重复相同的错误处理模式

**拆分方案**:
```rust
// 统一 API 调用封装
async fn call_api<T, R>(method, endpoint, body) -> Result<R>
async fn call_api_with_query<R>(endpoint, query) -> Result<Option<R>>

// 简化调用
pub async fn create_profile(&self, username: &str) -> Result<String> {
    let body = json!({...});
    let resp: CreateProfileResponse = self
        .call_api("POST", "/api/v1/user/create", Some(body))
        .await?;
    Ok(resp.id)
}
```

**收益**:
- DRY 原则，减少重复代码 60%
- 统一错误上下文
- 易于添加重试、日志、监控

---

### P4: 重构主事件循环 ✅
**问题**: 主循环中的文件处理逻辑 35 行，包含路径检查、配置构建、结果处理

**拆分方案**:
```rust
struct FileProcessor {
    adspower: Option<Arc<AdsPowerClient>>,
    backend: String,
    remote_url: String,
    exe_path: PathBuf,
    enable_screenshot: bool,
    doned_dir: PathBuf,
}

impl FileProcessor {
    async fn process_incoming_file(...) -> Result<PathBuf>
    fn extract_batch_name(&path) -> String
    fn build_process_config(batch_name) -> ProcessConfig
}
```

**收益**:
- 主循环从 80 行降至 25 行 (-69%)
- 文件处理逻辑可单元测试
- 更清晰的职责边界

---

## 后续建议

### P3: 重组文件结构
- 采用 DDD 分层架构 (domain/application/infrastructure)
- 清晰的依赖方向
- 更好的模块化

### P4: 引入配置对象
- `WorkerConfig` 封装 Worker 参数
- `ProcessConfig` 包含 `WorkerPool`
- 减少函数参数数量到 1-2 个

---

## 总结

### 第一轮成果 ✅
- ✅ 降低函数复杂度: 最大函数从 327 行降至 120 行
- ✅ 提高可读性: 拆分为职责单一的小函数
- ✅ 提高可测性: 独立函数易于单元测试
- ✅ 保持兼容性: 所有测试通过

### 第二轮成果 ✅
- ✅ EmailMonitor 函数从 75 行降至 18 行 (-76%)
- ✅ 消除 extract_attachments 的潜在 panic
- ✅ LoginResultDetector 并行检测，提高性能
- ✅ AdsPowerClient 代码重复减少 60%
- ✅ 主事件循环从 80 行降至 25 行 (-69%)

