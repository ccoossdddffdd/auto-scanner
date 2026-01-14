use crate::browser::{playwright_adapter::PlaywrightAdapter, BrowserAdapter};
use crate::models::{Account, WorkerResult};
use anyhow::{Context, Result};
use tracing::{error, info};

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

    let adapter_result: Result<Box<dyn BrowserAdapter>> = match backend.as_str() {
        "playwright" | "cdp" | "adspower" => match PlaywrightAdapter::new(&remote_url).await {
            Ok(adapter) => Ok(Box::new(adapter)),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to initialize Playwright adapter: {}",
                e
            )),
        },
        _ => Err(anyhow::anyhow!(
            "Unsupported backend in worker: {}",
            backend
        )),
    };

    let adapter = match adapter_result {
        Ok(a) => a,
        Err(e) => {
            error!("Browser initialization failed for {}: {}", username, e);
            let result = WorkerResult {
                status: "登录失败".to_string(),
                captcha: "未知".to_string(),
                two_fa: "未知".to_string(),
                message: format!("浏览器初始化失败: {}", e),
            };
            println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
            return Err(e);
        }
    };

    let result = match perform_login(adapter.as_ref(), &account, enable_screenshot).await {
        Ok(outcome) => {
            info!(
                "Login process finished for {}. Success: {}",
                username, outcome.status
            );
            outcome
        }
        Err(e) => {
            error!("Login failed for {}: {}", username, e);
            WorkerResult {
                status: "登录失败".to_string(),
                captcha: "未知".to_string(),
                two_fa: "未知".to_string(),
                message: format!("登录错误: {}", e),
            }
        }
    };

    println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
    info!("Worker completed for {}", username);
    Ok(())
}

async fn perform_login(
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
            fs::create_dir_all(screenshot_dir).context("Failed to create screenshot directory")?;
        }

        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let safe_username = account.username.replace('@', "_").replace('.', "_");
        let filename = format!("screenshot/login_{}_{}.png", safe_username, timestamp);

        adapter.take_screenshot(&filename).await?;
        info!("Screenshot saved to {}", filename);
    }

    Ok(result)
}
