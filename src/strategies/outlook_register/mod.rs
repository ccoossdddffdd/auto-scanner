pub mod constants;
pub mod generator;

use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use constants::*;
use generator::{UserInfo, UserInfoGenerator};
use rand::Rng;
use std::time::Duration;
use tracing::{info, warn};

use super::BaseStrategy;

use crate::infrastructure::adspower::ProfileConfig;

pub fn get_profile_config() -> ProfileConfig {
    ProfileConfig {
        group_id: "0".to_string(),
        domain_name: "outlook.com".to_string(),
        open_urls: vec!["https://signup.live.com/".to_string()],
    }
}

pub struct OutlookRegisterStrategy;

impl OutlookRegisterStrategy {
    pub fn new() -> Self {
        Self
    }

    async fn random_sleep(&self) {
        let (secs, millis) = {
            let mut rng = rand::rng();
            (rng.random_range(1..=3), rng.random_range(0..1000))
        };
        info!("Sleeping for {}.{} seconds...", secs, millis);
        tokio::time::sleep(Duration::from_secs(secs) + Duration::from_millis(millis)).await;
    }

    async fn type_with_delay(
        &self,
        adapter: &dyn BrowserAdapter,
        selector: &str,
        text: &str,
    ) -> Result<()> {
        adapter.type_text(selector, text).await?;
        // Small delay after typing to simulate human speed/processing
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn handle_data_permission_modal(&self, adapter: &dyn BrowserAdapter) -> Result<()> {
        info!("Checking for data permission modal...");
        let agree_button_selector = AGREE_BUTTON_SELECTORS.join(", ");

        if let Ok(true) = adapter.is_visible(&agree_button_selector).await {
            info!(
                "Data permission modal detected. Clicking '{}'...",
                agree_button_selector
            );
            if let Err(e) = adapter.click(&agree_button_selector).await {
                warn!("Failed to click data permission button: {}", e);
            } else {
                info!("Clicked 'Agree and Continue' on data permission modal");
                self.random_sleep().await;
            }
        } else {
            info!("Data permission modal not found (or not visible), skipping...");
        }
        Ok(())
    }

    async fn fill_email(
        &self,
        adapter: &dyn BrowserAdapter,
        email: &str,
        next_button_selector: &str,
    ) -> Result<()> {
        info!("Filling email...");
        let email_selector = "input[name=\"MemberName\"], input[type=\"email\"], input[name=\"email\"], input[name=\"loginfmt\"]";
        adapter
            .wait_for_element(email_selector)
            .await
            .context("Waiting for email input")?;
        self.type_with_delay(adapter, email_selector, email).await?;

        info!("Clicking Next button...");
        adapter
            .wait_for_element(next_button_selector)
            .await
            .context("Waiting for Next button")?;
        adapter
            .click(next_button_selector)
            .await
            .context("Clicking Next after email")?;
        self.random_sleep().await;
        Ok(())
    }

    async fn fill_password(
        &self,
        adapter: &dyn BrowserAdapter,
        password: &str,
        next_button_selector: &str,
    ) -> Result<()> {
        info!("Filling password...");
        let password_selector =
            "input[name=\"PasswordInput\"], input[type=\"password\"], input[name=\"passwd\"]";
        adapter
            .wait_for_element(password_selector)
            .await
            .context("Waiting for password input")?;
        self.type_with_delay(adapter, password_selector, password)
            .await?;

        adapter
            .click(next_button_selector)
            .await
            .context("Clicking Next after password")?;
        self.random_sleep().await;
        Ok(())
    }

    async fn fill_birth_date(
        &self,
        adapter: &dyn BrowserAdapter,
        user_info: &UserInfo,
        next_button_selector: &str,
    ) -> Result<()> {
        info!("Filling country and birth date...");

        // Fill Birth Year
        info!("Filling Birth Year: {}", user_info.birth_year);
        let birth_year_selector = BIRTH_YEAR_SELECTORS.join(", ");
        match adapter
            .type_text(&birth_year_selector, &user_info.birth_year.to_string())
            .await
        {
            Ok(_) => {}
            Err(_) => {
                info!("Failed to type birth year, trying select...");
                adapter
                    .select_option(
                        "select[name=\"BirthYear\"]",
                        &user_info.birth_year.to_string(),
                    )
                    .await
                    .ok();
            }
        }

        // Fill Birth Month
        info!("Filling Birth Month: {}", user_info.birth_month);
        let month_val = user_info.birth_month.to_string();

        let native_month_selector = "select[name=\"BirthMonth\"]";
        let custom_month_selector_joined = BIRTH_MONTH_SELECTORS.join(", ");

        if let Ok(true) = adapter.is_visible(native_month_selector).await {
            info!("Detected native select for Birth Month. Using select_option.");
            adapter
                .select_option(native_month_selector, &month_val)
                .await
                .context("Selecting birth month (native)")?;
        } else {
            info!("Native select not visible, attempting custom dropdown interaction.");
            let wait_selector = format!(
                "{}, {}",
                native_month_selector, custom_month_selector_joined
            );
            if let Err(e) = adapter.wait_for_element(&wait_selector).await {
                warn!(
                    "Timeout waiting for Birth Month element: {}. Continuing to try click anyway...",
                    e
                );
            }

            if let Err(e) = adapter.click(&custom_month_selector_joined).await {
                warn!(
                    "Failed to click BirthMonthDropdown with primary selectors: {}. Trying fallback strategy...",
                    e
                );
                if let Err(e2) = adapter
                    .select_option("select[name=\"BirthMonth\"]", &month_val)
                    .await
                {
                    warn!("Fallback select month failed: {}", e2);
                    return Err(anyhow::anyhow!(
                        "Failed to interact with Birth Month field: {}",
                        e
                    ));
                }
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;

                let mut month_texts = vec![format!("{}月", month_val), month_val.clone()];
                month_texts.extend(
                    get_month_names(user_info.birth_month)
                        .into_iter()
                        .map(|s| s.to_string()),
                );

                let mut month_clicked = false;
                for text in month_texts {
                    if text.is_empty() {
                        continue;
                    }
                    let selectors = vec![
                        format!("text=\"{}\"", text),
                        format!("text={}", text),
                        format!("[role=\"option\"]:has-text(\"{}\")", text),
                    ];

                    for sel in selectors {
                        if adapter.click(&sel).await.is_ok() {
                            info!("Clicked month option with selector: {}", sel);
                            month_clicked = true;
                            break;
                        }
                    }
                    if month_clicked {
                        break;
                    }
                }

                if !month_clicked {
                    warn!("Failed to click any month option for value: {}", month_val);
                }
            }
        }

        // Fill Birth Day
        info!("Filling Birth Day: {}", user_info.birth_day);
        let day_val = user_info.birth_day.to_string();

        let birth_day_dropdown_selector = BIRTH_DAY_SELECTORS.join(", ");

        // 尝试方法1: 点击下拉框并选择选项
        if let Err(e) = adapter.click(&birth_day_dropdown_selector).await {
            warn!("Failed to click BirthDayDropdown: {}", e);
            // 回退到原生 select
            if let Err(e2) = adapter
                .select_option("select[name=\"BirthDay\"]", &day_val)
                .await
            {
                warn!("Fallback select day failed: {}", e2);
            }
        } else {
            tokio::time::sleep(Duration::from_millis(500)).await;

            // 生成多种日期文本格式
            let day_texts = vec![
                day_val.clone(),                                         // "1"
                format!("{}日", day_val),                                // "1日"
                format!("{:02}", day_val.parse::<u32>().unwrap_or(1)),   // "01"
                format!("{:02}日", day_val.parse::<u32>().unwrap_or(1)), // "01日"
            ];

            let mut day_clicked = false;
            for text in day_texts {
                // 尝试多种选择器
                let selectors = vec![
                    format!("text=\"{}\"", text),                        // 严格匹配
                    format!("text={}", text),                            // 宽松匹配
                    format!("[role=\"option\"]:has-text(\"{}\")", text), // role option
                    format!("div:has-text(\"{}\")", text),               // div 元素
                    format!("li:has-text(\"{}\")", text),                // li 元素
                ];

                for sel in selectors {
                    if adapter.click(&sel).await.is_ok() {
                        info!("Clicked day option with selector: {}", sel);
                        day_clicked = true;
                        break;
                    }
                }

                if day_clicked {
                    break;
                }
            }

            if !day_clicked {
                warn!("Failed to click any day option for value: {}", day_val);
                // 最后尝试：直接输入
                info!("Attempting to type day value directly...");
                let input_selector =
                    "input[name=\"BirthDay\"], [aria-label=\"Day\"], [aria-label=\"日\"]";
                if adapter.type_text(input_selector, &day_val).await.is_ok() {
                    info!("Successfully typed day value");
                }
            }
        }

        if let Err(e) = adapter.click(next_button_selector).await {
            warn!("Click next button failed: {}", e);
        }
        self.random_sleep().await;
        Ok(())
    }

    async fn fill_name(
        &self,
        adapter: &dyn BrowserAdapter,
        user_info: &UserInfo,
        next_button_selector: &str,
    ) -> Result<()> {
        info!("Filling name...");
        let first_name_selector = FIRST_NAME_SELECTORS.join(", ");
        adapter
            .wait_for_element(&first_name_selector)
            .await
            .context("Waiting for first name input")?;
        self.type_with_delay(adapter, &first_name_selector, &user_info.first_name)
            .await?;

        let last_name_selector = LAST_NAME_SELECTORS.join(", ");
        adapter
            .wait_for_element(&last_name_selector)
            .await
            .context("Waiting for last name input")?;
        self.type_with_delay(adapter, &last_name_selector, &user_info.last_name)
            .await?;

        adapter
            .click(next_button_selector)
            .await
            .context("Clicking Next after name")?;
        self.random_sleep().await;
        Ok(())
    }

    async fn check_verification_and_errors(&self, adapter: &dyn BrowserAdapter) -> Result<()> {
        let has_error = if let Ok(true) = adapter.is_visible(".alert-error").await {
            true
        } else {
            matches!(adapter.is_visible(".error").await, Ok(true))
                || matches!(
                    adapter.is_visible("div[aria-live='assertive']").await,
                    Ok(true)
                )
        };

        if has_error {
            let error_text = adapter
                .get_text("div[aria-live='assertive']")
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            let is_bot_detection = BOT_KEYWORDS
                .iter()
                .any(|k| error_text.to_lowercase().contains(k));

            if is_bot_detection {
                warn!(
                    "Bot detection prompt detected in error field: {}. Waiting 15 seconds...",
                    error_text
                );
                tokio::time::sleep(Duration::from_secs(15)).await;
            } else {
                warn!("Registration flow failed with error: {}", error_text);
                return Err(anyhow::anyhow!("Registration flow failed: {}", error_text));
            }
        }

        info!("Checking for potential bot detection...");
        tokio::time::sleep(Duration::from_secs(3)).await;

        let bot_selector = "iframe[id*='enforcement'], #enforcementFrame, iframe[src*='arkose'], #hipEnforcementContainer";
        if let Ok(true) = adapter.is_visible(bot_selector).await {
            warn!("Bot detection/Captcha encountered. Waiting 15 seconds...");
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
        Ok(())
    }
}

impl Default for OutlookRegisterStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BaseStrategy for OutlookRegisterStrategy {
    async fn run(&self, adapter: &dyn BrowserAdapter, _account: &Account) -> Result<WorkerResult> {
        info!("Starting Outlook Registration Strategy...");

        let user_info = UserInfoGenerator::generate();
        let full_email = format!("{}@outlook.com", user_info.email_username);

        info!(
            "Generated User Info: {} {}, Email: {}",
            user_info.first_name, user_info.last_name, full_email
        );

        // 1. Navigate to Signup Page
        info!("Navigating to signup page...");
        adapter.navigate("https://signup.live.com/").await?;
        self.random_sleep().await;

        // Common selector for "Next" buttons
        let next_button_selector = NEXT_BUTTON_SELECTORS.join(", ");

        // Execute Steps
        self.handle_data_permission_modal(adapter).await?;
        self.fill_email(adapter, &full_email, &next_button_selector)
            .await?;
        self.fill_password(adapter, &user_info.password, &next_button_selector)
            .await?;
        self.fill_birth_date(adapter, &user_info, &next_button_selector)
            .await?;
        self.fill_name(adapter, &user_info, &next_button_selector)
            .await?;
        self.check_verification_and_errors(adapter).await?;

        // Construct result
        let mut data = serde_json::Map::new();
        data.insert(
            "email".to_string(),
            serde_json::Value::String(full_email.clone()),
        );
        data.insert(
            "password".to_string(),
            serde_json::Value::String(user_info.password.clone()),
        );
        data.insert(
            "first_name".to_string(),
            serde_json::Value::String(user_info.first_name),
        );
        data.insert(
            "last_name".to_string(),
            serde_json::Value::String(user_info.last_name),
        );
        data.insert(
            "birth_year".to_string(),
            serde_json::Value::Number(serde_json::Number::from(user_info.birth_year)),
        );

        Ok(WorkerResult {
            status: "处理中".to_string(),
            message: "已填写基础信息，准备进入验证环节".to_string(),
            data: Some(data),
        })
    }
}
