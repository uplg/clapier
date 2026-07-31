//! The fleet register: one entry per rabbit, remembered from what
//! already travels on the wire. Nothing is asked of the rabbits:
//!
//! - the `m` query param on HTTP requests binds a MAC to a peer IP and
//!   dates the last `bc.jsp` boot fetch;
//! - the UDP log channel's pulse lines (`garenne 0.8.2 up=42s link=4`)
//!   carry the running version, uptime and link state.
//!
//! Data only - the UDP socket and HTTP hooks live in the `clapier`
//! binary, rendering in `clapier-pages`.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Instant;

/// What a pulse line carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pulse {
    pub version: String,
    pub uptime_s: u64,
    pub link: u8,
    /// The sender's MAC (12 lowercase hex), carried by garenne 0.11.1+
    /// so the fleet re-identifies rabbits from the pulse alone after a
    /// server restart. Older pulses simply lack it.
    pub mac: Option<String>,
}

/// Parses a log-channel line as a pulse. The channel also carries free
/// prose (`button: click`, `reassoc: scanning ...`); anything that is
/// not exactly a pulse is `None`, never an error.
pub fn parse_pulse(line: &str) -> Option<Pulse> {
    let mut words = line.split_whitespace();
    if words.next() != Some("garenne") {
        return None;
    }
    let version = words.next()?.to_string();
    let uptime_s = words
        .next()?
        .strip_prefix("up=")?
        .strip_suffix('s')?
        .parse()
        .ok()?;
    let link = words.next()?.strip_prefix("link=")?.parse().ok()?;
    // Trailing fields are optional and order-free, so the format can
    // grow without breaking older parsers (this one included).
    let mac = words
        .filter_map(|w| w.strip_prefix("mac="))
        .map(|m| m.replace(':', "").to_ascii_lowercase())
        .find(|m| m.len() == 12 && m.chars().all(|c| c.is_ascii_hexdigit()));
    Some(Pulse {
        version,
        uptime_s,
        link,
        mac,
    })
}

/// What a polled `/status` answer carries, beyond what the pulse
/// already said. The rabbit's page is line-oriented `key=value`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub rssi: Option<i64>,
    pub ears: Option<String>,
    pub audio: Option<String>,
    pub choreo: Option<String>,
    pub tcp_retx: Option<u64>,
    pub reassoc: Option<u64>,
}

/// Parses a `/status` body. Unknown keys are ignored, missing keys stay
/// `None`: the dashboard renders what this garenne version publishes.
pub fn parse_status(body: &str) -> Status {
    let mut status = Status::default();
    for line in body.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        match key {
            "rssi" => status.rssi = value.parse().ok(),
            "ears" => status.ears = Some(value.to_string()),
            "audio" => status.audio = Some(value.to_string()),
            "choreo" => status.choreo = Some(value.to_string()),
            "tcp_retx" => status.tcp_retx = value.parse().ok(),
            "reassoc" => status.reassoc = value.parse().ok(),
            _ => {}
        }
    }
    status
}

/// One rabbit as the wire has shown it so far. A pulse heard before any
/// HTTP request yields an entry with an IP and no MAC; the first
/// request carrying `m=` claims it.
#[derive(Clone)]
pub struct Rabbit {
    /// 12 lowercase hex chars, exactly what `clapier_vl::rabbit_id`
    /// yields; `None` for a rabbit only heard pulsing.
    pub mac: Option<String>,
    pub ip: IpAddr,
    pub last_boot: Option<Instant>,
    pub last_pulse: Option<Instant>,
    pub pulse: Option<Pulse>,
    pub status: Option<Status>,
    pub last_status: Option<Instant>,
}

/// A thread-safe register of every rabbit seen since startup. Bounded
/// by the LAN itself - a rabbit is a physical object.
pub struct Fleet {
    rabbits: Mutex<Vec<Rabbit>>,
}

impl Fleet {
    pub fn new() -> Self {
        Self {
            rabbits: Mutex::new(Vec::new()),
        }
    }

    /// An HTTP request carrying a valid `m` param: bind the MAC to the
    /// peer IP, adopting any MAC-less entry the pulses created there.
    /// `boot` marks a `bc.jsp` fetch, the rabbit's first breath.
    pub fn seen_http(&self, mac: &str, ip: IpAddr, boot: bool, at: Instant) {
        let mut rabbits = self.rabbits.lock().expect("fleet lock");
        let entry = match rabbits.iter_mut().find(|r| r.mac.as_deref() == Some(mac)) {
            Some(entry) => entry,
            None => {
                let adopted = rabbits.iter_mut().find(|r| r.mac.is_none() && r.ip == ip);
                match adopted {
                    Some(entry) => entry,
                    None => {
                        rabbits.push(Rabbit {
                            mac: None,
                            ip,
                            last_boot: None,
                            last_pulse: None,
                            pulse: None,
                            status: None,
                            last_status: None,
                        });
                        rabbits.last_mut().expect("just pushed")
                    }
                }
            }
        };
        entry.mac = Some(mac.to_string());
        entry.ip = ip; // a re-lease moves the rabbit, follow it
        if boot {
            entry.last_boot = Some(at);
        }
    }

    /// A pulse heard from `ip`: update the rabbit living there, or open
    /// a MAC-less entry until an HTTP request names it.
    pub fn pulse(&self, ip: IpAddr, pulse: Pulse, at: Instant) {
        let mut rabbits = self.rabbits.lock().expect("fleet lock");
        match rabbits.iter_mut().find(|r| r.ip == ip) {
            Some(entry) => {
                if pulse.mac.is_some() {
                    entry.mac = pulse.mac.clone();
                }
                entry.last_pulse = Some(at);
                entry.pulse = Some(pulse);
            }
            None => rabbits.push(Rabbit {
                mac: pulse.mac.clone(),
                ip,
                last_boot: None,
                last_pulse: Some(at),
                pulse: Some(pulse),
                status: None,
                last_status: None,
            }),
        }
    }

