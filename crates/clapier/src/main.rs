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

    /// Base content tree to serve behind the overlay (a tree containing
    /// vl/); optional since the overlay can carry everything
    #[arg(long)]
    root: Option<PathBuf>,

    /// Overlay tree tried before the root: rabbits/<mac>/… for requests
    /// carrying a valid `m` query param, then common/…
    #[arg(long)]
    overlay: Option<PathBuf>,

    /// Rabbit IP: its requests get a 🐰 tag in logs and on the status page
    #[arg(long)]
    rabbit: Option<IpAddr>,

    /// UDP port of the rabbits' broadcast log channel, listened to for
    /// the fleet table's pulses; 0 turns the listener off
    #[arg(long, default_value_t = 9999)]
    pulse_port: u16,

    /// Script the pilot page's speech box runs (one argument: the
    /// sentence); unset leaves the box disabled
    #[arg(long)]
    say_script: Option<PathBuf>,

    /// Seconds between /status polls of the fleet's rabbits; 0 turns
    /// the poller off
    #[arg(long, default_value_t = 30)]
    poll_secs: u64,
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

    if args.root.is_none() && args.overlay.is_none() {
        anyhow::bail!("nothing to serve: pass --root, --overlay or both");
    }
    let root = args
        .root
        .map(|r| {
            r.canonicalize()
                .with_context(|| format!("content root not found: {}", r.display()))
        })
        .transpose()?;
    if let Some(root) = &root
        && !root.join("vl").is_dir()
    {
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

    let app = AppState::new(root.clone(), overlay.clone(), args.rabbit, args.say_script);
    if args.pulse_port != 0 {
        tokio::spawn(clapier::listen_pulses(app.clone(), args.pulse_port));
    }
    if args.poll_secs != 0 {
        tokio::spawn(clapier::poll_statuses(
            app.clone(),
            std::time::Duration::from_secs(args.poll_secs),
        ));
    }
    let router = clapier::router(app);
    let listener = tokio::net::TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("cannot bind {}", args.bind))?;
    match (&root, &overlay) {
        (Some(r), Some(o)) => info!(
            "clapier listening on http://{} - serving {} + overlay {}",
            args.bind,
            r.display(),
            o.display()
        ),
        (Some(r), None) => info!(
            "clapier listening on http://{} - serving {}",
            args.bind,
            r.display()
        ),
        (None, Some(o)) => info!(
            "clapier listening on http://{} - serving overlay {}",
            args.bind,
            o.display()
        ),
        (None, None) => unreachable!("rejected above"),
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
