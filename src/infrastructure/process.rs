use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

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
                if let Ok(old_pid) = content.trim().parse::<i32>() {
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
        let pid: i32 = pid_str.trim().parse().context("Failed to parse PID")?;

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
            .parse::<i32>()
            .context("Invalid PID in file")?;

        info!("Stopping process with PID {}", pid);

        if self.check_process_running(pid) {
            signal::kill(Pid::from_raw(pid), Signal::SIGTERM).context("Failed to send SIGTERM")?;
            info!("Sent SIGTERM to process {}", pid);
        } else {
            warn!("Process {} not found", pid);
        }

        let _ = fs::remove_file(&self.pid_file);
        Ok(())
    }

    pub fn remove_pid_file(&self) {
        let _ = fs::remove_file(&self.pid_file);
    }

    fn check_process_running(&self, pid: i32) -> bool {
        signal::kill(Pid::from_raw(pid), None).is_ok()
    }
}
