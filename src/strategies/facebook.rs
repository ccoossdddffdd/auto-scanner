use super::LoginStrategy;
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Local;
use std::fs;
use std::path::Path;
use tracing::info;

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
    Failed,
}

/// 登录结果检测器
struct LoginResultDetector;

impl LoginResultDetector {
    async fn detect_status(adapter: &dyn BrowserAdapter) -> LoginStatus {
        // 并行检测多个状态
        let (is_success, has_captcha, has_2fa) = tokio::join!(
            Self::check_success(adapter),
            Self::check_captcha(adapter),
            Self::check_2fa(adapter),
        );

        if is_success {
            LoginStatus::Success
        } else if has_captcha {
            LoginStatus::Captcha
        } else if has_2fa {
            LoginStatus::TwoFactor
        } else {
            LoginStatus::Failed
        }
    }

    async fn check_success(adapter: &dyn BrowserAdapter) -> bool {
        adapter
            .is_visible("a[aria-label='Facebook']")
            .await
            .unwrap_or(false)
            || adapter
                .is_visible("div[role='navigation']")
                .await
                .unwrap_or(false)
    }

    async fn check_captcha(adapter: &dyn BrowserAdapter) -> bool {
        adapter
            .is_visible("input[name='captcha_response']")
            .await
            .unwrap_or(false)
    }

    async fn check_2fa(adapter: &dyn BrowserAdapter) -> bool {
        adapter
            .is_visible("input[name='approvals_code']")
            .await
            .unwrap_or(false)
    }
}

#[async_trait]
impl LoginStrategy for FacebookLoginStrategy {
    async fn login(
        &self,
        adapter: &dyn BrowserAdapter,
        account: &Account,
        enable_screenshot: bool,
    ) -> Result<WorkerResult> {
        info!("Navigating to Facebook...");
        adapter.navigate("https://www.facebook.com").await?;

        info!("Waiting for email input...");
        adapter.wait_for_element("input[name='email']").await?;

        info!("Typing credentials...");
        adapter
            .type_text("input[name='email']", &account.username)
            .await?;
        adapter
            .type_text("input[name='pass']", &account.password)
            .await?;

        info!("Clicking login button...");
        adapter.click("button[name='login']").await?;

        // Wait for navigation or state change
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;

        // 检测登录结果
        let status = LoginResultDetector::detect_status(adapter).await;
        let mut result = WorkerResult {
            status: "登录失败".to_string(),
            captcha: "不需要".to_string(),
            two_fa: "不需要".to_string(),
            message: "未知失败".to_string(),
        };

        match status {
            LoginStatus::Success => {
                info!("Login detected as successful");
                result.status = "登录成功".to_string();
                result.message = "成功".to_string();
            }
            LoginStatus::Captcha => {
                info!("Captcha detected");
                result.captcha = "需要".to_string();
                result.message = "检测到验证码".to_string();
            }
            LoginStatus::TwoFactor => {
                info!("2FA detected");
                result.two_fa = "需要".to_string();
                result.message = "检测到 2FA".to_string();
            }
            LoginStatus::Failed => {
                // 保持默认值
            }
        }

        if enable_screenshot {
            self.take_screenshot(adapter, &account.username).await?;
        }

        Ok(result)
    }
}

impl FacebookLoginStrategy {
    async fn take_screenshot(&self, adapter: &dyn BrowserAdapter, username: &str) -> Result<()> {
        info!("Taking screenshot...");
        let screenshot_dir = Path::new("screenshot");
        if !screenshot_dir.exists() {
            fs::create_dir_all(screenshot_dir).context("Failed to create screenshot directory")?;
        }

        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let safe_username = username.replace(['@', '.'], "_");
        let filename = format!("screenshot/login_{}_{}.png", safe_username, timestamp);

        adapter.take_screenshot(&filename).await?;
        info!("Screenshot saved to {}", filename);

        Ok(())
    }
}
