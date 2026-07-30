use std::{
    io::IsTerminal,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Parser;
use clapier::AppState;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// clapier - the Nabaztag:tag's HTTP burrow.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Listen address
    #[arg(long, default_value = "0.0.0.0:80")]
    bind: SocketAddr,

    /// Content root to serve (the tree containing vl/)
    #[arg(long)]
    root: PathBuf,

    /// Overlay tree tried before the root: rabbits/<mac>/… for requests
    /// carrying a valid `m` query param, then common/…
    #[arg(long)]
    overlay: Option<PathBuf>,

    /// Rabbit IP: its requests get a 🐰 tag in logs and on the status page
    #[arg(long)]
    rabbit: Option<IpAddr>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let root = args
        .root
        .canonicalize()
        .with_context(|| format!("content root not found: {}", args.root.display()))?;
    if !root.join("vl").is_dir() {
        warn!(
            "no vl/ under {} - the rabbit will not find bc.jsp there",
            root.display()
        );
    }

    // Fail fast on a bad overlay path: silently serving the base tree
    // when a deploy is believed active would be worse than not starting.
    let overlay = args
        .overlay
        .map(|o| {
            o.canonicalize()
                .with_context(|| format!("overlay not found: {}", o.display()))
        })
        .transpose()?;

    let app = AppState::new(root.clone(), overlay.clone(), args.rabbit);
    let router = clapier::router(app);
    let listener = tokio::net::TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("cannot bind {}", args.bind))?;
    match &overlay {
        Some(o) => info!(
            "clapier listening on http://{} - serving {} + overlay {}",
            args.bind,
            root.display(),
            o.display()
        ),
        None => info!(
            "clapier listening on http://{} - serving {}",
            args.bind,
            root.display()
        ),
    }
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    info!("clapier shut down");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
