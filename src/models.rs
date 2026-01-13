use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub username: String,
    pub password: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account {
            username: "test@example.com".to_string(),
            password: "password123".to_string(),
        };
        
        assert_eq!(account.username, "test@example.com");
        assert_eq!(account.password, "password123");
    }

    #[test]
    fn test_account_serialization() {
        let account = Account {
            username: "user@test.com".to_string(),
            password: "secret".to_string(),
        };
        
        let serialized = serde_json::to_string(&account).unwrap();
        let deserialized: Account = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(account, deserialized);
    }
}
