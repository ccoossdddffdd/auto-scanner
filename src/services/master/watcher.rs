use crate::services::file_policy::FilePolicyService;
use anyhow::Result;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{error, info};

pub struct InputWatcher {
    _watcher: RecommendedWatcher, // Keep alive
}

impl InputWatcher {
    pub fn new(input_path: PathBuf, tx: mpsc::Sender<PathBuf>) -> Result<Self> {
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                match res {
                    Ok(event) => {
                        if let EventKind::Create(_) | EventKind::Modify(_) = event.kind {
                            for path in event.paths {
                                if FilePolicyService::is_supported_file(&path) {
                                    // Use blocking_send because this is a sync callback
                                    if let Err(e) = tx.blocking_send(path.clone()) {
                                        error!("发送文件事件失败: {}", e);
                                    } else {
                                        info!("检测到文件变动: {:?}", path);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => error!("文件监控错误: {}", e),
                }
            },
            Config::default(),
        )?;

        watcher.watch(&input_path, RecursiveMode::NonRecursive)?;
        info!("文件监控已启动: {:?}", input_path);

        Ok(Self { _watcher: watcher })
    }
}
