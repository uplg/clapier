//! Composition of the burrow: state, router, request recording.
//!
//! The pieces live in their own crates - `clapier-vl` speaks the rabbit's
//! dialect, `clapier-journal` remembers requests, `clapier-pages` renders
//! for humans. This crate only wires them together.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, Form, Query, State},
    http::{Method, Uri, header},
    response::{Html, Redirect, Response},
    routing::{get, post},
};
use clapier_fleet::Fleet;
use clapier_journal::{Hit, Journal};
use clapier_pages as pages;
use clapier_vl::ContentTree;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;

/// Number of requests kept for the status page.
const HISTORY: usize = 200;

pub struct AppState {
    pub tree: ContentTree,
    pub rabbit: Option<IpAddr>,
    pub fleet: Fleet,
    /// The speech pipeline lives Mac-side (TTS model, encoder); the
    /// pilot page shells out to it when configured.
    pub say_script: Option<PathBuf>,
    /// The garenne bytecode to install for rabbits whose burrow has no
    /// bc.jsp yet (the adoption); unset turns adoption off.
    pub garenne: Option<PathBuf>,
    started: Instant,
    journal: Journal,
}

impl AppState {
    pub fn new(
        base: Option<PathBuf>,
        overlay: Option<PathBuf>,
        rabbit: Option<IpAddr>,
        say_script: Option<PathBuf>,
        garenne: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            tree: ContentTree { base, overlay },
            rabbit,
            fleet: Fleet::new(),
            say_script,
            garenne,
            started: Instant::now(),
            journal: Journal::new(HISTORY),
        })
    }

    /// The adoption: install garenne as `bc.jsp` in the rabbit's burrow.
    /// Atomic within the overlay filesystem, so the rabbit never fetches
    /// half a brain. Returns false when adoption is off or impossible.
    fn adopt(&self, mac: &str) -> bool {
        let (Some(bin), Some(overlay)) = (&self.garenne, &self.tree.overlay) else {
            return false;
        };
        let dir = overlay.join("rabbits").join(mac).join("vl");
        let install = || -> std::io::Result<()> {
            std::fs::create_dir_all(&dir)?;
            let tmp = dir.join(".bc.jsp.tmp");
            std::fs::copy(bin, &tmp)?;
            std::fs::rename(&tmp, dir.join("bc.jsp"))
        };
        match install() {
            Ok(()) => {
                info!("🐰 adopted {mac}: garenne installed in its burrow");
                true
            }
            Err(e) => {
                info!("could not adopt {mac}: {e}");
                false
            }
        }
    }
}

/// Ask a rabbit its `/status` page, in its own dialect: HTTP/1.0, read
/// until the connection closes.
async fn fetch_status(ip: IpAddr) -> anyhow::Result<String> {
    let exchange = async {
        let mut sock = tokio::net::TcpStream::connect((ip, 80)).await?;
        sock.write_all(b"GET /status HTTP/1.0\r\n\r\n").await?;
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await?;
        let text = String::from_utf8_lossy(&buf).into_owned();
        Ok(text
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default())
    };
    tokio::time::timeout(Duration::from_secs(8), exchange).await?
}

/// Enrich the fleet with each named rabbit's `/status`, forever. One
/// rabbit at a time and a gentle cadence: the radio is shared and the
/// dashboard is a guest on it.
pub async fn poll_statuses(app: Arc<AppState>, every: Duration) {
    loop {
        tokio::time::sleep(every).await;
        let targets: Vec<IpAddr> = app
            .fleet
            .snapshot()
            .iter()
            .filter(|r| r.mac.is_some())
            .map(|r| r.ip)
            .collect();
        for ip in targets {
            if let Ok(body) = fetch_status(ip).await {
                app.fleet
                    .status(ip, clapier_fleet::parse_status(&body), Instant::now());
            }
        }
    }
}

/// One command over the rabbit's UDP control port, reply awaited.
pub async fn ctl_send(ip: IpAddr, cmd: &str) -> anyhow::Result<String> {
    let sock = tokio::net::UdpSocket::bind(("0.0.0.0", 0)).await?;
    // Broadcast targets are legal here, like the Python tool allowed.
    socket2::SockRef::from(&sock).set_broadcast(true)?;
    sock.send_to(format!("grn1 {cmd}").as_bytes(), (ip, 9998))
        .await?;
    let mut buf = [0u8; 2048];
    let (n, _) = tokio::time::timeout(Duration::from_secs(3), sock.recv_from(&mut buf)).await??;
    Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
}

