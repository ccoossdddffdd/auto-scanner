# 实施计划 (plan.md) - 浏览器后端适配器模式实现

## 阶段 1: 抽象层定义
本阶段重点是定义核心 Trait 和错误处理机制，为多后端支持打下架构基础。

- [x] **Task: 定义 `BrowserError` 错误类型**
  - 在 `src/` 下创建或更新错误处理模块。
  - 定义涵盖导航失败、元素未找到、连接超时等场景的枚举。
- [x] **Task: 定义 `BrowserAdapter` Trait**
  - 在 `src/` 中定义异步 Trait `BrowserAdapter`。
  - 包含 `navigate`, `type_text`, `click`, `wait_for_element`, `is_visible`, `get_cookies`, `set_cookies`, `take_screenshot` 等方法。
- [x] **Task: Conductor - User Manual Verification '阶段 1: 抽象层定义' (Protocol in workflow.md)**

## 阶段 2: Playwright 适配器实现
本阶段将实现第一个具体的适配器，使其能够通过远程调试端口控制 Chrome。
*Note: Originally attempted to use `chromiumoxide`, but reverted to `playwright` (rust crate) as per user request. Used `connect_over_cdp_builder` to support existing CDP sessions.*

- [x] **Task: 实现 `PlaywrightAdapter` 基础结构与连接逻辑**
  - 创建 `src/browser/playwright_adapter.rs`。
  - 实现连接到给定远程调试 URL 的逻辑。
  - **TDD**: 编写测试确保能够成功建立连接（或在连接失败时返回正确错误）。
- [x] **Task: 完成 `BrowserAdapter` Trait 方法实现**
  - 逐一实现 Trait 中定义的方法。
  - 封装 `playwright` 的底层调用。
- [x] **Task: 编写 Playwright 适配器集成测试**
  - **TDD**: 编写测试用例验证各个方法在真实（或模拟远程调试）环境下的表现。
- [x] **Task: Conductor - User Manual Verification '阶段 2: Playwright 适配器实现' (Protocol in workflow.md)**

## 阶段 3: CLI 集成与登录逻辑重构
本阶段将适配器集成到现有程序中，并更新命令行参数。

- [x] **Task: 更新 `src/cli.rs` 参数定义**
  - 增加 `--backend` (可选: playwright/cdp) 和 `--remote-url` 参数。
- [x] **Task: 重构账号验证逻辑以使用适配器**
  - 修改 `src/main.rs` 或相关业务逻辑模块。
  - 将具体的浏览器操作替换为对 `BrowserAdapter` 接口的调用。
  - 实现基于命令行参数的适配器工厂或注入逻辑。
- [x] **Task: 端到端流程验证 (Facebook 登录)**
  - 使用远程调试模式下的 Chrome 运行程序，验证 Facebook 登录和状态记录功能。
- [x] **Task: Conductor - User Manual Verification '阶段 3: CLI 集成与登录逻辑重构' (Protocol in workflow.md)**
