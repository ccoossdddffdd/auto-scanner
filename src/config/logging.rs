use std::env;
use tracing::Level;

/// 日志配置
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// 日志级别 (trace, debug, info, warn, error)
    pub level: Level,
    /// 日志格式 (json, pretty, compact)
    pub format: LogFormat,
}

/// 日志格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// JSON 格式 (适合生产环境)
    Json,
    /// 易读格式 (适合开发环境)
    Pretty,
    /// 紧凑格式
    Compact,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            format: LogFormat::Pretty,
        }
    }
}

impl LogConfig {
    /// 从环境变量创建配置
    pub fn from_env() -> Self {
        let level =
            Self::parse_level(&env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()));
        let format =
            Self::parse_format(&env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string()));

        Self { level, format }
    }

    /// 解析日志级别
    fn parse_level(s: &str) -> Level {
        match s.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" | "warning" => Level::WARN,
            "error" => Level::ERROR,
            _ => {
                eprintln!("Invalid LOG_LEVEL: {}, using INFO", s);
                Level::INFO
            }
        }
    }

    /// 解析日志格式
    fn parse_format(s: &str) -> LogFormat {
        match s.to_lowercase().as_str() {
            "json" => LogFormat::Json,
            "pretty" => LogFormat::Pretty,
            "compact" => LogFormat::Compact,
            _ => {
                eprintln!("Invalid LOG_FORMAT: {}, using Pretty", s);
                LogFormat::Pretty
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LogConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert_eq!(config.format, LogFormat::Pretty);
    }

    #[test]
    fn test_parse_level() {
        assert_eq!(LogConfig::parse_level("trace"), Level::TRACE);
        assert_eq!(LogConfig::parse_level("DEBUG"), Level::DEBUG);
        assert_eq!(LogConfig::parse_level("info"), Level::INFO);
        assert_eq!(LogConfig::parse_level("WARN"), Level::WARN);
        assert_eq!(LogConfig::parse_level("error"), Level::ERROR);
        assert_eq!(LogConfig::parse_level("invalid"), Level::INFO);
    }

    #[test]
    fn test_parse_format() {
        assert_eq!(LogConfig::parse_format("json"), LogFormat::Json);
        assert_eq!(LogConfig::parse_format("PRETTY"), LogFormat::Pretty);
        assert_eq!(LogConfig::parse_format("compact"), LogFormat::Compact);
        assert_eq!(LogConfig::parse_format("invalid"), LogFormat::Pretty);
    }
}
