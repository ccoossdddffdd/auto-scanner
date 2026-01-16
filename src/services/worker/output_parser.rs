use crate::core::models::WorkerResult;

pub struct WorkerOutputParser;

impl WorkerOutputParser {
    pub fn parse(stdout: &str) -> Option<WorkerResult> {
        if let Some(start_idx) = stdout.find("<<WORKER_RESULT>>") {
            let content_start = start_idx + "<<WORKER_RESULT>>".len();
            if let Some(end_idx) = stdout[content_start..].find("<<WORKER_RESULT>>") {
                let json_str = &stdout[content_start..content_start + end_idx];
                if let Ok(result) = serde_json::from_str::<WorkerResult>(json_str) {
                    return Some(result);
                }
            }
        }
        None
    }
}
