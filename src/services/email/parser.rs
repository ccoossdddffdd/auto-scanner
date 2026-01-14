use mail_parser::Message;

/// 邮件解析器
pub struct EmailParser;

impl EmailParser {
    /// 解析发件人地址
    pub fn parse_from_address(parsed: &Message) -> String {
        parsed
            .from()
            .and_then(|l| l.first())
            .and_then(|a| a.address.as_ref())
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    /// 解析主题
    pub fn parse_subject(parsed: &Message) -> String {
        parsed.subject().unwrap_or("").to_string()
    }
}
