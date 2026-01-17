# BitBrowser 集成指南

本文档介绍如何在 Auto Scanner 中使用 BitBrowser 指纹浏览器。

## 什么是 BitBrowser？

BitBrowser 是一款浏览器指纹管理工具，用于创建和管理多个浏览器配置文件，每个配置文件都有独特的浏览器指纹，可用于：

- 多账号管理
- 反爬虫保护
- 隐私浏览
- 自动化测试

## 前置要求

1. **安装 BitBrowser 客户端**
   - 从 [BitBrowser 官网](https://www.bitbrowser.cn/) 下载并安装
   - 启动 BitBrowser 应用程序

2. **启用 API 功能**
   - 在 BitBrowser 设置中启用本地 API
   - 默认 API 端口：`54345`

## 配置步骤

### 1. 环境变量配置

在启动 Auto Scanner 之前，设置以下环境变量：

```bash
# BitBrowser API 地址（默认）
export BITBROWSER_API_URL=http://127.0.0.1:54345

# BitBrowser API Key（从 BitBrowser 设置中获取）
export BITBROWSER_API_KEY=your_api_key_here

# 如果系统使用了代理，需要排除本地地址
export NO_PROXY=127.0.0.1,localhost

# 或者如果 BitBrowser 使用不同的端口
export BITBROWSER_API_URL=http://127.0.0.1:YOUR_PORT
```

**获取 API Key：**
1. 打开 BitBrowser 客户端
2. 进入 **设置** → **本地 API**
3. 确保"启用本地 API"已开启
4. 复制显示的 **API Key**（一串长字符串）
5. 设置环境变量 `BITBROWSER_API_KEY`

### 2. 使用 BitBrowser 后端

启动 Master 时指定 `bitbrowser` 作为后端：

```bash
./auto-scanner master --backend bitbrowser --threads 4
```

### 3. 代理配置（可选）

BitBrowser 支持两种代理配置方式：

#### 方式 1：使用代理池（推荐）

创建 `proxies.csv` 文件：

```csv
host,port,type,username,password,refresh_url
proxy1.example.com,1080,socks5,user1,pass1,
proxy2.example.com,8080,http,user2,pass2,
```

Auto Scanner 会自动为每个 Worker 线程分配固定代理（粘性分配）。

#### 方式 2：使用 BitBrowser 内置代理

如果不提供代理池，Auto Scanner 将使用 BitBrowser 的无代理配置。

## 工作原理

### 配置文件管理

1. **自动创建配置文件**
   - Auto Scanner 启动时，会为每个 Worker 线程自动创建一个 BitBrowser 配置文件
   - 配置文件命名：`auto-scanner-worker-0`, `auto-scanner-worker-1`, ...

2. **浏览器指纹配置**
   - 随机 Chrome 版本（100-120）
   - 随机操作系统版本（Windows 10, Windows 11, macOS）
   - 自动生成其他浏览器指纹参数

3. **生命周期管理**
   - 任务执行前：启动浏览器
   - 任务执行中：通过 WebSocket 连接控制浏览器
   - 任务完成后：关闭并删除浏览器配置文件

### API 调用流程

```
┌─────────────────┐
│  Auto Scanner   │
│     Master      │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────┐
│        BitBrowser API               │
│  http://127.0.0.1:54345             │
└────────┬────────────────────────────┘
         │
         ├─► /browser/list (获取配置文件)
         ├─► /browser/update (创建配置文件)
         ├─► /browser/open (启动浏览器)
         ├─► /browser/close (关闭浏览器)
         └─► /browser/delete (删除配置文件)
```

## 完整使用示例

### 示例 1：基本使用

```bash
# 1. 设置环境变量
export BITBROWSER_API_URL=http://127.0.0.1:54345
export INPUT_DIR=./input
export DONED_DIR=./doned

# 2. 启动 BitBrowser 客户端
# （手动打开 BitBrowser 应用程序）

# 3. 启动 Auto Scanner Master
./auto-scanner master --backend bitbrowser --threads 4 --strategy facebook
```

### 示例 2：使用代理池

```bash
# 1. 创建代理池配置
cat > proxies.csv << EOF
host,port,type,username,password,refresh_url
proxy1.example.com,1080,socks5,user1,pass1,
proxy2.example.com,8080,http,user2,pass2,
EOF

# 2. 设置环境变量
export BITBROWSER_API_URL=http://127.0.0.1:54345
export INPUT_DIR=./input
export DONED_DIR=./doned
export PROXY_POOL_CSV=./proxies.csv
export PROXY_POOL_STRATEGY=Sticky  # 粘性分配

# 3. 启动 Master
./auto-scanner master --backend bitbrowser --threads 4
```

### 示例 3：Outlook 注册模式

```bash
# 1. 设置环境变量
export BITBROWSER_API_URL=http://127.0.0.1:54345
export INPUT_DIR=./input
export DONED_DIR=./doned

# 2. 启动注册循环（无限模式）
./auto-scanner master \
  --backend bitbrowser \
  --strategy outlook_register \
  --threads 2 \
  --register-count 0  # 0 表示无限循环
```

## 与 AdsPower 的对比

| 特性 | AdsPower | BitBrowser |
|------|----------|------------|
| API 端口 | 50325 | 54345 |
| API 认证 | 需要 API Key | 无需认证 |
| 代理配置 | 支持代理池 ID + 动态配置 | 支持动态配置 |
| 配置文件管理 | `/api/v1/user/*` | `/browser/*` |
| 响应格式 | `{code, msg, data}` | `{success, msg, data}` |
| WebSocket URL | `data.ws.puppeteer` | `data.ws.puppeteer` |

## 故障排查

### 问题 1：连接失败

**症状：**
```
无法连接到 BitBrowser API (http://127.0.0.1:54345)
```

**解决方法：**
1. 确认 BitBrowser 客户端已启动
2. 检查 BitBrowser 设置中的 API 端口是否为 `54345`
3. 检查防火墙是否阻止了本地连接

### 问题 2：配置文件创建失败

**症状：**
```
BitBrowser API 错误 (/browser/update): ...
```

**解决方法：**
1. 检查 BitBrowser 是否有足够的权限
2. 确认 BitBrowser 版本是否支持 API 功能
3. 查看 BitBrowser 日志了解详细错误

### 问题 3：浏览器启动失败

**症状：**
```
启动浏览器失败: ...
```

**解决方法：**
1. 确认系统有足够的资源（内存、CPU）
2. 减少 `--threads` 参数值
3. 检查 BitBrowser 配置文件是否损坏

## 日志示例

成功运行时的日志输出：

```
[INFO] 正在确保后端就绪: bitbrowser
[INFO] 正在检查 BitBrowser API 连接性...
[INFO] BitBrowser API 已就绪
[INFO] Master 已启动。监控目录: ./input
[INFO] 正在为 4 个 Worker 检查 BitBrowser 配置文件...
[INFO] 正在创建缺失的配置文件: auto-scanner-worker-0
[INFO] 正在创建配置文件 auto-scanner-worker-0，Chrome 版本: 115.0.5790.102
[INFO] 为 Worker 0 分配固定代理: proxy1.example.com:1080
[INFO] 已创建配置文件 auto-scanner-worker-0，ID: 123456
[INFO] BitBrowser 浏览器已启动: ws://127.0.0.1:9223/devtools/browser/xxx
```

## 最佳实践

1. **资源管理**
   - 根据系统资源调整 `--threads` 参数
   - 每个浏览器实例约占用 200-500MB 内存

2. **代理使用**
   - 使用 Sticky 策略确保每个 Worker 使用固定代理
   - 定期检查代理健康状态

3. **错误处理**
   - 启用详细日志：`RUST_LOG=debug ./auto-scanner master ...`
   - 监控 BitBrowser 客户端日志

4. **安全建议**
   - 不要在公网暴露 BitBrowser API
   - 定期更新 BitBrowser 客户端版本
   - 使用防火墙限制 API 访问

## 支持与反馈

如果遇到问题或有改进建议，请：

1. 查看 [GitHub Issues](https://github.com/ccoossdddffdd/auto-scanner/issues)
2. 提交新的 Issue 并附上详细日志
3. 参考 [开发指南](../AGENTS.md) 了解架构细节
