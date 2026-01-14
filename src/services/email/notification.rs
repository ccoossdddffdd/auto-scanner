use crate::services::email::sender::EmailSender;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// 邮件通知器
pub struct EmailNotifier {
    sender: EmailSender,
}

impl EmailNotifier {
    pub fn new(smtp_server: String, smtp_port: u16, username: String, password: String) -> Self {
        Self {
            sender: EmailSender::new(smtp_server, smtp_port, username, password),
        }
    }

    /// 发送成功通知
    pub async fn send_success_notification(&self, to: &str, processed_file: PathBuf) -> Result<()> {
        info!(
            "Sending success notification to {} for file: {:?}",
            to, processed_file
        );

        let subject = "处理成功";
        let body = format!("文件已成功处理: {:?}", processed_file);

        self.sender
            .send_text_email(to, subject, &body)
            .await
            .context("Failed to send success notification")?;

        Ok(())
    }

    /// 发送失败通知
    pub async fn send_failure_notification(
        &self,
        to: &str,
        error_message: &str,
        processed_file: Option<PathBuf>,
    ) -> Result<()> {
        info!(
            "Sending failure notification to {} with error: {}",
            to, error_message
        );

        let subject = "处理失败";
        let body = if let Some(file) = processed_file {
            format!("文件处理失败: {:?}\n错误: {}", file, error_message)
        } else {
            format!("处理失败\n错误: {}", error_message)
        };

        self.sender
            .send_text_email(to, subject, &body)
            .await
            .context("Failed to send failure notification")?;

        Ok(())
    }

    /// 发送已收到确认
    pub async fn send_received_confirmation(&self, to: &str) -> Result<()> {
        info!("Sending '已收到' confirmation to {}", to);

        self.sender
            .send_text_email(to, "Re: 已收到", "已收到")
            .await
            .context("Failed to send received confirmation")?;

        Ok(())
    }
}
