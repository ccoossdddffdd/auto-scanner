use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::path::Path;
use tracing::info;

/// SMTP邮件发送器
pub struct EmailSender {
    smtp_server: String,
    smtp_port: u16,
    username: String,
    password: String,
}

impl EmailSender {
    /// 创建新的EmailSender实例
    pub fn new(smtp_server: String, smtp_port: u16, username: String, password: String) -> Self {
        Self {
            smtp_server,
            smtp_port,
            username,
            password,
        }
    }

    /// 发送简单文本邮件
    pub async fn send_text_email(&self, to: &str, subject: &str, body: &str) -> Result<()> {
        info!("Sending text email to {}: {}", to, subject);

        let email = Message::builder()
            .from(self.username.parse()?)
            .to(to.parse()?)
            .subject(subject)
            .body(body.to_string())?;

        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let mailer = SmtpTransport::builder_dangerous(&self.smtp_server)
            .port(self.smtp_port)
            .credentials(creds)
            .build();

        mailer.send(&email).context("Failed to send text email")?;

        info!("Text email sent successfully to {}", to);
        Ok(())
    }

    /// 发送带附件的邮件
    pub async fn send_email_with_attachment(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        attachment_path: &Path,
    ) -> Result<()> {
        info!(
            "Sending email with attachment to {}: {:?}",
            to, attachment_path
        );

        // 读取附件文件
        let attachment_data = tokio::fs::read(attachment_path)
            .await
            .context("Failed to read attachment file")?;

        // 获取文件名和MIME类型
        let filename = attachment_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid attachment filename")?;

        let content_type = mime_guess::from_path(attachment_path)
            .first_or_octet_stream()
            .to_string();

        // 构建带附件的邮件
        let email = Message::builder()
            .from(self.username.parse()?)
            .to(to.parse()?)
            .subject(subject)
            .multipart(
                lettre::message::MultiPart::mixed()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(body.to_string()),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::parse(&content_type)?)
                            .header(lettre::message::header::ContentDisposition::attachment(
                                filename,
                            ))
                            .body(attachment_data),
                    ),
            )?;

        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let mailer = SmtpTransport::builder_dangerous(&self.smtp_server)
            .port(self.smtp_port)
            .credentials(creds)
            .build();

        mailer
            .send(&email)
            .context("Failed to send email with attachment")?;

        info!("Email with attachment sent successfully to {}", to);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_sender_creation() {
        let sender = EmailSender::new(
            "smtp.example.com".to_string(),
            587,
            "test@example.com".to_string(),
            "password".to_string(),
        );

        assert_eq!(sender.smtp_server, "smtp.example.com");
        assert_eq!(sender.smtp_port, 587);
        assert_eq!(sender.username, "test@example.com");
    }
}
