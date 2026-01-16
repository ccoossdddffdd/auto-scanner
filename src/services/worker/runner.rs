use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::{
    mock_adapter::MockBrowserAdapter, playwright_adapter::PlaywrightAdapter, BrowserAdapter,
};
use crate::strategies::{facebook_login::FacebookLoginStrategy, BaseStrategy};
use anyhow::Result;
use tracing::{error, info};

pub async fn run(
    username: String,
    password: String,
    remote_url: String,
    backend: String,
    strategy_name: String,
) -> Result<()> {
    info!("Worker 已启动。账号: {}, 策略: {}", username, strategy_name);

    let account = Account::new(username.clone(), password);

    let adapter_result: Result<Box<dyn BrowserAdapter>> = match backend.as_str() {
        "playwright" | "cdp" | "adspower" => match PlaywrightAdapter::new(&remote_url).await {
            Ok(adapter) => Ok(Box::new(adapter)),
            Err(e) => Err(anyhow::anyhow!("初始化 Playwright 适配器失败: {}", e)),
        },
        "mock" => Ok(Box::new(MockBrowserAdapter::new())),
        _ => Err(anyhow::anyhow!("Worker 不支持的后端: {}", backend)),
    };

    let adapter = match adapter_result {
        Ok(a) => a,
        Err(e) => {
            error!("{} 浏览器初始化失败: {}", username, e);
            let result = WorkerResult {
                status: "初始化失败".to_string(),
                message: format!("浏览器初始化失败: {}", e),
                data: None,
            };
            println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
            return Err(e);
        }
    };

    let strategy: Box<dyn BaseStrategy> = match strategy_name.as_str() {
        "facebook_login" => Box::new(FacebookLoginStrategy::new()),
        _ => {
            return Err(anyhow::anyhow!("不支持的策略: {}", strategy_name));
        }
    };

    let result = match strategy.run(adapter.as_ref(), &account).await {
        Ok(outcome) => {
            info!(
                "Strategy execution finished for {}. Success: {}",
                username, outcome.status
            );
            outcome
        }
        Err(e) => {
            error!("Strategy execution failed for {}: {}", username, e);
            WorkerResult {
                status: "执行失败".to_string(),
                message: format!("执行错误: {}", e),
                data: None,
            }
        }
    };

    println!("RESULT_JSON:{}", serde_json::to_string(&result)?);
    info!("{} Worker 执行完成", username);
    Ok(())
}
