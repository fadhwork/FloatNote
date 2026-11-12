//! When should the overlay step aside? A few seconds after the music pauses or stops. Pure logic,
//! unit-tested; the window itself is hidden and shown by the main loop in lib.rs.

use std::time::{Duration, Instant};

/// Paused (a short break): wait a little longer before hiding.
pub const PAUSED_DELAY: Duration = Duration::from_secs(5);
/// Stopped (nothing is playing any more): hide sooner.
pub const STOPPED_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug)]
pub struct PlayState {
    kind: Kind,
    since: Instant,
    /// Music has played at least once this session. Until then the overlay stays visible, so a fresh
    /// launch with nothing playing still shows its hint instead of vanishing.
    seen_playing: bool,
}

impl PlayState {
    pub fn new(now: Instant) -> Self {
        PlayState { kind: Kind::Stopped, since: now, seen_playing: false }
    }

    pub fn update(&mut self, now: Instant, kind: Kind) {
        if kind == Kind::Playing {
            self.seen_playing = true;
        }
        if kind != self.kind {
            self.kind = kind;
            self.since = now;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.kind == Kind::Playing
    }

    /// True once the music has been paused for 5 s, or stopped for 3 s (after something has played).
    pub fn should_hide(&self, now: Instant) -> bool {
        if !self.seen_playing {
            return false;
        }
        let delay = match self.kind {
            Kind::Playing => return false,
            Kind::Paused => PAUSED_DELAY,
            Kind::Stopped => STOPPED_DELAY,
        };
        now.saturating_duration_since(self.since) >= delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(base: Instant, s: u64) -> Instant {
        base + Duration::from_secs(s)
    }

    #[test]
    fn pause_waits_five_seconds_stop_waits_three() {
        let t0 = Instant::now();
        let mut p = PlayState::new(t0);
        p.update(t0, Kind::Playing);
        assert!(!p.should_hide(secs(t0, 100)));

        p.update(secs(t0, 10), Kind::Paused);
        assert!(!p.should_hide(secs(t0, 14)));
        assert!(p.should_hide(secs(t0, 15)));

        p.update(secs(t0, 20), Kind::Stopped);
        assert!(!p.should_hide(secs(t0, 22)));
        assert!(p.should_hide(secs(t0, 23)));
    }

    #[test]
    fn playing_again_brings_it_back_and_restarts_the_timer() {
        let t0 = Instant::now();
        let mut p = PlayState::new(t0);
        p.update(t0, Kind::Playing);
        p.update(secs(t0, 5), Kind::Paused);
        assert!(p.should_hide(secs(t0, 11)));
        p.update(secs(t0, 12), Kind::Playing);
        assert!(p.is_playing() && !p.should_hide(secs(t0, 13)));
        p.update(secs(t0, 20), Kind::Paused);
        assert!(!p.should_hide(secs(t0, 24)));
    }

    #[test]
    fn a_fresh_launch_with_nothing_playing_stays_visible() {
        let t0 = Instant::now();
        let mut p = PlayState::new(t0);
        assert!(!p.should_hide(secs(t0, 60)));
        p.update(secs(t0, 1), Kind::Paused); // paused before FloatNote ever saw it play
        assert!(!p.should_hide(secs(t0, 60)));
    }

    #[test]
    fn repeated_reports_of_the_same_state_do_not_reset_the_timer() {
        let t0 = Instant::now();
        let mut p = PlayState::new(t0);
        p.update(t0, Kind::Playing);
        p.update(secs(t0, 1), Kind::Paused);
        p.update(secs(t0, 3), Kind::Paused);
        p.update(secs(t0, 5), Kind::Paused);
        assert!(p.should_hide(secs(t0, 6)));
    }
}
