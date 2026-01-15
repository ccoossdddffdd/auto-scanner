use super::LoginStrategy;
use crate::core::models::{Account, WorkerResult};
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Local;
use std::fs;
use std::path::Path;
use tracing::info;

#[derive(Default)]
pub struct FacebookLoginStrategy;

impl FacebookLoginStrategy {
    pub fn new() -> Self {
        Self
    }
}

/// 登录状态枚举
enum LoginStatus {
    Success,
    Captcha,
    TwoFactor,
    WrongPassword,
    AccountLocked,
    Failed,
}

/// 登录结果检测器
struct LoginResultDetector;

impl LoginResultDetector {
    async fn detect_status(adapter: &dyn BrowserAdapter) -> LoginStatus {
        // 并行检测多个状态
        let (is_success, has_captcha, has_2fa, wrong_password, account_locked) = tokio::join!(
            Self::check_success(adapter),
            Self::check_captcha(adapter),
            Self::check_2fa(adapter),
            Self::check_wrong_password(adapter),
            Self::check_account_locked(adapter),
        );

        if is_success {
            LoginStatus::Success
        } else if has_captcha {
            LoginStatus::Captcha
        } else if has_2fa {
            LoginStatus::TwoFactor
        } else if account_locked {
            LoginStatus::AccountLocked
        } else if wrong_password {
            LoginStatus::WrongPassword
        } else {
            LoginStatus::Failed
        }
    }

    async fn check_success(adapter: &dyn BrowserAdapter) -> bool {
        // 首先检查 URL 是否表明登录失败
        if let Ok(url) = adapter.get_current_url().await {
            info!("Current URL: {}", url);
            // 如果 URL 包含这些路径，说明未成功登录
            if url.contains("/login") || url.contains("/checkpoint") {
                info!("URL contains /login or /checkpoint, login failed");
                return false;
            }
        }

        // 检查是否还存在登录表单（如果存在说明未登录）
        let email_visible = adapter
            .is_visible("input[name='email']")
            .await
            .unwrap_or(false);
        let pass_visible = adapter
            .is_visible("input[name='pass']")
            .await
            .unwrap_or(false);

        info!(
            "Login form visibility - email: {}, password: {}",
            email_visible, pass_visible
        );

        if email_visible && pass_visible {
            info!("Login form still visible, login failed");
            return false;
        }

        // 检查登录成功后才有的元素
        let success_indicators = [
            // 个人资料/账号菜单按钮
            "[aria-label*='Your profile']",
            "[aria-label*='Account']",
            "[aria-label*='个人主页']",
            // 创建帖子区域
            "[role='dialog']",
            "div[role='main']",
            // 搜索框
            "input[type='search']",
            "input[aria-label*='Search']",
        ];

        for selector in &success_indicators {
            if let Ok(visible) = adapter.is_visible(selector).await {
                info!("Checking success indicator '{}': {}", selector, visible);
                if visible {
                    info!("Found success indicator: {}", selector);
                    return true;
                }
            }
        }

        info!("No success indicators found, login failed");
        false
    }

    async fn check_captcha(adapter: &dyn BrowserAdapter) -> bool {
        adapter
            .is_visible("input[name='captcha_response']")
            .await
            .unwrap_or(false)
    }

    async fn check_2fa(adapter: &dyn BrowserAdapter) -> bool {
        // 首先检查 URL 是否包含 two_step_verification
        if let Ok(url) = adapter.get_current_url().await {
            if url.contains("two_step_verification") {
                return true;
            }
        }

        // 检查页面元素
        adapter
            .is_visible("input[name='approvals_code']")
            .await
            .unwrap_or(false)
    }

