use super::AccountSource;
use crate::core::models::Account;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;
use tracing::{info, warn};

pub struct CsvAccountSource;

#[async_trait]
impl AccountSource for CsvAccountSource {
    async fn read(&self, path: &Path) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)> {
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

    async fn write(&self, path: &Path, headers: &[String], records: &[Vec<String>]) -> Result<()> {
        info!("Writing results to CSV file: {}", path.display());
        let mut wtr = csv::Writer::from_path(path)?;

        wtr.write_record(headers)?;

        for record in records {
            wtr.write_record(record)?;
        }

        wtr.flush()?;
        Ok(())
    }
}
