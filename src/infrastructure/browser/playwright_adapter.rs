use super::{BrowserAdapter, BrowserCookie, BrowserError};
use async_trait::async_trait;
use playwright::api::{Browser, BrowserContext, BrowserType, Page};
use playwright::Playwright;
use tokio::time::{timeout, Duration};
use tracing::info;

pub struct PlaywrightAdapterBuilder {
    remote_url: String,
    connect_timeout: Duration,
}

impl PlaywrightAdapterBuilder {
    pub fn new(remote_url: impl Into<String>) -> Self {
        Self {
            remote_url: remote_url.into(),
            connect_timeout: Duration::from_secs(10),
        }
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub async fn build(self) -> Result<PlaywrightAdapter, BrowserError> {
        info!("正在初始化 Playwright...");
        let playwright = self.init_playwright().await?;
        let chromium = playwright.chromium();

        info!(
            "正在连接到浏览器 {} ({:?} 超时)...",
            self.remote_url, self.connect_timeout
        );

        let browser = self.connect_cdp(&chromium).await?;
        info!("成功连接到浏览器。");

        let context = Self::get_or_create_context(&browser).await?;
        let page = Self::get_or_create_page(&context).await?;

        Ok(PlaywrightAdapter {
            _playwright: playwright,
            _browser: browser,
            _context: context,
            page,
        })
    }

    async fn init_playwright(&self) -> Result<Playwright, BrowserError> {
        Playwright::initialize()
            .await
            .map_err(|e| BrowserError::ConnectionFailed(format!("初始化 Playwright 失败: {}", e)))
    }

    async fn connect_cdp(&self, chromium: &BrowserType) -> Result<Browser, BrowserError> {
        let result = timeout(
            self.connect_timeout,
            chromium
                .connect_over_cdp_builder(&self.remote_url)
                .connect_over_cdp(),
        )
        .await;

        match result {
            Ok(inner_result) => inner_result.map_err(|e| {
                let msg = Self::format_connection_error(&e);
                BrowserError::ConnectionFailed(msg)
            }),
            Err(_) => Err(BrowserError::ConnectionFailed(format!(
                "连接到 {} 超时 ({:?})",
                self.remote_url, self.connect_timeout
            ))),
        }
    }

    async fn get_or_create_context(browser: &Browser) -> Result<BrowserContext, BrowserError> {
        info!("正在获取浏览器上下文...");
        let contexts = browser
            .contexts()
            .map_err(|e| BrowserError::Other(format!("获取上下文失败: {}", e)))?;

        if let Some(ctx) = contexts.into_iter().next() {
            info!("使用现有上下文。");
            Ok(ctx)
        } else {
            info!("正在创建新上下文...");
            browser
                .context_builder()
                .build()
                .await
                .map_err(|e| BrowserError::Other(format!("创建上下文失败: {}", e)))
        }
    }

    async fn get_or_create_page(context: &BrowserContext) -> Result<Page, BrowserError> {
        info!("正在获取页面...");
        let pages = context
            .pages()
            .map_err(|e| BrowserError::Other(format!("获取页面失败: {}", e)))?;

        if let Some(p) = pages.into_iter().next() {
            info!("使用现有页面。");
            Ok(p)
        } else {
            info!("正在创建新页面...");
            context
                .new_page()
                .await
                .map_err(|e| BrowserError::Other(format!("创建新页面失败: {}", e)))
        }
    }

    fn format_connection_error(e: &playwright::Error) -> String {
        format!(
            "通过 CDP 连接失败: {}.\n\
             请确保 Chrome 已启用远程调试运行。\n\
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
        )
    }
}

pub struct PlaywrightAdapter {
    _playwright: Playwright,
    _browser: Browser,
    _context: BrowserContext,
    page: Page,
}

impl PlaywrightAdapter {
    pub async fn new(remote_url: &str) -> Result<Self, BrowserError> {
        PlaywrightAdapterBuilder::new(remote_url).build().await
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
                BrowserError::ElementNotFound(format!("填充元素 {} 失败: {}", selector, e))
            })?;
        Ok(())
    }

    async fn click(&self, selector: &str) -> Result<(), BrowserError> {
        self.page
            .click_builder(selector)
            .click()
            .await
            .map_err(|e| {
                BrowserError::ElementNotFound(format!("点击元素 {} 失败: {}", selector, e))
            })?;
        Ok(())
    }

    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError> {
        self.page
            .wait_for_selector_builder(selector)
            .wait_for_selector()
            .await
            .map_err(|e| BrowserError::Timeout(format!("等待 {} 超时: {}", selector, e)))?;
        Ok(())
    }

    async fn is_visible(&self, selector: &str) -> Result<bool, BrowserError> {
        use tracing::debug;

        // 首先尝试查询元素是否存在
        let element = match self.page.query_selector(selector).await {
            Ok(Some(el)) => el,
            Ok(None) => {
                debug!("未找到元素: {}", selector);
                return Ok(false);
            }
            Err(e) => {
                debug!("查询选择器 '{}' 错误: {}", selector, e);
                return Ok(false);
            }
        };

        // 检查元素是否可见
        match element.is_visible().await {
            Ok(visible) => {
                debug!("元素 '{}' 可见性: {}", selector, visible);
                Ok(visible)
            }
            Err(e) => {
                debug!("检查 '{}' 可见性失败: {}", selector, e);
                Ok(false)
            }
        }
    }

    async fn get_cookies(&self) -> Result<Vec<BrowserCookie>, BrowserError> {
        let cookies = self
            ._context
            .cookies(&[])
            .await
            .map_err(|e| BrowserError::Other(format!("获取 Cookies 失败: {}", e)))?;

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
        Err(BrowserError::Other("set_cookies 尚未完全实现".to_string()))
    }

    async fn take_screenshot(&self, path: &str) -> Result<(), BrowserError> {
        self.page
            .screenshot_builder()
            .path(std::path::PathBuf::from(path))
            .screenshot()
            .await
            .map_err(|e| BrowserError::Other(format!("截图失败: {}", e)))?;
        Ok(())
    }

    async fn get_current_url(&self) -> Result<String, BrowserError> {
        self.page
            .url()
            .map_err(|e| BrowserError::Other(format!("获取当前 URL 失败: {}", e)))
    }

    async fn get_text(&self, selector: &str) -> Result<String, BrowserError> {
        let element = self
            .page
            .query_selector(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("查询失败: {}", e)))?
            .ok_or_else(|| BrowserError::ElementNotFound(selector.to_string()))?;

        element
            .text_content()
            .await
            .map_err(|e| BrowserError::Other(format!("获取文本内容失败: {}", e)))?
            .ok_or_else(|| BrowserError::Other("元素没有文本内容".to_string()))
    }

    async fn get_all_text(&self, selector: &str) -> Result<Vec<String>, BrowserError> {
        let elements = self
            .page
            .query_selector_all(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("查询失败: {}", e)))?;

        let mut texts = Vec::new();
        for element in elements {
            if let Ok(Some(text)) = element.text_content().await {
                if !text.trim().is_empty() {
                    texts.push(text);
                }
            }
        }

        Ok(texts)
    }

    async fn select_option(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        self.page
            .select_option_builder(selector)
            .add_value(value.to_string())
            .select_option()
            .await
            .map(|_: Vec<String>| ()) // Discard the return value
            .map_err(|e| {
                BrowserError::ElementNotFound(format!("选择下拉选项失败 {}: {}", selector, e))
            })?;
        Ok(())
    }

    async fn get_content(&self) -> Result<String, BrowserError> {
        self.page
            .content()
            .await
            .map_err(|e| BrowserError::Other(format!("获取页面内容失败: {}", e)))
    }
}
