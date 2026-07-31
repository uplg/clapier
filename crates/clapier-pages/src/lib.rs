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
    pub rssi: Option<i64>,
    pub audio: Option<String>,
    pub choreo: Option<String>,
}

/// One rabbit on the pilot page: identity plus a one-line state.
pub struct PilotRabbit {
    pub mac: String,
    pub ip: String,
    pub state: String,
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
    let dash = |o: &Option<String>| match o {
        Some(s) => escape(s),
        None => "-".to_string(),
    };
    let mut fleet_table = String::new();
    for row in fleet {
        let _ = write!(
            fleet_table,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
<td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&row.mac),
            escape(&row.ip),
            escape(&row.version),
            ago(&row.last_boot),
            ago(&row.last_pulse),
            row.uptime.map_or("-".to_string(), humanize),
            row.link.map_or("-".to_string(), |l| l.to_string()),
            row.rssi.map_or("-".to_string(), |r| r.to_string()),
            dash(&row.audio),
            dash(&row.choreo),
        );
    }
    let fleet_section = if fleet.is_empty() {
        "<p>no rabbit heard yet</p>".to_string()
    } else {
        format!(
            "<table><tr><th>rabbit</th><th>ip</th><th>garenne</th>\
<th>last bc.jsp</th><th>last pulse</th><th>uptime</th><th>link</th>\
<th>rssi</th><th>audio</th><th>choreo</th></tr>{fleet_table}</table>\
<p><a href=\"/_clapier/pilot\">pilot the rabbits</a></p>"
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

/// The pilot page: one control block per rabbit. No auto-refresh here,
/// a human is typing. Every form posts to an endpoint that vets the
/// command before the radio hears anything.
pub fn render_pilot(
    rabbits: &[PilotRabbit],
    chors: &[String],
    message: Option<&str>,
    say_enabled: bool,
) -> String {
    let extra_css = "form{margin:.4rem 0}input,select,textarea,button{font:inherit;\
background:inherit;color:inherit;border:1px solid #8886;border-radius:4px;\
padding:.25rem .5rem}textarea{width:100%;min-height:4rem}button{cursor:pointer}\
fieldset{border:1px solid #8884;border-radius:6px;margin:1rem 0;padding:.5rem 1rem}\
legend{padding:0 .4rem}p.m{background:#7c5cff20;padding:.4rem .6rem;border-radius:4px}";
    let message_line = message.map_or(String::new(), |m| {
        format!("<p class=\"m\">{}</p>", escape(m))
    });
    let mut blocks = String::new();
    for rabbit in rabbits {
        let mac = escape(&rabbit.mac);
        let ip = escape(&rabbit.ip);
        let state = if rabbit.state.is_empty() {
            "state not polled yet".to_string()
        } else {
            escape(&rabbit.state)
        };
        let say_form = if say_enabled {
            "<form method=\"post\" action=\"/_clapier/say\">\
<textarea name=\"text\" maxlength=\"300\" \
placeholder=\"a sentence for Estelle\"></textarea>\
<button>say it</button></form>"
                .to_string()
        } else {
            "<p><code>--say-script</code> not configured, the rabbit stays quiet</p>".to_string()
        };
        let _ = write!(
            blocks,
            "<fieldset><legend>🐰 {mac} <code>{ip}</code></legend>\
<p>{state}</p>\
<form method=\"post\" action=\"/_clapier/ctl\">\
<input type=\"hidden\" name=\"ip\" value=\"{ip}\">\
<button name=\"cmd\" value=\"ping\">ping</button> \
<button name=\"cmd\" value=\"dance\">salute</button> \
<button name=\"cmd\" value=\"stop\">stop audio</button> \
<button name=\"cmd\" value=\"reboot\">reboot</button></form>\
<form method=\"post\" action=\"/_clapier/ctl\">\
<input type=\"hidden\" name=\"ip\" value=\"{ip}\">\
<select name=\"cmd\">{chor_options_named}</select> \
<button>dance it</button></form>\
<form method=\"post\" action=\"/_clapier/dance\">\
<input type=\"hidden\" name=\"mac\" value=\"{mac}\">\
<input type=\"hidden\" name=\"ip\" value=\"{ip}\">\
<textarea name=\"cdl\" \
placeholder=\"fps,t,led,4,124,92,255,t,motor,0,90,0,0,...\"></textarea>\
<button>encode and dance</button></form>\
{say_form}\
</fieldset>",
            chor_options_named = {
                let mut opts = String::new();
                for chor in chors {
                    let esc = escape(chor);
                    let _ = write!(opts, "<option value=\"chor {esc}\">{esc}</option>");
                }
                opts
            },
        );
    }
    if rabbits.is_empty() {
        blocks = "<p>no named rabbit yet - one boot fetch and it appears here</p>".to_string();
    }
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>clapier pilot</title><style>{CSS}{extra_css}</style>\
<body><h1>🐰 pilot</h1>\
<p><a href=\"/_clapier\">back to status</a></p>\
{message_line}{blocks}</body></html>"
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
