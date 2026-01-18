# Agent Browser 集成指南

## 概述

Agent Browser 是 Vercel Labs 开发的轻量级浏览器自动化 CLI 工具，专为 AI 代理设计。作为 Playwright 的备用方案，它提供了更简单的安装和使用体验。

## 特性

- ✅ **轻量级** - 基于 CLI，无需重量级运行时
- ✅ **快速启动** - Rust 核心，Node.js 回退
- ✅ **AI 友好** - 支持语义选择器和可访问性树快照
- ✅ **会话隔离** - 每个 Worker 使用独立的浏览器会话
- ✅ **JSON 输出** - 易于编程集成

## 安装

### 1. 安装 agent-browser CLI

```bash
# 使用 npm 安装（推荐）
npm install -g agent-browser

# 下载 Chromium
agent-browser install

# Linux 用户还需要安装系统依赖
agent-browser install --with-deps
```

### 2. 验证安装

```bash
agent-browser --version
```

## 配置

### 环境变量

```bash
# .env 文件
# Agent Browser CLI 路径（可选，默认使用 PATH 中的 agent-browser）
AGENT_BROWSER_PATH=/usr/local/bin/agent-browser
```

## 使用方法

### Master 模式

使用 agent-browser 后端启动 Master：

```bash
# 基本用法
./auto-scanner master --backend agent-browser --strategy facebook_login

# Outlook 注册（注册 5 个账号）
./auto-scanner master \
  --backend agent-browser \
  --strategy outlook_register \
  --thread-count 2 \
  --register-count 5

# 守护进程模式
./auto-scanner master \
  --backend agent-browser \
  --strategy facebook_login \
  --daemon
```

### Worker 模式

Worker 会自动使用 Master 指定的后端：

```bash
./auto-scanner worker \
  --username test@example.com \
  --password password123 \
  --backend agent-browser \
  --strategy facebook_login
```

## 工作原理

### 架构

```
┌─────────────────────────────────────────┐
│  Auto Scanner                           │
│  ├─ AgentBrowserAdapter                 │
│  │  ├─ 会话管理                        │
│  │  └─ 命令执行                        │
│  │                                      │
│  └─ agent-browser CLI                  │
│     ├─ Chromium 控制                   │
│     └─ 会话隔离                        │
└─────────────────────────────────────────┘
```

### 会话管理

每个 Worker 都会创建一个独立的 agent-browser 会话：

```rust
// 自动生成唯一会话名
AgentBrowserAdapter::new(Some("test@example.com".to_string()))
// 会话名: test@example.com
```

会话之间完全隔离：
- 独立的 Cookie 和存储
- 独立的导航历史
- 独立的浏览器实例

### 命令执行

所有操作都通过 CLI 命令执行：

```bash
# 导航
agent-browser --session user1 --json open "https://example.com"

# 点击
agent-browser --session user1 --json click "#submit"

# 输入文本
agent-browser --session user1 --json fill "#email" "test@example.com"

# 截图
agent-browser --session user1 --json screenshot result.png
```

## API 映射

| BrowserAdapter 方法 | agent-browser 命令 |
|-------------------|-------------------|
| `navigate(url)` | `open <url>` |
| `click(selector)` | `click <selector>` |
| `type_text(sel, text)` | `type <selector> <text>` |
| `wait_for_element(sel)` | `wait <selector>` |
| `is_visible(sel)` | `is visible <selector>` |
| `get_text(sel)` | `get text <selector>` |
| `take_screenshot(path)` | `screenshot <path>` |
| `get_cookies()` | `cookies` |
| `set_cookies(cookies)` | `cookies set <name> <value>` |

## 优势与限制

### 优势

1. **安装简单** - 只需 npm install，无需复杂的浏览器驱动配置
2. **轻量级** - 比 Playwright 更小的内存占用
3. **独立会话** - 天然支持多会话隔离
4. **JSON 输出** - 易于解析和集成
5. **AI 优化** - 提供可访问性树快照，适合 AI 代理

### 限制

1. **功能较少** - 相比 Playwright，支持的功能较少
2. **性能开销** - 每次操作都需要启动新进程
3. **错误处理** - 依赖 CLI 的退出码和输出
4. **并发限制** - 每个会话都是独立进程

## 故障排查

### 问题 1: agent-browser 未找到

**错误信息：**
```
agent-browser 未安装或不在 PATH 中
```

**解决方法：**
```bash
# 全局安装
npm install -g agent-browser

# 或设置自定义路径
export AGENT_BROWSER_PATH=/path/to/agent-browser
```

### 问题 2: Chromium 未安装

**错误信息：**
```
Error: Chromium not found
```

**解决方法：**
```bash
agent-browser install
```

### 问题 3: Linux 依赖缺失

**错误信息：**
```
Error: Missing system libraries
```

**解决方法：**
```bash
# 安装系统依赖
agent-browser install --with-deps

# 或手动安装
npx playwright install-deps chromium
```

### 问题 4: 会话冲突

**错误信息：**
```
Session already in use
```

**解决方法：**
```bash
# 列出活动会话
agent-browser session list

# 清理会话
agent-browser --session <name> close
```

## 性能对比

| 后端 | 启动时间 | 内存占用 | 并发能力 | 功能完整性 |
|-----|---------|---------|---------|-----------|
| Playwright | ~2s | 150-200MB | 高 | ⭐⭐⭐⭐⭐ |
| Agent Browser | ~1s | 80-120MB | 中 | ⭐⭐⭐⭐ |
| AdsPower | ~5s | 200-300MB | 中 | ⭐⭐⭐⭐⭐ |
| BitBrowser | ~5s | 200-300MB | 中 | ⭐⭐⭐⭐⭐ |

## 最佳实践

### 1. 合理选择后端

- **Playwright** - 功能完整，适合复杂场景
- **Agent Browser** - 轻量快速，适合简单场景
- **AdsPower/BitBrowser** - 指纹浏览器，适合需要隐私保护的场景

### 2. 会话管理

```rust
// 使用账号名作为会话名，便于追踪
AgentBrowserAdapter::new(Some(username.clone())).await?
```

### 3. 错误处理

```rust
// Agent Browser 的错误信息来自 CLI 输出
match adapter.navigate(url).await {
    Ok(_) => info!("导航成功"),
    Err(e) => error!("导航失败: {}", e),
}
```

### 4. 资源清理

```rust
// 显式关闭会话
adapter.close_session().await?;
```

## 示例

### 完整的登录流程

```rust
use crate::infrastructure::browser::agent_browser_adapter::AgentBrowserAdapter;

let adapter = AgentBrowserAdapter::new(Some("user@example.com".to_string())).await?;

// 导航到登录页
adapter.navigate("https://example.com/login").await?;

// 填写表单
adapter.type_text("#email", "user@example.com").await?;
adapter.type_text("#password", "password123").await?;

// 点击登录按钮
adapter.click("#submit").await?;

// 等待登录完成
adapter.wait_for_element("#dashboard").await?;

// 截图
adapter.take_screenshot("logged_in.png").await?;

// 清理
adapter.close_session().await?;
```

## 参考资料

- [Agent Browser 官方仓库](https://github.com/vercel-labs/agent-browser)
- [Agent Browser 文档](https://github.com/vercel-labs/agent-browser#readme)
- [Playwright 对比](docs/PLAYWRIGHT_VS_AGENT_BROWSER.md)

## 更新日志

- **v0.1.1** - 添加 Agent Browser 支持
  - 实现 AgentBrowserAdapter
  - 支持会话隔离
  - 完整的 BrowserAdapter 接口实现