/// The pilot page's gate: only commands a button should reach, with
/// their arguments bounded. The volume floor is not a style choice:
/// below 40 the amplifier's draw has already crashed a supply and
/// wedged the radio.
pub fn vet_ctl(cmd: &str) -> Result<String, String> {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    match words.as_slice() {
        [] => Err("empty command".to_string()),
        [verb @ ("ping" | "dance" | "reboot" | "stop" | "conf" | "heap" | "fetch")] => {
            Ok((*verb).to_string())
        }
        ["vol", n] => match n.parse::<u32>() {
            Ok(n) if (40..=254).contains(&n) => Ok(format!("vol {n}")),
            Ok(_) => Err("vol stays at 40 or above, the supply remembers".to_string()),
            Err(_) => Err("vol wants a number".to_string()),
        },
        ["color", c] if c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()) => {
            Ok(format!("color {c}"))
        }
        ["led", i, c]
            if i.len() == 1
                && ('0'..='4').contains(&i.chars().next().unwrap_or('9'))
                && c.len() == 6
                && c.chars().all(|ch| ch.is_ascii_hexdigit()) =>
        {
            Ok(format!("led {i} {c}"))
        }
        ["ears", a, b] => match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(a), Ok(b)) if a <= 16 && b <= 16 => Ok(format!("ears {a} {b}")),
            _ => Err("ears wants two positions in 0..16".to_string()),
        },
        [verb @ ("chor" | "play"), path]
            if path.starts_with("/vl/") && !path.contains("..") && path.len() < 128 =>
        {
            Ok(format!("{verb} {path}"))
        }
        _ => Err(format!("command not allowed: {cmd}")),
    }
}

fn mac_ok(mac: &str) -> bool {
    mac.len() == 12
        && mac
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_lets_buttons_through() {
        assert_eq!(vet_ctl("ping"), Ok("ping".to_string()));
        assert_eq!(vet_ctl("  dance  "), Ok("dance".to_string()));
        assert_eq!(vet_ctl("vol 40"), Ok("vol 40".to_string()));
        assert_eq!(vet_ctl("color 7c5cff"), Ok("color 7c5cff".to_string()));
        assert_eq!(vet_ctl("ears 8 8"), Ok("ears 8 8".to_string()));
        assert_eq!(
            vet_ctl("chor /vl/config/chor/taichi.chor"),
            Ok("chor /vl/config/chor/taichi.chor".to_string())
        );
    }

    #[test]
    fn the_gate_remembers_the_supply() {
        assert!(vet_ctl("vol 39").is_err());
        assert!(vet_ctl("vol 0").is_err());
    }

    #[test]
    fn the_gate_stops_everything_else() {
        assert!(vet_ctl("").is_err());
        assert!(vet_ctl("log 0").is_err());
        assert!(vet_ctl("reassoc").is_err());
        assert!(vet_ctl("chor /etc/passwd").is_err());
        assert!(vet_ctl("chor /vl/../secret").is_err());
        assert!(vet_ctl("ears 99 0").is_err());
        assert!(vet_ctl("color zzzzzz").is_err());
    }

    #[test]
    fn macs_are_twelve_lowercase_hex() {
        assert!(mac_ok("0019db9c2815"));
        assert!(!mac_ok("0019DB9C2815"));
        assert!(!mac_ok("0019db9c28"));
        assert!(!mac_ok("0019db9c28xy"));
    }
}

fn pilot_redirect(msg: &str) -> Redirect {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let encoded = utf8_percent_encode(msg, NON_ALPHANUMERIC).to_string();
    Redirect::to(&format!("/_clapier/pilot?m={encoded}"))
}

/// The log channel is shared: the ad-hoc listener scripts want the same
/// broadcast datagrams clapier does. SO_REUSEPORT makes the port a
/// party line instead of a fight - every bound socket receives each
/// broadcast (they also set it).
pub fn log_socket(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // Windows has no SO_REUSEPORT; the party line stays a unix affair and
    // the fleet listener simply owns the port there.
    #[cfg(unix)]
    sock.set_reuse_port(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&SocketAddr::from(([0, 0, 0, 0], port)).into())?;
    tokio::net::UdpSocket::from_std(sock.into())
}

