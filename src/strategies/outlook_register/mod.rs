pub mod generator;

use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use generator::UserInfoGenerator;
use rand::Rng;
use std::time::Duration;
use tracing::info;

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
        // This part is often complex with dropdowns. We will try best effort or just wait.
        // For automation robustness, we might just assert we reached this page.
        info!("Filling country and birth date...");

        // Country is usually pre-filled or requires complex select. We skip changing country for now.

        // Birth Month: select[name="BirthMonth"]
        // Birth Day: select[name="BirthDay"]
        // Birth Year: input[name="BirthYear"] or select

        // Handling selects with Playwright adapter usually requires `select_option`.
        // Our Adapter interface doesn't have `select_option` yet, only `click` and `type_text`.
        // We might need to implement `select_option` in Adapter or simulate clicks.
        // For now, let's assume we can type into Year and maybe click Month/Day.

        // Let's stop the automation flow here for "Phase 1" as requested: "实现 3-5s 的随机延迟" and "fake info".
        // Completing the full registration with captcha is out of scope for a basic strategy.

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
