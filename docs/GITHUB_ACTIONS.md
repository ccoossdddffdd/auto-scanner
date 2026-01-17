# GitHub Actions 自动构建指南

## 概述

项目已配置 GitHub Actions，可自动构建多平台版本：
- ✅ Windows (x86_64)
- ✅ Linux (x86_64)
- ✅ macOS (x86_64 Intel)
- ✅ macOS (ARM64 Apple Silicon)

## 工作流说明

### 1. CI 构建 (`.github/workflows/ci.yml`)

**触发条件**:
- 推送到 `main` 或 `develop` 分支
- 向 `main` 分支提交 Pull Request

**执行任务**:
- ✅ 运行单元测试
- ✅ 检查代码编译（三个平台）
- ✅ Clippy 代码检查
- ✅ 代码格式检查

**查看结果**:
```
https://github.com/YOUR_USERNAME/auto-scanner/actions
```

### 2. Release 构建 (`.github/workflows/release.yml`)

**触发条件**:
- 推送以 `v` 开头的 tag（如 `v0.1.0`）
- 手动触发（GitHub Actions 页面）

**执行任务**:
- ✅ 自动创建 GitHub Release
- ✅ 编译四个平台的二进制文件
- ✅ 自动上传到 Release 页面
- ✅ 压缩打包（Windows: `.zip`, Linux/macOS: `.tar.gz`）

**输出文件**:
- `auto-scanner-windows-x64.exe.zip`
- `auto-scanner-linux-x64.tar.gz`
- `auto-scanner-macos-x64.tar.gz`
- `auto-scanner-macos-arm64.tar.gz`

## 使用方法

### 方式 1: 创建 Release Tag（推荐）

```bash
# 1. 确保所有改动已提交
git add -A
git commit -m "feat: 准备发布 v0.1.0"

# 2. 创建并推送 tag
git tag -a v0.1.0 -m "Release version 0.1.0"
git push origin v0.1.0

# 3. 等待构建完成（约 10-15 分钟）
# 访问 https://github.com/YOUR_USERNAME/auto-scanner/releases

# 4. 下载对应平台的文件
```

### 方式 2: 手动触发构建

1. 访问 GitHub 仓库页面
2. 点击 **Actions** 标签
3. 选择 **Release Build** 工作流
4. 点击 **Run workflow**
5. 选择分支（通常是 `main`）
6. 点击 **Run workflow** 确认
7. 等待构建完成
8. 在 **Artifacts** 区域下载文件

### 方式 3: 使用 GitHub CLI

```bash
# 创建 release（自动触发构建）
gh release create v0.1.0 --title "Release v0.1.0" --notes "发布说明"

# 查看构建状态
gh run list --workflow=release.yml

# 查看构建日志
gh run view <run-id> --log
```

## 构建矩阵详情

| 平台 | 目标 | 产物名称 | 格式 |
|------|------|----------|------|
| Windows x64 | `x86_64-pc-windows-msvc` | `auto-scanner-windows-x64.exe.zip` | ZIP |
| Linux x64 | `x86_64-unknown-linux-gnu` | `auto-scanner-linux-x64.tar.gz` | TAR.GZ |
| macOS Intel | `x86_64-apple-darwin` | `auto-scanner-macos-x64.tar.gz` | TAR.GZ |
| macOS Silicon | `aarch64-apple-darwin` | `auto-scanner-macos-arm64.tar.gz` | TAR.GZ |

## 性能优化

工作流已配置：
- ✅ **依赖缓存**: 加速编译（复用 cargo registry 和 target）
- ✅ **并行构建**: 四个平台同时编译
- ✅ **二进制压缩**: strip 减小文件体积
- ✅ **失败容错**: 单个平台失败不影响其他平台

## 版本号规范

