# Refactoring Plan

- [ ] **1. Normalize IPC Communication**
    - [ ] Modify `src/services/worker/runner.rs` to output results with frame delimiters (e.g., `<<RESULT>>...<<RESULT>>`).
    - [ ] Modify the Master side (likely `src/services/worker/coordinator.rs` or `processor.rs`) to parse the delimited JSON, ignoring other stdout noise.

- [ ] **2. Abstract Browser Environment Interface**
    - [ ] Define `BrowserEnvironmentManager` trait.
    - [ ] Implement trait for `AdsPowerClient`.
    - [ ] Update `MasterContext` to use `Box<dyn BrowserEnvironmentManager>`.

- [ ] **3. Extract File Policy Service**
    - [ ] Create `src/services/file_policy.rs`.
    - [ ] Move file extension checking and ignore rules from `master.rs`.
    - [ ] Move path generation logic from `processor.rs`.

- [ ] **4. Centralized Configuration**
    - [ ] Create `src/core/config.rs` (or `src/config.rs`).
    - [ ] Define `AppConfig` struct.
    - [ ] Refactor `main.rs` to load config.
    - [ ] Inject config into services instead of using `std::env::var`.

- [ ] **5. Deconstruct `MasterContext::run`**
    - [ ] Extract `InputWatcher` for file events.
    - [ ] Extract `JobScheduler` for concurrency management.
    - [ ] Simplify `master.rs` to coordinate these components.
