use mail_parser::{Message, MimeHeaders};

/// 附件信息
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub size: usize,
}

/// 附件处理器
pub struct AttachmentHandler;

impl AttachmentHandler {
    /// 提取邮件中的附件
    pub fn extract_attachments(parsed: &Message) -> Vec<Attachment> {
        let mut attachments = Vec::new();

        for part in &parsed.parts {
            if part.is_text() {
                continue;
            }

            if let Some(filename) = part.attachment_name() {
                if !Self::is_valid_attachment(filename) {
                    continue;
                }

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

        attachments
    }

    /// 验证附件格式
    fn is_valid_attachment(filename: &str) -> bool {
        let lower = filename.to_lowercase();
        lower.ends_with(".csv")
            || lower.ends_with(".txt")
            || lower.ends_with(".xls")
            || lower.ends_with(".xlsx")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_attachment() {
        assert!(AttachmentHandler::is_valid_attachment("accounts.csv"));
        assert!(AttachmentHandler::is_valid_attachment("data.txt"));
        assert!(AttachmentHandler::is_valid_attachment("report.xls"));
        assert!(AttachmentHandler::is_valid_attachment("ACCOUNTS.CSV"));
        assert!(!AttachmentHandler::is_valid_attachment("document.pdf"));
        assert!(!AttachmentHandler::is_valid_attachment("image.jpg"));
    }
}
