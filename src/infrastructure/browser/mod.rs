use async_trait::async_trait;
use thiserror::Error;

pub mod playwright_adapter;
pub mod mock_adapter;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Navigation failed: {0}")]
    NavigationFailed(String),
    #[error("Element not found: {0}")]
    ElementNotFound(String),
    #[error("Timeout waiting for element: {0}")]
    Timeout(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Browser error: {0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<f64>,
    pub http_only: Option<bool>,
    pub secure: Option<bool>,
    pub same_site: Option<String>,
}

#[async_trait]
pub trait BrowserAdapter: Send + Sync {
    /// Navigate to a specific URL
    async fn navigate(&self, url: &str) -> Result<(), BrowserError>;

    /// Type text into an element identified by selector
    async fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError>;

    /// Click an element identified by selector
    async fn click(&self, selector: &str) -> Result<(), BrowserError>;

    /// Wait for an element to appear in the DOM
    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError>;

    /// Check if an element is visible
    async fn is_visible(&self, selector: &str) -> Result<bool, BrowserError>;

    /// Get all cookies
    async fn get_cookies(&self) -> Result<Vec<BrowserCookie>, BrowserError>;

    /// Set cookies
    async fn set_cookies(&self, cookies: &[BrowserCookie]) -> Result<(), BrowserError>;

    /// Take a screenshot and save it to the specified path
    async fn take_screenshot(&self, path: &str) -> Result<(), BrowserError>;
}
