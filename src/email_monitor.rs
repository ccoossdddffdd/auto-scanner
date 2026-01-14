use crate::email_sender::EmailSender;
use crate::file_tracker::FileTracker;
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{info, warn};

/// 邮件配置
#[derive(Clone, Debug)]
pub struct EmailConfig {
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval: u64,
    pub processed_folder: String,
    pub subject_filter: String,
    pub input_dir: std::path::PathBuf,
    pub doned_dir: std::path::PathBuf,
}

impl EmailConfig {
    /// 从环境变量创建配置
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            imap_server: std::env::var("EMAIL_IMAP_SERVER")
                .unwrap_or_else(|_| "outlook.office365.com".to_string()),
            imap_port: std::env::var("EMAIL_IMAP_PORT")
                .unwrap_or_else(|_| "993".to_string())
                .parse()
                .context("Invalid EMAIL_IMAP_PORT")?,
            smtp_server: std::env::var("EMAIL_SMTP_SERVER")
                .unwrap_or_else(|_| "smtp.office365.com".to_string()),
            smtp_port: std::env::var("EMAIL_SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .context("Invalid EMAIL_SMTP_PORT")?,
            username: std::env::var("EMAIL_USERNAME").context("EMAIL_USERNAME not set")?,
            password: std::env::var("EMAIL_PASSWORD").context("EMAIL_PASSWORD not set")?,
            poll_interval: std::env::var("EMAIL_POLL_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .context("Invalid EMAIL_POLL_INTERVAL")?,
            processed_folder: std::env::var("EMAIL_PROCESSED_FOLDER")
                .unwrap_or_else(|_| "已处理".to_string()),
            subject_filter: std::env::var("EMAIL_SUBJECT_FILTER")
                .unwrap_or_else(|_| "FB账号".to_string()),
            input_dir: std::env::var("INPUT_DIR")
                .unwrap_or_else(|_| "input".to_string())
                .into(),
            doned_dir: std::env::var("DONED_DIR")
                .unwrap_or_else(|_| "input/doned".to_string())
                .into(),
        })
    }
}

/// 邮件监控器
pub struct EmailMonitor {
    config: EmailConfig,
    file_tracker: Arc<FileTracker>,
    email_sender: EmailSender,
}

impl EmailMonitor {
    /// 创建新的邮件监控器
    pub fn new(config: EmailConfig, file_tracker: Arc<FileTracker>) -> Result<EmailMonitor> {
        let email_sender = EmailSender::new(
            config.smtp_server.clone(),
            config.smtp_port,
            config.username.clone(),
            config.password.clone(),
        );

        Ok(Self {
            config,
            file_tracker,
            email_sender,
        })
    }

    /// 启动邮件监控
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting email monitoring...");
        info!(
            "IMAP Server: {}:{}",
            self.config.imap_server, self.config.imap_port
        );
        info!(
            "SMTP Server: {}:{}",
            self.config.smtp_server, self.config.smtp_port
        );
        info!("Poll interval: {} seconds", self.config.poll_interval);
        info!("Subject filter: {}", self.config.subject_filter);
        info!("Input directory: {:?}", self.config.input_dir);
        info!("Processed folder: {}", self.config.processed_folder);

        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(self.config.poll_interval));

        loop {
            interval.tick().await;

            info!("Checking for new emails...");

            // IMAP连接和邮件处理将在下一阶段实现
            // 当前阶段只确保结构可以编译通过

            // 定期清理旧记录
            if let Err(e) = self.file_tracker.cleanup_old_records() {
                warn!("Failed to cleanup old records: {}", e);
            }
        }
    }

    /// 发送"已处理"成功通知
    pub async fn send_success_notification(
        &self,
        to: &str,
        processed_file: std::path::PathBuf,
    ) -> Result<()> {
        info!("Sending '已处理' success notification to {}", to);

        let body = "已处理".to_string();

        self.email_sender
            .send_email_with_attachment(to, "Re: 已处理", &body, &processed_file)
            .await
            .context("Failed to send success notification")?;

        Ok(())
    }

    /// 发送"处理失败"通知
    pub async fn send_failure_notification(
        &self,
        to: &str,
        error_message: &str,
        processed_file: Option<std::path::PathBuf>,
    ) -> Result<()> {
        info!("Sending '处理失败' failure notification to {}", to);

        let body = format!("处理失败\n\n错误信息: {}", error_message);

        if let Some(ref file_path) = processed_file {
            self.email_sender
                .send_email_with_attachment(to, "Re: 处理失败", &body, file_path)
                .await
                .context("Failed to send failure notification with attachment")?;
        } else {
            self.email_sender
                .send_text_email(to, "Re: 处理失败", &body)
                .await
                .context("Failed to send failure notification")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_from_env() {
        std::env::set_var("EMAIL_USERNAME", "test@example.com");
        std::env::set_var("EMAIL_PASSWORD", "password");

        let config = EmailConfig::from_env();
        assert!(config.is_ok());

        if let Ok(config) = config {
            assert_eq!(config.username, "test@example.com");
            assert_eq!(config.password, "password");
            assert_eq!(config.poll_interval, 60);
        }
    }
}
