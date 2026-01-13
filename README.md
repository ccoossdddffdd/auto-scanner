# Auto Scanner

自动化 Facebook 账号验证工具。

## 功能特性

- 从 CSV 文件批量导入账号信息
- 使用 SQLite 数据库存储账号数据
- 支持账号状态跟踪
- 异步处理，高性能
- 完整的日志记录

## 技术栈

- **Rust** - 系统编程语言
- **Tokio** - 异步运行时
- **SQLx** - 数据库访问
- **Clap** - 命令行参数解析
- **Tracing** - 日志跟踪

## 环境要求

- Rust 1.70 或更高版本
- Cargo 包管理器

## 构建说明

### 1. 安装 Rust

如果还未安装 Rust，请访问 [https://rustup.rs/](https://rustup.rs/) 或执行：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 克隆项目

```bash
git clone <repository-url>
cd auto-scanner
```

### 3. 构建项目

#### 开发构建

```bash
cargo build
```

#### 发布构建（优化版本）

```bash
cargo build --release
```

### 4. 运行测试

```bash
cargo test
```

## 使用说明

### 准备 CSV 文件

创建包含账号信息的 CSV 文件，格式如下：

```csv
username,password
user1@example.com,password123
user2@example.com,password456
```

**注意：** 
- CSV 文件必须包含 `username` 和 `password` 列头
- 每行代表一个账号

### 运行程序

#### 使用开发构建

```bash
cargo run -- -i <CSV文件路径>
```

或

```bash
cargo run -- --input <CSV文件路径>
```

#### 使用发布构建

```bash
./target/release/auto-scanner -i <CSV文件路径>
```

### 示例

```bash
# 使用相对路径
cargo run -- -i test_accounts.csv

# 使用绝对路径
cargo run -- -i /path/to/accounts.csv

# 使用发布版本
./target/release/auto-scanner --input accounts.csv
```

### 查看帮助信息

```bash
cargo run -- --help
```

或

```bash
./target/release/auto-scanner --help
```

## 数据库

程序会自动在当前目录创建 `auto-scanner.db` SQLite 数据库文件，用于存储：

- 账号凭证（用户名、密码）
- 账号状态（pending、verified、failed 等）
- 检查时间戳
- 创建和更新时间

### 数据库结构

```sql
accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    last_checked_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
)
```

## 项目结构

```
auto-scanner/
├── Cargo.toml              # 项目配置和依赖
├── Cargo.lock              # 依赖锁定文件
├── README.md               # 项目说明文档
├── migrations/             # 数据库迁移文件
│   └── 001_create_accounts_table.sql
├── src/                    # 源代码目录
│   ├── main.rs            # 程序入口
│   ├── lib.rs             # 库入口
│   ├── cli.rs             # 命令行参数定义
│   ├── csv_reader.rs      # CSV 文件读取
│   ├── database.rs        # 数据库操作
│   └── models.rs          # 数据模型
└── target/                 # 编译输出目录
```

## 开发指南

### 添加新依赖

```bash
cargo add <package-name>
```

### 格式化代码

```bash
cargo fmt
```

### 代码检查

```bash
cargo clippy
```

### 清理构建文件

```bash
cargo clean
```

## 日志级别

程序使用 `tracing` 进行日志记录，可以通过环境变量控制日志级别：

```bash
# 设置日志级别为 debug
RUST_LOG=debug cargo run -- -i accounts.csv

# 设置日志级别为 info（默认）
RUST_LOG=info cargo run -- -i accounts.csv

# 设置日志级别为 warn
RUST_LOG=warn cargo run -- -i accounts.csv
```

## 许可证

请查看项目的 LICENSE 文件了解许可信息。

## 贡献

欢迎提交 Issue 和 Pull Request。

## 注意事项

- 请妥善保管账号凭证信息，不要泄露 CSV 文件和数据库文件
- 建议在生产环境使用发布构建版本以获得最佳性能
- 定期备份数据库文件
