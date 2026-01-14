use super::{BrowserAdapter, BrowserCookie, BrowserError};
use async_trait::async_trait;
use playwright::api::{Browser, BrowserContext, Page};
use playwright::Playwright;

pub struct PlaywrightAdapter {
    _playwright: Playwright,
    _browser: Browser,
    _context: BrowserContext,
    page: Page,
}

impl PlaywrightAdapter {
    pub async fn new(remote_url: &str) -> Result<Self, BrowserError> {
        let playwright = Playwright::initialize().await.map_err(|e| {
            BrowserError::ConnectionFailed(format!("Failed to initialize Playwright: {}", e))
        })?;

        let chromium = playwright.chromium();

        let browser = chromium
            .connect_over_cdp_builder(remote_url)
            .connect_over_cdp()
            .await
            .map_err(|e| {
                let msg = format!(
                    "Failed to connect over CDP: {}.\n\
                     Ensure Chrome is running with remote debugging enabled.\n\
                     \n\
                     Mac:\n\
                     /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-debug\n\
                     \n\
                     Windows:\n\
                     start chrome.exe --remote-debugging-port=9222 --user-data-dir=C:\\tmp\\chrome-debug\n\
                     \n\
                     Linux:\n\
                     google-chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-debug\n",
                    e
                );
                BrowserError::ConnectionFailed(msg)
            })?;

        let contexts = browser
            .contexts()
            .map_err(|e| BrowserError::Other(format!("Failed to get contexts: {}", e)))?;

        let context = contexts.into_iter().next();
        let context = if let Some(ctx) = context {
            ctx
        } else {
            browser
                .context_builder()
                .build()
                .await
                .map_err(|e| BrowserError::Other(format!("Failed to create context: {}", e)))?
        };

        let pages = context
            .pages()
            .map_err(|e| BrowserError::Other(format!("Failed to get pages: {}", e)))?;

        let page = if let Some(p) = pages.into_iter().next() {
            p
        } else {
            context
                .new_page()
                .await
                .map_err(|e| BrowserError::Other(format!("Failed to create new page: {}", e)))?
        };

        Ok(Self {
            _playwright: playwright,
            _browser: browser,
            _context: context,
            page,
        })
    }
}

#[async_trait]
impl BrowserAdapter for PlaywrightAdapter {
    async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        self.page
            .goto_builder(url)
            .goto()
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;
        Ok(())
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        self.page
            .fill_builder(selector, text)
            .fill()
            .await
            .map_err(|e| {
                BrowserError::ElementNotFound(format!("Failed to fill element {}: {}", selector, e))
            })?;
        Ok(())
    }

    async fn click(&self, selector: &str) -> Result<(), BrowserError> {
        self.page
            .click_builder(selector)
            .click()
            .await
            .map_err(|e| {
                BrowserError::ElementNotFound(format!(
                    "Failed to click element {}: {}",
                    selector, e
                ))
            })?;
        Ok(())
    }

    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError> {
        self.page
            .wait_for_selector_builder(selector)
            .wait_for_selector()
            .await
            .map_err(|e| {
                BrowserError::Timeout(format!("Timeout waiting for {}: {}", selector, e))
            })?;
        Ok(())
    }

    async fn is_visible(&self, selector: &str) -> Result<bool, BrowserError> {
        // Manual visibility check using evaluate
        let script = format!(
            "document.querySelector('{}') && document.querySelector('{}').offsetParent !== null",
            selector, selector
        );

        let val: bool = self
            .page
            .evaluate(&script, ())
            .await
            .map_err(|e| BrowserError::Other(format!("Failed to check visibility: {}", e)))?;

        Ok(val)
    }

    async fn get_cookies(&self) -> Result<Vec<BrowserCookie>, BrowserError> {
        let cookies = self
            ._context
            .cookies(&[])
            .await
            .map_err(|e| BrowserError::Other(format!("Failed to get cookies: {}", e)))?;

        Ok(cookies
            .into_iter()
            .map(|c| BrowserCookie {
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                expires: c.expires,
                http_only: c.http_only,
                secure: c.secure,
                same_site: c.same_site.map(|s| format!("{:?}", s)),
            })
            .collect())
    }

    async fn set_cookies(&self, _cookies: &[BrowserCookie]) -> Result<(), BrowserError> {
        Err(BrowserError::Other(
            "set_cookies not fully implemented".to_string(),
        ))
    }

    async fn take_screenshot(&self, path: &str) -> Result<(), BrowserError> {
        self.page
            .screenshot_builder()
            .path(std::path::PathBuf::from(path))
            .screenshot()
            .await
            .map_err(|e| BrowserError::Other(format!("Failed to take screenshot: {}", e)))?;
        Ok(())
    }
}