建议遵循 [语义化版本](https://semver.org/lang/zh-CN/)：

```
v<major>.<minor>.<patch>

示例：
v0.1.0  - 初始版本
v0.2.0  - 添加新功能
v0.2.1  - 修复 bug
v1.0.0  - 稳定版本
```

## 下载和使用

### Windows

```powershell
# 1. 下载
# 从 GitHub Releases 页面下载 auto-scanner-windows-x64.exe.zip

# 2. 解压
Expand-Archive auto-scanner-windows-x64.exe.zip -DestinationPath .

# 3. 运行
.\auto-scanner.exe master --threads 4
```

### Linux

```bash
# 1. 下载
wget https://github.com/YOUR_USERNAME/auto-scanner/releases/download/v0.1.0/auto-scanner-linux-x64.tar.gz

# 2. 解压
tar xzf auto-scanner-linux-x64.tar.gz

# 3. 添加执行权限
chmod +x auto-scanner

# 4. 运行
./auto-scanner master --threads 4
```

### macOS

```bash
# Intel Mac
wget https://github.com/YOUR_USERNAME/auto-scanner/releases/download/v0.1.0/auto-scanner-macos-x64.tar.gz
tar xzf auto-scanner-macos-x64.tar.gz

# Apple Silicon (M1/M2/M3)
wget https://github.com/YOUR_USERNAME/auto-scanner/releases/download/v0.1.0/auto-scanner-macos-arm64.tar.gz
tar xzf auto-scanner-macos-arm64.tar.gz

# 移除隔离属性
xattr -d com.apple.quarantine auto-scanner

# 运行
./auto-scanner master --threads 4
```

## 故障排查

### 构建失败

**问题**: Actions 显示红色 ❌

**排查步骤**:
```bash
# 1. 查看构建日志
点击 GitHub Actions -> 选择失败的构建 -> 查看日志

# 2. 本地测试编译
cargo build --release

# 3. 检查依赖
cargo check
```

### 无法下载 Release 文件

**问题**: Releases 页面无文件

**原因**: 构建未完成或失败

**解决方案**:
1. 等待 GitHub Actions 完成（约 10-15 分钟）
2. 检查 Actions 页面是否有错误
3. 手动触发重新构建

### Windows Defender 拦截

**问题**: 下载的 `.exe` 被标记为威胁

**原因**: 未签名的二进制文件

**解决方案**:
```powershell
# 方式1: 允许运行
右键 -> 属性 -> 解除锁定

# 方式2: 添加例外
Windows 安全中心 -> 病毒和威胁防护 -> 添加排除项
```

## 高级配置

### 自定义构建目标

编辑 `.github/workflows/release.yml`：

```yaml
matrix:
  include:
    # 添加新目标
    - os: ubuntu-latest
      target: aarch64-unknown-linux-gnu  # Linux ARM64
      artifact_name: auto-scanner
      asset_name: auto-scanner-linux-arm64
```

### 添加代码签名

```yaml
- name: Sign binary (Windows)
  if: matrix.os == 'windows-latest'
  run: |
    # 使用 signtool 签名
    signtool sign /f cert.pfx /p ${{ secrets.CERT_PASSWORD }} auto-scanner.exe
```

### 自定义 Release 说明

编辑 tag 消息：
```bash
git tag -a v0.1.0 -m "
Release v0.1.0

新增功能:
- 代理池管理
- 跨平台支持

修复问题:
- 修复内存泄漏
"
git push origin v0.1.0
```

## CI/CD 流程图

```
┌─────────────────────────────────────────────────┐
│ 1. 推送代码到 main/develop 分支                  │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ 2. 触发 CI 工作流                                │
│    - 运行测试                                    │
│    - 检查编译（三个平台）                         │
│    - Clippy & 格式检查                           │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ 3. 创建 Release Tag (v0.1.0)                    │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ 4. 触发 Release 工作流                           │
│    - 创建 GitHub Release                         │
│    - 并行编译四个平台                             │
│    - 压缩打包                                    │
│    - 上传到 Release 页面                         │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│ 5. 用户下载对应平台的二进制文件                    │
└─────────────────────────────────────────────────┘
```

## 相关链接

- [GitHub Actions 文档](https://docs.github.com/actions)
- [创建 Release](https://docs.github.com/repositories/releasing-projects-on-github)
- [GitHub CLI](https://cli.github.com/)

## 费用说明

GitHub Actions 对公共仓库**完全免费**，私有仓库有月度免费额度：
- 免费账号: 2000 分钟/月
- Pro 账号: 3000 分钟/月

单次全平台构建约消耗 **30-40 分钟**。
