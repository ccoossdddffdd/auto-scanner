use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 处理状态枚举
#[derive(Debug, Clone)]
pub enum ProcessingStatus {
    /// 已收到
    Received { timestamp: DateTime<Local> },
    /// 已下载
    Downloaded {
        timestamp: DateTime<Local>,
        file_path: PathBuf,
    },
    /// 处理中
    Processing {
        timestamp: DateTime<Local>,
        file_path: PathBuf,
    },
    /// 处理成功
    Success {
        timestamp: DateTime<Local>,
        processed_file: PathBuf,
    },
    /// 处理失败
    Failed {
        timestamp: DateTime<Local>,
        error_message: String,
        processed_file: Option<PathBuf>,
    },
}

/// 邮件元数据
#[derive(Debug, Clone)]
pub struct EmailMetadata {
    pub from: String,
    pub subject: String,
    pub original_filename: String,
}

/// 文件追踪器
pub struct FileTracker {
    email_status: Arc<Mutex<HashMap<String, ProcessingStatus>>>,
    file_to_email: Arc<Mutex<HashMap<String, String>>>,
    email_metadata: Arc<Mutex<HashMap<String, EmailMetadata>>>,
}

impl FileTracker {
    /// 创建新的文件追踪器
    pub fn new() -> Self {
        Self {
            email_status: Arc::new(Mutex::new(HashMap::new())),
            file_to_email: Arc::new(Mutex::new(HashMap::new())),
            email_metadata: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册新邮件
    pub fn register_email(&self, email_id: &str) -> Result<()> {
        let mut status = self.email_status.lock().unwrap();
        status.insert(
            email_id.to_string(),
            ProcessingStatus::Received {
                timestamp: Local::now(),
            },
        );
        Ok(())
    }

    /// 存储邮件元数据
    pub fn store_email_metadata(&self, email_id: &str, metadata: EmailMetadata) -> Result<()> {
        let mut email_meta = self.email_metadata.lock().unwrap();
        email_meta.insert(email_id.to_string(), metadata);
        Ok(())
    }

    /// 更新为已下载状态
    pub fn mark_downloaded(&self, email_id: &str, file_path: PathBuf) -> Result<()> {
        let mut status = self.email_status.lock().unwrap();
        let mut mapping = self.file_to_email.lock().unwrap();

        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        status.insert(
            email_id.to_string(),
            ProcessingStatus::Downloaded {
                timestamp: Local::now(),
                file_path: file_path.clone(),
            },
        );
        mapping.insert(filename, email_id.to_string());

        Ok(())
    }

    /// 更新为处理中状态
    pub fn mark_processing(&self, email_id: &str, file_path: PathBuf) -> Result<()> {
        let mut status = self.email_status.lock().unwrap();
        status.insert(
            email_id.to_string(),
            ProcessingStatus::Processing {
                timestamp: Local::now(),
                file_path,
            },
        );
        Ok(())
    }

    /// 标记处理成功
    pub fn mark_success(&self, email_id: &str, processed_file: PathBuf) -> Result<()> {
        let mut status = self.email_status.lock().unwrap();
        status.insert(
            email_id.to_string(),
            ProcessingStatus::Success {
                timestamp: Local::now(),
                processed_file,
            },
        );
        Ok(())
    }

    /// 标记处理失败
    pub fn mark_failed(
        &self,
        email_id: &str,
        error: String,
        processed_file: Option<PathBuf>,
    ) -> Result<()> {
        let mut status = self.email_status.lock().unwrap();
        status.insert(
            email_id.to_string(),
            ProcessingStatus::Failed {
                timestamp: Local::now(),
                error_message: error,
                processed_file,
            },
        );
        Ok(())
    }

    /// 更新文件路径映射（用于文件转换，如 txt -> csv）
    pub fn update_file_path(&self, old_path: &Path, new_path: &Path) -> Result<()> {
        let mut mapping = self.file_to_email.lock().unwrap();

        let old_filename = old_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let new_filename = new_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Some(email_id) = mapping.remove(&old_filename) {
            mapping.insert(new_filename, email_id);
        }

        Ok(())
    }

    /// 通过文件名查找邮件ID
    pub fn find_email_by_file(&self, filename: &str) -> Option<String> {
        let mapping = self.file_to_email.lock().unwrap();
        mapping.get(filename).cloned()
    }

    /// 获取邮件状态
    pub fn get_status(&self, email_id: &str) -> Option<ProcessingStatus> {
        let status = self.email_status.lock().unwrap();
        status.get(email_id).cloned()
    }

    /// 获取邮件元数据
    pub fn get_email_metadata(&self, email_id: &str) -> Option<EmailMetadata> {
        let email_meta = self.email_metadata.lock().unwrap();
        email_meta.get(email_id).cloned()
    }

    /// 清理旧记录（超过24小时）
    pub fn cleanup_old_records(&self) -> Result<()> {
        let cutoff = Local::now() - chrono::Duration::hours(24);
        let mut status = self.email_status.lock().unwrap();

        status.retain(|_, value| match value {
            ProcessingStatus::Received { timestamp } => *timestamp > cutoff,
            ProcessingStatus::Downloaded { timestamp, .. } => *timestamp > cutoff,
            ProcessingStatus::Processing { timestamp, .. } => *timestamp > cutoff,
            ProcessingStatus::Success { timestamp, .. } => *timestamp > cutoff,
            ProcessingStatus::Failed { timestamp, .. } => *timestamp > cutoff,
        });

        Ok(())
    }

    /// 获取所有邮件ID
    pub fn get_all_email_ids(&self) -> Vec<String> {
        let status = self.email_status.lock().unwrap();
        status.keys().cloned().collect()
    }
}

impl Default for FileTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tracker_creation() {
        let tracker = FileTracker::new();
        let ids = tracker.get_all_email_ids();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_register_email() {
        let tracker = FileTracker::new();
        tracker.register_email("12345").unwrap();

        let status = tracker.get_status("12345");
        assert!(status.is_some());

        if let Some(ProcessingStatus::Received { timestamp }) = status {
            assert!(timestamp <= Local::now());
        } else {
            panic!("Expected Received status");
        }
    }

    #[test]
    fn test_mark_downloaded() {
        let tracker = FileTracker::new();
        let file_path = PathBuf::from("/tmp/test.csv");
        tracker.mark_downloaded("12345", file_path.clone()).unwrap();

        let status = tracker.get_status("12345");
        assert!(status.is_some());

        if let Some(ProcessingStatus::Downloaded {
            file_path: path, ..
        }) = status
        {
            assert_eq!(path, PathBuf::from("/tmp/test.csv"));
        } else {
            panic!("Expected Downloaded status");
        }
    }

    #[test]
    fn test_find_email_by_file() {
        let tracker = FileTracker::new();
        let file_path = PathBuf::from("/tmp/test.csv");
        tracker.mark_downloaded("12345", file_path.clone()).unwrap();

        let email_id = tracker.find_email_by_file("test.csv");
        assert_eq!(email_id, Some("12345".to_string()));
    }

    #[test]
    fn test_store_and_get_metadata() {
        let tracker = FileTracker::new();
        let metadata = EmailMetadata {
            from: "sender@example.com".to_string(),
            subject: "Test Subject".to_string(),
            original_filename: "test.csv".to_string(),
        };

        tracker
            .store_email_metadata("12345", metadata.clone())
            .unwrap();

        let retrieved = tracker.get_email_metadata("12345");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().from, "sender@example.com");
    }

    #[test]
    fn test_mark_success_and_failed() {
        let tracker = FileTracker::new();

        tracker
            .mark_success("12345", PathBuf::from("/processed/test.csv"))
            .unwrap();
        let status = tracker.get_status("12345");
        assert!(matches!(status, Some(ProcessingStatus::Success { .. })));

        tracker
            .mark_failed(
                "67890",
                "Test error".to_string(),
                Some(PathBuf::from("/processed/test2.csv")),
            )
            .unwrap();
        let status2 = tracker.get_status("67890");
        assert!(matches!(status2, Some(ProcessingStatus::Failed { .. })));
    }
}
