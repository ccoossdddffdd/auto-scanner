use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "auto-scanner")]
#[command(about = "自动化浏览器交互与账号验证工具", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// 以 Master 模式运行，管理 Worker 和账号
    Master {
        /// 使用的浏览器后端
        #[arg(long, default_value = "playwright")]
        backend: String,

        /// 浏览器的基础远程调试 URL
        #[arg(long, default_value = "http://localhost:9222")]
        remote_url: String,

        /// 并发启动的 Worker 数量
        #[arg(long, default_value = "1")]
        thread_count: usize,

        /// 使用的自动化策略
        #[arg(long, default_value = "facebook_login")]
        strategy: String,

        /// 停止正在运行的 Master 进程
        #[arg(long, default_value = "false")]
        stop: bool,

        /// 作为后台守护进程运行
        #[arg(long, default_value = "false")]
        daemon: bool,

        /// 检查 Master 进程是否正在运行
        #[arg(long, default_value = "false")]
        status: bool,

        /// 启用邮件监控
        #[arg(long, default_value = "false")]
        enable_email_monitor: bool,

        /// 邮件轮询间隔（秒）
        #[arg(long, default_value = "60")]
        email_poll_interval: u64,

        /// Outlook 注册策略的总注册数量（0 表示无限循环）
        #[arg(long, default_value = "0")]
        register_count: usize,
    },
    /// 以 Worker 模式运行，执行单个任务（通常由 Master 调用）
    Worker {
        /// 账号用户名
        #[arg(long)]
        username: String,

        /// 账号密码
        #[arg(long)]
        password: String,

        /// 该 Worker 的特定远程调试 URL
        #[arg(long)]
        remote_url: String,

        /// 使用的浏览器后端
        #[arg(long, default_value = "playwright")]
        backend: String,

        /// 使用的自动化策略
        #[arg(long, default_value = "facebook_login")]
        strategy: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_master_mode() {
        let cli = Cli::try_parse_from(["auto-scanner", "master"]);
        assert!(cli.is_ok());
        if let Commands::Master { .. } = cli.unwrap().command {
            // Success
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
