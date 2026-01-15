# 邮件自动监控系统实施计划

## 一、需求概述

### 业务需求
1. **收信与发信**: 使用IMAP接收邮件，使用SMTP发送确认邮件
2. **自动处理**: 从邮箱收取新邮件，检查标题包含"FB账号"且附件为.txt/.csv/.xls/.xlsx格式
3. **文件保存**: 自动保存附件到 --input 目录
4. **邮件回复**:
   - 收到后立刻回复"已收到"
   - 处理完毕后回复"已处理"，并发回修改后移动到doned目录的文件
   - 处理失败时回复"处理失败"，包含处理后的文件
5. **邮件管理**: 标记已读且移动到"已处理"文件夹
6. **轮询间隔**: 1分钟检查一次
7. **邮箱类型**: Outlook邮箱

### 技术需求
- 使用环境变量配置
- 中文文件夹名"已处理"
- 文件名添加时间戳避免冲突
- 不限制文件大小

## 二、技术架构

### 协议选择
```
收信: IMAP - imap crate (3.0.0-alpha.15)
发信: SMTP - lettre crate (0.11.19)
解析: mail-parser (0.11.1)
```

### Outlook配置
```
IMAP服务器: outlook.office365.com
IMAP端口: 993 (SSL/TLS)
SMTP服务器: smtp.office365.com
SMTP端口: 587 (STARTTLS)
```

### 模块设计
```
src/
├── email_sender.rs      (新建) - SMTP邮件发送模块
├── email_monitor.rs     (新建) - IMAP邮件监控核心
├── file_tracker.rs      (新建) - 文件处理追踪
├── cli.rs              (修改) - 添加邮件相关CLI参数
├── master.rs           (修改) - 集成邮件监控
└── lib.rs              (修改) - 声明新模块
```

### 业务流程
```
1. IMAP每60秒轮询检查新邮件
2. 筛选标题包含"FB账号"的邮件
3. 验证附件格式（.txt/.csv/.xls/.xlsx）
4. 下载附件到input目录（文件名添加时间戳）
5. 立刻发送"已收到"回复
6. 文件进入现有master处理流程
7. 监控doned目录，处理完成后：
   - 成功: 发送"已处理" + 处理后的文件
   - 失败: 发送"处理失败" + 处理后的文件
8. 标记邮件已读并移动到"已处理"文件夹
```

## 三、数据结构设计

### EmailConfig (email_monitor.rs)
```rust
pub struct EmailConfig {
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval: u64,
    pub processed_folder: String,
    pub subject_filter: String,
    pub input_dir: PathBuf,
    pub doned_dir: PathBuf,
}
```

### EmailInfo (email_monitor.rs)
```rust
pub struct EmailInfo {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub attachments: Vec<Attachment>,
    pub received_at: DateTime<Local>,
    pub original_filename: String,
    pub saved_file_path: PathBuf,
}
```

### ProcessingStatus (file_tracker.rs)
```rust
pub enum ProcessingStatus {
    Received { timestamp: DateTime<Local> },
    Downloaded { timestamp: DateTime<Local>, file_path: PathBuf },
    Processing { timestamp: DateTime<Local>, file_path: PathBuf },
    Success { timestamp: DateTime<Local>, processed_file: PathBuf },
    Failed { timestamp: DateTime<Local>, error_message: String, processed_file: Option<PathBuf> },
}
```

## 四、依赖包清单

```toml
[dependencies]
# 邮件相关
imap = { version = "3.0.0-alpha.15", features = ["tls-native"] }
lettre = { version = "0.11.19", features = ["tokio1", "smtp-transport", "native-tls", "builder"] }
mail-parser = "0.11.1"
native-tls = "0.2.12"

# 辅助库
mime = "0.3.17"
mime_guess = "2.0.5"
base64 = "0.22.1"
```

## 五、环境变量配置

```bash
# IMAP配置
EMAIL_IMAP_SERVER=outlook.office365.com
EMAIL_IMAP_PORT=993
EMAIL_USERNAME=your_email@outlook.com
EMAIL_PASSWORD=your_app_password

# SMTP配置
EMAIL_SMTP_SERVER=smtp.office365.com
EMAIL_SMTP_PORT=587

# 监控配置
EMAIL_POLL_INTERVAL=60
EMAIL_PROCESSED_FOLDER=已处理
EMAIL_SUBJECT_FILTER=FB账号
```

## 六、实施阶段

### Phase 1: 基础架构
- 添加依赖到Cargo.toml
- 创建email_sender.rs
- 创建file_tracker.rs
- 创建基础email_monitor.rs结构

### Phase 2: SMTP发送功能
- 实现EmailSender
- 实现发送文本邮件
- 实现发送带附件邮件
- 单元测试

### Phase 3: 文件追踪功能
- 实现FileTracker
- 实现状态管理
- 实现邮件元数据存储
- 单元测试

### Phase 4: IMAP接收功能
- 实现IMAP连接
- 实现邮件列表获取
- 实现邮件筛选
- 实现附件下载
- 单元测试

### Phase 5: 核心监控逻辑
- 实现轮询机制
- 实现"已收到"立即回复
- 实现邮件处理流程
- 实现标记和移动

### Phase 6: 集成到Master
- 扩展CLI参数
- 扩展MasterConfig
- 集成email_monitor任务
- 添加doned目录监控
- 实现状态通知回调

### Phase 7: 测试和优化
- 集成测试
- 错误处理完善
- 性能优化
- 文档更新

## 七、代码量估算

| 模块 | 预估行数 |
|------|----------|
| email_sender.rs | ~150行 |
| file_tracker.rs | ~200行 |
| email_monitor.rs | ~600行 |
| master.rs (更新) | +200行 |
| cli.rs (更新) | +15行 |
| lib.rs (更新) | +3行 |
| Cargo.toml (更新) | +8行 |
| **总计** | **~1,176行** |

## 八、里程碑

- ✅ Milestone 1: 基础架构完成
- ✅ Milestone 2: SMTP发送功能完成
- ✅ Milestone 3: 文件追踪完成
- ✅ Milestone 4: IMAP接收完成
- ✅ Milestone 5: 监控逻辑完成
- ✅ Milestone 6: Master集成完成
- ✅ Milestone 7: 测试通过

## 九、注意事项

1. **IMAP API**: imap crate 3.0.0-alpha版本API可能不稳定，需要仔细测试
2. **附件解析**: 支持多种MIME类型和编码
3. **并发安全**: 使用Arc<Mutex<>>保护共享状态
4. **错误处理**: 完善重试机制和错误恢复
5. **安全性**: 不在日志中打印敏感信息，使用TLS
6. **文件名冲突**: 添加时间戳确保唯一性

## 十、后续优化方向

1. 支持更多邮箱服务（Gmail、QQ邮箱等）
2. 支持OAuth2认证
3. 添加邮件内容模板
4. 实现更智能的文件名匹配
5. 添加性能监控和统计
6. 支持批量操作
