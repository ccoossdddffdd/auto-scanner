use crate::core::models::WorkerResult;
use crate::infrastructure::browser::BrowserAdapter;
use anyhow::{Context, Result};
use tracing::info;

/// 移动版 Facebook 登录状态枚举
#[derive(Debug)]
pub enum MobileLoginStatus {
    Success,
    Captcha,
    TwoFactor,
    WrongPassword,
    AccountLocked,
    Failed,
}

/// 移动版 Facebook 登录结果检测器
pub struct MobileLoginDetector;

impl MobileLoginDetector {
    pub async fn detect_status(adapter: &dyn BrowserAdapter) -> MobileLoginStatus {
        // 并行检测多个状态
        let (is_success, has_captcha, has_2fa, account_locked, wrong_password) = tokio::join!(
            Self::check_success(adapter),
            Self::check_captcha(adapter),
            Self::check_2fa(adapter),
            Self::check_account_locked(adapter),
            Self::check_wrong_password(adapter),
        );

        if is_success {
            MobileLoginStatus::Success
        } else if has_captcha {
            MobileLoginStatus::Captcha
        } else if has_2fa {
            MobileLoginStatus::TwoFactor
        } else if account_locked {
            MobileLoginStatus::AccountLocked
        } else if wrong_password {
            MobileLoginStatus::WrongPassword
        } else {
            MobileLoginStatus::Failed
        }
    }

    async fn check_success(adapter: &dyn BrowserAdapter) -> bool {
        // 检查 URL 是否表明登录失败
        if let Ok(url) = adapter.get_current_url().await {
            info!("Mobile - Current URL: {}", url);

            // 如果 URL 包含这些路径，说明未成功登录
            if url.contains("/login") || url.contains("/checkpoint") {
                info!("Mobile - URL contains /login or /checkpoint, login failed");
                return false;
            }
        }

        // 移动版检查是否还存在登录表单
        let email_visible = adapter
            .is_visible("input[name='email']")
            .await
            .unwrap_or(false);
        let pass_visible = adapter
            .is_visible("input[name='pass']")
            .await
            .unwrap_or(false);

        info!(
            "Mobile - Login form visibility - email: {}, password: {}",
            email_visible, pass_visible
        );

        if email_visible && pass_visible {
            info!("Mobile - Login form still visible, login failed");
            return false;
        }

        // 移动版登录成功的特征元素
        let success_indicators = [
            // 移动版首页元素
            "div#MComposer",
            "div[role='feed']",
            // 移动版导航
            "div#mJewelNav",
            // 移动版菜单
            "div[data-sigil='MTopNavMenu']",
            // 通用的主内容区域
            "div[role='main']",
            // 移动版的搜索框
            "form[action='/search/']",
        ];

        for selector in &success_indicators {
            if let Ok(visible) = adapter.is_visible(selector).await {
                info!(
                    "Mobile - Checking success indicator '{}': {}",
                    selector, visible
                );
                if visible {
                    info!("Mobile - Found success indicator: {}", selector);
                    return true;
                }
            }
        }

        info!("Mobile - No success indicators found, login failed");
        false
    }

    async fn check_captcha(adapter: &dyn BrowserAdapter) -> bool {
        // URL 检测
        if let Ok(url) = adapter.get_current_url().await {
            if url.contains("captcha")
                || url.contains("checkpoint") && url.contains("828281030927956")
            {
                info!("Mobile - URL indicates captcha required");
                return true;
            }
        }

        // 移动版验证码元素
        let captcha_selectors = [
            "input[name='captcha_response']",
            "iframe[src*='recaptcha']",
            "iframe[src*='hcaptcha']",
            "div[data-sigil='captcha']",
            "img[alt*='captcha']",
        ];

        for selector in &captcha_selectors {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                info!("Mobile - Found captcha element: {}", selector);
                return true;
            }
        }

