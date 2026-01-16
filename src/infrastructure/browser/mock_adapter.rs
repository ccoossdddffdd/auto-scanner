use super::{BrowserAdapter, BrowserCookie, BrowserError};
use async_trait::async_trait;
use tracing::info;

#[derive(Default)]
pub struct MockBrowserAdapter;

impl MockBrowserAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BrowserAdapter for MockBrowserAdapter {
    async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        info!("[Mock] Navigating to {}", url);
        Ok(())
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        info!("[Mock] Typing '{}' into {}", text, selector);
        Ok(())
    }

    async fn click(&self, selector: &str) -> Result<(), BrowserError> {
        info!("[Mock] Clicking {}", selector);
        Ok(())
    }

    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError> {
        info!("[Mock] Waiting for element {}", selector);
        Ok(())
    }

    async fn is_visible(&self, selector: &str) -> Result<bool, BrowserError> {
        info!("[Mock] Checking visibility of {}", selector);

        // 模拟登录表单不存在（登录成功后不可见）
        if selector == "input[name='email']" || selector == "input[name='pass']" {
            return Ok(false);
        }

        // 模拟登录成功后的元素可见
        if selector.contains("Your profile")
            || selector.contains("Account")
            || selector.contains("个人主页")
            || selector == "div[role='main']"
            || selector == "input[type='search']"
            || selector.contains("Search")
        {
            return Ok(true);
        }

        // 其他元素默认不可见
        Ok(false)
    }

    async fn get_cookies(&self) -> Result<Vec<BrowserCookie>, BrowserError> {
        info!("[Mock] Getting cookies");
        Ok(vec![])
    }

    async fn set_cookies(&self, _cookies: &[BrowserCookie]) -> Result<(), BrowserError> {
        info!("[Mock] Setting cookies");
        Ok(())
    }

    async fn take_screenshot(&self, path: &str) -> Result<(), BrowserError> {
        info!("[Mock] Taking screenshot to {}", path);
        // Create a dummy file
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| BrowserError::Other(e.to_string()))?;
        }

        let mut file = File::create(path)
            .await
            .map_err(|e| BrowserError::Other(e.to_string()))?;
        file.write_all(b"mock screenshot")
            .await
            .map_err(|e| BrowserError::Other(e.to_string()))?;

        Ok(())
    }

    async fn get_current_url(&self) -> Result<String, BrowserError> {
        info!("[Mock] Getting current URL");
        Ok("https://www.facebook.com/".to_string())
    }

    async fn get_text(&self, selector: &str) -> Result<String, BrowserError> {
        info!("[Mock] Getting text from {}", selector);
        Ok("123 friends".to_string())
    }

    async fn get_all_text(&self, selector: &str) -> Result<Vec<String>, BrowserError> {
        info!("[Mock] Getting all text from {}", selector);
        Ok(vec!["5 friends".to_string(), "123 other text".to_string()])
    }

    async fn select_option(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        info!("[Mock] Selecting option '{}' in {}", value, selector);
        Ok(())
    }
}
