use super::AccountSource;
use crate::core::models::Account;
use anyhow::{Context, Result};
use async_trait::async_trait;
use calamine::{open_workbook, Reader, Xls, Xlsx};
use rust_xlsxwriter::Workbook;
use std::path::Path;
use tracing::{info, warn};

pub struct ExcelAccountSource;

#[async_trait]
impl AccountSource for ExcelAccountSource {
    async fn read(&self, path: &Path) -> Result<(Vec<Account>, Vec<Vec<String>>, Vec<String>)> {
        info!("Reading accounts from Excel file: {}", path.display());

        // Check availability by trying to open with specific types
        if open_workbook::<Xlsx<_>, _>(path).is_err() && open_workbook::<Xls<_>, _>(path).is_err() {
            anyhow::bail!("Unsupported or invalid Excel file");
        }

        let range = if let Ok(mut wb) = open_workbook::<Xlsx<_>, _>(path) {
            if let Some(Ok(r)) = wb.worksheet_range_at(0) {
                r
            } else {
                anyhow::bail!("No sheet found in XLSX")
            }
        } else if let Ok(mut wb) = open_workbook::<Xls<_>, _>(path) {
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
    }

    async fn write(&self, path: &Path, headers: &[String], records: &[Vec<String>]) -> Result<()> {
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
    }
}
