//! Politeness towards the free Google translate endpoint that translation and romanization share.
//! Pure logic (no network, no clock of its own), unit-tested; `gtx.rs` does the actual requests.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Minimum time between two requests to the service, whichever feature sends them.
pub const MIN_GAP: Duration = Duration::from_millis(350);
/// After the service keeps answering 429, stop asking for this long.
pub const COOLDOWN: Duration = Duration::from_secs(60);
/// Tries per request (the first one plus retries after a 429 or a server error).
pub const MAX_ATTEMPTS: u32 = 4;

/// Hands out send times so requests never come faster than `MIN_GAP`, and enforces a cooldown.
#[derive(Debug)]
pub struct Pacer {
    next: Instant,
    blocked_until: Option<Instant>,
}

impl Pacer {
    pub fn new(now: Instant) -> Self {
        Pacer { next: now, blocked_until: None }
    }

    /// Reserve the next slot. `Ok(wait)`: sleep that long, then send. `Err(left)`: still cooling down.
    pub fn reserve(&mut self, now: Instant) -> Result<Duration, Duration> {
        if let Some(until) = self.blocked_until {
            if until > now {
                return Err(until - now);
            }
            self.blocked_until = None;
        }
        let start = self.next.max(now);
        self.next = start + MIN_GAP;
        Ok(start - now)
    }

    /// The service refused us for good: nobody sends anything until the cooldown is over.
    pub fn trip(&mut self, now: Instant) {
        self.blocked_until = Some(now + COOLDOWN);
    }
}

/// How long to wait before retry number `attempt` (0 = the first retry): 1 s, 2 s, 4 s, 8 s, and never
/// less than the server's own `Retry-After` (capped at 20 s).
pub fn backoff(attempt: u32, retry_after_secs: Option<u64>) -> Duration {
    let own = Duration::from_secs(1u64 << attempt.min(3));
    match retry_after_secs {
        Some(s) => own.max(Duration::from_secs(s.min(20))),
        None => own,
    }
}

/// Repeated lines (a chorus) only need to be asked about once. Returns the distinct texts in first-seen
/// order, and for every input the position of its text in that list.
pub fn dedupe<'a>(texts: &[&'a str]) -> (Vec<&'a str>, Vec<usize>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut unique: Vec<&'a str> = Vec::new();
    let mut which = Vec::with_capacity(texts.len());
    for &t in texts {
        let idx = *seen.entry(t).or_insert_with(|| {
            unique.push(t);
            unique.len() - 1
        });
        which.push(idx);
    }
    (unique, which)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_spaced_out() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0);
        assert_eq!(p.reserve(t0), Ok(Duration::ZERO));
        assert_eq!(p.reserve(t0), Ok(MIN_GAP));
        assert_eq!(p.reserve(t0), Ok(MIN_GAP * 2));
        // Much later the queue has drained: no waiting.
        assert_eq!(p.reserve(t0 + Duration::from_secs(10)), Ok(Duration::ZERO));
    }

    #[test]
    fn cooldown_blocks_everyone_then_ends() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0);
        p.trip(t0);
        assert_eq!(p.reserve(t0 + Duration::from_secs(10)), Err(Duration::from_secs(50)));
        assert_eq!(p.reserve(t0 + COOLDOWN), Ok(Duration::ZERO));
    }

    #[test]
    fn backoff_doubles_and_respects_retry_after() {
        assert_eq!(backoff(0, None), Duration::from_secs(1));
        assert_eq!(backoff(1, None), Duration::from_secs(2));
        assert_eq!(backoff(2, None), Duration::from_secs(4));
        assert_eq!(backoff(3, None), Duration::from_secs(8));
        assert_eq!(backoff(9, None), Duration::from_secs(8));
        assert_eq!(backoff(0, Some(5)), Duration::from_secs(5));
        assert_eq!(backoff(3, Some(2)), Duration::from_secs(8));
        assert_eq!(backoff(0, Some(3600)), Duration::from_secs(20));
    }

    #[test]
    fn repeated_lines_are_asked_once() {
        let (unique, which) = dedupe(&["a", "b", "a", "c", "b", "a"]);
        assert_eq!(unique, vec!["a", "b", "c"]);
        assert_eq!(which, vec![0, 1, 0, 2, 1, 0]);
        let (u, w) = dedupe(&[]);
        assert!(u.is_empty() && w.is_empty());
    }
}
