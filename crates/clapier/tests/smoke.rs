//! The rabbit's protocol, played against a real socket: HTTP/1.0, query
//! string on bc.jsp, and reading until the server closes the connection.

use std::io::{Read, Write};
use std::net::SocketAddr;

use clapier::AppState;

async fn spawn_server(root: std::path::PathBuf) -> SocketAddr {
    spawn_server_with_overlay(root, None).await
}

async fn spawn_server_with_overlay(
    root: std::path::PathBuf,
    overlay: Option<std::path::PathBuf>,
) -> SocketAddr {
    let app = AppState::new(root, overlay, None);
    let router = clapier::router(app);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("serve");
    });
    addr
}

fn raw_request(addr: SocketAddr, request: &'static [u8]) -> Vec<u8> {
    let mut sock = std::net::TcpStream::connect(addr).expect("connect");
    sock.write_all(request).expect("send");
    let mut buf = Vec::new();
    // Only returns once the server closes the connection - which is the
    // point: the rabbit reads exactly like this.
    sock.read_to_end(&mut buf).expect("read");
    buf
}

#[tokio::test]
async fn rabbit_fetches_bytecode_over_http10() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vl = dir.path().join("vl");
    std::fs::create_dir(&vl).expect("mkdir vl");
    let bytecode: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    std::fs::write(vl.join("bc.jsp"), &bytecode).expect("write bc.jsp");

    let addr = spawn_server(dir.path().to_path_buf()).await;
    let response = tokio::task::spawn_blocking(move || {
        raw_request(addr, b"GET /vl/bc.jsp?sn=0013&v=13 HTTP/1.0\r\n\r\n")
    })
    .await
    .expect("join");

    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("end of headers")
        + 4;
    let head = String::from_utf8_lossy(&response[..header_end]).to_ascii_lowercase();
    assert!(
        head.starts_with("http/1.0 200") || head.starts_with("http/1.1 200"),
        "unexpected status: {head}"
    );
    assert!(head.contains("connection: close"), "headers: {head}");
    assert!(
        head.contains(&format!("content-length: {}", bytecode.len())),
        "headers: {head}"
    );
    assert_eq!(
        &response[header_end..],
        &bytecode[..],
        "bytecode must arrive intact"
    );
}

/// The tribe overlay: a request carrying the boot's `m` param gets its
/// rabbit's file, an unknown or absent identity gets the common overlay,
/// a file absent from the overlay falls back to the base tree, and a
/// hostile `m` is ignored rather than reaching the filesystem.
#[tokio::test]
async fn overlay_routes_the_tribe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base");
    let overlay = dir.path().join("overlay");
    for sub in [
        "base/vl",
        "overlay/common/vl",
        "overlay/rabbits/0019db9c2815/vl",
    ] {
        std::fs::create_dir_all(dir.path().join(sub)).expect("mkdir");
    }
    std::fs::write(base.join("vl/bc.jsp"), b"BASE").expect("write");
    std::fs::write(base.join("vl/crontab.forth"), b"FORTH").expect("write");
    std::fs::write(overlay.join("common/vl/bc.jsp"), b"COMMON").expect("write");
    std::fs::write(overlay.join("rabbits/0019db9c2815/vl/bc.jsp"), b"CANARY").expect("write");

    let addr = spawn_server_with_overlay(base, Some(overlay)).await;
    let cases: &[(&'static [u8], &'static [u8])] = &[
        // The canary rabbit, exactly as the boot asks (double slash included).
        (
            b"GET /vl//bc.jsp?v=0.0.0.13&m=00:19:db:9c:28:15&h=4 HTTP/1.0\r\n\r\n",
            b"CANARY",
        ),
        // Another rabbit: no dedicated tree, gets the common overlay.
        (
            b"GET /vl/bc.jsp?m=aa:bb:cc:dd:ee:ff HTTP/1.0\r\n\r\n",
            b"COMMON",
        ),
        // No identity at all: common overlay still wins over base.
        (b"GET /vl/bc.jsp HTTP/1.0\r\n\r\n", b"COMMON"),
        // Not in the overlay: base tree serves it.
        (
            b"GET /vl/crontab.forth?m=00:19:db:9c:28:15 HTTP/1.0\r\n\r\n",
            b"FORTH",
        ),
        // Hostile identity: ignored, treated as absent.
        (b"GET /vl/bc.jsp?m=../../etc HTTP/1.0\r\n\r\n", b"COMMON"),
    ];
    for (request, expected) in cases {
        let response = tokio::task::spawn_blocking(move || raw_request(addr, request))
            .await
            .expect("join");
        let header_end = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("end of headers")
            + 4;
        assert_eq!(
            &response[header_end..],
            *expected,
            "request: {}",
            String::from_utf8_lossy(request)
        );
    }
}

#[tokio::test]
async fn missing_file_gives_404() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("vl")).expect("mkdir vl");

    let addr = spawn_server(dir.path().to_path_buf()).await;
    let response = tokio::task::spawn_blocking(move || {
        raw_request(addr, b"GET /vl/missing.jsp HTTP/1.0\r\n\r\n")
    })
    .await
    .expect("join");

    let head = String::from_utf8_lossy(&response).to_ascii_lowercase();
    assert!(head.contains(" 404 "), "response: {head}");
}
