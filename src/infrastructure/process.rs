use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

pub struct PidManager {
    pid_file: PathBuf,
}

impl PidManager {
    pub fn new<P: Into<PathBuf>>(pid_file: P) -> Self {
        Self {
            pid_file: pid_file.into(),
        }
    }

    pub fn write_pid(&self) -> Result<()> {
        let pid = std::process::id();
        if self.pid_file.exists() {
            if let Ok(content) = fs::read_to_string(&self.pid_file) {
                if let Ok(old_pid) = content.trim().parse::<u32>() {
                    if self.check_process_running(old_pid) {
                        anyhow::bail!("Process is already running (PID: {})", old_pid);
                    }
                }
            }
        }
        fs::write(&self.pid_file, pid.to_string()).context("Failed to write PID file")?;
        info!("Written PID {} to {:?}", pid, self.pid_file);
        Ok(())
    }

    pub fn check_status(&self) -> Result<()> {
        if !self.pid_file.exists() {
            println!("Not running");
            return Ok(());
        }

        let pid_str = fs::read_to_string(&self.pid_file).context("Failed to read PID file")?;
        let pid: u32 = pid_str.trim().parse().context("Failed to parse PID")?;

        if self.check_process_running(pid) {
            println!("Running (PID: {})", pid);
        } else {
            println!("Not running (Stale PID file found)");
        }

        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if !self.pid_file.exists() {
            info!("No PID file found. Process might not be running.");
            return Ok(());
        }

        let content = fs::read_to_string(&self.pid_file).context("Failed to read PID file")?;
        let pid = content
            .trim()
            .parse::<u32>()
            .context("Invalid PID in file")?;

        info!("Stopping process with PID {}", pid);

        if self.check_process_running(pid) {
            self.kill_process(pid)?;
            info!("Sent termination signal to process {}", pid);
        } else {
            warn!("Process {} not found", pid);
        }

        let _ = fs::remove_file(&self.pid_file);
        Ok(())
    }

    pub fn remove_pid_file(&self) {
        let _ = fs::remove_file(&self.pid_file);
    }

    #[cfg(unix)]
    fn check_process_running(&self, pid: u32) -> bool {
        signal::kill(Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(windows)]
    fn check_process_running(&self, pid: u32) -> bool {
        use std::process::Command;

        // 使用 tasklist 检查进程是否存在
        Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn kill_process(&self, pid: u32) -> Result<()> {
        signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).context("Failed to send SIGTERM")
    }

    #[cfg(windows)]
    fn kill_process(&self, pid: u32) -> Result<()> {
        use std::process::Command;

        // 使用 taskkill 终止进程
        let output = Command::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])
            .output()
            .context("Failed to execute taskkill")?;

        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "Failed to kill process: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        }
    }
}
