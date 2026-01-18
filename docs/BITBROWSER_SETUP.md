# BitBrowser 集成完整配置指南

## ✅ 已完成的工作

### 1. 代码修复
- ✅ 所有 API 调用改为 POST 方法（BitBrowser 要求）
- ✅ 添加 `X-API-KEY` 请求头支持
- ✅ 正确的请求体格式（JSON body）
- ✅ 禁用 HTTP 代理（使用 `no_proxy()` 构建客户端）

### 2. 环境变量支持
- `BITBROWSER_API_URL`: API 地址（默认 http://127.0.0.1:54345）
- `BITBROWSER_API_KEY`: API 鉴权密钥（必需）
- `NO_PROXY`: 排除本地地址的代理设置

### 3. API 接口实现
- `/browser/list` - 列出浏览器窗口
- `/browser/update` - 创建/更新窗口
- `/browser/open` - 打开浏览器
- `/browser/close` - 关闭浏览器
- `/browser/delete` - 删除窗口

## 🚀 配置步骤

### 步骤 1: 在 BitBrowser 中启用本地 API

1. 打开 BitBrowser 客户端（比特浏览器）
2. 点击右上角 **设置** ⚙️
3. 找到 **"本地 API"** 或 **"Local API"** 选项
4. 确保以下设置：
   - ✅ **启用本地 API** - 开关打开
   - ✅ **启用 API 鉴权** - 开关打开
   - 📝 **API 端口**: 54345（或自定义）
   - 🔑 **API Key**: 复制显示的密钥

5. 保存设置并重启 BitBrowser

### 步骤 2: 配置环境变量

创建或编辑 `.env.bitbrowser` 文件：

```bash
# BitBrowser 配置
export BITBROWSER_API_URL=http://127.0.0.1:54345
export BITBROWSER_API_KEY=paste_your_api_key_here

# 目录配置
export INPUT_DIR=input
export DONED_DIR=input/doned

# 禁用系统代理（重要！）
export NO_PROXY=127.0.0.1,localhost

# 日志级别
export RUST_LOG=info
```

### 步骤 3: 测试连接

```bash
# 加载环境变量
source .env.bitbrowser

# 测试 API 连接
curl -X POST http://127.0.0.1:54345/browser/list \
  -H "Content-Type: application/json" \
  -H "X-API-KEY: $BITBROWSER_API_KEY" \
  -d '{"page":0,"pageSize":1}'
```

**预期响应：**
```json
{
  "success": true,
  "data": {
    "list": [...],
    "total": 10
  }
}
```

**常见错误：**
- `503 Service Unavailable` → BitBrowser API 未启用或被代理拦截
- `API Token错误` → API Key 错误或未设置
- `Connection refused` → BitBrowser 未运行或端口错误

### 步骤 4: 运行 Outlook 注册流程

```bash
# 加载环境变量
source .env.bitbrowser

# 启动 Master（注册 1 个账号）
./target/release/auto-scanner master \
  --backend bitbrowser \
  --thread-count 1 \
  --strategy outlook_register \
  --register-count 1
```

## 📊 工作流程

```
┌─────────────────────────────────────────────────────────┐
│  Auto Scanner                                           │
│  ├─ 读取环境变量                                        │
│  │  ├─ BITBROWSER_API_URL                              │
│  │  └─ BITBROWSER_API_KEY                              │
│  │                                                       │
│  ├─ 连接 BitBrowser API                                 │
│  │  └─ POST /browser/list (验证连接)                    │
│  │                                                       │
│  ├─ 为每个 Worker 创建浏览器配置文件                    │
│  │  └─ POST /browser/update                             │
│  │      ├─ 设置随机指纹                                 │
│  │      └─ 配置代理（如果有代理池）                     │
│  │                                                       │
│  ├─ 启动浏览器窗口                                      │
│  │  └─ POST /browser/open                               │
│  │      └─ 返回 WebSocket URL                           │
│  │                                                       │
│  ├─ 执行自动化任务（Outlook 注册）                      │
│  │  └─ 通过 Puppeteer/CDP 控制浏览器                   │
│  │                                                       │
│  └─ 清理资源                                            │
│     ├─ POST /browser/close                              │
│     └─ POST /browser/delete                             │
└─────────────────────────────────────────────────────────┘
```

## 🔍 故障排查

### 问题 1: "API Token错误"

**原因：**
- API Key 未设置或错误
- BitBrowser 未启用 API 鉴权

**解决方法：**
1. 确认环境变量 `BITBROWSER_API_KEY` 已正确设置
2. 在 BitBrowser 设置中复制正确的 API Key
3. 确保"启用 API 鉴权"选项已打开

### 问题 2: "503 Service Unavailable"

**原因：**
- BitBrowser API 未启用
- 系统代理拦截了本地请求

**解决方法：**
```bash
# 禁用代理
export NO_PROXY=127.0.0.1,localhost
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY

# 或者修改系统代理设置，排除 127.0.0.1
```

### 问题 3: "Connection refused"

**原因：**
- BitBrowser 未运行
- API 端口错误

**解决方法：**
1. 确认 BitBrowser 正在运行
2. 检查设置中的 API 端口号
3. 更新 `BITBROWSER_API_URL` 环境变量

### 问题 4: 浏览器启动失败

**原因：**
- 系统资源不足
- 并发线程数过多

**解决方法：**
```bash
# 减少并发线程数
./target/release/auto-scanner master \
  --backend bitbrowser \
  --thread-count 1  # 从 1 开始测试
  --strategy outlook_register \
  --register-count 1
```

## 📝 完整示例

```bash
#!/bin/bash

# 1. 配置环境变量
export BITBROWSER_API_URL=http://127.0.0.1:54345
export BITBROWSER_API_KEY=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzUxMiJ9...
export INPUT_DIR=input
export DONED_DIR=input/doned
export NO_PROXY=127.0.0.1,localhost
export RUST_LOG=info

# 2. 测试连接
echo "测试 BitBrowser API 连接..."
curl -X POST $BITBROWSER_API_URL/browser/list \
  -H "Content-Type: application/json" \
  -H "X-API-KEY: $BITBROWSER_API_KEY" \
  -d '{"page":0,"pageSize":1}' | python3 -m json.tool

# 3. 运行 Outlook 注册
echo -e "\n开始 Outlook 注册流程..."
./target/release/auto-scanner master \
  --backend bitbrowser \
  --thread-count 1 \
  --strategy outlook_register \
  --register-count 1
```

## 📚 相关文档

- [BitBrowser 官方文档](https://doc2.bitbrowser.cn/)
- [BitBrowser API 文档](https://doc2.bitbrowser.cn/jiekou.html)
- [GitHub API 示例](https://github.com/BitBrowser01/BitBrowser-API-docs)
- [Auto Scanner 开发指南](AGENTS.md)

## ⚠️ 注意事项

1. **代理设置**：系统代理会影响本地 API 调用，必须设置 `NO_PROXY`
2. **API Key 安全**：不要将 API Key 提交到代码仓库
3. **资源限制**：每个浏览器实例约占用 200-500MB 内存
4. **速率限制**：建议在 Worker 之间添加延迟（代码中已实现）

