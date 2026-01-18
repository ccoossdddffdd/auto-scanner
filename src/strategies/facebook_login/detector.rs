use super::constants::FacebookConfig;
use crate::infrastructure::browser::BrowserAdapter;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    Success,
    Captcha,
    TwoFactor,
    WrongPassword,
    UserNotFound,
    AccountLocked,
    Failed,
}

pub struct LoginStatusDetector<'a> {
    config: &'a FacebookConfig,
}

impl<'a> LoginStatusDetector<'a> {
    pub fn new(config: &'a FacebookConfig) -> Self {
        Self { config }
    }

    pub async fn detect(&self, adapter: &dyn BrowserAdapter) -> LoginStatus {
        let current_url = adapter.get_current_url().await.unwrap_or_default();
        info!("Current URL during detection: {}", current_url);

        if self.check_success(adapter, &current_url).await {
            return LoginStatus::Success;
        }

        // Parallel checks for failure conditions
        let (has_captcha, has_2fa, wrong_password, user_not_found, account_locked) = tokio::join!(
            self.check_captcha(adapter, &current_url),
            self.check_2fa(adapter, &current_url),
            self.check_wrong_password(adapter, &current_url),
            self.check_user_not_found(adapter, &current_url),
            self.check_account_locked(adapter, &current_url),
        );

        if has_captcha {
            LoginStatus::Captcha
        } else if has_2fa {
            LoginStatus::TwoFactor
        } else if wrong_password {
            LoginStatus::WrongPassword
        } else if user_not_found {
            LoginStatus::UserNotFound
        } else if account_locked {
            LoginStatus::AccountLocked
        } else {
            LoginStatus::Failed
        }
    }

    async fn check_success(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("/login") || url.contains("/checkpoint") {
            return false;
        }

        let email_visible = adapter
            .is_visible(&self.config.selectors.login_form.email)
            .await
            .unwrap_or(false);
        let pass_visible = adapter
            .is_visible(&self.config.selectors.login_form.pass)
            .await
            .unwrap_or(false);

        if email_visible && pass_visible {
            return false;
        }

        if self
            .check_any_visible(adapter, &self.config.selectors.indicators.profile)
            .await
        {
            return true;
        }

        if self
            .check_any_visible(adapter, &self.config.selectors.indicators.elements)
            .await
        {
            return true;
        }

        false
    }

    async fn check_captcha(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("captcha")
            || self
                .config
                .urls
                .checkpoints
                .iter()
                .any(|id| url.contains("checkpoint") && url.contains(id))
        {
            return true;
        }

        if self
            .check_any_visible(adapter, &self.config.selectors.captcha)
            .await
        {
            return true;
        }

        self.check_keywords_in_containers(adapter, &self.config.keywords.captcha)
            .await
    }

    async fn check_2fa(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("two_step_verification") {
            return true;
        }
        adapter
            .is_visible(&self.config.selectors.two_fa_input)
            .await
            .unwrap_or(false)
    }

    async fn check_wrong_password(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("/login") && url.contains("error") {
            info!("URL indicates login error (possibly wrong password)");
        }
        // Note: URL check alone might be weak for wrong password, usually there is text.
        // But we keep existing logic + text check.

        if self
            .check_keywords_in_containers(adapter, &self.config.keywords.wrong_password)
            .await
        {
            return true;
        }

        // Fallback: Check body text for wrong password keywords
        if let Ok(body_text) = adapter.get_text("body").await {
            let text_lower = body_text.to_lowercase();
            for keyword in &self.config.keywords.wrong_password {
                if text_lower.contains(&keyword.to_lowercase()) {
                    info!("Matched wrong_password keyword '{}' in body text", keyword);
                    return true;
                }
            }
        }

        false
    }

    async fn check_user_not_found(&self, adapter: &dyn BrowserAdapter, _url: &str) -> bool {
        if self
            .check_keywords_in_containers(adapter, &self.config.keywords.user_not_found)
            .await
        {
            return true;
        }

        // Fallback: Check body text for specific long error messages
        // This is safe because the keywords are very specific
        if let Ok(body_text) = adapter.get_text("body").await {
            info!(
                "Body text (first 200 chars): {}",
                body_text.chars().take(200).collect::<String>()
            );
            let text_lower = body_text.to_lowercase();
            for keyword in &self.config.keywords.user_not_found {
                if text_lower.contains(&keyword.to_lowercase()) {
                    info!("Matched user_not_found keyword '{}' in body text", keyword);
                    return true;
                }
            }
        }

        false
    }

    async fn check_account_locked(&self, adapter: &dyn BrowserAdapter, url: &str) -> bool {
        if url.contains("/checkpoint") && !url.contains("two_step") && !url.contains("2fa") {
            return true;
        }

        if url.contains("locked") || url.contains("disabled") || url.contains("suspended") {
            return true;
        }

        if self
            .check_any_visible(adapter, &self.config.selectors.locked_indicators)
            .await
        {
            return true;
        }

        self.check_keywords_in_containers(adapter, &self.config.keywords.account_locked)
            .await
    }

    // --- Helper Methods ---

    async fn check_any_visible(&self, adapter: &dyn BrowserAdapter, selectors: &[String]) -> bool {
        for selector in selectors {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    return true;
                }
            }
        }
        false
    }

    async fn check_keywords_in_containers(
        &self,
        adapter: &dyn BrowserAdapter,
        keywords: &[String],
    ) -> bool {
        for selector in &self.config.selectors.error_containers {
            if let Ok(visible) = adapter.is_visible(selector).await {
                if visible {
                    if let Ok(text) = adapter.get_text(selector).await {
                        info!("Found text in error container '{}': {}", selector, text);
                        let text_lower = text.to_lowercase();
                        for keyword in keywords {
                            if text_lower.contains(&keyword.to_lowercase()) {
                                info!("Matched keyword '{}'", keyword);
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
