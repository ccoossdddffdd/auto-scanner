use crate::infrastructure::imap::{ImapClient, ImapSession};
use crate::services::email::attachment::{Attachment, AttachmentHandler};
use crate::services::email::config::EmailConfig;
use crate::services::email::notification::EmailNotifier;
use crate::services::email::parser::EmailParser;
use crate::services::email::tracker::FileTracker;
use anyhow::{Context, Result};
use chrono::Local;
use futures::StreamExt;
use mail_parser::MessageParser;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

/// 邮件监控器
pub struct EmailMonitor {
    config: EmailConfig,
    file_tracker: Arc<FileTracker>,
    notifier: EmailNotifier,
}

impl EmailMonitor {
    /// 创建新的邮件监控器
    pub fn new(config: EmailConfig, file_tracker: Arc<FileTracker>) -> Result<EmailMonitor> {
        let notifier = EmailNotifier::new(
            config.smtp_server.clone(),
            config.smtp_port,
            config.username.clone(),
            config.password.clone(),
        );

        Ok(Self {
            config,
            file_tracker,
            notifier,
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
        let mut session = self.create_imap_session().await?;
        let uid_set = self.search_unread_emails(&mut session).await?;

        if uid_set.is_empty() {
            info!("No new unread emails found");
            session
                .logout()
                .await
                .context("Failed to logout from IMAP")?;
            return Ok(());
        }

        info!("Found {} unread emails", uid_set.len());
        self.process_email_batch(&uid_set, &mut session).await?;

        session
            .logout()
            .await
            .context("Failed to logout from IMAP")?;

        Ok(())
    }

    /// 创建 IMAP 会话
    async fn create_imap_session(&self) -> Result<ImapSession> {
        let imap_client = ImapClient::new(
            self.config.imap_server.clone(),
            self.config.imap_port,
            self.config.username.clone(),
            self.config.password.clone(),
        );
        imap_client.connect().await
    }

    /// 搜索未读邮件
    async fn search_unread_emails(&self, session: &mut ImapSession) -> Result<Vec<u32>> {
        let inbox = session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        info!("Mailbox selected: {:?}", inbox);

        let search_result = session
            .search("UNSEEN")
            .await
            .context("Failed to search for unread emails")?;

        Ok(search_result.iter().copied().collect())
    }

    /// 批量处理邮件
    async fn process_email_batch(&self, uids: &[u32], session: &mut ImapSession) -> Result<()> {
        for uid in uids {
            if let Err(e) = self.fetch_and_process_email(*uid, session).await {
                error!("Failed to process email UID {}: {}", uid, e);
            }
        }
        Ok(())
    }

    /// 获取并处理单个邮件
    async fn fetch_and_process_email(&self, uid: u32, session: &mut ImapSession) -> Result<()> {
        let email_data = self.fetch_email_data(uid, session).await?;

        let msg =
            email_data.ok_or_else(|| anyhow::anyhow!("No data returned for email UID {}", uid))?;

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
        parsed: &mail_parser::Message<'_>,
        session: &mut ImapSession,
    ) -> Result<()> {
        let from = EmailParser::parse_from_address(parsed);
        let subject = EmailParser::parse_subject(parsed);

        info!("Processing email from: {}, subject: {}", from, subject);

        // 检查主题过滤
        if !subject.contains(&self.config.subject_filter) {
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

    /// 处理附件
    async fn process_attachments(
        &self,
        uid: u32,
        parsed: &mail_parser::Message<'_>,
        from: &str,
        session: &mut ImapSession,
    ) -> Result<()> {
        let attachments = AttachmentHandler::extract_attachments(parsed);

        if attachments.is_empty() {
            info!("Email has no valid attachments");
            self.handle_no_valid_attachments(uid, from).await?;
            return Ok(());
        }

        // 立即回复"已收到"
        self.notifier.send_received_confirmation(from).await?;

        // 下载所有有效附件
        for attachment in &attachments {
            let _ = self.download_attachment(uid, attachment, from).await?;
        }

        // 标记邮件已读并移动到"已处理"文件夹
        self.mark_and_move_email(uid, session).await?;

        Ok(())
    }

    /// 处理无有效附件的情况
    async fn handle_no_valid_attachments(&self, uid: u32, from: &str) -> Result<()> {
        info!(
            "Email {} has no valid attachments, marking as processed",
            uid
        );

        // 发送"处理失败"通知
        let error_message = "无有效附件格式（.txt/.csv/.xls/.xlsx）";
        self.notifier
            .send_failure_notification(from, error_message, None)
            .await?;

        // 标记为失败
        self.file_tracker
            .mark_failed(&uid.to_string(), error_message.to_string(), None)?;

        Ok(())
    }

    /// 下载附件
    async fn download_attachment(
        &self,
        uid: u32,
        attachment: &Attachment,
        from: &str,
    ) -> Result<PathBuf> {
        info!("Downloading attachment: {}", attachment.filename);

        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_filename = format!("{}_{}", timestamp, attachment.filename);
        let file_path = self.config.input_dir.join(&safe_filename);

        fs::write(&file_path, &attachment.data).context("Failed to write attachment to file")?;

        info!("Attachment saved to: {:?}", file_path);

        // 存储元数据
        use crate::services::email::tracker::EmailMetadata;
        let metadata = EmailMetadata {
            from: from.to_string(),
            subject: format!("Email UID: {}", uid),
            original_filename: attachment.filename.clone(),
        };

        if let Err(e) = self.file_tracker.register_email(&uid.to_string()) {
            warn!("Failed to register email: {}", e);
        }
        if let Err(e) = self
            .file_tracker
            .store_email_metadata(&uid.to_string(), metadata)
        {
            warn!("Failed to store email metadata: {}", e);
        }

        if let Err(e) = self
            .file_tracker
            .mark_downloaded(&uid.to_string(), file_path.clone())
        {
            warn!("Failed to mark email as downloaded: {}", e);
        }

        Ok(file_path)
    }

    /// 标记并移动邮件
    async fn mark_and_move_email(&self, uid: u32, session: &mut ImapSession) -> Result<()> {
        info!("Marking email {} as read and moving to processed", uid);

        // 标记已读
        if let Err(e) = session.store(format!("{}", uid), "+FLAGS (\\Seen)").await {
            warn!("Failed to mark email as read: {}", e);
        }

        // 移动到已处理文件夹
        let dest_folder = &self.config.processed_folder;
        if let Err(e) = session
            .mv(format!("{}", uid), dest_folder)
            .await
            .context(format!("Failed to move email to {}", dest_folder))
        {
            warn!("Could not move email: {}", e);
            // 不返回错误，因为邮件已经处理完成
        }

        Ok(())
    }

    /// 发送成功通知
    pub async fn send_success_notification(&self, to: &str, processed_file: PathBuf) -> Result<()> {
        self.notifier
            .send_success_notification(to, processed_file)
            .await
    }

    /// 发送失败通知
    pub async fn send_failure_notification(
        &self,
        to: &str,
        error_message: &str,
        processed_file: Option<PathBuf>,
    ) -> Result<()> {
        self.notifier
            .send_failure_notification(to, error_message, processed_file)
            .await
    }
}
