use anyhow::{Context, Result};
use crate::browser::{playwright_adapter::PlaywrightAdapter, BrowserAdapter};
use crate::models::Account;
use crate::database::Database;
use tracing::{info, error};

pub struct LoginOutcome {
    pub success: bool,
    pub captcha: Option<String>,
    pub two_fa: Option<String>,
}

pub async fn run(username: String, password: String, remote_url: String, backend: String) -> Result<()> {
    info!("Worker started for account: {}", username);
    
    let account = Account::new(username.clone(), password);

    let adapter: Box<dyn BrowserAdapter> = match backend.as_str() {
        "playwright" | "cdp" => {
            Box::new(PlaywrightAdapter::new(&remote_url).await
                .context("Failed to initialize Playwright adapter in worker")?)
        },
        _ => anyhow::bail!("Unsupported backend in worker: {}", backend),
    };

    let db = Database::new("auto-scanner.db").await?;

    match perform_login(adapter.as_ref(), &account).await {
        Ok(outcome) => {
            info!("Login process finished for {}. Success: {}", username, outcome.success);
            db.update_login_result(&username, outcome.success, outcome.captcha.as_deref(), outcome.two_fa.as_deref()).await?;
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

async fn perform_login(adapter: &dyn BrowserAdapter, account: &Account) -> Result<LoginOutcome> {
    info!("Navigating to Facebook...");
    adapter.navigate("https://www.facebook.com").await?;
    
    info!("Waiting for email input...");
    adapter.wait_for_element("input[name='email']").await?;
    
    info!("Typing credentials...");
    adapter.type_text("input[name='email']", &account.username).await?;
    adapter.type_text("input[name='pass']", &account.password).await?;
    
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
    if adapter.is_visible("a[aria-label='Facebook']").await.unwrap_or(false) || 
       adapter.is_visible("div[role='navigation']").await.unwrap_or(false) {
        info!("Login detected as successful");
        outcome.success = true;
    } else if adapter.is_visible("input[name='captcha_response']").await.unwrap_or(false) {
        info!("Captcha detected");
        outcome.captcha = Some("Detected".to_string());
    } else if adapter.is_visible("input[name='approvals_code']").await.unwrap_or(false) {
        info!("2FA detected");
        outcome.two_fa = Some("Detected".to_string());
    }

    info!("Taking screenshot...");
    let filename = format!("login_{}.png", account.username.replace('@', "_").replace('.', "_"));
    adapter.take_screenshot(&filename).await?;
    
    Ok(outcome)
}
