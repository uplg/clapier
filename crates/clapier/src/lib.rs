//! Composition of the burrow: state, router, request recording.
//!
//! The pieces live in their own crates - `clapier-vl` speaks the rabbit's
//! dialect, `clapier-journal` remembers requests, `clapier-pages` renders
//! for humans. This crate only wires them together.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, State},
    http::{Method, Uri, header},
    response::{Html, Response},
    routing::get,
};
use clapier_fleet::Fleet;
use clapier_journal::{Hit, Journal};
use clapier_pages as pages;
use clapier_vl::ContentTree;
use tracing::info;

/// Number of requests kept for the status page.
const HISTORY: usize = 200;

pub struct AppState {
    pub tree: ContentTree,
    pub rabbit: Option<IpAddr>,
    pub fleet: Fleet,
    started: Instant,
    journal: Journal,
}

impl AppState {
    pub fn new(base: PathBuf, overlay: Option<PathBuf>, rabbit: Option<IpAddr>) -> Arc<Self> {
        Arc::new(Self {
            tree: ContentTree { base, overlay },
            rabbit,
            fleet: Fleet::new(),
            started: Instant::now(),
            journal: Journal::new(HISTORY),
        })
    }
}

/// The log channel is shared: the ad-hoc listener scripts want the same
/// broadcast datagrams clapier does. SO_REUSEPORT makes the port a
/// party line instead of a fight - every bound socket receives each
/// broadcast (they also set it).
fn pulse_socket(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_port(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&SocketAddr::from(([0, 0, 0, 0], port)).into())?;
    tokio::net::UdpSocket::from_std(sock.into())
}

/// Feeds the fleet register from the rabbits' UDP log channel. The
/// channel is broadcast on the LAN, so hearing every rabbit only takes
/// joining the port. Non-pulse chatter (`button: click`, reassoc
/// traces) is left to the human listener scripts.
pub async fn listen_pulses(app: Arc<AppState>, port: u16) {
    let sock = match pulse_socket(port) {
        Ok(sock) => sock,
        Err(err) => {
            tracing::warn!("cannot hear pulses, udp {port} unavailable: {err}");
            return;
        }
    };
    info!("hearing pulses on udp {port}");
    let mut buf = [0u8; 512];
    loop {
        let (n, src) = match sock.recv_from(&mut buf).await {
            Ok(received) => received,
            Err(err) => {
                tracing::warn!("pulse listener stopped: {err}");
                return;
            }
        };
        let line = String::from_utf8_lossy(&buf[..n]);
        if let Some(pulse) = clapier_fleet::parse_pulse(&line) {
            app.fleet.pulse(src.ip(), pulse, Instant::now());
        }
    }
}

pub fn router(app: Arc<AppState>) -> Router {
    Router::new()
        .route("/_clapier", get(status_page))
        .route("/_clapier/health", get(|| async { "ok" }))
        .fallback(serve_content)
        .with_state(app)
}

async fn serve_content(
    State(app): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
) -> Response {
    let mut resp = clapier_vl::respond(&app.tree, &method, &uri).await;
    if let Some(mac) = clapier_vl::rabbit_id(&uri) {
        let boot = uri.path().ends_with("bc.jsp");
        app.fleet.seen_http(&mac, peer.ip(), boot, Instant::now());
    }
    record(&app, peer.ip(), method.clone(), &uri, &resp);
    if method == Method::HEAD {
        *resp.body_mut() = Body::empty();
    }
    resp
}

fn record(app: &AppState, peer: IpAddr, method: Method, uri: &Uri, resp: &Response) {
    let bytes = resp
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let status = resp.status();
    let tag = if Some(peer) == app.rabbit {
        "🐰 "
    } else {
        ""
    };
    info!("{tag}{peer} {method} {uri} → {} {bytes} B", status.as_u16());
    app.journal.record(Hit {
        at: Instant::now(),
        peer,
        method,
        uri: uri.to_string(),
        status,
        bytes,
    });
}

async fn status_page(State(app): State<Arc<AppState>>) -> Html<String> {
    let snapshot = app.journal.snapshot();
    let rabbit = match app.rabbit {
        None => pages::Rabbit::NotConfigured,
        Some(ip) => match snapshot.iter().rev().find(|h| h.peer == ip) {
            Some(hit) => pages::Rabbit::Seen(ip.to_string(), hit.at.elapsed()),
            None => pages::Rabbit::NotSeen(ip.to_string()),
        },
    };
    let fleet: Vec<pages::FleetRow> = app
        .fleet
        .snapshot()
        .iter()
        .map(|r| pages::FleetRow {
            mac: r.mac.clone().unwrap_or_else(|| "?".to_string()),
            ip: r.ip.to_string(),
            version: r
                .pulse
                .as_ref()
                .map_or("-".to_string(), |p| p.version.clone()),
            last_boot: r.last_boot.map(|at| at.elapsed()),
            last_pulse: r.last_pulse.map(|at| at.elapsed()),
            uptime: r
                .pulse
                .as_ref()
                .map(|p| std::time::Duration::from_secs(p.uptime_s)),
            link: r.pulse.as_ref().map(|p| p.link),
        })
        .collect();
    let rows: Vec<pages::Row> = snapshot
        .iter()
        .rev()
        .map(|hit| pages::Row {
            ago: hit.at.elapsed(),
            peer: hit.peer.to_string(),
            request: format!("{} {}", hit.method, hit.uri),
            status: hit.status.as_u16(),
            bytes: hit.bytes,
            rabbit: Some(hit.peer) == app.rabbit,
        })
        .collect();
    let serving = match &app.tree.overlay {
        Some(overlay) => format!(
            "{} + overlay {}",
            app.tree.base.display(),
            overlay.display()
        ),
        None => app.tree.base.display().to_string(),
    };
    Html(pages::render_status(
        app.started.elapsed(),
        &serving,
        &rabbit,
        &fleet,
        &rows,
    ))
}
