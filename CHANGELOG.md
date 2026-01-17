# 更新日志

## [Unreleased] - 2026-01-17

### 新增功能

#### 代理池管理系统
- 🎯 实现 `ProxyPoolManager` 支持从 CSV 文件加载代理配置
- 🔄 三种代理分配策略：
  - **轮询（RoundRobin）**: 依次分配代理，均衡使用
  - **随机（Random）**: 随机选择代理，避免检测规律
  - **粘性（Sticky）**: 为每个 Worker 分配固定代理（推荐）
- ✅ 自动健康检查机制，通过 `ipinfo.io` 验证代理可用性
- 🚫 黑名单管理，自动跳过失效代理
- 🔐 支持带用户名/密码认证的代理
- 🔄 支持动态代理 IP 刷新 URL
- 📦 集成到 `AdsPowerClient`，提供 `with_proxy_pool()` 方法
- 🎯 新增 `create_profile_for_worker()` 方法，实现粘性代理分配

#### 跨平台支持
- 🖥️ 支持 macOS、Linux、Windows 三大平台
- ⚙️ 条件编译 Unix 专用依赖（`nix`、`daemonize`）
- 🔄 跨平台进程管理：
  - Unix: 使用 `kill` 信号
  - Windows: 使用 `taskkill` 命令
- 📡 统一信号处理抽象（`ShutdownSignal` 结构体）：
  - Unix: SIGTERM / SIGINT
  - Windows: Ctrl+C / Ctrl+Break
- 💡 Windows 上提供友好提示，建议使用 Windows 服务替代 daemon 模式

### 技术实现

#### 核心模块
- `src/infrastructure/proxy_pool.rs` (340+ 行)
  - 代理池管理器核心实现
  - CSV 解析和配置加载
  - 分配策略实现
  - 健康检查和黑名单逻辑
  - 单元测试覆盖

- `src/infrastructure/adspower.rs`
  - 新增 `with_proxy_pool()` 方法绑定代理池
  - 修改 `create_profile()` 支持动态代理配置
  - 新增 `create_profile_for_worker()` 实现粘性分配
  - 代理优先级：代理池 > 环境变量回退

- `src/infrastructure/adspower/types.rs`
  - 扩展 `UserProxyConfig` 支持完整代理参数
  - 新增 `with_proxy()` 和 `with_proxyid()` 构造方法

#### 平台适配
- `src/infrastructure/process.rs`
  - 跨平台进程检测（Unix: `kill -0`, Windows: `tasklist`）
  - 跨平台进程终止（Unix: `SIGTERM`, Windows: `taskkill /F`）

- `src/infrastructure/daemon.rs`
  - 条件编译 `daemonize` 功能（仅 Unix）
  - Windows 上显示友好错误提示

- `src/services/master/server.rs`
  - 新增 `ShutdownSignal` 抽象跨平台信号处理
  - 条件编译不同平台的信号接收逻辑

### 文档

#### 新增文档
- �� `docs/PROXY_POOL_GUIDE.md` - 代理池完整使用指南
  - 配置方式和字段说明
  - 代码集成示例
  - 分配策略对比
  - 高级功能（健康检查、黑名单管理）
  - 故障排查指南

- 📖 `docs/CROSS_PLATFORM.md` - 跨平台构建和部署指南
  - 平台特性对比表
  - 各平台构建说明
  - 平台特定注意事项
  - 交叉编译指南
  - GitHub Actions 模板
  - 故障排查和性能对比

#### 配置文件
- 📝 `proxies.csv.example` - 代理配置示例文件
- 🔒 `.gitignore` - 新增 `proxies.csv` 排除规则

#### 更新文档
- 📝 `AGENTS.md` - 更新架构说明，添加代理池和跨平台部分

### 测试

- ✅ macOS 编译通过（Apple Silicon / Intel）
- ✅ 单元测试通过：
  - `proxy_pool::tests::test_proxy_pool_round_robin`
  - `proxy_pool::tests::test_blacklist`
- ✅ Release 构建成功
- ✅ 库和二进制文件均可编译

### 依赖变更

#### Cargo.toml
```diff
+# Unix-specific dependencies
+[target.'cfg(unix)'.dependencies]
+nix = { version = "0.30.1", features = ["signal"] }
+daemonize = "0.5.0"

-nix = { version = "0.30.1", features = ["signal"] }
-daemonize = "0.5.0"
```

### 使用示例

#### 代理池配置
```rust
use auto_scanner::infrastructure::proxy_pool::{ProxyPoolManager, ProxyStrategy};
use auto_scanner::infrastructure::adspower::{AdsPowerClient, AdsPowerConfig};
use std::sync::Arc;

// 1. 加载代理池
let proxy_pool = Arc::new(
    ProxyPoolManager::from_csv("./proxies.csv")?
        .with_strategy(ProxyStrategy::Sticky)
);

// 2. 健康检查
proxy_pool.health_check().await?;

// 3. 集成到 AdsPower
let config = AdsPowerConfig::from_env()?;
let client = AdsPowerClient::new(config)
    .with_proxy_pool(proxy_pool);

// 4. 创建环境（自动使用代理池）
let user_id = client.create_profile_for_worker("worker-0", 0, None).await?;
```

#### Windows 运行
```powershell
# 直接运行
.\auto-scanner.exe master --threads 4

# 或创建 Windows 服务
sc create AutoScanner binPath= "C:\auto-scanner\auto-scanner.exe master --threads 4"
sc start AutoScanner
```

### 破坏性变更

无

### 弃用

无

### 已知问题

- Windows 不支持 daemon 模式，需要使用 Windows 服务或直接运行
- 交叉编译需要额外配置工具链

### 下一步计划

- [ ] 在 Windows 环境实际测试
- [ ] 添加 GitHub Actions 自动构建多平台二进制文件
- [ ] 实现代理池的 API 管理接口（可选）
- [ ] 添加代理性能监控和统计（可选）

---

## 贡献者

- @vale - 代理池管理系统和跨平台支持实现

## 统计

- 13 个文件修改
- +1530 行新增代码
- -126 行删除代码
- 4 个新文件
- 2 份完整文档
