use anyhow::Result;
use auto_scanner::core::cli::{Cli, Commands};
use auto_scanner::core::config::AppConfig;
use auto_scanner::infrastructure::daemon::start_daemon;
use auto_scanner::infrastructure::logging::init_logging;
use auto_scanner::services::{master, worker};
use clap::Parser;

const PID_FILE: &str = "auto-scanner-master.pid";

fn main() -> Result<()> {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Master {
            backend,
            remote_url,
            thread_count,
            strategy,
            stop,
            daemon,
            status,
            enable_email_monitor,
            email_poll_interval,
        } => {
            if daemon && !stop && !status {
                start_daemon(PID_FILE, "logs/auto-scanner.out", "logs/auto-scanner.err")?;
            }

            // 创建运行时并运行主进程
            let master_config = master::MasterConfig {
                backend,
                remote_url,
                thread_count,
                strategy,
                stop,
                daemon,
                status,
                enable_email_monitor,
                email_poll_interval,
                exe_path: None,
            };

            // 初始化日志（需要先于配置加载，以便记录配置加载过程中的警告）
            init_logging("auto-scanner", master_config.daemon)?;

            let app_config = AppConfig::from_env(master_config)?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async { master::run(app_config).await })
        }
        Commands::Worker {
            username,
            password,
            remote_url,
            backend,
            strategy,
        } => {
            // 初始化 Worker 日志
            init_logging("auto-scanner-worker", false)?;

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                worker::run(username, password, remote_url, backend, strategy).await
            })
        }
    };

    // 处理错误，提供友好的提示
    if let Err(e) = result {
        let error_msg = e.to_string();
        if error_msg.contains("AdsPower") || error_msg.contains("adspower") {
            eprintln!("\n❌ {}", error_msg);
            std::process::exit(1);
        } else {
            return Err(e);
        }
    }

    Ok(())
}
