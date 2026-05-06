use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init(log_dir: Option<&Path>) -> Option<WorkerGuard> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,server=debug"));

    let stderr_layer = fmt::layer().with_target(true).with_writer(std::io::stderr);

    if let Some(dir) = log_dir {
        let appender = tracing_appender::rolling::daily(dir, "server.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let file_layer = fmt::layer()
            .json()
            .with_target(true)
            .with_current_span(true)
            .with_span_list(false)
            .with_writer(writer);
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .init();
        None
    }
}
