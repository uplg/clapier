//! Human-facing HTML for the burrow. The rabbit never reads these pages;
//! humans checking on the rabbit do. Presentation only - no I/O, no state.

use std::fmt::Write as _;
use std::time::Duration;

const CSS: &str = "body{font:14px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;margin:2rem auto;\
max-width:60rem;padding:0 1rem;background:#fff;color:#1a1a1a}\
h1{font-size:1.3rem}h2{font-size:1rem;margin:1.5rem 0 0}code{opacity:.7}\
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

/// One rabbit of the fleet table. Strings arrive pre-formatted; `-`
/// marks what the wire has not shown yet.
pub struct FleetRow {
    pub mac: String,
    pub ip: String,
    pub version: String,
    pub last_boot: Option<Duration>,
    pub last_pulse: Option<Duration>,
    pub uptime: Option<Duration>,
    pub link: Option<u8>,
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

pub fn render_status(
    uptime: Duration,
    root: &str,
    rabbit: &Rabbit,
    fleet: &[FleetRow],
    rows: &[Row],
) -> String {
    let rabbit_line = match rabbit {
        Rabbit::NotConfigured => "rabbit not configured (--rabbit)".to_string(),
        Rabbit::NotSeen(ip) => format!("🐰 {} - not seen yet", escape(ip)),
        Rabbit::Seen(ip, ago) => {
            format!("🐰 {} - last seen {} ago", escape(ip), humanize(*ago))
        }
    };
    let ago = |d: &Option<Duration>| match d {
        Some(d) => format!("{} ago", humanize(*d)),
        None => "-".to_string(),
    };
    let mut fleet_table = String::new();
    for row in fleet {
        let _ = write!(
            fleet_table,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&row.mac),
            escape(&row.ip),
            escape(&row.version),
            ago(&row.last_boot),
            ago(&row.last_pulse),
            row.uptime.map_or("-".to_string(), humanize),
            row.link.map_or("-".to_string(), |l| l.to_string()),
        );
    }
    let fleet_section = if fleet.is_empty() {
        "<p>no rabbit heard yet</p>".to_string()
    } else {
        format!(
            "<table><tr><th>rabbit</th><th>ip</th><th>garenne</th>\
<th>last bc.jsp</th><th>last pulse</th><th>uptime</th><th>link</th></tr>{fleet_table}</table>"
        )
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
<h2>fleet</h2>{fleet_section}\
<h2>requests</h2>\
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
