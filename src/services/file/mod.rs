use crate::core::models::Account;
use anyhow::{Context, Result};
use async_trait::async_trait;
use calamine::{open_workbook, Reader, Xls, Xlsx};
use rust_xlsxwriter::Workbook;
use std::path::Path;
use tracing::{info, warn};

pub mod csv_reader;
pub mod excel_handler;

#[async_trait]
pub trait AccountSource: Send + Sync {
    async fn read(&self, path: &Path) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)>;
    async fn write(&self, path: &Path, headers: &[String], records: &[Vec<String>]) -> Result<()>;
}

pub struct CsvSource;

#[async_trait]
impl AccountSource for CsvSource {
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

pub struct ExcelSource;

#[async_trait]
impl AccountSource for ExcelSource {
    async fn read(&self, path: &Path) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            info!("Reading accounts from Excel file: {}", path.display());

            // Helper to get range from either XLS or XLSX
            let range = if let Ok(mut wb) = open_workbook::<Xlsx<_>, _>(&path) {
                if let Some(Ok(r)) = wb.worksheet_range_at(0) {
                    r
                } else {
                    anyhow::bail!("No sheet found in XLSX")
                }
            } else if let Ok(mut wb) = open_workbook::<Xls<_>, _>(&path) {
                if let Some(Ok(r)) = wb.worksheet_range_at(0) {
                    r
                } else {
                    anyhow::bail!("No sheet found in XLS")
                }
            } else {
                anyhow::bail!("Could not open file as XLSX or XLS")
            };

            let mut rows = range.rows();

            // Read headers
            let headers: Vec<String> = if let Some(header_row) = rows.next() {
                header_row.iter().map(|cell| cell.to_string()).collect()
            } else {
                return Ok((vec![], vec![], vec![]));
            };

            let mut accounts = Vec::new();
            let mut records = Vec::new();

            // Find indices for username and password
            let username_idx = headers
                .iter()
                .position(|h| {
                    h.to_lowercase().contains("username")
                        || h.to_lowercase().contains("email")
                        || h.to_lowercase().contains("用户")
                })
                .context("Username column not found")?;
            let password_idx = headers
                .iter()
                .position(|h| {
                    h.to_lowercase().contains("password")
                        || h.to_lowercase().contains("pass")
                        || h.to_lowercase().contains("密码")
                })
                .context("Password column not found")?;

            for (index, row) in rows.enumerate() {
                let row_strings: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();

                if row_strings.len() <= std::cmp::max(username_idx, password_idx) {
                    warn!("Skipping row {} due to insufficient columns", index + 1);
                    continue;
                }

                let username = row_strings[username_idx].clone();
                let password = row_strings[password_idx].clone();

                if username.is_empty() || password.is_empty() {
                    warn!("Skipping row {} due to empty credentials", index + 1);
                    continue;
                }

                accounts.push(Account::new(username, password));
                records.push(row_strings);
            }

            info!("Successfully read {} accounts from Excel", accounts.len());
            Ok((accounts, records, headers))
        })
        .await?
    }

    async fn write(&self, path: &Path, headers: &[String], records: &[Vec<String>]) -> Result<()> {
        let path = path.to_path_buf();
        let headers = headers.to_vec();
        let records = records.to_vec();

        tokio::task::spawn_blocking(move || {
            info!("Writing results to Excel file: {}", path.display());

            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            // Write headers
            for (col, header) in headers.iter().enumerate() {
                worksheet.write_string(0, col as u16, header)?;
            }

            // Write records
            for (row_idx, record) in records.iter().enumerate() {
                for (col_idx, cell) in record.iter().enumerate() {
                    worksheet.write_string((row_idx + 1) as u32, col_idx as u16, cell)?;
                }
            }

            workbook.save(path)?;
            Ok(())
        })
        .await?
    }
}

pub fn get_account_source(path: &Path) -> Box<dyn AccountSource> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension == "xls" || extension == "xlsx" {
        Box::new(ExcelSource)
    } else {
        Box::new(CsvSource)
    }
}
