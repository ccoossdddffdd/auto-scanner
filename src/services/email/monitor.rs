use crate::infrastructure::imap::ImapClient;
use crate::services::email::config::EmailConfig;
use crate::services::email::imap_service::ImapService;
use crate::services::email::notification::EmailNotifier;
use crate::services::email::processor::EmailProcessor;
use crate::services::email::tracker::FileTracker;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// 邮件监控器
pub struct EmailMonitor {
    config: EmailConfig,
    file_tracker: Arc<FileTracker>,
    notifier: EmailNotifier,
    imap_service: Mutex<Box<dyn ImapService>>,
    processor: EmailProcessor,
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

        let imap_service = Box::new(ImapClient::new(
            config.imap_server.clone(),
            config.imap_port,
            config.username.clone(),
            config.password.clone(),
        ));

        let processor = EmailProcessor::new(
            file_tracker.clone(),
            config.input_dir.clone(),
            config.subject_filter.clone(),
        );

        Ok(Self {
            config,
            file_tracker,
            notifier,
            imap_service: Mutex::new(imap_service),
            processor,
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
        let mut imap_service = self.imap_service.lock().await;
        imap_service.connect().await?;
        let uids = imap_service.search_unseen().await?;

        if uids.is_empty() {
            info!("No new unread emails found");
            imap_service
                .logout()
                .await
                .context("Failed to logout from IMAP")?;
            return Ok(());
        }

        info!("Found {} unread emails", uids.len());
        
        // 我们不能在 process_email_batch 中持有锁，因为那会导致长时间的锁定
        // 但是 ImapService 本身是有状态的（session），如果我们在循环中每次都 lock，
        // 那么多个任务可能会交错使用同一个 session（虽然 ImapClient 是同步的，这里用 async mutex 保证互斥）
        // 实际上 check_and_process_emails 是被 start_monitoring 循环调用的，而 start_monitoring 本身是单任务的。
        // 除非有其他地方也调用 check_and_process_emails（目前没有）。
        // 所以在这里持有锁是安全的。
        
        for uid in uids {
            if let Err(e) = self.fetch_and_process_email(uid, &mut **imap_service).await {
                error!("Failed to process email UID {}: {}", uid, e);
            }
        }

        imap_service
            .logout()
            .await
            .context("Failed to logout from IMAP")?;

        Ok(())
    }

    /// 获取并处理单个邮件
    async fn fetch_and_process_email(&self, uid: u32, imap_service: &mut dyn ImapService) -> Result<()> {
        let raw_data = imap_service.fetch_email(uid).await?;

        let raw_bytes =
            raw_data.ok_or_else(|| anyhow::anyhow!("No data returned for email UID {}", uid))?;

        let parsed = self.processor.parse_email(&raw_bytes)?;
        self.process_email_workflow(uid, &parsed, imap_service).await?;

        Ok(())
    }

    /// 处理邮件工作流
    async fn process_email_workflow(
        &self,
        uid: u32,
        parsed: &mail_parser::Message<'_>,
        imap_service: &mut dyn ImapService,
    ) -> Result<()> {
        if !self.processor.should_process(parsed) {
            return Ok(());
        }

        let (from, subject) = self.processor.extract_metadata(parsed);
        info!("Processing email from: {}, subject: {}", from, subject);

        // 处理附件
        self.process_attachments(uid, parsed, &from, imap_service).await?;

        Ok(())
    }

    /// 处理附件
    async fn process_attachments(
        &self,
        uid: u32,
        parsed: &mail_parser::Message<'_>,
        from: &str,
        imap_service: &mut dyn ImapService,
    ) -> Result<()> {
        let attachments = self.processor.get_attachments(parsed);

        if attachments.is_empty() {
            info!("Email has no valid attachments");
            self.handle_no_valid_attachments(uid, from).await?;
            return Ok(());
        }

        // 立即回复"已收到"
        self.notifier.send_received_confirmation(from).await?;

        // 下载所有有效附件
        for attachment in &attachments {
            let _ = self
                .processor
                .save_attachment(uid, attachment, from)?;
        }

        // 标记邮件已读并移动到"已处理"文件夹
        self.mark_and_move_email(uid, imap_service).await?;

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
        self.processor.mark_failed(uid, error_message)?;

        Ok(())
    }

    /// 标记并移动邮件
    async fn mark_and_move_email(&self, uid: u32, imap_service: &mut dyn ImapService) -> Result<()> {
        info!("Marking email {} as read and moving to processed", uid);

        // 标记已读
        if let Err(e) = imap_service.mark_as_read(uid).await {
            warn!("Failed to mark email as read: {}", e);
        }

        // 移动到已处理文件夹
        let dest_folder = self.config.processed_folder.clone();
        if let Err(e) = imap_service.move_email(uid, &dest_folder).await {
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
