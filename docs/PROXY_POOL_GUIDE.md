# 代理池管理系统使用指南

## 概述

Auto Scanner 现在支持本地代理池管理，可以为每个 Worker 分配独立的代理，提高并发处理能力和反检测效果。

## 功能特性

- ✅ **多种分配策略**: 轮询（Round Robin）、随机（Random）、粘性（Sticky）
- ✅ **健康检查**: 自动验证代理可用性
- ✅ **黑名单管理**: 自动标记和跳过失效代理
- ✅ **动态配置**: 支持带认证的代理和动态 IP 刷新
- ✅ **回退机制**: 代理池失效时自动使用环境变量配置

## 快速开始

### 1. 创建代理池配置文件

```bash
cp proxies.csv.example proxies.csv
```

编辑 `proxies.csv`：

```csv
host,port,type,username,password,refresh_url
192.168.1.100,8080,http,user1,pass123,
103.21.45.67,3128,socks5,user2,pass456,http://api.proxy.com/refresh?id=1
45.77.123.45,1080,https,,,
```

### 2. 配置环境变量

在 `.env` 文件中添加：

```bash
PROXY_POOL_PATH=./proxies.csv
PROXY_STRATEGY=sticky
ADSPOWER_PROXYID=fallback_proxy_id  # 回退方案
```

### 3. 代码集成

```rust
use auto_scanner::infrastructure::proxy_pool::{ProxyPoolManager, ProxyStrategy};
use auto_scanner::infrastructure::adspower::{AdsPowerClient, AdsPowerConfig};
use std::sync::Arc;

// 加载代理池
let proxy_pool = Arc::new(
    ProxyPoolManager::from_csv("./proxies.csv")?
        .with_strategy(ProxyStrategy::Sticky)
);

// 健康检查
proxy_pool.health_check().await?;

// 创建客户端
let config = AdsPowerConfig::from_env()?;
let client = AdsPowerClient::new(config)
    .with_proxy_pool(proxy_pool);

// 创建环境（自动使用代理池）
let user_id = client.create_profile("worker-0", None).await?;
```

## 代理分配策略

| 策略 | 特点 | 适用场景 |
|------|------|----------|
| **Sticky** | Worker 固定代理 | 账号关联场景（推荐） |
| **Round Robin** | 轮询分配 | 均衡使用代理池 |
| **Random** | 随机选择 | 避免检测规律 |

## 架构说明

```
Master 进程
  ├─ ProxyPoolManager（代理池管理器）
  │   ├─ 加载 CSV 配置
  │   ├─ 健康检查
  │   └─ 黑名单管理
  │
  ├─ AdsPowerClient（浏览器管理）
  │   └─ 集成代理池
  │
  └─ Worker 进程
      └─ 独立代理环境
```

## 高级功能

### 健康检查

```rust
// 验证所有代理可用性
proxy_pool.health_check().await?;

// 定期检查（后台任务）
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        let _ = proxy_pool.health_check().await;
    }
});
```

### 黑名单管理

```rust
// 标记失效代理
proxy_pool.mark_failed("192.168.1.100", "8080").await;

// 查看可用代理数量
let available = proxy_pool.available_count().await;
let total = proxy_pool.total_count();
println!("可用: {}/{}", available, total);

// 清除黑名单
proxy_pool.clear_blacklist().await;
```

### 为 Worker 分配固定代理

```rust
// 推荐：使用专用方法
let user_id = client.create_profile_for_worker(
    "worker-0",
    0,  // Worker 索引
    None
).await?;

// 或手动获取
let proxy = proxy_pool.get_for_worker(0).await;
```

## 配置字段说明

### CSV 字段

| 字段 | 必填 | 说明 | 示例 |
|------|------|------|------|
| `host` | ✅ | 代理 IP | `192.168.1.100` |
| `port` | ✅ | 端口 | `8080` |
| `type` | ✅ | 类型 | `http`/`https`/`socks5`/`ssh` |
| `username` | ❌ | 账号 | `user123` |
| `password` | ❌ | 密码 | `pass456` |
| `refresh_url` | ❌ | 刷新 URL | `http://api.com/refresh?id=1` |

### 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `PROXY_POOL_PATH` | ✅ | 代理池配置文件路径 |
| `PROXY_STRATEGY` | ❌ | 分配策略（默认 `round_robin`） |
| `ADSPOWER_PROXYID` | ❌ | 回退方案代理 ID |

## 故障排查

### 所有代理被标记为失效

```bash
# 1. 验证配置格式
cat proxies.csv

# 2. 测试代理连通性
curl -x http://user:pass@IP:PORT https://ipinfo.io/json

# 3. 代码中清除黑名单
proxy_pool.clear_blacklist().await;
```

### 代理池未生效

```bash
# 检查环境变量
echo $PROXY_POOL_PATH

# 验证文件存在
ls -l ./proxies.csv
```

### AdsPower 返回代理错误

- 确认 `type` 值为 `http`/`https`/`socks5`/`ssh`
- 检查 `host` 和 `port` 格式
- 验证认证信息（如有）

## 性能建议

1. **优先使用 Sticky 策略** - 避免频繁切换代理
2. **代理数量 ≥ Worker 数量** - 确保环境隔离
3. **定期健康检查** - 每 5-10 分钟检查一次
4. **监控黑名单增长** - 快速增长表明代理质量问题

## 安全提示

⚠️ **保护代理凭证**:
- `proxies.csv` 已自动加入 `.gitignore`
- 不要提交到版本控制系统
- 生产环境使用加密存储

## 相关文件

- `src/infrastructure/proxy_pool.rs` - 核心实现
- `src/infrastructure/adspower.rs` - AdsPower 集成
- `proxies.csv.example` - 配置示例
- `AGENTS.md` - 架构文档

## 测试

```bash
# 运行单元测试
cargo test --lib proxy_pool

# 编译检查
cargo check
```
