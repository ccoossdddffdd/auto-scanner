use crate::infrastructure::imap::{ImapClient, ImapSession};
use crate::services::email::sender::EmailSender;
use crate::services::email::tracker::FileTracker;
use anyhow::{Context, Result};
use chrono::Local;
use futures::StreamExt;
use mail_parser::{Message, MessageParser, MimeHeaders};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

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
    pub input_dir: PathBuf,
    pub doned_dir: PathBuf,
}

impl EmailConfig {
    /// 从.env文件创建配置
    pub fn from_env() -> Result<Self> {
        // 加载.env文件
        dotenv::dotenv().ok();

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
            username: std::env::var("EMAIL_USERNAME")
                .context("EMAIL_USERNAME not set in .env file")?,
            password: std::env::var("EMAIL_PASSWORD")
                .context("EMAIL_PASSWORD not set in .env file")?,
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

/// 附件信息
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub size: usize,
}

/// 邮件解析器
struct EmailParser;

impl EmailParser {
    fn parse_from_address(parsed: &Message) -> String {
        parsed
            .from()
            .and_then(|l| l.first())
            .and_then(|a| a.address.as_ref())
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    fn parse_subject(parsed: &Message) -> String {
        parsed.subject().unwrap_or("").to_string()
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

    /// 获取文件追踪器
    pub fn get_file_tracker(&self) -> Arc<FileTracker> {
        self.file_tracker.clone()
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

        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(self.config.poll_interval));

        loop {
            interval.tick().await;

            if let Err(e) = self.check_and_process_emails().await {
                error!("Email processing error: {}", e);
            }

            // 定期清理旧记录
            if let Err(e) = self.file_tracker.cleanup_old_records() {
                warn!("Failed to cleanup old records: {}", e);
            }
        }
    }

    /// 检查并处理新邮件
    async fn check_and_process_emails(&self) -> Result<()> {
        let imap_client = ImapClient::new(
            self.config.imap_server.clone(),
            self.config.imap_port,
            self.config.username.clone(),
            self.config.password.clone(),
        );
        let mut session = imap_client.connect().await?;

        // 选择收件箱
        let inbox = session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        info!("Mailbox selected: {:?}", inbox);

        // 搜索未读邮件
        let search_result = session
            .search("UNSEEN")
            .await
            .context("Failed to search for unread emails")?;

        let uid_set: Vec<u32> = search_result.iter().copied().collect();

        if uid_set.is_empty() {
            info!("No new unread emails found");
            session
                .logout()
                .await
                .context("Failed to logout from IMAP")?;
            return Ok(());
        }

        info!("Found {} unread emails", uid_set.len());

        // 处理每封邮件
        for uid in &uid_set {
            if let Err(e) = self.fetch_and_process_email(*uid, &mut session).await {
                error!("Failed to process email UID {}: {}", uid, e);
            }
        }

        session
            .logout()
            .await
            .context("Failed to logout from IMAP")?;

        Ok(())
    }

    /// 获取并处理单个邮件
    async fn fetch_and_process_email(&self, uid: u32, session: &mut ImapSession) -> Result<()> {
        let email_data = self.fetch_email_data(uid, session).await?;

        if email_data.is_none() {
            warn!("No data returned for email UID {}", uid);
            return Ok(());
        }

        let msg = email_data.unwrap();
        if let Some(raw) = msg.body() {
            let parsed = MessageParser::default()
                .parse(raw)
                .context("Failed to parse email")?;

            self.process_email_workflow(uid, &parsed, session).await?;
        } else {
            warn!("Unexpected fetch result type for email UID {}", uid);
        }

        Ok(())
    }

    /// 获取邮件数据
    async fn fetch_email_data(
        &self,
        uid: u32,
        session: &mut ImapSession,
    ) -> Result<Option<async_imap::types::Fetch>> {
        let mut fetch_stream = session
            .fetch(uid.to_string(), "RFC822")
            .await
            .context("Failed to fetch email")?;

        let mut data = None;
        if let Some(msg) = fetch_stream.next().await {
            data = Some(msg.context("Failed to read fetch result")?);
        }

        Ok(data)
    }

    /// 处理邮件工作流
    async fn process_email_workflow(
        &self,
        uid: u32,
        parsed: &Message<'_>,
        session: &mut ImapSession,
    ) -> Result<()> {
        let from = EmailParser::parse_from_address(parsed);
        let subject = EmailParser::parse_subject(parsed);

        info!("Processing email from: {}, subject: {}", from, subject);

        // 检查主题过滤
        if !self.should_process_email(&subject) {
            info!(
                "Email subject does not contain '{}', skipping",
                self.config.subject_filter
            );
            return Ok(());
        }

        // 注册邮件
        self.file_tracker.register_email(&uid.to_string())?;

        // 处理附件
        self.process_attachments(uid, parsed, &from, session)
            .await?;

        Ok(())
    }

    /// 检查是否应该处理该邮件
    fn should_process_email(&self, subject: &str) -> bool {
        subject.contains(&self.config.subject_filter)
    }

    /// 处理附件
    async fn process_attachments(
        &self,
        uid: u32,
        parsed: &Message<'_>,
        from: &str,
        session: &mut ImapSession,
    ) -> Result<()> {
        let attachments = self.extract_attachments(parsed)?;

        if attachments.is_empty() {
            info!("Email has no valid attachments");
            self.handle_no_valid_attachments(uid, from).await?;
            return Ok(());
        }

        // 立即回复"已收到"
        self.send_received_confirmation(from).await?;

        // 下载所有有效附件
        for attachment in &attachments {
            let _ = self.download_attachment(uid, attachment, from).await?;
        }

        // 标记邮件已读并移动到"已处理"文件夹
        self.mark_and_move_email(uid, session).await?;

        Ok(())
    }

    /// 提取附件
    fn extract_attachments(&self, parsed: &Message) -> Result<Vec<Attachment>> {
        let mut attachments = Vec::new();

        for part in &parsed.parts {
            if part.is_text() {
                continue;
            }

            if let Some(filename) = part.attachment_name() {
                if !self.is_valid_attachment(filename) {
                    continue;
                }

                // 安全获取 content_type
                let content_type = part
                    .content_type()
                    .map(|ct| {
                        if let Some(subtype) = ct.subtype() {
                            format!("{}/{}", ct.c_type, subtype)
                        } else {
                            ct.c_type.to_string()
                        }
                    })
                    .unwrap_or_else(|| "application/octet-stream".to_string());

                let attachment = Attachment {
                    filename: filename.to_string(),
                    content_type,
                    data: part.contents().to_vec(),
                    size: part.body.len(),
                };
                attachments.push(attachment);
            }
        }

        Ok(attachments)
    }

    /// 验证附件格式
    fn is_valid_attachment(&self, filename: &str) -> bool {
        let lower = filename.to_lowercase();
        lower.ends_with(".csv")
            || lower.ends_with(".txt")
            || lower.ends_with(".xls")
            || lower.ends_with(".xlsx")
    }

    /// 处理无有效附件的情况
    async fn handle_no_valid_attachments(&self, uid: u32, from: &str) -> Result<()> {
        info!(
            "Email {} has no valid attachments, marking as processed",
            uid
        );

        // 发送"处理失败"通知
        let error_message = "无有效附件格式（.txt/.csv/.xls/.xlsx）";
        self.send_failure_notification(from, error_message, None)
            .await?;

        // 标记为失败
        self.file_tracker
            .mark_failed(&uid.to_string(), error_message.to_string(), None)?;

        Ok(())
    }

    /// 发送"已收到"确认
    async fn send_received_confirmation(&self, to: &str) -> Result<()> {
        info!("Sending '已收到' confirmation to {}", to);

        self.email_sender
            .send_text_email(to, "Re: 已收到", "已收到")
            .await
            .context("Failed to send received confirmation")?;

        Ok(())
    }

    /// 下载附件到input目录
    async fn download_attachment(
        &self,
        uid: u32,
        attachment: &Attachment,
        _from: &str,
    ) -> Result<PathBuf> {
        // 生成唯一文件名（添加邮件UID和时间戳）
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let ext = attachment.filename.rsplit('.').next().unwrap_or("");
        let unique_filename = format!("{}_{}_{}", uid, timestamp, ext);

        let save_path = self.config.input_dir.join(&unique_filename);

        // 确保目录存在
        fs::create_dir_all(&self.config.input_dir).context("Failed to create input directory")?;

        // 保存文件
        fs::write(&save_path, &attachment.data).context("Failed to write attachment file")?;

        info!(
            "Saved attachment: {:?} ({} bytes)",
            save_path, attachment.size
        );

        // 更新追踪器
        self.file_tracker
            .mark_downloaded(&uid.to_string(), save_path.clone())?;

        Ok(save_path)
    }

    /// 标记邮件已读并移动到"已处理"文件夹
    async fn mark_and_move_email(&self, uid: u32, session: &mut ImapSession) -> Result<()> {
        info!(
            "Marking email {} as read and moving to '{}",
            uid, self.config.processed_folder
        );

        // 先尝试创建文件夹
        let _ = session.create(&self.config.processed_folder).await;

        // 标记为已读（\Seen flag）
        let _ = session
            .store(uid.to_string(), "+FLAGS.SILENT (\\Seen)")
            .await;

        // 移动邮件到"已处理"文件夹
        session
            .mv(uid.to_string(), &self.config.processed_folder)
            .await
            .context("Failed to move email to processed folder")?;

        info!(
            "Successfully marked email {} as read and moved to '{}'",
            uid, self.config.processed_folder
        );

        Ok(())
    }

    /// 发送"已处理"成功通知
    pub async fn send_success_notification(&self, to: &str, processed_file: PathBuf) -> Result<()> {
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
        processed_file: Option<PathBuf>,
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
    fn test_is_valid_attachment() {
        let config = EmailConfig {
            imap_server: "".to_string(),
            imap_port: 993,
            smtp_server: "".to_string(),
            smtp_port: 587,
            username: "".to_string(),
            password: "".to_string(),
            poll_interval: 60,
            processed_folder: "".to_string(),
            subject_filter: "".to_string(),
            input_dir: PathBuf::new(),
            doned_dir: PathBuf::new(),
        };

        let monitor = EmailMonitor::new(config, Arc::new(FileTracker::new())).unwrap();

        assert!(monitor.is_valid_attachment("accounts.csv"));
        assert!(monitor.is_valid_attachment("data.xlsx"));
        assert!(monitor.is_valid_attachment("info.txt"));
        assert!(monitor.is_valid_attachment("spreadsheet.xls"));
        assert!(!monitor.is_valid_attachment("image.jpg"));
        assert!(!monitor.is_valid_attachment("document.pdf"));
        assert!(!monitor.is_valid_attachment("archive.zip"));
    }

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
