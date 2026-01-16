# Refactoring Plan

- [x] **1. Normalize IPC Communication**
- [x] **2. Abstract Browser Environment Interface**
- [x] **3. Extract File Policy Service**
- [x] **4. Centralized Configuration**
- [x] **5. Deconstruct `MasterContext::run`**

# Phase 2 Refactoring

- [x] **1. Refactor EmailMonitor**
- [x] **2. Decouple WorkerCoordinator Strategy**
- [x] **3. Extract Worker Process Executor**
- [x] **4. Purify File Operations**
- [x] **5. PlaywrightAdapter Builder**

# Phase 3 Refactoring

- [x] **1. Refactor Facebook Strategy Run**
- [x] **2. Abstract AdsPower Fingerprint Generation**
- [x] **3. Encapsulate Master Lifecycle**
- [x] **4. Generalize Login Status Detection**
- [x] **5. Simplify Playwright Connection**

# Phase 4 Refactoring

- [ ] **1. Abstract Time Provider**
    - [ ] Create `TimeProvider` trait.
    - [ ] Implement `SystemTimeProvider` and `MockTimeProvider`.
    - [ ] Inject into `FileTracker`.

- [ ] **2. Unified State Update**
    - [ ] Create `update_context` helper method in `FileTracker`.
    - [ ] Refactor all `mark_*` methods to use it.

- [ ] **3. Typed Strategy Factory**
    - [ ] Create `WorkerStrategy` enum.
    - [ ] Implement `FromStr` for `WorkerStrategy`.
    - [ ] Update `StrategyFactory` and `Coordinator`.

- [ ] **4. Inject Worker Orchestrator**
    - [ ] Create `WorkerOrchestrator` trait.
    - [ ] Implement for `WorkerCoordinator`.
    - [ ] Inject into `process_accounts`.

- [ ] **5. Separate Config Loading**
    - [ ] Create `AppConfig::from_env`.
    - [ ] Make `AppConfig::new` a pure constructor.
