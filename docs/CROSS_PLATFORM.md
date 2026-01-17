# Auto Scanner 跨平台支持指南

## 概述

Auto Scanner 现已支持多平台运行，包括：
- ✅ **macOS** (x86_64 / Apple Silicon)
- ✅ **Linux** (x86_64 / ARM64)
- ✅ **Windows** (x86_64)

## 平台特性对比

| 功能 | macOS | Linux | Windows | 说明 |
|------|-------|-------|---------|------|
| 基础功能 | ✅ | ✅ | ✅ | 文件监控、浏览器自动化、邮件监控 |
| Daemon 模式 | ✅ | ✅ | ❌ | Windows 不支持，使用服务或直接运行 |
| 信号处理 | SIGTERM/SIGINT | SIGTERM/SIGINT | Ctrl+C/Ctrl+Break | 跨平台适配 |
| 进程管理 | PID + kill | PID + kill | PID + taskkill | 自动检测平台 |
| AdsPower | ✅ | ✅ | ✅ | 浏览器指纹管理 |
| Playwright | ✅ | ✅ | ✅ | 本地浏览器自动化 |

## 构建说明

### macOS / Linux

```bash
# 1. 克隆项目
git clone <your-repo>
cd auto-scanner

# 2. 安装依赖
# macOS:
brew install rust

# Linux:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. 构建
cargo build --release

# 4. 运行
./target/release/auto-scanner master --threads 4
```

### Windows

```powershell
# 1. 安装 Rust
# 访问 https://rustup.rs/ 下载安装器

# 2. 克隆项目
git clone <your-repo>
cd auto-scanner

# 3. 构建
cargo build --release

# 4. 运行
.\target\release\auto-scanner.exe master --threads 4
```

## 平台特定注意事项

### Windows

#### ❌ **不支持 Daemon 模式**

```bash
# ❌ 错误用法
auto-scanner master --threads 4 --daemon

# 错误提示：
# Daemon mode is not supported on Windows.
# Please run the program directly or use Windows Service instead.
```

**替代方案：**

1. **直接运行（推荐）**
   ```powershell
   .\auto-scanner.exe master --threads 4
   ```

2. **使用 Windows 服务**
   ```powershell
   # 创建服务
   sc create AutoScanner binPath= "C:\path\to\auto-scanner.exe master --threads 4"
   
   # 启动服务
   sc start AutoScanner
   
   # 停止服务
   sc stop AutoScanner
   ```

3. **使用任务计划程序**
   - 打开"任务计划程序"
   - 创建基本任务
   - 设置触发器为"系统启动时"
   - 操作：启动程序 `auto-scanner.exe`

#### 🛑 **停止程序**

```powershell
# 方法1: Ctrl+C（前台运行）
# 直接按 Ctrl+C

# 方法2: taskkill（后台运行）
taskkill /F /IM auto-scanner.exe

# 方法3: 使用 stop 命令
.\auto-scanner.exe stop
```

#### 📁 **路径分隔符**

```rust
// ✅ 使用标准库自动处理
use std::path::PathBuf;
let path = PathBuf::from("input").join("data.csv");

// ❌ 避免硬编码路径分隔符
let path = "input/data.csv";  // Unix 风格
let path = "input\\data.csv"; // Windows 风格
```

### Unix (macOS / Linux)

#### ✅ **Daemon 模式**

```bash
# 后台守护进程模式
auto-scanner master --threads 4 --daemon

# 检查状态
auto-scanner status

# 停止
auto-scanner stop
```

#### 🛑 **信号处理**

```bash
# 优雅停止（推荐）
kill -TERM $(cat master.pid)

# 强制停止
kill -KILL $(cat master.pid)

# 使用内置命令
auto-scanner stop
```

## 交叉编译

### 在 macOS 上为 Linux 编译

```bash
# 1. 安装目标
rustup target add x86_64-unknown-linux-gnu

# 2. 安装交叉编译工具链
brew install FiloSottile/musl-cross/musl-cross

# 3. 配置
cat > .cargo/config.toml << EOF
[target.x86_64-unknown-linux-gnu]
linker = "x86_64-linux-musl-gcc"
EOF

# 4. 编译
cargo build --release --target x86_64-unknown-linux-gnu
```

### 在 Linux 上为 Windows 编译

```bash
# 1. 安装 MinGW
sudo apt install mingw-w64

# 2. 添加目标
rustup target add x86_64-pc-windows-gnu

# 3. 编译
cargo build --release --target x86_64-pc-windows-gnu
```

### 在 macOS/Linux 上为 Windows 编译（推荐使用 CI）

**GitHub Actions 示例：**

```yaml
# .github/workflows/build.yml
name: Cross-Platform Build

on: [push, pull_request]

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: auto-scanner-linux
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: auto-scanner-windows.exe
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: auto-scanner-macos

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: ${{ matrix.target }}
          override: true

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Upload Artifact
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/auto-scanner*
```

