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

- [ ] **1. Refactor Facebook Strategy Run**
    - [ ] Create `FacebookResultBuilder` to handle `WorkerResult` construction.
    - [ ] Simplify `run` function to focus on flow control.

- [ ] **2. Abstract AdsPower Fingerprint Generation**
    - [ ] Create `FingerprintGenerator` module.
    - [ ] Define `CreateProfileRequest` struct with `serde`.
    - [ ] Refactor `create_profile` in `adspower.rs`.

- [ ] **3. Encapsulate Master Lifecycle**
    - [ ] Create `MasterServer` struct.
    - [ ] Implement `bootstrap`, `start`, `shutdown` methods.
    - [ ] Refactor `run` function in `master/mod.rs`.

- [ ] **4. Generalize Login Status Detection**
    - [ ] Define `DetectionRule` struct.
    - [ ] Implement generic `check_rule` method.
    - [ ] Refactor `LoginStatusDetector` to use rules.

- [ ] **5. Simplify Playwright Connection**
    - [ ] Extract `connect_cdp` logic.
    - [ ] Extract error formatting.
    - [ ] Flatten `PlaywrightAdapterBuilder::build`.
