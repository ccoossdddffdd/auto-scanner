# Auto Scanner - Agent Guidelines

## Build/Lint/Test Commands

### Core Commands
```bash
# Build development version
cargo build

# Build optimized release version
cargo build --release

# Check for compilation errors
cargo check

# Format code
cargo fmt

# Run clippy linter (with warnings as errors)
cargo clippy -- -D warnings

# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### Testing Specific Components

```bash
# Run specific test
cargo test test_name

# Run tests in specific file
cargo test --test filename

# Run tests with backtrace
cargo test -- --nocapture --backtrace

# Run integration tests only
cargo test --test '*'

# Run unit tests only (exclude integration tests)
cargo test --lib

# Run tests with coverage (requires grcov)
cargo install grcov
RUSTFLAGS="-Cinstrument-coverage" LLVM_PROFILE_FILE="coverage-%p-%m.profraw" cargo test
grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing -o ./target/coverage/html
```

### Development Workflow

```bash
# Full development cycle
cargo fmt && cargo clippy -- -D warnings && cargo test && cargo build

# Quick check before commit
cargo fmt --check && cargo clippy -- -D warnings && cargo test

# Clean build artifacts
cargo clean
```

## Code Style Guidelines

### Project Structure
- `src/lib.rs` - Library entry point with module declarations
- `src/main.rs` - Binary entry point
- `src/cli.rs` - Command line interface definitions
- `src/models.rs` - Data structures and types
- `src/master.rs` - Master process logic
- `src/worker.rs` - Worker process logic
- `src/browser/` - Browser automation components
- `src/csv_reader.rs` - CSV file handling
- `src/excel_handler.rs` - Excel file handling
- `src/adspower.rs` - AdsPower integration

### Import Organization

#### Standard Library Imports
```rust
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
```

#### External Crate Imports
Group by crate, alphabetized within groups:
```rust
use anyhow::{Context, Result};
use async_channel;
use chrono::Local;
use clap::Parser;
use nix::sys::signal::{self, Signal};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
```

#### Internal Module Imports
```rust
use crate::adspower::AdsPowerClient;
use crate::csv_reader::read_accounts_from_csv;
use crate::models::WorkerResult;
```

### Naming Conventions

#### Functions and Variables
- `snake_case` for functions, variables, and methods
- `UPPER_SNAKE_CASE` for constants
- Descriptive names preferred over abbreviations

```rust
// Good
pub async fn process_file(path: &Path, config: ProcessConfig) -> Result<()> {
    let batch_name = generate_batch_name(path)?;
    let accounts = read_accounts_from_csv(path).await?;
    // ...
}

// Avoid
pub async fn proc_file(p: &Path, cfg: ProcessConfig) -> Result<()> {
    let bn = gen_batch_name(p)?;
    let accs = read_csv(p).await?;
    // ...
}
```

#### Types and Traits
- `PascalCase` for structs, enums, traits
- `UPPER_SNAKE_CASE` for enum variants (when not PascalCase)

```rust
#[derive(Clone)]
pub struct MasterConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
}

pub enum BrowserBackend {
    Playwright,
    AdsPower,
}
```

#### Modules and Files
- `snake_case` for module names and filenames
- `lib.rs` and `main.rs` are exceptions

### Type Definitions and Complexity

#### Avoid Complex Type Aliases
Use type aliases to simplify complex return types:

```rust
// Good - Use type alias for complex return types
type ExcelResult = (Vec<Account>, Vec<Vec<String>>, Vec<String>);

pub fn read_accounts_from_excel(path: P) -> Result<ExcelResult> {
    // ...
}
```

#### Configuration Structs
Use structs for function parameters with 3+ arguments:

```rust
// Good - Use config struct for many parameters
#[derive(Clone)]
pub struct MasterConfig {
    pub backend: String,
    pub remote_url: String,
    pub thread_count: usize,
    pub enable_screenshot: bool,
    pub stop: bool,
    pub daemon: bool,
    pub status: bool,
}

pub async fn run(input_dir: Option<String>, config: MasterConfig) -> Result<()> {
    // ...
}
```

### Error Handling

#### Use `anyhow::Result` for Application Errors
```rust
use anyhow::{Context, Result};

