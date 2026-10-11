mod auth;
mod bang;
mod config;
mod gocompat;
mod gojson;
mod handler;
mod korean;
mod pages;
mod server;
#[cfg(test)]
mod server_tests;

use tracing::{error, info, warn};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn get_env(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_owned(),
    }
}

/// Exits on SIGINT/SIGTERM/SIGHUP the way the Go runtime does by default,
/// which also matters when running as PID 1 in a container.
async fn exit_on_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let kinds = [
        (SignalKind::interrupt(), 2),
        (SignalKind::terminate(), 15),
        (SignalKind::hangup(), 1),
    ];
    let mut waits = Vec::new();
    for (kind, num) in kinds {
        if let Ok(mut s) = signal(kind) {
            waits.push(Box::pin(async move {
                s.recv().await;
                num
            }));
        }
    }
    let (num, _, _) = futures_util::future::select_all(waits).await;
    std::process::exit(128 + num);
}

fn main() {
    let (writer, _guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .buffered_lines_limit(4096)
        .finish(std::io::stdout());
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_target(false)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(writer)
        .init();

    let config_path = get_env("BANGS_CONFIG_PATH", "./bangs.yaml");
    let port = get_env("BANGS_PORT", "8080");
    let auth = auth::AuthKeys::parse(&get_env("BANGS_AUTH_KEYS", ""));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to start runtime");
    runtime.block_on(async {
        if auth.len() > 0 {
            info!(keys = auth.len(), "query key authentication enabled");
            if auth.has_short_key() {
                warn!(
                    min_length = auth::MIN_RECOMMENDED_LEN,
                    "auth key is shorter than recommended; use e.g. `openssl rand -hex 32`"
                );
            }
        }
        let server = server::Server::with_auth(&config_path, auth);

        // Kept alive for the life of the process.
        let _watcher = match server.watch_config() {
            Ok(w) => Some(w),
            Err(e) => {
                warn!(error = %e, "failed to watch config file");
                None
            }
        };

        info!(port = %port, "starting server");
        let listener = match server::resolve_port(&port).and_then(server::listen) {
            Ok(l) => l,
            Err(e) => {
                error!(error = %e, "server failed");
                drop(_guard);
                std::process::exit(1);
            }
        };
        tokio::spawn(exit_on_signal());
        server.serve(listener).await;
    });
}
