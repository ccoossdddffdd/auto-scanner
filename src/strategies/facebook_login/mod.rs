use super::BaseStrategy;
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::adspower::ProfileConfig;
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::info;
pub mod constants;
pub mod detector;
pub mod result_builder;
use constants::FacebookConfig;
use detector::{LoginStatus, LoginStatusDetector};
use result_builder::FacebookResultBuilder;
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

    async fn perform_login(&self, adapter: &dyn BrowserAdapter, account: &Account) -> Result<()> {
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

        let friends_count = if status == LoginStatus::Success {
            self.get_friends_count(adapter).await.ok()
        } else {
            None
        };

        Ok(FacebookResultBuilder::build(status, friends_count))
    }
}
