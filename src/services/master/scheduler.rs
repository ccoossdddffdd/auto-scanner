use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct JobScheduler {
    processing_files: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl JobScheduler {
    pub fn new() -> Self {
        Self {
            processing_files: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn try_schedule(&self, path: PathBuf) -> bool {
        let mut processing = self
            .processing_files
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        processing.insert(path)
    }

    pub fn mark_completed(&self, path: &PathBuf) {
        let mut processing = self
            .processing_files
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        processing.remove(path);
    }
}
