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

        // 4. Fill Name
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

        // 5. Fill Country and Birth Date
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
        // Month is usually a select. Codegen failed to find `select[name="BirthMonth"]`.
        // It might be `select[id="BirthMonth"]` or similar.
        // Let's try multiple selectors or use the new `select_option` method.
        // Common MS selector: `select[aria-label="Birth month"]` or similar.
        // Let's try generic approach: `select` inside birth date container?
        // Let's try `#BirthMonth` again with `select_option`.

        let month_val = user_info.birth_month.to_string();
        // Note: value might need to be "1" or "01" or "January".
        // Usually numeric value "1" works for MS.

        if let Err(e) = adapter.select_option("#BirthMonth", &month_val).await {
            warn!("Failed to select month with #BirthMonth: {}", e);
            // Try name
            if let Err(e2) = adapter
                .select_option("select[name=\"BirthMonth\"]", &month_val)
                .await
            {
                warn!("Failed to select month with name: {}", e2);
            }
        }
        self.random_sleep().await;

        // Fill Birth Day
        info!("Filling Birth Day: {}", user_info.birth_day);
        let day_val = user_info.birth_day.to_string();
        if let Err(e) = adapter.select_option("#BirthDay", &day_val).await {
            warn!("Failed to select day with #BirthDay: {}", e);
            if let Err(e2) = adapter
                .select_option("select[name=\"BirthDay\"]", &day_val)
                .await
            {
                warn!("Failed to select day with name: {}", e2);
            }
        }
        self.random_sleep().await;

        // Click Next
        adapter
            .click("#iSignupAction")
            .await
            .context("Clicking Next after birth date")?;
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
