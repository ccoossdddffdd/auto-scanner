use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::warn;

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
        dotenv::dotenv().ok();

        let config = Self {
            imap_server: Self::env_or("EMAIL_IMAP_SERVER", "outlook.office365.com"),
            imap_port: Self::env_parse("EMAIL_IMAP_PORT", 993)?,
            smtp_server: Self::env_or("EMAIL_SMTP_SERVER", "smtp.office365.com"),
            smtp_port: Self::env_parse("EMAIL_SMTP_PORT", 587)?,
            username: Self::env_required("EMAIL_USERNAME")?,
            password: Self::env_required("EMAIL_PASSWORD")?,
            poll_interval: Self::env_parse("EMAIL_POLL_INTERVAL", 60)?,
            processed_folder: Self::env_or("EMAIL_PROCESSED_FOLDER", "已处理"),
            subject_filter: Self::env_or("EMAIL_SUBJECT_FILTER", "FB账号"),
            input_dir: Self::env_or("INPUT_DIR", "input").into(),
            doned_dir: Self::env_or("DONED_DIR", "input/doned").into(),
        };

        config.validate()?;
        Ok(config)
    }

    /// 验证配置有效性
    fn validate(&self) -> Result<()> {
        // 验证端口范围
        if self.imap_port == 0 {
            anyhow::bail!("Invalid IMAP port: {}", self.imap_port);
        }
        if self.smtp_port == 0 {
            anyhow::bail!("Invalid SMTP port: {}", self.smtp_port);
        }

        // 验证服务器地址
        if self.imap_server.is_empty() {
            anyhow::bail!("IMAP server cannot be empty");
        }
        if self.smtp_server.is_empty() {
            anyhow::bail!("SMTP server cannot be empty");
        }

        // 验证轮询间隔
        if self.poll_interval == 0 {
            anyhow::bail!("Poll interval must be greater than 0");
        }
        if self.poll_interval > 3600 {
            warn!(
                "Poll interval {} is very long (>1 hour), is this intended?",
                self.poll_interval
            );
        }

        // 验证目录路径
        if self.input_dir.to_str().is_none_or(|s| s.is_empty()) {
            anyhow::bail!("Input directory path is invalid");
        }
        if self.doned_dir.to_str().is_none_or(|s| s.is_empty()) {
            anyhow::bail!("Doned directory path is invalid");
        }

        Ok(())
    }

    /// 读取环境变量或使用默认值
    fn env_or(key: &str, default: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| default.to_string())
    }

    /// 读取并解析环境变量，失败时使用默认值
    fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        match std::env::var(key) {
            Ok(val) => val
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid {}: {}", key, e)),
            Err(_) => Ok(default),
        }
    }

    /// 读取必需的环境变量
    fn env_required(key: &str) -> Result<String> {
        std::env::var(key).context(format!("{} not set in .env file", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_from_env() {
        std::env::set_var("EMAIL_USERNAME", "test@example.com");
        std::env::set_var("EMAIL_PASSWORD", "password123");

        let config = EmailConfig::from_env();
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.username, "test@example.com");
        assert_eq!(config.password, "password123");
        assert_eq!(config.imap_port, 993);
    }
}
