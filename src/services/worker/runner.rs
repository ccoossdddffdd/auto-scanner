use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::{
    mock_adapter::MockBrowserAdapter, playwright_adapter::PlaywrightAdapter, BrowserAdapter,
};
use crate::strategies::{facebook::FacebookLoginStrategy, LoginStrategy};
use anyhow::Result;
use tracing::{error, info};

pub async fn run(
    username: String,
    password: String,
    remote_url: String,
    backend: String,
    strategy_name: String,
) -> Result<()> {
    info!(
        "Worker started for account: {}. Strategy: {}",
        username, strategy_name
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
        "mock" => Ok(Box::new(MockBrowserAdapter::new())),
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
                friends_count: None,
            };
            println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
            return Err(e);
        }
    };

    let strategy: Box<dyn LoginStrategy> = match strategy_name.as_str() {
        "facebook" => Box::new(FacebookLoginStrategy::new()),
        _ => {
            return Err(anyhow::anyhow!("Unsupported strategy: {}", strategy_name));
        }
    };

    let result = match strategy.login(adapter.as_ref(), &account).await {
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
                friends_count: None,
            }
        }
    };

    println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
    info!("Worker completed for {}", username);
    Ok(())
}
