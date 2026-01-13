# Specification: Project Infrastructure Setup

## 1. Overview
本 Track 旨在为 "auto-scanner" 项目搭建基础架构。主要目标是初始化 Rust 项目结构，实现命令行参数解析以接收 CSV 文件路径，编写逻辑读取该 CSV 文件中的账号信息，并设计和初始化 SQLite 数据库表结构以存储后续的验证结果。

## 2. Functional Requirements
*   **CLI 初始化**: 使用 `clap` 初始化命令行工具，接收 `--input` (或 `-i`) 参数，用于指定账号 CSV 文件的路径。
*   **CSV 读取**: 使用 `csv` crate 读取指定路径的 CSV 文件。
    *   预期 CSV 格式包含表头，至少包含 `username` (或 `email`) 和 `password` 字段。
    *   能够处理文件不存在或格式错误的异常情况。
*   **数据库初始化**: 使用 `sqlx` 初始化 SQLite 数据库。
    *   在运行时自动创建数据库文件（如果不存在）。
    *   自动执行 SQL 迁移脚本以创建必要的表（例如 `accounts` 表）。
    *   `accounts` 表应包含字段：`id`, `username`, `password`, `status` (默认 "pending"), `last_checked_at` 等。
*   **日志配置**: 初始化 `tracing` 日志系统，支持在终端输出带时间戳的日志。

## 3. Non-Functional Requirements
*   **模块化**: 项目结构应清晰分离，例如分为 `cli`, `storage`, `models` 等模块。
*   **异步**: 所有 I/O 操作（文件读取、数据库操作）应使用 `tokio` 进行异步处理。
*   **错误处理**: 使用 `anyhow` 或 `thiserror` 进行统一的错误处理和向上传递。

## 4. Acceptance Criteria
*   运行 `cargo run -- --help` 能正确显示帮助信息和参数说明。
*   运行 `cargo run -- -i accounts.csv` 能成功读取文件并在日志中打印读取到的账号数量（不打印密码）。
*   程序运行后，本地生成 SQLite 数据库文件，且包含结构正确的 `accounts` 表。
*   代码能够通过 `cargo check` 和 `cargo clippy` 检查。

