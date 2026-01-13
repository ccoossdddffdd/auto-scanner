# Implementation Plan - Infrastructure Setup

## Phase 1: Project Initialization & CLI
- [~] Task: Initialize Rust project
    - [ ] Sub-task: Run `cargo init`
    - [ ] Sub-task: Add dependencies (`clap`, `tokio`, `tracing`, `tracing-subscriber`, `anyhow`) to `Cargo.toml`
- [ ] Task: Implement CLI with `clap`
    - [ ] Sub-task: Define CLI struct with input path argument
    - [ ] Sub-task: Implement `main.rs` to parse args and initialize logger
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Project Initialization & CLI' (Protocol in workflow.md)

## Phase 2: CSV Parsing Module
- [ ] Task: Define Account Model
    - [ ] Sub-task: Create `models` module and define `Account` struct
    - [ ] Sub-task: Add `serde` dependency for serialization
- [ ] Task: Implement CSV Reader
    - [ ] Sub-task: Add `csv` dependency
    - [ ] Sub-task: Implement function to read CSV and deserialize into `Vec<Account>`
    - [ ] Sub-task: Add unit tests for CSV parsing logic
- [ ] Task: Conductor - User Manual Verification 'Phase 2: CSV Parsing Module' (Protocol in workflow.md)

## Phase 3: Database & Storage Layer
- [ ] Task: Setup `sqlx` and SQLite
    - [ ] Sub-task: Add `sqlx` (with `sqlite`, `runtime-tokio`) dependency
    - [ ] Sub-task: Install `sqlx-cli` (optional, or rely on runtime migration)
- [ ] Task: Database Initialization Logic
    - [ ] Sub-task: Write SQL migration script for `accounts` table
    - [ ] Sub-task: Implement database connection and automated migration execution on startup
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Database & Storage Layer' (Protocol in workflow.md)

## Phase 4: Integration
- [ ] Task: Wire everything together in `main.rs`
    - [ ] Sub-task: Call CSV reader
    - [ ] Sub-task: Call DB init
    - [ ] Sub-task: (Temporary) Insert read accounts into DB to verify flow
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Integration' (Protocol in workflow.md)

