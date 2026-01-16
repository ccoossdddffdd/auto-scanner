use super::BaseStrategy;
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::adspower::ProfileConfig;
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::info;

pub mod constants;
use constants::FacebookConfig;

pub fn get_profile_config() -> ProfileConfig {
    ProfileConfig {
        group_id: "0".to_string(),
        domain_name: "facebook.com".to_string(),
        open_urls: vec!["https://www.facebook.com".to_string()],
    }
}

pub struct FacebookLoginStrategy {
    config: FacebookConfig,
}

impl Default for FacebookLoginStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl FacebookLoginStrategy {
    pub fn new() -> Self {
        Self {
            config: FacebookConfig::default(),
        }
    }

    pub fn with_config(config: FacebookConfig) -> Self {
        Self { config }
    }

    async fn perform_login(
        &self,
        adapter: &dyn BrowserAdapter,
        account: &Account,
    ) -> Result<()> {
        info!("Navigating to Facebook...");
        adapter.navigate(&self.config.urls.base).await?;

        info!("Waiting for email input...");
        adapter
            .wait_for_element(&self.config.selectors.login_form.email)
            .await?;

        self.check_mobile_version(adapter).await?;

        info!("Typing credentials...");
        adapter
            .type_text(&self.config.selectors.login_form.email, &account.username)
            .await?;
        adapter
            .type_text(&self.config.selectors.login_form.pass, &account.password)
            .await?;

        info!("Clicking login button...");
        adapter
            .click(&self.config.selectors.login_form.login_btn)
            .await?;

        // Wait for navigation or state change
        // TODO: Replace fixed sleep with dynamic wait in future refactoring
        tokio::time::sleep(std::time::Duration::from_secs(
            self.config.timeouts.login_wait_secs,
        ))
        .await;

        self.check_mobile_version(adapter).await?;

        Ok(())
    }

    async fn check_mobile_version(&self, adapter: &dyn BrowserAdapter) -> Result<()> {
        if let Ok(url) = adapter.get_current_url().await {
            info!("Current URL check: {}", url);
            if url.contains(&self.config.urls.mobile_check) {
                anyhow::bail!(
                    "Browser redirected to mobile version ({}), which is not supported.",
                    self.config.urls.mobile_check
                );
            }
        }
        Ok(())
    }

    async fn get_friends_count(&self, adapter: &dyn BrowserAdapter) -> Result<u32> {
        info!("Getting friends count...");

        adapter
            .navigate(&self.config.urls.friends)
            .await
            .context("Failed to navigate to friends page")?;

        tokio::time::sleep(std::time::Duration::from_secs(
            self.config.timeouts.page_load_secs,
        ))
        .await;

        if let Ok(url) = adapter.get_current_url().await {
            info!("Navigated to friends page, current URL: {}", url);
        }

        for selector in &self.config.selectors.friends_count {
            if let Ok(texts) = adapter.get_all_text(selector).await {
                for (index, text) in texts.iter().enumerate() {
                    let trimmed = text.trim();
                    if let Some(count) = self.extract_number_from_text(trimmed) {
                        if count > 0 && count < 10000 {
                            info!(
                                "✓ Extracted friends count {} from selector '{}', element {}",
                                count,
                                selector,
                                index + 1
                            );
                            return Ok(count);
                        }
                    }
                }
            }
        }

        info!("Could not extract friends count from any selector, returning 0");
        Ok(0)
    }

    fn extract_number_from_text(&self, text: &str) -> Option<u32> {
        let cleaned = text.replace([',', ' ', '\n', '\t'], "");
        let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        digits.parse::<u32>().ok()
    }
}

#[async_trait]
impl BaseStrategy for FacebookLoginStrategy {
    async fn run(&self, adapter: &dyn BrowserAdapter, account: &Account) -> Result<WorkerResult> {
        self.perform_login(adapter, account).await?;

        let detector = LoginStatusDetector::new(&self.config);
        let status = detector.detect(adapter).await;

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
            LoginStatus::Failed => {}
        }

        Ok(result)
    }
}

enum LoginStatus {
    Success,
    Captcha,
    TwoFactor,
    WrongPassword,
    AccountLocked,
    Failed,
}

struct LoginStatusDetector<'a> {
    config: &'a FacebookConfig,
}

impl<'a> LoginStatusDetector<'a> {
    fn new(config: &'a FacebookConfig) -> Self {
        Self { config }
    }

    async fn detect(&self, adapter: &dyn BrowserAdapter) -> LoginStatus {
        let current_url = adapter.get_current_url().await.unwrap_or_default();

        if self.check_success(adapter, &current_url).await {
            return LoginStatus::Success;
        }

        let (has_captcha, has_2fa, wrong_password, account_locked) = tokio::join!(
            self.check_captcha(adapter, &current_url),
            self.check_2fa(adapter, &current_url),
            self.check_wrong_password(adapter, &current_url),
            self.check_account_locked(adapter, &current_url),
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

    async fn check_success(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        info!("Current URL: {}", url);
        if url.contains("/login") || url.contains("/checkpoint") {
            return false;
        }

        let email_visible = adapter
            .is_visible(&self.config.selectors.login_form.email)
            .await
            .unwrap_or(false);
        let pass_visible = adapter
            .is_visible(&self.config.selectors.login_form.pass)
            .await
            .unwrap_or(false);

        if email_visible && pass_visible {
            return false;
        }

        for selector in &self.config.selectors.indicators.profile {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    return true;
                }
            }
        }

        for selector in &self.config.selectors.indicators.elements {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    return true;
                }
            }
        }

        false
    }

    async fn check_captcha(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("captcha")
            || self
                .config
                .urls
                .checkpoints
                .iter()
                .any(|id| url.contains("checkpoint") && url.contains(id))
        {
            return true;
        }

        for selector in &self.config.selectors.captcha {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    return true;
                }
            }
        }

        for selector in &self.config.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        for keyword in &self.config.keywords.captcha {
                            if text_lower.contains(keyword) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    async fn check_2fa(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("two_step_verification") {
            return true;
        }
        adapter
            .is_visible(&self.config.selectors.two_fa_input)
            .await
            .unwrap_or(false)
    }

    async fn check_wrong_password(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("/login") && url.contains("error") {
            info!("URL indicates login error (possibly wrong password)");
        }

        for selector in &self.config.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        for keyword in &self.config.keywords.wrong_password {
                            if text_lower.contains(&keyword.to_lowercase()) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    async fn check_account_locked(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("/checkpoint")
            && !url.contains("two_step")
            && !url.contains("2fa")
        {
            return true;
        }

        if url.contains("locked") || url.contains("disabled") || url.contains("suspended") {
            return true;
        }

        for selector in &self.config.selectors.locked_indicators {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                return true;
            }
        }

        for selector in &self.config.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        for keyword in &self.config.keywords.account_locked {
                            if text_lower.contains(keyword) {
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
