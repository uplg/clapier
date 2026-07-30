//! Bounded in-memory request journal. Data only - rendering lives in
//! `clapier-pages`, wiring in the `clapier` binary.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Instant;

use http::{Method, StatusCode};

/// One served request.
#[derive(Clone)]
pub struct Hit {
    pub at: Instant,
    pub peer: IpAddr,
    pub method: Method,
    pub uri: String,
    pub status: StatusCode,
    pub bytes: usize,
}

/// A thread-safe ring of the most recent hits.
pub struct Journal {
    capacity: usize,
    hits: Mutex<VecDeque<Hit>>,
}

impl Journal {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            hits: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn record(&self, hit: Hit) {
        let mut hits = self.hits.lock().expect("journal lock");
        if hits.len() >= self.capacity {
            hits.pop_front();
        }
        hits.push_back(hit);
    }

    /// Oldest first, like the ring itself.
    pub fn snapshot(&self) -> Vec<Hit> {
        self.hits
            .lock()
            .expect("journal lock")
            .iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn hit(uri: &str) -> Hit {
        Hit {
            at: Instant::now(),
            peer: IpAddr::V4(Ipv4Addr::LOCALHOST),
            method: Method::GET,
            uri: uri.to_string(),
            status: StatusCode::OK,
            bytes: 0,
        }
    }

    #[test]
    fn ring_is_bounded_and_ordered() {
        let journal = Journal::new(2);
        journal.record(hit("/a"));
        journal.record(hit("/b"));
        journal.record(hit("/c"));
        let snapshot = journal.snapshot();
        let uris: Vec<&str> = snapshot.iter().map(|h| h.uri.as_str()).collect();
        assert_eq!(uris, ["/b", "/c"]);
    }
}
