use crate::models::Account;
use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn};

pub async fn read_accounts_from_csv<P: AsRef<Path>>(path: P) -> Result<Vec<Account>> {
    let path = path.as_ref();
    info!("Reading accounts from CSV file: {}", path.display());

    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read CSV file: {}", path.display()))?;

    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let mut accounts = Vec::new();

    for (index, result) in reader.deserialize().enumerate() {
        match result {
            Ok(account) => accounts.push(account),
            Err(e) => {
                warn!("Skipping row {} due to parse error: {}", index + 1, e);
            }
        }
    }

    info!("Successfully read {} accounts from CSV", accounts.len());
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_valid_csv() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "username,password\nuser1@test.com,pass123\nuser2@test.com,pass456"
        )
        .unwrap();

        let accounts = read_accounts_from_csv(temp_file.path()).await.unwrap();

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].username, "user1@test.com");
        assert_eq!(accounts[0].password, "pass123");
        assert_eq!(accounts[1].username, "user2@test.com");
        assert_eq!(accounts[1].password, "pass456");
    }

    #[tokio::test]
    async fn test_read_csv_with_invalid_row() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "username,password\nuser1@test.com,pass123\ninvalid_row\nuser2@test.com,pass456"
        )
        .unwrap();

        let accounts = read_accounts_from_csv(temp_file.path()).await.unwrap();

        // Should skip the invalid row and read 2 valid accounts
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].username, "user1@test.com");
        assert_eq!(accounts[1].username, "user2@test.com");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let result = read_accounts_from_csv("/nonexistent/file.csv").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_empty_csv() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "username,password").unwrap();

        let accounts = read_accounts_from_csv(temp_file.path()).await.unwrap();

        assert_eq!(accounts.len(), 0);
    }
}