        false
    }

    async fn check_2fa(adapter: &dyn BrowserAdapter) -> bool {
        // URL 检测
        if let Ok(url) = adapter.get_current_url().await {
            if url.contains("two_step_verification")
                || url.contains("checkpoint") && url.contains("2fa")
            {
                info!("Mobile - URL indicates 2FA required");
                return true;
            }
        }

        // 移动版 2FA 元素
        let twofa_selectors = [
            "input[name='approvals_code']",
            "input[aria-label*='code']",
            "div[data-sigil='m-login-two-step']",
        ];

        for selector in &twofa_selectors {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                info!("Mobile - Found 2FA element: {}", selector);
                return true;
            }
        }

        false
    }

    async fn check_wrong_password(adapter: &dyn BrowserAdapter) -> bool {
        // 移动版错误提示元素
        let error_selectors = [
            "div[data-sigil='m-login-notice']",
            "div[role='alert']",
            "div.login_error_box",
            "#error_box",
        ];

        for selector in &error_selectors {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Mobile - Found error message: {}", text);

                        // 密码错误关键词（多语言）
                        let password_keywords = [
                            "password",
                            "密码",
                            "incorrect",
                            "wrong",
                            "错误",
                            "不正确",
                            "contraseña",
                            "incorrecta",
                            "mot de passe",
                            "passwort",
                            "falsch",
                            "senha",
                            "incorreta",
                            "パスワード",
                            "ログイン情報",
                            "誤り",
                            "비밀번호",
                            "mật khẩu",
                            "sai",
                        ];

                        for keyword in &password_keywords {
                            if text_lower.contains(keyword) {
                                info!("Mobile - Detected wrong password via keyword: {}", keyword);
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
        // URL 检测
        if let Ok(url) = adapter.get_current_url().await {
            if url.contains("/checkpoint") && !url.contains("two_step") {
                info!("Mobile - URL indicates account may be locked");
                return true;
            }
            if url.contains("locked") || url.contains("disabled") || url.contains("suspended") {
                info!("Mobile - URL indicates account locked/disabled");
                return true;
            }
        }

        // 移动版账号锁定元素
        let locked_indicators = [
            "div[data-sigil='m-account-locked']",
            "button[name='submit[Continue]']",
        ];

        for selector in &locked_indicators {
            if adapter.is_visible(selector).await.unwrap_or(false) {
                info!("Mobile - Found account locked indicator: {}", selector);
            }
        }

        // 检查错误消息
        let error_selectors = [
            "div[data-sigil='m-login-notice']",
            "div[role='alert']",
            "#error_box",
        ];

        for selector in &error_selectors {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        let text_lower = text.to_lowercase();
                        info!("Mobile - Checking error for locked: {}", text);

                        // 账号锁定关键词
                        let locked_keywords = [
                            "locked",
                            "disabled",
                            "suspended",
                            "restricted",
                            "temporarily",
                            "锁定",
                            "封禁",
                            "停用",
                            "限制",
                            "暂时",
                            "bloqueada",
                            "desactivada",
                            "bloqué",
                            "désactivé",
                            "gesperrt",
                            "bloqueada",
                            "ロック",
                            "무효",
                            "잠금",
                            "bị khóa",
                            "dikunci",
                            "ล็อค",
                        ];

                        for keyword in &locked_keywords {
                            if text_lower.contains(keyword) {
                                info!("Mobile - Detected account locked via keyword: {}", keyword);
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

/// 移动版好友数量获取
pub struct MobileFriendsCounter;

impl MobileFriendsCounter {
    pub async fn get_count(adapter: &dyn BrowserAdapter) -> Result<u32> {
        info!("Mobile - Getting friends count...");

        // 导航到移动版好友页面
        adapter
            .navigate("https://m.facebook.com/friends/center/friends/")
            .await
            .context("Failed to navigate to mobile friends page")?;

        // 等待页面加载
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        if let Ok(url) = adapter.get_current_url().await {
            info!("Mobile - Navigated to friends page: {}", url);
        }

        // 移动版好友数量选择器
        let selectors = [
            // 移动版好友数量标题
            "div[data-sigil='m-friends-header'] span",
            "h3",
            "div[role='main'] h3",
            // 通用的文本元素
            "span",
        ];

        for selector in &selectors {
            if let Ok(texts) = adapter.get_all_text(selector).await {
                info!(
                    "Mobile - Found {} elements for selector '{}'",
                    texts.len(),
                    selector
                );

                for (index, text) in texts.iter().enumerate() {
                    let trimmed = text.trim();
                    info!("Mobile - Element {}: '{}'", index + 1, trimmed);

                    if let Some(count) = Self::extract_number(trimmed) {
                        if count > 0 && count < 10000 {
                            info!("Mobile - Extracted friends count: {}", count);
                            return Ok(count);
                        }
                    }
                }
            }
        }

        info!("Mobile - Could not extract friends count, returning 0");
        Ok(0)
    }

    fn extract_number(text: &str) -> Option<u32> {
        let cleaned = text.replace([',', ' ', '\n', '\t'], "");
        let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.is_empty() {
            return None;
        }

        digits.parse::<u32>().ok()
    }
}

/// 根据结果生成 WorkerResult
pub fn create_result(status: MobileLoginStatus, friends_count: Option<u32>) -> WorkerResult {
    let mut result = WorkerResult {
        status: "登录失败".to_string(),
        captcha: "不需要".to_string(),
        two_fa: "不需要".to_string(),
        message: "未知失败".to_string(),
        friends_count: None,
    };

    match status {
        MobileLoginStatus::Success => {
            info!("Mobile - Login successful");
            result.status = "登录成功".to_string();
            result.message = "成功(移动版)".to_string();
            result.friends_count = friends_count;
        }
        MobileLoginStatus::Captcha => {
            info!("Mobile - Captcha detected");
            result.captcha = "需要".to_string();
            result.message = "检测到验证码(移动版)".to_string();
        }
        MobileLoginStatus::TwoFactor => {
            info!("Mobile - 2FA detected");
            result.two_fa = "需要".to_string();
            result.message = "检测到 2FA(移动版)".to_string();
        }
        MobileLoginStatus::AccountLocked => {
            info!("Mobile - Account locked");
            result.message = "账号已锁定(移动版)".to_string();
        }
        MobileLoginStatus::WrongPassword => {
            info!("Mobile - Wrong password");
            result.message = "密码错误(移动版)".to_string();
        }
        MobileLoginStatus::Failed => {
            info!("Mobile - Login failed");
            result.message = "登录失败(移动版)".to_string();
        }
    }

    result
}
