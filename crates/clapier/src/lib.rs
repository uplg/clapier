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
use clapier_journal::{Hit, Journal};
use clapier_pages as pages;
use tracing::info;

/// Number of requests kept for the status page.
const HISTORY: usize = 200;

pub struct AppState {
    pub root: PathBuf,
    pub rabbit: Option<IpAddr>,
    started: Instant,
    journal: Journal,
}

impl AppState {
    pub fn new(root: PathBuf, rabbit: Option<IpAddr>) -> Arc<Self> {
        Arc::new(Self {
            root,
            rabbit,
            started: Instant::now(),
            journal: Journal::new(HISTORY),
        })
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
    let mut resp = clapier_vl::respond(&app.root, &method, &uri).await;
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
    Html(pages::render_status(
        app.started.elapsed(),
        &app.root.display().to_string(),
        &rabbit,
        &rows,
    ))
}
