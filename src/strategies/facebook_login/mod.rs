use super::BaseStrategy;
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use crate::infrastructure::adspower::ProfileConfig;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::info;

pub mod constants;
use constants::FacebookConfig;

// Initialize global configuration
static CONFIG: once_cell::sync::Lazy<FacebookConfig> =
    once_cell::sync::Lazy::new(FacebookConfig::default);

pub fn get_profile_config() -> ProfileConfig {
    ProfileConfig {
        group_id: "0".to_string(),
        domain_name: "facebook.com".to_string(),
        open_urls: vec!["https://www.facebook.com".to_string()],
    }
}

#[derive(Default)]
pub struct FacebookLoginStrategy;

impl FacebookLoginStrategy {
    pub fn new() -> Self {
        Self
    }
}

/// 登录状态枚举
enum LoginStatus {
    Success,
    Captcha,
    TwoFactor,
    WrongPassword,
    AccountLocked,
    Failed,
}

/// 登录结果检测器
struct LoginResultDetector;

impl LoginResultDetector {
    async fn detect_status(adapter: &dyn BrowserAdapter) -> LoginStatus {
        // Optimization: Get URL once and pass it to check functions if needed.
        // However, some checks might navigate or need fresh state, but for status detection
        // which is usually read-only, passing the URL is a good optimization.
        // NOTE: Currently check functions re-fetch URL. We can optimize this by passing the URL string.

        let current_url = adapter.get_current_url().await.unwrap_or_default();

        // 1. Check success first (most common case)
        if Self::check_success(adapter, &current_url).await {
            return LoginStatus::Success;
        }

        // 2. If not success, check for specific error states in parallel
        let (has_captcha, has_2fa, wrong_password, account_locked) = tokio::join!(
            Self::check_captcha(adapter, &current_url),
            Self::check_2fa(adapter, &current_url),
            Self::check_wrong_password(adapter, &current_url),
            Self::check_account_locked(adapter, &current_url),
        );

        if has_captcha {
            LoginStatus::Captcha
        } else if has_2fa {
            LoginStatus::TwoFactor
        } else if wrong_password {
            LoginStatus::WrongPassword
        } else if account_locked {
            LoginStatus::AccountLocked
        } else {
            LoginStatus::Failed
        }
    }

    async fn check_success(adapter: &dyn BrowserAdapter, url: &str) -> bool {
        // Use cached URL
        info!("Current URL: {}", url);
        if url.contains("/login") || url.contains("/checkpoint") {
            info!("URL contains /login or /checkpoint, login failed");
            return false;
        }

        // 检查是否还存在登录表单（如果存在说明未登录）
        let email_visible = adapter
            .is_visible(&CONFIG.selectors.login_form.email)
            .await
            .unwrap_or(false);
        let pass_visible = adapter
            .is_visible(&CONFIG.selectors.login_form.pass)
            .await
            .unwrap_or(false);

        info!(
            "Login form visibility - email: {}, password: {}",
            email_visible, pass_visible
        );

        if email_visible && pass_visible {
            info!("Login form still visible, login failed");
            return false;
        }

        // 检查登录成功后才有的元素
        // 个人资料/账号菜单按钮
        for selector in &CONFIG.selectors.indicators.profile {
            if let Ok(visible) = adapter.is_visible(selector).await {
                info!("Checking profile indicator '{}': {}", selector, visible);
                if visible {
                    info!("Found profile indicator: {}", selector);
                    return true;
                }
            }
        }

        // 通用成功元素
        for selector in &CONFIG.selectors.indicators.elements {
            if let Ok(visible) = adapter.is_visible(selector).await {
                info!("Checking success element '{}': {}", selector, visible);
                if visible {
                    info!("Found success element: {}", selector);
                    return true;
                }
            }
        }

        info!("No success indicators found, login failed");
        false
    }

