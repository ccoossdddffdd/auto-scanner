use anyhow::{Context, Result};
use daemonize::Daemonize;
use std::fs::File;

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
