use anyhow::{Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};

pub struct FilePolicyService;

impl FilePolicyService {
    pub fn is_supported_file(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        // Check for ignored files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains(".done-") || name.contains(".result.") {
                return false;
            }
            if name.starts_with("~$") {
                // Ignore Excel temp files
                return false;
            }
        }

        // Check extension
        path.extension().is_some_and(|ext| {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "csv" | "xls" | "xlsx" | "txt")
        })
    }

    pub fn generate_processed_path(path: &Path, doned_dir: &Path) -> Result<PathBuf> {
        let now = Local::now().format("%Y%m%d-%H%M%S");
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("无效的文件名")?;

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("csv");

        let new_name = format!("{}.done-{}.{}", file_name, now, extension);
        Ok(doned_dir.join(new_name))
    }
}
