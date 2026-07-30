//! File service speaking the rabbit's dialect.
//!
//! A 2005 Nabaztag:tag runs its TCP stack inside a 2006 bytecode VM and
//! fetches everything (`/vl/bc.jsp`, Forth scripts, MP3s, choreographies)
//! over HTTP/1.0. The contract, explicit and tested here:
//!
//! - full bodies with `Content-Length`, never chunked;
//! - `Connection: close` on every response - the rabbit reads until the
//!   peer closes;
//! - query string ignored when resolving paths - except the `m` param
//!   (the rabbit's MAC, sent by the boot as `bc.jsp?v=…&m=00:19:…`),
//!   which routes the request through the tribe overlay;
//! - path traversal refused, percent-encoded or not; a malformed `m` is
//!   ignored, never an error - the rabbit must eat.

use std::path::{Component, Path, PathBuf};

use axum::{
    body::Body,
    http::{HeaderValue, Method, StatusCode, Uri, header},
    response::Response,
};

/// What a request is resolved against: an optional overlay in front of
/// the base tree. Overlay lookups are tried per-rabbit
/// (`<overlay>/rabbits/<mac>/…`, when the request carries a valid `m`)
/// then common (`<overlay>/common/…`), and serve regular files only;
/// directories, index files and listings remain the base tree's business.
pub struct ContentTree {
    pub base: PathBuf,
    pub overlay: Option<PathBuf>,
}

/// Serves `uri` from the content tree.
///
/// Always returns a complete response (including errors) honoring the
/// rabbit contract. `HEAD` gets the same headers as `GET`; the caller is
/// expected to drop the body (axum does not do it for fallback handlers).
pub async fn respond(tree: &ContentTree, method: &Method, uri: &Uri) -> Response {
    if *method != Method::GET && *method != Method::HEAD {
        return plain(StatusCode::METHOD_NOT_ALLOWED, "method not allowed\n");
    }
    if let Some(overlay) = &tree.overlay {
        let mut roots = Vec::with_capacity(2);
        if let Some(id) = rabbit_id(uri) {
            roots.push(overlay.join("rabbits").join(id));
        }
        roots.push(overlay.join("common"));
        for root in roots {
            // fs::read fails on directories, so a hit here is a real file.
            if let Some(path) = resolve(&root, uri.path())
                && let Ok(bytes) = tokio::fs::read(&path).await
            {
                return base_response(StatusCode::OK, content_type(&path), bytes);
            }
        }
    }
    let root = &tree.base;
    let Some(path) = resolve(root, uri.path()) else {
        return plain(StatusCode::BAD_REQUEST, "path rejected\n");
    };
    let path = match tokio::fs::metadata(&path).await {
        Ok(meta) if meta.is_dir() => {
            if !uri.path().ends_with('/') {
                return redirect(&format!("{}/", uri.path()));
            }
            let index = path.join("index.html");
            if tokio::fs::metadata(&index).await.is_ok() {
                index
            } else {
                return match listing(&path, uri.path()).await {
                    Ok(page) => base_response(
                        StatusCode::OK,
                        "text/html; charset=utf-8",
                        page.into_bytes(),
                    ),
                    Err(_) => plain(StatusCode::NOT_FOUND, "not found\n"),
                };
            }
        }
        Ok(_) => path,
        Err(_) => return plain(StatusCode::NOT_FOUND, "not found\n"),
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => base_response(StatusCode::OK, content_type(&path), bytes),
        Err(_) => plain(StatusCode::NOT_FOUND, "not found\n"),
    }
}

/// Resolves a URL path under `root`, refusing any traversal (`..`),
/// percent-encoded or not. The query string never reaches this point:
/// `Uri::path()` excludes it, which is exactly the behavior the rabbit's
/// `bc.jsp?sn=…` requests rely on.
fn resolve(root: &Path, raw: &str) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    if decoded.contains('\0') {
        return None;
    }
    let mut clean = root.to_path_buf();
    for comp in Path::new(decoded.as_ref()).components() {
        match comp {
            Component::Normal(c) => clean.push(c),
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    Some(clean)
}

/// Extracts the rabbit identity from the `m` query param and normalizes
/// it to 12 lowercase hex chars - the only alphabet that may ever reach a
/// filesystem path. Accepts `00:19:db:9c:28:15` (the boot's format) or
/// bare `0019db9c2815`; anything else is treated as absent.
fn rabbit_id(uri: &Uri) -> Option<String> {
    let raw = uri
        .query()?
        .split('&')
        .find_map(|kv| kv.strip_prefix("m="))?;
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    let parts: Vec<&str> = decoded.split(':').collect();
    let joined = match parts.len() {
        1 => decoded.to_string(),
        6 if parts.iter().all(|p| p.len() == 2) => parts.concat(),
        _ => return None,
    };
    (joined.len() == 12 && joined.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| joined.to_ascii_lowercase())
}

fn content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "yaml" | "yml" => "text/yaml",
        "txt" | "forth" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "mp3" => "audio/mpeg",
        "wav" => "audio/x-wav",
        "mid" | "midi" => "audio/midi",
        _ => "application/octet-stream",
    }
}

