# Auto Scanner

[![Release Build](https://github.com/ccoossdddffdd/auto-scanner/actions/workflows/release.yml/badge.svg)](https://github.com/ccoossdddffdd/auto-scanner/actions/workflows/release.yml)
[![CI Build](https://github.com/ccoossdddffdd/auto-scanner/actions/workflows/ci.yml/badge.svg)](https://github.com/ccoossdddffdd/auto-scanner/actions/workflows/ci.yml)

高性能、异步的浏览器自动化工具，采用 Master-Worker 架构，支持多平台（Windows/Linux/macOS）和指纹浏览器集成。

## 特性

- 🚀 **高性能异步架构**：基于 Tokio 的异步运行时，高效并发处理
- 🎯 **代理池管理**：支持轮询、随机、粘性分配三种策略
- 🖥️ **跨平台支持**：完整支持 Windows、Linux、macOS（Intel & Apple Silicon）
- 🌐 **指纹浏览器集成**：深度集成 AdsPower、BitBrowser 浏览器指纹管理
- 🤖 **多浏览器后端**：支持 Playwright、Agent Browser、指纹浏览器
- 📧 **邮件自动化**：支持 IMAP 邮件监控和自动化处理
- 🔄 **Master-Worker 架构**：灵活的分布式任务处理
- 📦 **多种输入格式**：支持 CSV、Excel 文件输入

## 快速开始

### 下载

从 [Releases 页面](https://github.com/ccoossdddffdd/auto-scanner/releases) 下载对应平台的二进制文件：

- **Windows**: `auto-scanner-windows-x64.exe.zip`
- **Linux**: `auto-scanner-linux-x64.tar.gz`
- **macOS (Intel)**: `auto-scanner-macos-x64.tar.gz`
- **macOS (Apple Silicon)**: `auto-scanner-macos-arm64.tar.gz`

### macOS 用户注意事项 🍎

下载后首次运行可能遇到安全警告：

> "Apple无法验证 auto-scanner 是否包含可能危害Mac安全或泄漏隐私的恶意软件"

**解决方法：**

```bash
# 方法 1：移除隔离标志（推荐）
xattr -d com.apple.quarantine auto-scanner

# 方法 2：或在 Finder 中右键点击 → 选择"打开" → 确认
```

这是正常现象，因为开源项目的二进制文件未经过 Apple 公证（需要 $99/年的开发者账号）。本软件是安全的：
- ✅ 开源代码可审计
- ✅ GitHub Actions 公开构建
- ✅ 可自行从源码编译

### 基本使用

```bash
# Master 模式：监控文件并分发任务
./auto-scanner master --threads 4

# Worker 模式：执行具体任务
./auto-scanner worker --strategy facebook

# 查看帮助
./auto-scanner --help
```

## 架构概览

```
┌─────────────┐
│   Master    │  文件监控、任务分发、生命周期管理
└──────┬──────┘
       │
       ├─────┬─────┬─────┐
       ▼     ▼     ▼     ▼
   Worker Worker Worker Worker  并发执行任务
```

### 核心组件

- **Master**: 中枢神经系统，负责文件监控、并发控制和任务分发
- **Worker**: 独立进程，执行浏览器自动化任务
- **Strategy**: 可插拔的自动化策略（Facebook、Outlook等）
- **Proxy Pool**: 代理池管理，支持健康检查和自动切换
- **AdsPower/BitBrowser**: 浏览器指纹环境管理

## 代理池配置

创建 `proxies.csv` 文件：

```csv
host,port,type,username,password,refresh_url
proxy1.example.com,1080,socks5,user1,pass1,http://api.example.com/refresh/1
proxy2.example.com,8080,http,user2,pass2,
```

支持三种分配策略：
- **RoundRobin**: 轮询分配（默认）
- **Random**: 随机选择
- **Sticky**: 固定分配（每个 Worker 使用同一代理）

详细配置请参考：[代理池管理指南](docs/PROXY_POOL_GUIDE.md)

## 环境变量

```bash
# 必需配置
export INPUT_DIR=./input              # 输入文件目录
export DONED_DIR=./doned              # 完成文件目录

# AdsPower 配置（如果使用 AdsPower）
export ADSPOWER_API_URL=http://127.0.0.1:50325
export ADSPOWER_API_KEY=your_api_key
export ADSPOWER_PROXYID=your_proxy_id

# BitBrowser 配置（如果使用 BitBrowser）
export BITBROWSER_API_URL=http://127.0.0.1:54345
export BITBROWSER_API_KEY=your_api_key_here

# Agent Browser 配置（如果使用 Agent Browser）
export AGENT_BROWSER_PATH=/usr/local/bin/agent-browser  # 可选，默认使用 PATH 中的

# 邮件配置（可选）
export IMAP_SERVER=imap.gmail.com
export IMAP_PORT=993
export IMAP_USERNAME=your_email@gmail.com
export IMAP_PASSWORD=your_password
```

## 从源码构建

### 前置要求

- Rust 1.70+ (`rustup` 推荐)
- OpenSSL 开发包
  - Ubuntu/Debian: `sudo apt-get install libssl-dev pkg-config`
  - macOS: `brew install openssl@3`
  - Windows: 自动处理（MinGW）

### 编译

```bash
# 克隆仓库
git clone https://github.com/ccoossdddffdd/auto-scanner.git
cd auto-scanner

# 构建 release 版本
cargo build --release

# 运行测试
cargo test

# 代码质量检查
cargo clippy
cargo fmt --check
```

## 支持的策略

### Facebook 登录策略

- 自动登录验证
- 2FA/验证码处理
- Cookie 提取

### Outlook 注册策略

- 自动账号注册
- 表单填写
- 验证码识别

更多策略开发中...

## 跨平台支持

| 平台 | 架构 | 状态 |
|------|------|------|
| Windows | x64 | ✅ 支持 |
| Linux | x64 | ✅ 支持 |
| macOS | x64 (Intel) | ✅ 支持 |
| macOS | ARM64 (M1/M2) | ✅ 支持 |

详细信息：[跨平台支持文档](docs/CROSS_PLATFORM.md)

## 文档

- [开发指南](AGENTS.md) - 架构设计和开发规范
- [代理池管理](docs/PROXY_POOL_GUIDE.md) - 代理池配置和使用
- [BitBrowser 集成](docs/BITBROWSER_GUIDE.md) - BitBrowser 指纹浏览器集成指南
- [Agent Browser 集成](docs/AGENT_BROWSER_GUIDE.md) - Agent Browser 轻量级自动化指南
- [跨平台支持](docs/CROSS_PLATFORM.md) - 平台特定说明
- [GitHub Actions](docs/GITHUB_ACTIONS.md) - CI/CD 流程
- [更新日志](CHANGELOG.md) - 版本更新历史

## 贡献

欢迎贡献！请遵循以下步骤：

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

### 代码规范

- 运行 `cargo fmt` 格式化代码
- 运行 `cargo clippy` 检查代码质量
- 运行 `cargo test` 确保测试通过
- 遵循 Rust 最佳实践

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 致谢

- [Tokio](https://tokio.rs/) - 异步运行时
- [AdsPower](https://www.adspower.com/) - 指纹浏览器支持
- [Playwright](https://playwright.dev/) - 浏览器自动化

## 支持

- 📖 [文档](https://github.com/ccoossdddffdd/auto-scanner/tree/main/docs)
- 🐛 [提交问题](https://github.com/ccoossdddffdd/auto-scanner/issues)
- 💬 [讨论区](https://github.com/ccoossdddffdd/auto-scanner/discussions)

---

⭐ 如果这个项目对你有帮助，请给它一个 Star！
