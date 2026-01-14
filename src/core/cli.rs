use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "auto-scanner")]
#[command(about = "Automated Facebook account verification tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Run in master mode to manage workers and accounts
    Master {
        /// Directory to monitor for CSV files
        #[arg(short, long, value_name = "DIR", required_unless_present_any = ["stop", "status"])]
        input: Option<String>,

        /// Browser backend to use
        #[arg(long, default_value = "playwright")]
        backend: String,

        /// Base remote debugging URL for browser
        #[arg(long, default_value = "http://localhost:9222")]
        remote_url: String,

        /// Number of concurrent workers to spawn
        #[arg(long, default_value = "1")]
        thread_count: usize,

        /// Enable screenshots after login
        #[arg(long, default_value = "false")]
        enable_screenshot: bool,

        /// Stop the running master process
        #[arg(long, default_value = "false")]
        stop: bool,

        /// Run as a background daemon
        #[arg(long, default_value = "false")]
        daemon: bool,

        /// Check if the master process is running
        #[arg(long, default_value = "false")]
        status: bool,

        /// Enable email monitoring
        #[arg(long, default_value = "false")]
        enable_email_monitor: bool,

        /// Email polling interval in seconds
        #[arg(long, default_value = "60")]
        email_poll_interval: u64,
    },
    /// Run in worker mode to perform a single login (usually called by master)
    Worker {
        /// Account username
        #[arg(long)]
        username: String,

        /// Account password
        #[arg(long)]
        password: String,

        /// Specific remote debugging URL for this worker
        #[arg(long)]
        remote_url: String,

        /// Browser backend to use
        #[arg(long, default_value = "playwright")]
        backend: String,

        /// Enable screenshots after login
        #[arg(long, default_value = "false")]
        enable_screenshot: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_master_mode() {
        let cli = Cli::try_parse_from(["auto-scanner", "master", "-i", "accounts.csv"]);
        assert!(cli.is_ok());
        if let Commands::Master { input, .. } = cli.unwrap().command {
            assert_eq!(input, Some("accounts.csv".to_string()));
        } else {
            panic!("Expected Master command");
        }
    }

    #[test]
    fn test_cli_worker_mode() {
        let cli = Cli::try_parse_from([
            "auto-scanner",
            "worker",
            "--username",
            "user",
            "--password",
            "pass",
            "--remote-url",
            "http://localhost:9222",
        ]);
        assert!(cli.is_ok());
        if let Commands::Worker {
            username, password, ..
        } = cli.unwrap().command
        {
            assert_eq!(username, "user");
            assert_eq!(password, "pass");
        } else {
            panic!("Expected Worker command");
        }
    }
}