/// Bind the log port, then prove the socket actually hears: on macOS a
/// SO_REUSEPORT join races the dying previous instance's socket, the
/// kernel's group steering freezes on the corpse, and the survivor
/// stays deaf to broadcasts while looking perfectly bound. The roll
/// call catches it: send ourselves a datagram, and rebind until it
/// arrives.
async fn log_socket_verified(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    for attempt in 1..=5u32 {
        let sock = log_socket(port)?;
        // The probe must be a broadcast: a deaf socket still receives
        // unicast, which fooled the first version of this roll call.
        // Success is any datagram at all - the probe looping back, or a
        // rabbit's own two-second pulse.
        let probe = tokio::net::UdpSocket::bind(("0.0.0.0", 0)).await?;
        socket2::SockRef::from(&probe).set_broadcast(true)?;
        let _ = probe
            .send_to(b"clapier roll call", ("255.255.255.255", port))
            .await;
        let mut buf = [0u8; 64];
        match tokio::time::timeout(Duration::from_millis(2500), sock.recv_from(&mut buf)).await {
            Ok(Ok(_)) => return Ok(sock),
            _ => {
                tracing::warn!("udp {port} bound but deaf (attempt {attempt}), rebinding");
                drop(sock);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    // Last try is returned unverified rather than giving up the port.
    log_socket(port)
}

/// Feeds the fleet register from the rabbits' UDP log channel. The
/// channel is broadcast on the LAN, so hearing every rabbit only takes
/// joining the port. Non-pulse chatter (`button: click`, reassoc
/// traces) is left to the human listener scripts.
pub async fn listen_pulses(app: Arc<AppState>, port: u16) {
    let sock = match log_socket_verified(port).await {
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
        .route("/_clapier/pilot", get(pilot_page))
        .route("/_clapier/ctl", post(ctl_post))
        .route("/_clapier/dance", post(dance_post))
        .route("/_clapier/say", post(say_post))
        .fallback(serve_content)
        .with_state(app)
}

/// Every .chor the overlay can serve, as the /vl paths the rabbits ask.
async fn chor_library(app: &AppState) -> Vec<String> {
    async fn collect(dir: PathBuf, prefix: &str, out: &mut Vec<String>) {
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.ends_with(".chor") {
                    out.push(format!("{prefix}/{name}"));
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(overlay) = &app.tree.overlay {
        collect(
            overlay.join("common/vl/config/chor"),
            "/vl/config/chor",
            &mut out,
        )
        .await;
        for rabbit in app.fleet.snapshot() {
            if let Some(mac) = rabbit.mac {
                collect(
                    overlay.join("rabbits").join(&mac).join("vl/chor"),
                    "/vl/chor",
                    &mut out,
                )
                .await;
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(serde::Deserialize)]
pub struct PilotQuery {
    m: Option<String>,
}

async fn pilot_page(State(app): State<Arc<AppState>>, Query(q): Query<PilotQuery>) -> Html<String> {
    let rabbits: Vec<pages::PilotRabbit> = app
        .fleet
        .snapshot()
        .into_iter()
        .filter_map(|r| {
            let mac = r.mac?;
            let state = r.status.as_ref().map_or_else(String::new, |s| {
                let mut parts = Vec::new();
                if let Some(rssi) = s.rssi {
                    parts.push(format!("rssi {rssi}"));
                }
                if let Some(ears) = &s.ears {
                    parts.push(format!("ears {ears}"));
                }
                if let Some(audio) = &s.audio {
                    parts.push(format!("audio {audio}"));
                }
                if let Some(choreo) = &s.choreo {
                    parts.push(format!("choreo {choreo}"));
                }
                parts.join(" - ")
            });
            Some(pages::PilotRabbit {
                mac,
                ip: r.ip.to_string(),
                state,
            })
        })
        .collect();
    let chors = chor_library(&app).await;
    Html(pages::render_pilot(
        &rabbits,
        &chors,
        q.m.as_deref(),
        app.say_script.is_some(),
    ))
}

#[derive(serde::Deserialize)]
pub struct CtlForm {
    ip: String,
    cmd: String,
}

async fn ctl_post(State(app): State<Arc<AppState>>, Form(form): Form<CtlForm>) -> Redirect {
    let Ok(ip) = form.ip.parse::<IpAddr>() else {
        return pilot_redirect("bad ip");
    };
    if !app.fleet.snapshot().iter().any(|r| r.ip == ip) {
        return pilot_redirect("that ip is not a rabbit the fleet knows");
    }
    match vet_ctl(&form.cmd) {
        Err(reason) => pilot_redirect(&reason),
        Ok(cmd) => match ctl_send(ip, &cmd).await {
            Ok(reply) => pilot_redirect(&reply),
            Err(_) => pilot_redirect("no reply (rebooting, dancing hard, or asleep)"),
        },
    }
}

#[derive(serde::Deserialize)]
pub struct DanceForm {
    mac: String,
    ip: String,
    cdl: String,
}

async fn dance_post(State(app): State<Arc<AppState>>, Form(form): Form<DanceForm>) -> Redirect {
    if !mac_ok(&form.mac) {
        return pilot_redirect("bad mac");
    }
    let Ok(ip) = form.ip.parse::<IpAddr>() else {
        return pilot_redirect("bad ip");
    };
    let Some(overlay) = app.tree.overlay.clone() else {
        return pilot_redirect("no overlay to write into");
    };
    match clapier_chor::encode_cdl(&form.cdl) {
        Err(reason) => pilot_redirect(&format!("choreography rejected: {reason}")),
        Ok(bytes) => {
            let dir = overlay.join("rabbits").join(&form.mac).join("vl/chor");
            let tmp = dir.join(".pilot.chor.tmp");
            let dest = dir.join("pilot.chor");
            let written = async {
                tokio::fs::create_dir_all(&dir).await?;
                tokio::fs::write(&tmp, &bytes).await?;
                tokio::fs::rename(&tmp, &dest).await
            }
            .await;
            if written.is_err() {
                return pilot_redirect("could not install the choreography");
            }
            match ctl_send(ip, "chor /vl/chor/pilot.chor").await {
                Ok(reply) => pilot_redirect(&format!("{} bytes installed, {reply}", bytes.len())),
                Err(_) => pilot_redirect("installed, but the rabbit did not answer"),
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct SayForm {
    text: String,
}

async fn say_post(State(app): State<Arc<AppState>>, Form(form): Form<SayForm>) -> Redirect {
    let Some(script) = app.say_script.clone() else {
        return pilot_redirect("speech not configured: start clapier with --say-script");
    };
    let text = form.text.trim().to_string();
    if text.is_empty() || text.len() > 300 {
        return pilot_redirect("a sentence, not a silence nor a novel (300 chars max)");
    }
    // Generation takes tens of seconds on the TTS model; fire and
    // forget, the rabbit will start speaking when the MP3 lands.
    match tokio::process::Command::new(&script).arg(&text).spawn() {
        Ok(_) => pilot_redirect("Estelle is thinking, she speaks in a few seconds"),
        Err(_) => pilot_redirect("could not start the speech pipeline"),
    }
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
        // A rabbit asking for its bytecode and finding an empty burrow
        // gets garenne installed on the spot, then served normally.
        if boot
            && method == Method::GET
            && resp.status() == axum::http::StatusCode::NOT_FOUND
            && app.adopt(&mac)
        {
            resp = clapier_vl::respond(&app.tree, &method, &uri).await;
        }
        app.fleet.seen_http(&mac, peer.ip(), boot, Instant::now());
    }
    record(&app, peer.ip(), method.clone(), &uri, &resp);
    if method == Method::HEAD {
        *resp.body_mut() = Body::empty();
    }
    resp
}

fn record(app: &AppState, peer: IpAddr, method: Method, uri: &Uri, resp: &Response) {
    // Browser reflexes are not traffic worth remembering.
    if uri.path() == "/favicon.ico" {
        return;
    }
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
            rssi: r.status.as_ref().and_then(|s| s.rssi),
            audio: r.status.as_ref().and_then(|s| s.audio.clone()),
            choreo: r.status.as_ref().and_then(|s| s.choreo.clone()),
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
    let serving = match (&app.tree.base, &app.tree.overlay) {
        (Some(base), Some(overlay)) => {
            format!("{} + overlay {}", base.display(), overlay.display())
        }
        (Some(base), None) => base.display().to_string(),
        (None, Some(overlay)) => format!("overlay {}", overlay.display()),
        (None, None) => "nothing (empty tree)".to_string(),
    };
    Html(pages::render_status(
        app.started.elapsed(),
        &serving,
        &rabbit,
        &fleet,
        &rows,
    ))
}
