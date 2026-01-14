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

        let mut result = WorkerResult {
            status: "登录失败".to_string(),
            captcha: "不需要".to_string(),
            two_fa: "不需要".to_string(),
            message: "未知失败".to_string(),
        };

        if adapter
            .is_visible("a[aria-label='Facebook']")
            .await
            .unwrap_or(false)
            || adapter
                .is_visible("div[role='navigation']")
                .await
                .unwrap_or(false)
        {
            info!("Login detected as successful");
            result.status = "登录成功".to_string();
            result.message = "成功".to_string();
        } else if adapter
            .is_visible("input[name='captcha_response']")
            .await
            .unwrap_or(false)
        {
            info!("Captcha detected");
            result.captcha = "需要".to_string();
            result.message = "检测到验证码".to_string();
        } else if adapter
            .is_visible("input[name='approvals_code']")
            .await
            .unwrap_or(false)
        {
            info!("2FA detected");
            result.two_fa = "需要".to_string();
            result.message = "检测到 2FA".to_string();
        }

        if enable_screenshot {
            info!("Taking screenshot...");
            let screenshot_dir = Path::new("screenshot");
            if !screenshot_dir.exists() {
                fs::create_dir_all(screenshot_dir)
                    .context("Failed to create screenshot directory")?;
            }

            let timestamp = Local::now().format("%Y%m%d-%H%M%S");
            let safe_username = account.username.replace(['@', '.'], "_");
            let filename = format!("screenshot/login_{}_{}.png", safe_username, timestamp);

            adapter.take_screenshot(&filename).await?;
            info!("Screenshot saved to {}", filename);
        }

        Ok(result)
    }
}
