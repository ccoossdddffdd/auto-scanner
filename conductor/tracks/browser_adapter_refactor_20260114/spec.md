# 规范文档 (spec.md) - 浏览器后端适配器模式实现 (Playwright 远程调试版)

## 1. 概述 (Overview)

本 Track 的目标是重构 `auto-scanner` 的浏览器自动化逻辑，引入**适配器模式 (Adapter Pattern)**。虽然目前主要使用 Playwright，但通过定义通用的 `BrowserAdapter` 接口，为未来支持不同的浏览器后端（如原生 CDP、Selenium 等）奠定架构基础。首个实现将侧重于利用 Playwright 连接到已运行 Chrome 实例的**远程调试端口 (Remote Debugging Port)**。

## 2. 功能需求 (Functional Requirements)

- **定义通用接口 (BrowserAdapter Trait)**：
    - `navigate(url: &str)`：页面导航。
    - `type_text(selector: &str, text: &str)`：在指定元素输入文本。
    - `click(selector: &str)`：点击指定元素。
    - `wait_for_element(selector: &str)`：等待元素出现。
    - `is_visible(selector: &str) -> bool`：检查元素是否可见。
    - `get_cookies() / set_cookies()`：管理会话状态。
    - `take_screenshot(path: &str)`：截取屏幕截图。
- **实现 Playwright 适配器**：
    - 支持通过 **Remote Debugging URL** (例如 `http://localhost:9222`) 连接到现有的浏览器。
    - 封装 `playwright-rust` 的相关操作以符合 `BrowserAdapter` 接口。
- **命令行集成**：
    - 增加 `--backend <BACKEND>` 参数（目前仅支持 `playwright`），默认值为 `playwright`。
    - 增加 `--remote-url <URL>` 参数，用于指定 Chrome 的远程调试地址，默认值为 `http://localhost:9222`，仅在 `playwright` 后端时生效。
    - 增加 `--thread-count <COUNT>` 参数，用于指定启动登录流程的线程数，默认值为 `1`。
- **逻辑解耦**：
    - 将 Facebook 登录流程与具体的浏览器驱动库解耦，仅依赖 `BrowserAdapter`。

## 3. 非功能需求 (Non-Functional Requirements)

- **架构前瞻性**：适配器设计必须足够通用，确保未来替换底层驱动（如改用 `chromiumoxide`）时，上层业务逻辑（登录、扫描）无需修改。代码管理必须清晰简洁，避免引入复杂的条件分支。
- **异步处理**：完全兼容项目的 `tokio` 异步运行时，同时要支持多线程并发。

## 4. 验收标准 (Acceptance Criteria)

- [ ] 成功定义 `BrowserAdapter` Trait。
- [ ] 实现 `PlaywrightAdapter`，并支持通过远程调试端口连接浏览器。
- [ ] 能够通过 `PlaywrightAdapter` 成功在受控的 Chrome 实例中完成 Facebook 登录。
- [ ] 命令行参数 `--remote-url` 能够正确传递并建立连接。

## 5. 超出范围 (Out of Scope)

- 暂时不实现基于 `chromiumoxide` 的原生 Chrome 适配器。
- 暂时不实现由程序自动启动/管理浏览器进程的逻辑（假定浏览器已带远程调试参数运行）。
