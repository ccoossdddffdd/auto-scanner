use anyhow::{Context, Result};
use std::fs::File;

#[cfg(unix)]
use daemonize::Daemonize;

/// 启动后台守护进程（Unix-only）
#[cfg(unix)]
pub fn start_daemon(pid_file: &str, stdout_path: &str, stderr_path: &str) -> Result<()> {
    let stdout = File::create(stdout_path).context("Failed to create stdout file")?;
    let stderr = File::create(stderr_path).context("Failed to create stderr file")?;

    let daemonize = Daemonize::new()
        .pid_file(pid_file)
        .chown_pid_file(true)
        .working_directory(".")
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error starting daemon: {}", e);
            anyhow::bail!("Failed to daemonize: {}", e);
        }
    }
}

/// Windows 不支持传统的 Unix daemon，直接返回错误提示
#[cfg(windows)]
pub fn start_daemon(_pid_file: &str, _stdout_path: &str, _stderr_path: &str) -> Result<()> {
    anyhow::bail!(
        "Daemon mode is not supported on Windows.\n\
        Please run the program directly or use Windows Service instead.\n\
        Example: auto-scanner master --threads 4"
    )
}
