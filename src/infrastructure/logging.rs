use anyhow::Result;
use chrono::Local;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct PidTime;

impl tracing_subscriber::fmt::time::FormatTime for PidTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{} [{}]",
            Local::now().format("%Y-%m-%dT%H:%M:%S%.6fZ"),
            std::process::id()
        )
    }
}

pub fn init_logging(service_name: &str, is_daemon: bool) -> Result<()> {
    let file_name = format!("{}.log", service_name);
    let file_appender = tracing_appender::rolling::daily("logs", file_name);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard to prevent it from being dropped when the function returns
    // This is necessary because we're initializing the global subscriber
    std::mem::forget(_guard);

    let registry = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
    );

    if is_daemon {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_timer(PidTime),
            )
            .init();
    } else {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_timer(PidTime),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_timer(PidTime),
            )
            .init();
    }

    Ok(())
}
