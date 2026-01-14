use crate::core::models::Account;
use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn};

pub async fn read_accounts_from_csv<P: AsRef<Path>>(
    path: P,
) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)> {
    let path = path.as_ref();
    info!("Reading accounts from CSV file: {}", path.display());

    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read CSV file: {}", path.display()))?;

    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers_record = reader.headers()?.clone();
    let headers: Vec<String> = headers_record.iter().map(|s| s.to_string()).collect();

    let mut accounts = Vec::new();
    let mut records = Vec::new();

    for (index, result) in reader.records().enumerate() {
        match result {
            Ok(record) => {
                // Try to deserialize into Account
                // We provide headers so it can map by name
                match record.deserialize(Some(&headers_record)) {
                    Ok(account) => {
                        accounts.push(account);
                        records.push(record.iter().map(|s| s.to_string()).collect());
                    }
                    Err(e) => {
                        warn!(
                            "Skipping row {} due to deserialization error: {}",
                            index + 1,
                            e
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Skipping row {} due to parse error: {}", index + 1, e);
            }
        }
    }

    info!("Successfully read {} accounts from CSV", accounts.len());
    Ok((accounts, records, headers))
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
            "username,password,other_col\nuser1@test.com,pass123,val1\nuser2@test.com,pass456,val2"
        )
        .unwrap();

        let (accounts, records, headers) = read_accounts_from_csv(temp_file.path()).await.unwrap();

        assert_eq!(accounts.len(), 2);
        assert_eq!(records.len(), 2);
        assert_eq!(headers.len(), 3);

        assert_eq!(accounts[0].username, "user1@test.com");
        assert_eq!(records[0].get(2), Some(&"val1".to_string()));
    }
}