    async fn check_captcha(adapter: &dyn BrowserAdapter, url: &str) -> bool {
        // 方法1: 检查 URL 是否包含验证码标识
        if url.contains("captcha")
            || CONFIG
                .urls
                .checkpoints
                .iter()
                .any(|id| url.contains("checkpoint") && url.contains(id))
        {
            info!("URL indicates captcha required");
            return true;
        }

        // 方法2: 检查特定的验证码元素
        for selector in &CONFIG.selectors.captcha {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    info!("Found captcha element: {}", selector);
                    return true;
                }
            }
        }

        // 方法3: 检查错误消息中的验证码关键词
        for selector in &CONFIG.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Checking error message for captcha: {}", text);

                        for keyword in &CONFIG.keywords.captcha {
                            if text_lower.contains(keyword) {
                                info!("Detected captcha via keyword: {}", keyword);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    async fn check_2fa(adapter: &dyn BrowserAdapter, url: &str) -> bool {
        // 首先检查 URL 是否包含 two_step_verification
        if url.contains("two_step_verification") {
            return true;
        }

        // 检查页面元素
        adapter
            .is_visible(&CONFIG.selectors.two_fa_input)
            .await
            .unwrap_or(false)
    }

    async fn check_wrong_password(adapter: &dyn BrowserAdapter, url: &str) -> bool {
        // 方法1: 检查 URL 是否包含密码错误标识
        if url.contains("/login") && url.contains("error") {
            info!("URL indicates login error (possibly wrong password)");
        }

        // 方法2: 检查特定的错误元素
        for selector in &CONFIG.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        info!("Found error message for password check: {}", text);
                        let text_lower = text.to_lowercase();

                        // 仅检查小写文本
                        for keyword in &CONFIG.keywords.wrong_password {
                            if text_lower.contains(&keyword.to_lowercase()) {
                                info!(
                                    "Detected wrong password via keyword (case-insensitive): {}",
                                    keyword
                                );
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    async fn check_account_locked(adapter: &dyn BrowserAdapter, url: &str) -> bool {
        // 方法1: 检查 URL 模式
        // checkpoint 通常表示账号被限制
        if url.contains("/checkpoint") {
            info!("URL contains /checkpoint, account may be locked");

            // 进一步检查是否真的是锁定而不是其他 checkpoint
            // 如果不是 2FA checkpoint，很可能是账号锁定
            if !url.contains("two_step") && !url.contains("2fa") {
                return true;
            }
        }

        if url.contains("locked") || url.contains("disabled") || url.contains("suspended") {
            info!("URL indicates account locked/disabled/suspended");
            return true;
        }

        // 方法2: 检查特定的锁定页面元素
        for selector in &CONFIG.selectors.locked_indicators {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                info!("Found account locked indicator element: {}", selector);
            }
        }

        // 方法3: 检查错误消息文本
        for selector in &CONFIG.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Found error message for locked check: {}", text);

                        for keyword in &CONFIG.keywords.account_locked {
                            if text_lower.contains(keyword) {
                                info!("Detected account locked via keyword: {}", keyword);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

#[async_trait]
impl BaseStrategy for FacebookLoginStrategy {
    async fn run(&self, adapter: &dyn BrowserAdapter, account: &Account) -> Result<WorkerResult> {
        info!("Navigating to Facebook...");
        adapter.navigate(&CONFIG.urls.base).await?;

        info!("Waiting for email input...");
        adapter
            .wait_for_element(&CONFIG.selectors.login_form.email)
            .await?;

        // 检查当前域名，如果是移动版直接返回错误
        if let Ok(url) = adapter.get_current_url().await {
            info!("After navigation, current URL: {}", url);
            if url.contains(&CONFIG.urls.mobile_check) {
                anyhow::bail!(
                    "Mobile version ({}) is not supported. Please use desktop browser.",
                    CONFIG.urls.mobile_check
                );
            }
        }

        info!("Typing credentials...");
        adapter
            .type_text(&CONFIG.selectors.login_form.email, &account.username)
            .await?;
        adapter
            .type_text(&CONFIG.selectors.login_form.pass, &account.password)
            .await?;

        info!("Clicking login button...");
        adapter
            .click(&CONFIG.selectors.login_form.login_btn)
            .await?;

        // Wait for navigation or state change
        tokio::time::sleep(std::time::Duration::from_secs(
            CONFIG.timeouts.login_wait_secs,
        ))
        .await;

        // 再次检查是否跳转到移动版
        if let Ok(url) = adapter.get_current_url().await {
            info!("After login, current URL: {}", url);
            if url.contains(&CONFIG.urls.mobile_check) {
                anyhow::bail!(
                    "Browser redirected to mobile version ({}), which is not supported.",
                    CONFIG.urls.mobile_check
                );
            }
        }

        // 桌面版检测逻辑（原有逻辑）
        let status = LoginResultDetector::detect_status(adapter).await;

        let mut data = serde_json::Map::new();
        data.insert(
            "验证码".to_string(),
            serde_json::Value::String("不需要".to_string()),
        );
        data.insert(
            "2FA".to_string(),
            serde_json::Value::String("不需要".to_string()),
        );

        let mut result = WorkerResult {
            status: "登录失败".to_string(),
            message: "未知失败".to_string(),
            data: Some(data),
        };

        match status {
            LoginStatus::Success => {
                info!("Login detected as successful");
                result.status = "登录成功".to_string();
                result.message = "成功".to_string();

                // 获取好友数量
                if let Ok(count) = self.get_friends_count(adapter).await {
                    if let Some(data) = &mut result.data {
                        data.insert(
                            "好友数量".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(count)),
                        );
                    }
                    info!("Friends count: {}", count);
                }
            }
            LoginStatus::Captcha => {
                info!("Captcha detected");
                if let Some(data) = &mut result.data {
                    data.insert(
                        "验证码".to_string(),
                        serde_json::Value::String("需要".to_string()),
                    );
                }
                result.message = "检测到验证码".to_string();
            }
            LoginStatus::TwoFactor => {
                info!("2FA detected");
                if let Some(data) = &mut result.data {
                    data.insert(
                        "2FA".to_string(),
                        serde_json::Value::String("需要".to_string()),
                    );
                }
                result.message = "检测到 2FA".to_string();
            }
            LoginStatus::AccountLocked => {
                info!("Account locked detected");
                result.status = "登录失败".to_string();
                result.message = "账号已锁定".to_string();
            }
            LoginStatus::WrongPassword => {
                info!("Wrong password detected");
                result.status = "登录失败".to_string();
                result.message = "密码错误".to_string();
            }
            LoginStatus::Failed => {
                // 保持默认值
            }
        }

        Ok(result)
    }
}

impl FacebookLoginStrategy {
    async fn get_friends_count(&self, adapter: &dyn BrowserAdapter) -> Result<u32> {
        info!("Getting friends count...");

        // 导航到好友页面
        adapter
            .navigate(&CONFIG.urls.friends)
            .await
            .context("Failed to navigate to friends page")?;

        // 等待页面加载
        tokio::time::sleep(std::time::Duration::from_secs(
            CONFIG.timeouts.page_load_secs,
        ))
        .await;

        // 获取当前 URL 确认导航成功
        if let Ok(url) = adapter.get_current_url().await {
            info!("Navigated to friends page, current URL: {}", url);
        }

        // 尝试多个选择器获取好友数量
        for selector in &CONFIG.selectors.friends_count {
            // 获取所有匹配元素的文本
            if let Ok(texts) = adapter.get_all_text(selector).await {
                info!(
                    "Found {} elements for selector '{}', checking each...",
                    texts.len(),
                    selector
                );

                for (index, text) in texts.iter().enumerate() {
                    let trimmed = text.trim();
                    info!("Element {} from '{}': '{}'", index + 1, selector, trimmed);

                    // 尝试从文本中提取数字
                    if let Some(count) = Self::extract_number_from_text(trimmed) {
                        // 过滤掉明显不合理的数字
                        if count > 0 && count < 10000 {
                            info!(
                                "✓ Extracted friends count {} from selector '{}', element {}",
                                count,
                                selector,
                                index + 1
                            );
                            return Ok(count);
                        } else {
                            info!(
                                "✗ Skipping unreasonable count: {} from selector '{}', element {}",
                                count,
                                selector,
                                index + 1
                            );
                        }
                    }
                }
            }
        }

        info!("Could not extract friends count from any selector, returning 0");
        Ok(0)
    }

    fn extract_number_from_text(text: &str) -> Option<u32> {
        // 提取文本中的数字，支持逗号分隔的数字如 "1,234"
        let cleaned = text.replace([',', ' ', '\n', '\t'], "");
        let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.is_empty() {
            return None;
        }

        digits.parse::<u32>().ok()
    }
}
