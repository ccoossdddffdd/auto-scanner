pub mod generator;

use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use generator::UserInfoGenerator;
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
}

impl Default for OutlookRegisterStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BaseStrategy for OutlookRegisterStrategy {
    async fn run(
        &self,
        adapter: &dyn BrowserAdapter,
        _account: &Account, // We generate a new account, ignoring the input one
    ) -> Result<WorkerResult> {
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

        // Check for "Personal Data Export Permission" modal
        // "同意并继续" button
        // It might be an input[type="submit"] or button
        // Based on typical MS flows, try to find a submit button with "同意" or "Agree" text if we are stuck.
        // Or simply wait for the email input. If email input is not found, maybe we are blocked by this modal.

        info!("Checking for data permission modal...");

        // Use is_visible to avoid waiting 30s timeout if the button doesn't exist.
        // We check for both Chinese and potentially other common texts if needed, but primarily Chinese for now.
        let agree_button_selector = "button:has-text('同意并继续'), button:has-text('Agree and continue'), input[value='同意并继续']";

        if let Ok(true) = adapter.is_visible(agree_button_selector).await {
            info!(
                "Data permission modal detected. Clicking '{}'...",
                agree_button_selector
            );
            if let Err(e) = adapter.click(agree_button_selector).await {
                warn!("Failed to click data permission button: {}", e);
            } else {
                info!("Clicked 'Agree and Continue' on data permission modal");
                self.random_sleep().await;
            }
        } else {
            info!("Data permission modal not found (or not visible), skipping...");
        }

        // Define generic selectors
        let next_button_selector = "#iSignupAction:visible, input[type='submit']:visible, button[type='submit']:visible, button:has-text('Next'):visible, button:has-text('下一步'):visible";

        // 2. Fill Email
        // Note: Selectors are based on typical Microsoft flows but might need adjustment if DOM changes
        // Common selectors: input[type="email"], #MemberName
        info!("Filling email...");
        let email_selector = "input[name=\"MemberName\"], input[type=\"email\"], input[name=\"email\"], input[name=\"loginfmt\"]";
        adapter
            .wait_for_element(email_selector)
            .await
            .context("Waiting for email input")?;
        self.type_with_delay(adapter, email_selector, &full_email)
            .await?;

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

        // 3. Fill Password
        // Common selectors: input[type="password"], #PasswordInput
        info!("Filling password...");
        let password_selector =
            "input[name=\"PasswordInput\"], input[type=\"password\"], input[name=\"passwd\"]";
        adapter
            .wait_for_element(password_selector)
            .await
            .context("Waiting for password input")?;
        self.type_with_delay(adapter, password_selector, &user_info.password)
            .await?;

        adapter
            .click(next_button_selector)
            .await
            .context("Clicking Next after password")?;
        self.random_sleep().await;

        // 4. Fill Country and Birth Date
        info!("Filling country and birth date...");

        // Fill Birth Year
        info!("Filling Birth Year: {}", user_info.birth_year);
        // Sometimes it's an input, sometimes a select. Based on codegen, it's input[name="BirthYear"]
        // Try filling it first
        let birth_year_selector =
            "input[name=\"BirthYear\"], input[id=\"BirthYear\"], [aria-label=\"Birth year\"]";
        match adapter
            .type_text(birth_year_selector, &user_info.birth_year.to_string())
            .await
        {
            Ok(_) => {}
            Err(_) => {
                // If input fails, try selecting if it's a dropdown (unlikely for year usually, but possible)
                // For now assuming input worked or failed critically.
                // Let's try select just in case? No, keep simple.
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

        // The month selector is a custom dropdown (button + listbox)
        // ID: #BirthMonthDropdown (button)
        // Option list has role="listbox"
        // Options have role="option"

        let month_val = user_info.birth_month.to_string();

        // 1. Click the dropdown button to open the list
        let birth_month_dropdown_selector =
            "#BirthMonthDropdown, [id=\"BirthMonthDropdown\"], [aria-label=\"Birth month\"]";
        if let Err(e) = adapter.click(birth_month_dropdown_selector).await {
            warn!("Failed to click BirthMonthDropdown: {}", e);
            // Fallback to old methods just in case
            if let Err(e2) = adapter
                .select_option("select[name=\"BirthMonth\"]", &month_val)
                .await
            {
                warn!("Fallback select month failed: {}", e2);
            }
        } else {
            // 2. Wait for list to appear (small delay is usually enough or we can wait for role=listbox)
            tokio::time::sleep(Duration::from_millis(500)).await;

            // 3. Click the option. Options usually contain the number or text.
            // Construct a selector for the option.
            // Since we don't know the exact text (e.g. "1" vs "1月" vs "January"),
            // we can try to click by text matching the number.
            // Or use the ID pattern seen in codegen: "fluent-option..." (unreliable).
            // Best bet: text match.
            // Note: user_info.birth_month is u32 (1-12).
            // Page might show "1月", "1", "January".
            // Let's try "text=X" first.

            // let option_selector = format!("text={}", month_val); // Try strict match first? "1" might match many things.
            // Better: role=option and text contains value.
            // Playwright selector: [role="option"]:has-text("1") -- but adapter only supports basic text= or css.
            // Our adapter's `click` uses `page.click_builder(selector)`. Playwright supports pseudo-selectors.
            // Let's try `[role="option"] >> text="1月"` or just `text="1月"`.
            // Since we saw "1月" in codegen output.

            // Let's try adding "月" suffix if it's likely Chinese locale, or just the number.
            // Microsoft often uses localized text.
            // Let's try multiple potential text matches.
            let month_texts = vec![
                format!("{}月", month_val), // 1月
                month_val.clone(),          // 1
                                            // Add English months if needed
            ];

            let mut month_clicked = false;
            for text in month_texts {
                // let sel = format!("text=\"{}\"", text); // Quote text for exact match or safety
                // Actually playwright `text=Foo` is contains match usually?
                // `text="Foo"` is exact match.
                // Let's try `text=1月` (unquoted) for loose match?
                let loose_sel = format!("text={}", text);

                if let Ok(_) = adapter.click(&loose_sel).await {
                    info!("Clicked month option: {}", text);
                    month_clicked = true;
                    break;
                }
            }

            if !month_clicked {
                warn!("Failed to click any month option for value: {}", month_val);
            }
        }

        // Fill Birth Day
        info!("Filling Birth Day: {}", user_info.birth_day);
        let day_val = user_info.birth_day.to_string();

        // 1. Click the dropdown button
        let birth_day_dropdown_selector =
            "#BirthDayDropdown, [id=\"BirthDayDropdown\"], [aria-label=\"Birth day\"]";
        if let Err(e) = adapter.click(birth_day_dropdown_selector).await {
            warn!("Failed to click BirthDayDropdown: {}", e);
            if let Err(e2) = adapter
                .select_option("select[name=\"BirthDay\"]", &day_val)
                .await
            {
                warn!("Fallback select day failed: {}", e2);
            }
        } else {
            tokio::time::sleep(Duration::from_millis(500)).await;

            // 2. Click option
            // let option_selector = format!("text={}", day_val);
            // Since day is just a number usually (1, 2, ... 31), even in Chinese locale it might be just number or "1日".
            // Codegen for day wasn't shown but likely similar.
            let day_texts = vec![
                day_val.clone(),          // 1
                format!("{}日", day_val), // 1日
            ];

            let mut day_clicked = false;
            for text in day_texts {
                let loose_sel = format!("text={}", text);
                if let Ok(_) = adapter.click(&loose_sel).await {
                    info!("Clicked day option: {}", text);
                    day_clicked = true;
                    break;
                }
            }

            if !day_clicked {
                warn!("Failed to click any day option for value: {}", day_val);
            }
        }

        // Click Next
        // Sometimes the "Next" button ID changes or we need to click text="Next" / "下一步"
        // Also, sometimes there is a delay before the button becomes clickable.
        if let Err(e) = adapter.click(next_button_selector).await {
            warn!("Click next button failed: {}", e);
            // Fallback just in case generic selector missed something specific
        }
        self.random_sleep().await;

        // 5. Fill Name
        // First Name: input[name="FirstName"]
        // Last Name: input[name="LastName"]
        info!("Filling name...");
        let first_name_selector =
            "input[name=\"FirstName\"], input[id=\"FirstName\"], input[id=\"firstNameInput\"], [aria-label=\"First name\"]";
        adapter
            .wait_for_element(first_name_selector)
            .await
            .context("Waiting for first name input")?;
        self.type_with_delay(adapter, first_name_selector, &user_info.first_name)
            .await?;

        let last_name_selector =
            "input[name=\"LastName\"], input[id=\"LastName\"], input[id=\"lastNameInput\"], [aria-label=\"Last name\"]";
        adapter
            .wait_for_element(last_name_selector)
            .await
            .context("Waiting for last name input")?;
        self.type_with_delay(adapter, last_name_selector, &user_info.last_name)
            .await?;

        adapter
            .click(next_button_selector)
            .await
            .context("Clicking Next after name")?;
        self.random_sleep().await;

        // Final verification check
        // We expect to land on the captcha page or verification page.
        // If we are still on a page with "FirstName" input or "LastName", it means failure.
        // Or check for error messages.

        let has_error = if let Ok(true) = adapter.is_visible(".alert-error").await {
            true
        } else if let Ok(true) = adapter.is_visible(".error").await {
            true
        } else if let Ok(true) = adapter.is_visible("div[aria-live='assertive']").await {
            // MS often uses aria-live for errors
            true
        } else {
            false
        };

        if has_error {
            // Try to grab error text
            let error_text = adapter
                .get_text("div[aria-live='assertive']")
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Registration flow failed with error: {}", error_text);
            return Err(anyhow::anyhow!("Registration flow failed: {}", error_text));
        }

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
            status: "处理中".to_string(), // Or "Success" if we consider this part success
            message: "已填写基础信息，准备进入验证环节".to_string(),
            data: Some(data),
        })
    }
}