    async fn check_wrong_password(adapter: &dyn BrowserAdapter) -> bool {
        // 方法1: 检查 URL 是否包含密码错误标识
        if let Ok(url) = adapter.get_current_url().await {
            if url.contains("/login") && url.contains("error") {
                info!("URL indicates login error (possibly wrong password)");
            }
        }

        // 方法2: 检查特定的错误元素
        let error_selectors = [
            "div[role='alert']",
            "div._9ay7",
            "div[data-testid='royal_login_error']",
            "#error_box",
        ];

        for selector in &error_selectors {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Found error message for password check: {}", text);

                        // 多语言密码错误关键词
                        let password_keywords = [
                            // 英语
                            "password",
                            "incorrect",
                            "wrong",
                            // 中文
                            "密码",
                            "错误",
                            "不正确",
                            // 西班牙语
                            "contraseña",
                            "incorrecta",
                            // 法语
                            "mot de passe",
                            "incorrect",
                            // 德语
                            "passwort",
                            "falsch",
                            // 葡萄牙语
                            "senha",
                            "incorreta",
                            // 日语
                            "パスワード",
                            // 韩语
                            "비밀번호",
                            // 越南语
                            "mật khẩu",
                            "sai",
                            // 印尼语
                            "kata sandi",
                            "salah",
                            // 泰语
                            "รหัสผ่าน",
                            // 阿拉伯语
                            "كلمة المرور",
                        ];

                        for keyword in &password_keywords {
                            if text_lower.contains(keyword) {
                                info!("Detected wrong password via keyword: {}", keyword);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    async fn check_account_locked(adapter: &dyn BrowserAdapter) -> bool {
        // 方法1: 检查 URL 模式
        if let Ok(url) = adapter.get_current_url().await {
            // checkpoint 通常表示账号被限制
            if url.contains("/checkpoint") {
                info!("URL contains /checkpoint, account may be locked");

                // 进一步检查是否真的是锁定而不是其他 checkpoint
                // 如果不是 2FA checkpoint，很可能是账号锁定
                if !url.contains("two_step") && !url.contains("2fa") {
                    return true;
                }
            }

            if url.contains("locked") || url.contains("disabled") || url.contains("suspended") {
                info!("URL indicates account locked/disabled/suspended");
                return true;
            }
        }

        // 方法2: 检查特定的锁定页面元素
        let locked_indicators = [
            // 账号锁定页面的特征元素
            "div[data-testid='account_locked']",
            "div[data-testid='checkpoint_locked']",
            // 通用的限制/审核提示
            "button[name='submit[Continue]']", // checkpoint 页面通常有这个
        ];

        for selector in &locked_indicators {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                info!("Found account locked indicator element: {}", selector);
            }
        }

        // 方法3: 检查错误消息文本
        let error_selectors = [
            "div[role='alert']",
            "div._9ay7",
            "#error_box",
            "div[data-testid='error_message']",
        ];

        for selector in &error_selectors {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Found error message for locked check: {}", text);

                        // 多语言账号锁定关键词
                        let locked_keywords = [
                            // 英语
                            "locked",
                            "disabled",
                            "suspended",
                            "restricted",
                            "temporarily",
                            "violat",
                            // 中文
                            "锁定",
                            "封禁",
                            "停用",
                            "限制",
                            "暂时",
                            "违反",
                            // 西班牙语
                            "bloqueada",
                            "desactivada",
                            "suspendida",
                            // 法语
                            "bloqué",
                            "désactivé",
                            "suspendu",
                            // 德语
                            "gesperrt",
                            "deaktiviert",
                            // 葡萄牙语
                            "bloqueada",
                            "desativada",
                            // 日语
                            "ロック",
                            "無効",
                            // 韩语
                            "잠금",
                            "비활성",
                            // 越南语
                            "bị khóa",
                            "vô hiệu",
                            // 印尼语
                            "dikunci",
                            "dinonaktifkan",
                            // 泰语
                            "ล็อค",
                            "ปิดการใช้งาน",
                        ];

                        for keyword in &locked_keywords {
                            if text_lower.contains(keyword) {
                                info!("Detected account locked via keyword: {}", keyword);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

#[async_trait]
impl LoginStrategy for FacebookLoginStrategy {
    async fn login(
        &self,
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

        // 检测登录结果
        let status = LoginResultDetector::detect_status(adapter).await;
        let mut result = WorkerResult {
            status: "登录失败".to_string(),
            captcha: "不需要".to_string(),
            two_fa: "不需要".to_string(),
            message: "未知失败".to_string(),
            friends_count: None,
        };

        match status {
            LoginStatus::Success => {
                info!("Login detected as successful");
                result.status = "登录成功".to_string();
                result.message = "成功".to_string();

                // 获取好友数量
                if let Ok(count) = self.get_friends_count(adapter).await {
                    result.friends_count = Some(count);
                    info!("Friends count: {}", count);
                }
            }
            LoginStatus::Captcha => {
                info!("Captcha detected");
                result.captcha = "需要".to_string();
                result.message = "检测到验证码".to_string();
            }
            LoginStatus::TwoFactor => {
                info!("2FA detected");
                result.two_fa = "需要".to_string();
                result.message = "检测到 2FA".to_string();
            }
            LoginStatus::AccountLocked => {
                info!("Account locked detected");
                result.status = "登录失败".to_string();
                result.message = "账号已锁定".to_string();
            }
            LoginStatus::WrongPassword => {
                info!("Wrong password detected");
                result.status = "登录失败".to_string();
                result.message = "密码错误".to_string();
            }
            LoginStatus::Failed => {
                // 保持默认值
            }
        }

        if enable_screenshot {
            self.take_screenshot(adapter, &account.username).await?;
        }

        Ok(result)
    }
}

impl FacebookLoginStrategy {
    async fn get_friends_count(&self, adapter: &dyn BrowserAdapter) -> Result<u32> {
        info!("Getting friends count...");

        // 导航到好友页面
        adapter
            .navigate("https://www.facebook.com/me/friends")
            .await
            .context("Failed to navigate to friends page")?;

        // 等待页面加载
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // 获取当前 URL 确认导航成功
        if let Ok(url) = adapter.get_current_url().await {
            info!("Navigated to friends page, current URL: {}", url);
        }

        // 尝试多个选择器获取好友数量
        let selectors = [
            // 好友页面标题中的数字（最准确）
            "h2",
            "div[role='main'] h2",
            // 好友列表区域的 span
            "div[role='main'] span",
            // 侧边栏好友链接
            "a[href*='/friends/'] span",
        ];

        for selector in &selectors {
            // 获取所有匹配元素的文本
            if let Ok(texts) = adapter.get_all_text(selector).await {
                info!(
                    "Found {} elements for selector '{}', checking each...",
                    texts.len(),
                    selector
                );

                for (index, text) in texts.iter().enumerate() {
                    let trimmed = text.trim();
                    info!("Element {} from '{}': '{}'", index + 1, selector, trimmed);

                    // 尝试从文本中提取数字
                    if let Some(count) = Self::extract_number_from_text(trimmed) {
                        // 过滤掉明显不合理的数字
                        if count > 0 && count < 10000 {
                            info!(
                                "✓ Extracted friends count {} from selector '{}', element {}",
                                count,
                                selector,
                                index + 1
                            );
                            return Ok(count);
                        } else {
                            info!(
                                "✗ Skipping unreasonable count: {} from selector '{}', element {}",
                                count,
                                selector,
                                index + 1
                            );
                        }
                    }
                }
            }
        }

        info!("Could not extract friends count from any selector, returning 0");
        Ok(0)
    }

    fn extract_number_from_text(text: &str) -> Option<u32> {
        // 提取文本中的数字，支持逗号分隔的数字如 "1,234"
        let cleaned = text.replace([',', ' ', '\n', '\t'], "");
        let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.is_empty() {
            return None;
        }

        digits.parse::<u32>().ok()
    }

    async fn take_screenshot(&self, adapter: &dyn BrowserAdapter, username: &str) -> Result<()> {
        info!("Taking screenshot...");
        let screenshot_dir = Path::new("screenshot");
        if !screenshot_dir.exists() {
            fs::create_dir_all(screenshot_dir).context("Failed to create screenshot directory")?;
        }

        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let safe_username = username.replace(['@', '.'], "_");
        let filename = format!("screenshot/login_{}_{}.png", safe_username, timestamp);

        adapter.take_screenshot(&filename).await?;
        info!("Screenshot saved to {}", filename);

        Ok(())
    }
}
