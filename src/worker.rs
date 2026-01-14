use crate::browser::{playwright_adapter::PlaywrightAdapter, BrowserAdapter};
use crate::database::Database;
use crate::models::Account;
use anyhow::{Context, Result};
use tracing::{error, info};

pub struct LoginOutcome {
    pub success: bool,
    pub captcha: Option<String>,
    pub two_fa: Option<String>,
}

use chrono::Local;
use std::fs;
use std::path::Path;

pub async fn run(
    username: String,
    password: String,
    remote_url: String,
    backend: String,
    enable_screenshot: bool,
) -> Result<()> {
    info!(
        "Worker started for account: {}. Screenshot enabled: {}",
        username, enable_screenshot
    );

    let account = Account::new(username.clone(), password);

    // Initialize DB first to ensure we can log failures even if browser init fails
    let db = Database::new("auto-scanner.db").await?;

    let adapter_result: Result<Box<dyn BrowserAdapter>> = match backend.as_str() {
        "playwright" | "cdp" | "adspower" => {
            match PlaywrightAdapter::new(&remote_url).await {
                Ok(adapter) => Ok(Box::new(adapter)),
                Err(e) => Err(anyhow::anyhow!("Failed to initialize Playwright adapter: {}", e)),
            }
        },
        _ => Err(anyhow::anyhow!("Unsupported backend in worker: {}", backend)),
    };

    let adapter = match adapter_result {
        Ok(a) => a,
        Err(e) => {
            error!("Browser initialization failed for {}: {}", username, e);
            // Log failure to DB? currently update_login_result assumes login attempted.
            // We can treat this as a failed login attempt with no special status.
            db.update_login_result(&username, false, None, None).await?;
            return Err(e);
        }
    };

    match perform_login(adapter.as_ref(), &account, enable_screenshot).await {
        Ok(outcome) => {
            info!(
                "Login process finished for {}. Success: {}",
                username, outcome.success
            );
            db.update_login_result(
                &username,
                outcome.success,
                outcome.captcha.as_deref(),
                outcome.two_fa.as_deref(),
            )
            .await?;
        }
        Err(e) => {
            error!("Login failed for {}: {}", username, e);
            db.update_login_result(&username, false, None, None).await?;
            anyhow::bail!("Login execution failed: {}", e);
        }
    }

    info!("Worker completed for {}", username);
    Ok(())
}

async fn perform_login(
    adapter: &dyn BrowserAdapter,
    account: &Account,
    enable_screenshot: bool,
) -> Result<LoginOutcome> {
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

    // Check for success or specific states
    let mut outcome = LoginOutcome {
        success: false,
        captcha: None,
        two_fa: None,
    };

    // Placeholder logic for detecting states
    // In a real scenario, we would check for specific selectors or URL changes
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
        outcome.success = true;
    } else if adapter
        .is_visible("input[name='captcha_response']")
        .await
        .unwrap_or(false)
    {
        info!("Captcha detected");
        outcome.captcha = Some("Detected".to_string());
    } else if adapter
        .is_visible("input[name='approvals_code']")
        .await
        .unwrap_or(false)
    {
        info!("2FA detected");
        outcome.two_fa = Some("Detected".to_string());
    }

    if enable_screenshot {
        info!("Taking screenshot...");
        let screenshot_dir = Path::new("screenshot");
        if !screenshot_dir.exists() {
            fs::create_dir_all(screenshot_dir).context("Failed to create screenshot directory")?;
        }

        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let safe_username = account.username.replace('@', "_").replace('.', "_");
        let filename = format!("screenshot/login_{}_{}.png", safe_username, timestamp);

        adapter.take_screenshot(&filename).await?;
        info!("Screenshot saved to {}", filename);
    }

    Ok(outcome)
}
