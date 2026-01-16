use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub username: String,
    pub password: String,
    pub success: Option<bool>,
    pub captcha: Option<String>,
    pub two_fa: Option<String>,
    pub batch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerResult {
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Account {
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            success: None,
            captcha: None,
            two_fa: None,
            batch: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account::new("test@example.com".to_string(), "password123".to_string());

        assert_eq!(account.username, "test@example.com");
        assert_eq!(account.password, "password123");
    }

    #[test]
    fn test_account_serialization() {
        let account = Account::new("user@test.com".to_string(), "secret".to_string());

        let serialized = serde_json::to_string(&account).unwrap();
        let deserialized: Account = serde_json::from_str(&serialized).unwrap();

        assert_eq!(account, deserialized);
    }
}
