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
        // Simulate success scenario
        if selector == "a[aria-label='Facebook']" {
            return Ok(true);
        }
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
}