## 依赖差异

### 所有平台通用依赖

```toml
tokio = { version = "1.42", features = ["full"] }
playwright = "0.0.20"
reqwest = { version = "0.11.27", features = ["json"] }
csv = "1.3"
# ... 其他通用依赖
```

### Unix 专用依赖

```toml
[target.'cfg(unix)'.dependencies]
nix = { version = "0.30.1", features = ["signal"] }
daemonize = "0.5.0"
```

这些依赖在 Windows 上**不会**被编译和链接。

## 功能测试

### 测试信号处理

**Unix (macOS / Linux):**
```bash
# 启动
./auto-scanner master --threads 4 &
PID=$!

# 优雅停止
kill -TERM $PID

# 或使用内置命令
./auto-scanner stop
```

**Windows:**
```powershell
# 启动（新窗口）
Start-Process .\auto-scanner.exe -ArgumentList "master","--threads","4"

# 停止
.\auto-scanner.exe stop
# 或按 Ctrl+C（如果在前台运行）
```

### 测试进程管理

```bash
# 所有平台通用
auto-scanner status       # 检查状态
auto-scanner stop         # 停止服务
```

## 故障排查

### Windows: "系统找不到指定的文件"

**原因**: DLL 依赖缺失

**解决方案**:
```powershell
# 1. 安装 Visual C++ Redistributable
# https://aka.ms/vs/17/release/vc_redist.x64.exe

# 2. 或使用静态链接
$env:RUSTFLAGS="-C target-feature=+crt-static"
cargo build --release
```

### Linux: "error while loading shared libraries"

**原因**: 动态库缺失

**解决方案**:
```bash
# 检查依赖
ldd ./target/release/auto-scanner

# 安装缺失的库
sudo apt install libssl-dev pkg-config  # Ubuntu/Debian
sudo yum install openssl-devel          # CentOS/RHEL
```

### macOS: "无法打开，因为无法验证开发者"

**解决方案**:
```bash
# 方法1: 允许运行
xattr -d com.apple.quarantine ./auto-scanner

# 方法2: 系统偏好设置
# 系统偏好设置 -> 安全性与隐私 -> 通用 -> 仍要打开
```

## 性能差异

| 平台 | 启动时间 | 内存占用 | 文件监控 | 说明 |
|------|---------|---------|----------|------|
| Linux | ~200ms | ~50MB | inotify（最优） | 推荐生产环境 |
| macOS | ~300ms | ~60MB | FSEvents（优秀） | 开发友好 |
| Windows | ~500ms | ~80MB | 轮询（一般） | 资源占用稍高 |

## 推荐部署方案

### 开发环境
- **macOS**: 直接运行，方便调试
- **Windows**: 直接运行或 PowerShell 脚本
- **Linux**: Daemon 模式或 systemd 服务

### 生产环境
- **Linux**: systemd 服务（推荐）
  ```bash
  # /etc/systemd/system/auto-scanner.service
  [Unit]
  Description=Auto Scanner Service
  After=network.target

  [Service]
  Type=simple
  User=scanner
  WorkingDirectory=/opt/auto-scanner
  ExecStart=/opt/auto-scanner/auto-scanner master --threads 8
  Restart=always

  [Install]
  WantedBy=multi-user.target
  ```

- **Windows**: Windows 服务（推荐使用 nssm）
  ```powershell
  # 使用 nssm 创建服务
  nssm install AutoScanner "C:\auto-scanner\auto-scanner.exe" "master --threads 8"
  nssm start AutoScanner
  ```

## 相关文件

- `Cargo.toml` - 平台依赖配置
- `src/infrastructure/process.rs` - 跨平台进程管理
- `src/infrastructure/daemon.rs` - Daemon 实现（Unix-only）
- `src/services/master/server.rs` - 信号处理适配

## 技术实现细节

### 条件编译

```rust
// Unix 专用代码
#[cfg(unix)]
fn unix_specific() {
    use nix::sys::signal;
    // ...
}

// Windows 专用代码
#[cfg(windows)]
fn windows_specific() {
    use std::process::Command;
    Command::new("taskkill")
        .args(&["/PID", "1234", "/F"])
        .output();
}
```

### 信号抽象

```rust
struct ShutdownSignal {
    #[cfg(unix)]
    sigterm: tokio::signal::unix::Signal,
    #[cfg(windows)]
    ctrl_c: tokio::signal::windows::CtrlC,
}

// 统一接口
impl ShutdownSignal {
    async fn recv(&mut self) { /* 平台特定实现 */ }
}
```

## 贡献指南

在提交代码时，请确保：
1. ✅ 在所有平台上测试编译
2. ✅ 使用条件编译处理平台差异
3. ✅ 避免硬编码平台特定路径
4. ✅ 更新相关平台文档

## 许可证

本项目采用 MIT 许可证。
