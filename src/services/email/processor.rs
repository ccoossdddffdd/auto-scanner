use crate::services::email::attachment::{Attachment, AttachmentHandler};
use crate::services::email::parser::EmailParser;
use crate::services::email::tracker::{EmailMetadata, FileTracker};
use anyhow::{Context, Result};
use chrono::Local;
use mail_parser::MessageParser;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct EmailProcessor {
    file_tracker: Arc<FileTracker>,
    input_dir: PathBuf,
    subject_filter: String,
}

impl EmailProcessor {
    pub fn new(file_tracker: Arc<FileTracker>, input_dir: PathBuf, subject_filter: String) -> Self {
        Self {
            file_tracker,
            input_dir,
            subject_filter,
        }
    }

    pub fn parse_email<'a>(&self, raw_data: &'a [u8]) -> Result<mail_parser::Message<'a>> {
        MessageParser::default()
            .parse(raw_data)
            .context("Failed to parse email")
    }

    pub fn should_process(&self, parsed: &mail_parser::Message<'_>) -> bool {
        let subject = EmailParser::parse_subject(parsed);
        if !subject.contains(&self.subject_filter) {
            info!(
                "Email subject does not contain '{}', skipping",
                self.subject_filter
            );
            return false;
        }
        true
    }

    pub fn extract_metadata(&self, parsed: &mail_parser::Message<'_>) -> (String, String) {
        let from = EmailParser::parse_from_address(parsed);
        let subject = EmailParser::parse_subject(parsed);
        (from, subject)
    }

    pub fn get_attachments(&self, parsed: &mail_parser::Message<'_>) -> Vec<Attachment> {
        AttachmentHandler::extract_attachments(parsed)
    }

    pub fn save_attachment(
        &self,
        uid: u32,
        attachment: &Attachment,
        from: &str,
    ) -> Result<PathBuf> {
        info!("Downloading attachment: {}", attachment.filename);

        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_filename = format!("{}_{}", timestamp, attachment.filename);
        let file_path = self.input_dir.join(&safe_filename);

        fs::write(&file_path, &attachment.data).context("Failed to write attachment to file")?;

        info!("Attachment saved to: {:?}", file_path);

        // Store metadata
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

    pub fn mark_failed(&self, uid: u32, error_message: &str) -> Result<()> {
        self.file_tracker
            .mark_failed(&uid.to_string(), error_message.to_string(), None)?;
        Ok(())
    }
}