pub async fn process_file(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)
        .context("Failed to read file")?;

    // Use ? for error propagation
    let accounts = parse_accounts(&content)?;

    Ok(())
}
```

#### Context Messages
Provide meaningful context for errors:

```rust
// Good
let username_idx = headers.iter().position(|h| h.to_lowercase().contains("username"))
    .context("Username column not found")?;
let password_idx = headers.iter().position(|h| h.to_lowercase().contains("password"))
    .context("Password column not found")?;
```

#### Logging
Use appropriate log levels:

```rust
use tracing::{debug, error, info, warn};

info!("Starting master process with {} threads", thread_count);
warn!("Skipping row {} due to missing data", row_index);
error!("Failed to connect to browser: {}", e);
debug!("Processing file: {}", path.display());
```

### Async Programming

#### Async Function Signatures
```rust
pub async fn run_master(config: MasterConfig) -> Result<()> {
    // Async operations here
    Ok(())
}
```

#### Avoid Holding Locks Across Await Points
```rust
// Bad - Holding lock across await
let mut processing = processing_files.lock().unwrap();
tx.send(path).await?;  // Lock held during await!
drop(processing);

// Good - Scoped lock usage
{
    let mut processing = processing_files.lock().unwrap();
    processing.insert(path.clone());
}
tx.send(path).await?;  // Lock released
```

### Code Formatting

#### Use `cargo fmt`
- Always run `cargo fmt` before committing
- Follow standard Rust formatting conventions

#### Line Length
- Keep lines under 100 characters when possible
- Break long lines appropriately

#### Documentation
```rust
/// Processes a CSV file containing account information.
///
/// This function reads accounts from the specified file, validates the format,
/// and returns a list of account objects ready for processing.
///
/// # Arguments
/// * `path` - Path to the CSV file to process
/// * `config` - Processing configuration
///
/// # Returns
/// Returns a `Result` containing the processing result or an error
///
/// # Errors
/// Returns an error if the file cannot be read or parsed
pub async fn process_file(path: &Path, config: ProcessConfig) -> Result<()> {
    // Implementation
}
```

### Testing Guidelines

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account::new(
            "test@example.com".to_string(),
            "password123".to_string()
        );

        assert_eq!(account.username, "test@example.com");
        assert_eq!(account.password, "password123");
    }

    #[tokio::test]
    async fn test_async_function() {
        // Async test code
    }
}
```

#### Integration Tests
Place integration tests in `tests/` directory:

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_full_workflow() {
    // Integration test code
}
```

### Security Considerations

#### Avoid Logging Secrets
```rust
// Bad - Never log passwords
info!("Processing account: {} with password: {}", username, password);

// Good - Log safely
info!("Processing account: {}", username);
// password is not logged
```

#### Input Validation
Always validate user inputs:

```rust
pub fn validate_credentials(username: &str, password: &str) -> Result<()> {
    if username.is_empty() {
        anyhow::bail!("Username cannot be empty");
    }
    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }
    Ok(())
}
```

### Performance Guidelines

#### Use Efficient Data Structures
- `HashSet` for fast lookups
- `Vec` for ordered collections
- `Arc<Mutex<T>>` for shared mutable state in async contexts

#### Minimize Allocations
```rust
// Prefer reuse where possible
let mut buffer = String::with_capacity(1024);
file.read_to_string(&mut buffer)?;
```

### Git Commit Guidelines

#### Commit Messages
```
Fix: 修复所有 Rust Clippy 警告并优化代码结构

主要改进:
- 简化复杂类型定义
- 重构函数参数过多问题
- 修复异步锁持有问题

技术细节:
- 引入配置结构体减少参数
- 优化字符串操作
- 清理未使用的导入
```

#### Branch Naming
- `feature/feature-name` for new features
- `fix/bug-description` for bug fixes
- `refactor/component-name` for refactoring
- `docs/documentation-update` for documentation

### CI/CD Integration

The project uses GitHub Actions for CI/CD with the following checks:
- `cargo check` - Compilation verification
- `cargo clippy -- -D warnings` - Code quality
- `cargo test` - Test execution
- `cargo fmt --check` - Formatting verification

### Tooling

#### Recommended VS Code Extensions
- `rust-lang.rust-analyzer` - Language server
- `vadimcn.vscode-lldb` - Debugger
- `serayuzgur.crates` - Dependency management

#### Development Setup
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install additional tools
rustup component add rustfmt clippy
cargo install cargo-edit cargo-watch

# Development with auto-rebuild
cargo watch -x 'check' -x 'test'
```