fn base_response(status: StatusCode, ctype: &str, bytes: Vec<u8>) -> Response {
    let len = bytes.len();
    let mut resp = Response::new(Body::from(bytes));
    *resp.status_mut() = status;
    let headers = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(ctype) {
        headers.insert(header::CONTENT_TYPE, v);
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    headers.insert(header::CONNECTION, HeaderValue::from_static("close"));
    resp
}

fn plain(status: StatusCode, msg: &str) -> Response {
    base_response(status, "text/plain; charset=utf-8", msg.as_bytes().to_vec())
}

fn redirect(location: &str) -> Response {
    let mut resp = plain(StatusCode::MOVED_PERMANENTLY, "");
    let loc = match HeaderValue::from_str(location) {
        Ok(v) => v,
        Err(_) => HeaderValue::from_static("/"),
    };
    resp.headers_mut().insert(header::LOCATION, loc);
    resp
}

async fn listing(dir: &Path, uri_path: &str) -> std::io::Result<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut rd = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = rd.next_entry().await? {
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            name.push('/');
        }
        entries.push(name);
    }
    entries.sort();
    Ok(clapier_pages::render_listing(uri_path, &entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_traversal() {
        let root = Path::new("/srv");
        assert!(resolve(root, "/../etc/passwd").is_none());
        assert!(resolve(root, "/vl/../../etc/passwd").is_none());
        assert!(resolve(root, "/vl/%2e%2e/secret").is_none());
    }

    #[test]
    fn resolve_decodes_and_joins() {
        let root = Path::new("/srv");
        assert_eq!(
            resolve(root, "/vl/bc.jsp").unwrap(),
            PathBuf::from("/srv/vl/bc.jsp")
        );
        assert_eq!(
            resolve(root, "/vl/config%2eforth").unwrap(),
            PathBuf::from("/srv/vl/config.forth")
        );
        assert_eq!(resolve(root, "/").unwrap(), PathBuf::from("/srv"));
    }

    #[test]
    fn rabbit_id_accepts_boot_and_bare_forms() {
        let boot: Uri = "/vl/bc.jsp?v=0.0.0.13&m=00:19:DB:9c:28:15&h=4"
            .parse()
            .unwrap();
        assert_eq!(rabbit_id(&boot).as_deref(), Some("0019db9c2815"));
        let bare: Uri = "/status?m=0019db9c2815".parse().unwrap();
        assert_eq!(rabbit_id(&bare).as_deref(), Some("0019db9c2815"));
    }

    #[test]
    fn rabbit_id_rejects_everything_else() {
        for query in [
            "m=..",
            "m=../../etc/passwd",
            "m=%2e%2e%2f%2e%2e",
            "m=00:19:db:9c:28",       // five groups
            "m=00:19:db:9c:28:15:aa", // seven groups
            "m=00:19:db:9c:28:1g",    // non-hex
            "m=0019db9c2815ff",       // too long
            "m=",
            "x=1",
        ] {
            let uri: Uri = format!("/vl/bc.jsp?{query}").parse().unwrap();
            assert_eq!(rabbit_id(&uri), None, "query accepted: {query}");
        }
    }

    #[test]
    fn rabbit_content_types() {
        assert_eq!(
            content_type(Path::new("bc.jsp")),
            "application/octet-stream"
        );
        assert_eq!(
            content_type(Path::new("crontab.forth")),
            "text/plain; charset=utf-8"
        );
        assert_eq!(content_type(Path::new("1.mp3")), "audio/mpeg");
        assert_eq!(
            content_type(Path::new("taichi.chor")),
            "application/octet-stream"
        );
    }
}
