//! Human-facing HTML for the burrow. The rabbit never reads these pages;
//! humans checking on the rabbit do. Presentation only - no I/O, no state.

use std::fmt::Write as _;
use std::time::Duration;

const CSS: &str = "body{font:14px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;margin:2rem auto;\
max-width:60rem;padding:0 1rem;background:#fff;color:#1a1a1a}\
h1{font-size:1.3rem}code{opacity:.7}\
table{border-collapse:collapse;width:100%;margin-top:1rem}\
td,th{padding:.25rem .6rem;border-bottom:1px solid #8884;text-align:left;white-space:nowrap}\
tr.r{background:#7c5cff20}\
@media(prefers-color-scheme:dark){body{background:#101014;color:#d8d8de}}";

/// What the status page can say about the rabbit.
pub enum Rabbit {
    NotConfigured,
    NotSeen(String),
    Seen(String, Duration),
}

/// One request row on the status page.
pub struct Row {
    pub ago: Duration,
    pub peer: String,
    pub request: String,
    pub status: u16,
    pub bytes: usize,
    pub rabbit: bool,
}

pub fn render_status(uptime: Duration, root: &str, rabbit: &Rabbit, rows: &[Row]) -> String {
    let rabbit_line = match rabbit {
        Rabbit::NotConfigured => "rabbit not configured (--rabbit)".to_string(),
        Rabbit::NotSeen(ip) => format!("🐰 {} - not seen yet", escape(ip)),
        Rabbit::Seen(ip, ago) => {
            format!("🐰 {} - last seen {} ago", escape(ip), humanize(*ago))
        }
    };
    let mut table = String::new();
    for row in rows {
        let _ = write!(
            table,
            "<tr{}><td>{} ago</td><td>{}</td><td>{}</td><td>{}</td><td>{} B</td></tr>",
            if row.rabbit { " class=\"r\"" } else { "" },
            humanize(row.ago),
            escape(&row.peer),
            escape(&row.request),
            row.status,
            row.bytes,
        );
    }
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta http-equiv=\"refresh\" content=\"5\">\
<title>clapier</title><style>{CSS}</style>\
<body><h1>🐰 clapier</h1>\
<p>up {} - serving <code>{}</code></p>\
<p>{rabbit_line}</p>\
<table><tr><th>when</th><th>peer</th><th>request</th><th>status</th><th>size</th></tr>{table}</table>\
</body></html>",
        humanize(uptime),
        escape(root),
    )
}

pub fn render_listing(path: &str, entries: &[String]) -> String {
    let title = escape(path);
    let mut page = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>{title}</title>\
<body><h1>{title}</h1><ul>"
    );
    for entry in entries {
        let esc = escape(entry);
        let _ = write!(page, "<li><a href=\"{esc}\">{esc}</a></li>");
    }
    page.push_str("</ul></body></html>");
    page
}

pub fn humanize(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min {:02} s", s / 60, s % 60)
    } else if s < 86400 {
        format!("{} h {:02} min", s / 3600, (s % 3600) / 60)
    } else {
        format!("{} d {} h", s / 86400, (s % 86400) / 3600)
    }
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_is_readable() {
        assert_eq!(humanize(Duration::from_secs(42)), "42 s");
        assert_eq!(humanize(Duration::from_secs(125)), "2 min 05 s");
        assert_eq!(humanize(Duration::from_secs(7500)), "2 h 05 min");
        assert_eq!(humanize(Duration::from_secs(200_000)), "2 d 7 h");
    }

    #[test]
    fn escape_neutralizes_html() {
        assert_eq!(
            escape("<a href=\"x\">&"),
            "&lt;a href=&quot;x&quot;&gt;&amp;"
        );
    }

    #[test]
    fn listing_escapes_entries() {
        let page = render_listing("/vl/", &["<script>".to_string()]);
        assert!(!page.contains("<script>"));
    }
}
