use std::{io, net::SocketAddr, ops::Not, path};

use clap::Parser;
use ldml_api::{app, config};
use tokio::net::TcpListener;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    #[clap(long, default_value = "/etc/ldml-api.json")]
    /// Path to config file
    config: path::PathBuf,

    #[clap(long, default_value = "production")]
    /// Default profile to use when staging argument not set in a request
    profile: String,

    #[clap(short, long, default_value = "0.0.0.0:3000")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    //console_subscriber::init();
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if cfg!(debug_assertions) && std::env::var_os("RUST_LOG").is_none() {
        tracing_subscriber::fmt()
            .with_env_filter(concat!(
                env!("CARGO_CRATE_NAME"),
                "=debug,tower_http=debug,axum::rejection=trace"
            ))
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    let args = Args::parse();

    // Load configuraion
    let cfg =
        config::profiles::from(&args.config, &args.profile).unwrap_or_else(|err: io::Error| {
            tracing::error!(
                "Error loading config: {file}: {message}",
                file = args.config.to_string_lossy(),
                message = err.to_string()
            );
            std::process::exit(err.raw_os_error().unwrap_or_default());
        });
    tracing::info!(
        "loaded profiles: {profiles}",
        profiles = cfg
            .keys()
            .filter_map(|p| p.is_empty().not().then_some(p.as_ref()))
            .collect::<Vec<_>>()
            .join(", ")
    );

    tracing::debug!("listening on {addr}", addr = args.listen);
    let listener = TcpListener::bind(&args.listen).await?;
    axum::serve(
        listener,
        app(cfg)?
            .layer(CompressionLayer::new())
            .layer(TraceLayer::new_for_http())
            .into_make_service(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap_or_else(|err| {
        tracing::error!(
            "Error starting service listenng at {addr}: {message}",
            addr = args.listen,
            message = err.to_string()
        );
        std::process::exit(err.raw_os_error().unwrap_or_default());
    });

    tracing::info!("shutting down");
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {tracing::debug!("received SIGINT")},
        _ = terminate => {tracing::debug!("received SIGTERM")},
    }
}
