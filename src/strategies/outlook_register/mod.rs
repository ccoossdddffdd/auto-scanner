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
            (rng.random_range(3..=5), rng.random_range(0..1000))
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

        // Let's try to detect if we are on the modal.
        // A common way is to check for the "Agree and Continue" button.
        // Selector for "同意并继续" button might be dynamic, but let's try a few common ones or check page content.
        // For now, let's just blindly try to click it if email input is not found immediately?
        // No, that's slow.

        // Let's try to wait for email input with a shorter timeout, if fail, try to handle modal.
        // But our adapter wait_for_element has default timeout.

        // Better approach: Check if the modal exists.
        // Since we don't have `is_visible` in our simple adapter, we might need to rely on `wait_for_element` failure or just blindly click if present.
        // However, standard `wait_for_element` throws error.

        // Let's assume the modal MIGHT appear.
        // We can try to click the "Agree" button.
        // Based on the text "同意并继续", we can try an XPath or CSS.
        // Since our adapter mainly supports CSS selectors.
        // Let's try to find the button by ID or Class if possible.
        // In MS pages, primary action buttons often have id `iSignupAction` or `id__0` etc.
        // But `iSignupAction` is also the "Next" button.

        // If the modal is blocking, it likely has a specific button.
        // Let's try to click a button that contains "同意" if possible, but CSS :has-text is not standard (Playwright supports it).
        // Since we use Playwright adapter, we can use Playwright-specific selectors!
        // `text="同意并继续"` or `:text("同意并继续")`

        info!("Checking for data permission modal...");
        // Try to click "同意并继续" if it exists. We use a short timeout trick if possible,
        // but our adapter doesn't expose timeout control per call easily.
        // We can just try to click it. If it fails (not found), we ignore the error and proceed to email.
        // But `click` usually waits.

        // Let's try to find it first.
        // Using Playwright selector for text match
        let agree_button_selector = "text=同意并继续";
        // We attempt to click it. If it's not there, it might throw.
        // We should wrap this in a "try-catch" block, but in Rust it's Result.
        // However, if we wait for it and it's not there, we waste time.
        // Ideally we check if it exists.

        // For now, let's assume if we see it, we click it.
        // BUT, since we don't know if it appears, maybe we just proceed to email.
        // The user says "did not enter email flow", implying we are stuck.
        // So likely the modal IS there.
        // So let's try to click it.

        match adapter.click(agree_button_selector).await {
            Ok(_) => {
                info!("Clicked 'Agree and Continue' on data permission modal");
                self.random_sleep().await;
            }
            Err(_) => {
                info!("Data permission modal not found or not clickable, proceeding...");
            }
        }

        // 2. Fill Email
        // Note: Selectors are based on typical Microsoft flows but might need adjustment if DOM changes
        // Common selectors: input[type="email"], #MemberName
        info!("Filling email...");
        adapter
            .wait_for_element("input[name=\"MemberName\"]")
            .await
            .context("Waiting for email input")?;
        self.type_with_delay(adapter, "input[name=\"MemberName\"]", &full_email)
            .await?;

        adapter
            .click("#iSignupAction")
            .await
            .context("Clicking Next after email")?;
        self.random_sleep().await;

        // 3. Fill Password
        // Common selectors: input[type="password"], #PasswordInput
        info!("Filling password...");
        adapter
            .wait_for_element("input[name=\"PasswordInput\"]")
            .await
            .context("Waiting for password input")?;
        self.type_with_delay(
            adapter,
            "input[name=\"PasswordInput\"]",
            &user_info.password,
        )
        .await?;

        adapter
            .click("#iSignupAction")
            .await
            .context("Clicking Next after password")?;
        self.random_sleep().await;

        // 4. Fill Country and Birth Date
        info!("Filling country and birth date...");

        // Wait for country selector to ensure page loaded
        adapter
            .wait_for_element("select[name=\"Country\"]")
            .await
            .context("Waiting for country select")?;

        // Fill Birth Year
        info!("Filling Birth Year: {}", user_info.birth_year);
        // Sometimes it's an input, sometimes a select. Based on codegen, it's input[name="BirthYear"]
        // Try filling it first
        match adapter
            .type_text(
                "input[name=\"BirthYear\"]",
                &user_info.birth_year.to_string(),
            )
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
        self.random_sleep().await;

        // Fill Birth Month
        info!("Filling Birth Month: {}", user_info.birth_month);

        // The month selector is a custom dropdown (button + listbox)
        // ID: #BirthMonthDropdown (button)
        // Option list has role="listbox"
        // Options have role="option"

        let month_val = user_info.birth_month.to_string();

        // 1. Click the dropdown button to open the list
        if let Err(e) = adapter.click("#BirthMonthDropdown").await {
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

        self.random_sleep().await;

        // Fill Birth Day
        info!("Filling Birth Day: {}", user_info.birth_day);
        let day_val = user_info.birth_day.to_string();

        // 1. Click the dropdown button
        if let Err(e) = adapter.click("#BirthDayDropdown").await {
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

        self.random_sleep().await;

        // Click Next
        // Sometimes the "Next" button ID changes or we need to click text="Next" / "下一步"
        // Also, sometimes there is a delay before the button becomes clickable.
        if let Err(e) = adapter.click("#iSignupAction").await {
            warn!("Click #iSignupAction failed: {}, trying text match", e);
            if let Err(e2) = adapter.click("text=下一步").await {
                warn!("Click '下一步' failed: {}, trying 'Next'", e2);
                adapter
                    .click("text=Next")
                    .await
                    .context("Clicking Next after birth date")?;
            }
        }
        self.random_sleep().await;

        // 5. Fill Name
        // First Name: input[name="FirstName"]
        // Last Name: input[name="LastName"]
        info!("Filling name...");
        adapter
            .wait_for_element("input[name=\"FirstName\"]")
            .await
            .context("Waiting for first name input")?;
        self.type_with_delay(adapter, "input[name=\"FirstName\"]", &user_info.first_name)
            .await?;

        adapter
            .wait_for_element("input[name=\"LastName\"]")
            .await
            .context("Waiting for last name input")?;
        self.type_with_delay(adapter, "input[name=\"LastName\"]", &user_info.last_name)
            .await?;

        adapter
            .click("#iSignupAction")
            .await
            .context("Clicking Next after name")?;
        self.random_sleep().await;

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