    /// A polled `/status` answer from `ip`. Unknown IPs are ignored:
    /// the poller only asks rabbits the register already knows.
    pub fn status(&self, ip: IpAddr, status: Status, at: Instant) {
        let mut rabbits = self.rabbits.lock().expect("fleet lock");
        if let Some(entry) = rabbits.iter_mut().find(|r| r.ip == ip) {
            entry.status = Some(status);
            entry.last_status = Some(at);
        }
    }

    /// Every rabbit seen so far, named ones first, in MAC order.
    pub fn snapshot(&self) -> Vec<Rabbit> {
        let mut rabbits = self.rabbits.lock().expect("fleet lock").clone();
        rabbits.sort_by(|a, b| match (&a.mac, &b.mac) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.ip.cmp(&b.ip),
        });
        rabbits
    }
}

impl Default for Fleet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
    }

    #[test]
    fn status_page_parses_line_by_line() {
        let s = parse_status(
            "garenne=0.11.0\r\nrssi=-54\r\nears=0,0\r\naudio=playing 12B\r\n\
             choreo=idle\r\ntcp_retx=3\r\nreassoc=0\r\nnovel_key=x\r\n",
        );
        assert_eq!(s.rssi, Some(-54));
        assert_eq!(s.ears.as_deref(), Some("0,0"));
        assert_eq!(s.audio.as_deref(), Some("playing 12B"));
        assert_eq!(s.choreo.as_deref(), Some("idle"));
        assert_eq!(s.tcp_retx, Some(3));
        assert_eq!(s.reassoc, Some(0));
    }

    #[test]
    fn status_lands_on_the_known_rabbit_only() {
        let fleet = Fleet::new();
        fleet.pulse(
            ip(155),
            Pulse {
                version: "0.11.0".into(),
                uptime_s: 1,
                link: 4,
                mac: None,
            },
            Instant::now(),
        );
        fleet.status(ip(155), parse_status("rssi=-40\r\n"), Instant::now());
        fleet.status(ip(99), parse_status("rssi=-1\r\n"), Instant::now());
        let snap = fleet.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].status.as_ref().and_then(|s| s.rssi), Some(-40));
    }

    #[test]
    fn pulse_lines_parse() {
        assert_eq!(
            parse_pulse("garenne 0.8.2 up=42s link=4"),
            Some(Pulse {
                version: "0.8.2".to_string(),
                uptime_s: 42,
                link: 4,
                mac: None,
            })
        );
        // garenne 0.11.1 signs its pulses; the fleet re-identifies a
        // rabbit from the pulse alone after a server restart.
        assert_eq!(
            parse_pulse("garenne 0.11.1 up=42s link=4 mac=00:19:DB:9c:28:15").and_then(|p| p.mac),
            Some("0019db9c2815".to_string())
        );
        assert_eq!(
            parse_pulse("garenne 0.11.1 up=1s link=4 mac=not-a-mac").and_then(|p| p.mac),
            None
        );
        for line in [
            "button: click",
            "reassoc: scanning Freebox",
            "garenne",
            "garenne 0.8.2",
            "garenne 0.8.2 up=42 link=4",   // missing the s
            "garenne 0.8.2 up=s link=4",    // empty number
            "garenne 0.8.2 up=42s link=up", // non-numeric link
            "",
        ] {
            assert_eq!(parse_pulse(line), None, "line accepted: {line}");
        }
    }

    #[test]
    fn http_names_a_pulsing_rabbit() {
        let fleet = Fleet::new();
        let now = Instant::now();
        let pulse = parse_pulse("garenne 0.8.2 up=42s link=4").unwrap();
        fleet.pulse(ip(155), pulse.clone(), now);
        assert!(fleet.snapshot()[0].mac.is_none());

        fleet.seen_http("0019db9c2815", ip(155), true, now);
        let rabbits = fleet.snapshot();
        assert_eq!(rabbits.len(), 1, "the pulse entry must be adopted");
        assert_eq!(rabbits[0].mac.as_deref(), Some("0019db9c2815"));
        assert_eq!(rabbits[0].pulse, Some(pulse));
        assert!(rabbits[0].last_boot.is_some());
    }

    #[test]
    fn a_new_lease_moves_the_rabbit() {
        let fleet = Fleet::new();
        let now = Instant::now();
        fleet.seen_http("0019db9c2815", ip(155), true, now);
        fleet.seen_http("0019db9c2815", ip(160), false, now);
        let rabbits = fleet.snapshot();
        assert_eq!(rabbits.len(), 1);
        assert_eq!(rabbits[0].ip, ip(160));
        let pulse = parse_pulse("garenne 0.8.2 up=7s link=4").unwrap();
        fleet.pulse(ip(160), pulse, now);
        assert_eq!(fleet.snapshot()[0].pulse.as_ref().unwrap().uptime_s, 7);
    }

    #[test]
    fn named_rabbits_sort_first() {
        let fleet = Fleet::new();
        let now = Instant::now();
        let pulse = parse_pulse("garenne 0.8.2 up=1s link=4").unwrap();
        fleet.pulse(ip(170), pulse, now);
        fleet.seen_http("ffeeddccbbaa", ip(156), false, now);
        fleet.seen_http("0019db9c2815", ip(155), false, now);
        let macs: Vec<Option<String>> = fleet.snapshot().iter().map(|r| r.mac.clone()).collect();
        assert_eq!(
            macs,
            [
                Some("0019db9c2815".to_string()),
                Some("ffeeddccbbaa".to_string()),
                None
            ]
        );
    }
}
