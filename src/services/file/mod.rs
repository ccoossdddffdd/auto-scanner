pub mod csv_reader;
pub mod excel_handler;
pub mod operation;

use crate::core::models::Account;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait AccountSource {
    async fn read(&self, path: &Path) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)>;
    async fn write(&self, path: &Path, headers: &[String], records: &[Vec<String>]) -> Result<()>;
}

pub fn get_account_source(path: &Path) -> Box<dyn AccountSource + Send + Sync> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "csv" | "txt" => Box::new(csv_reader::CsvAccountSource),
        "xls" | "xlsx" => Box::new(excel_handler::ExcelAccountSource),
        _ => Box::new(csv_reader::CsvAccountSource), // Default to CSV for now
    }
}
