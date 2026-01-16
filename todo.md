# Refactoring Plan

- [x] **1. Normalize IPC Communication**
- [x] **2. Abstract Browser Environment Interface**
- [x] **3. Extract File Policy Service**
- [x] **4. Centralized Configuration**
- [x] **5. Deconstruct `MasterContext::run`**

# Phase 2 Refactoring

- [ ] **1. Refactor EmailMonitor**
    - [ ] Create `ImapService` trait and implementation.
    - [ ] Extract `AttachmentHandler`.
    - [ ] Extract `EmailProcessor`.
    - [ ] Update `EmailMonitor` to use dependency injection.

- [ ] **2. Decouple WorkerCoordinator Strategy**
    - [ ] Create `StrategyProfileProvider` trait.
    - [ ] Implement provider for Facebook strategy.
    - [ ] Inject provider into `WorkerCoordinator`.

- [ ] **3. Extract Worker Process Executor**
    - [ ] Create `ProcessExecutor` trait.
    - [ ] Extract `WorkerOutputParser` struct.
    - [ ] Refactor `WorkerCoordinator` to use executor.

- [ ] **4. Purify File Operations**
    - [ ] Refactor `prepare_input_file` to return status instead of calling monitor.
    - [ ] Move monitor update logic to `Master` or `Processor`.

- [ ] **5. PlaywrightAdapter Builder**
    - [ ] Create `PlaywrightAdapterBuilder`.
    - [ ] Simplify `PlaywrightAdapter::new`.
