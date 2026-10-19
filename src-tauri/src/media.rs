//! Reads "now playing" from Windows' System Media Transport Controls (SMTC).
//! That is the same source as the volume flyout, so it works with the Spotify
//! desktop app, browser tabs (YouTube, Spotify Web, ...), Apple Music, etc.
//! No login, token or API key needed.

use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    /// Normalised "title|artist", used as cache and offset key.
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// "player" when the position comes from the player, "own" when FloatNote keeps its own clock.
    pub clock: String,
}

fn make_key(title: &str, artist: &str) -> String {
    format!("{}|{}", title.trim().to_lowercase(), artist.trim().to_lowercase())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    /// 100 ns ticks between 1601-01-01 (Windows epoch) and 1970-01-01.
    const EPOCH_DIFF_TICKS: i64 = 116_444_736_000_000_000;

    pub struct Reader {
        mgr: Option<Manager>,
        // FloatNote's own song clock. Some players (or some versions of them) report no
        // usable timeline, so the position would be stuck at 0 and the lyrics never move.
        clock_key: String,
        clock_ms: i64,
        last_poll: Instant,
    }

    impl Reader {
        pub fn new() -> Self {
            Reader {
                mgr: None,
                clock_key: String::new(),
                clock_ms: 0,
                last_poll: Instant::now(),
            }
        }

        pub fn read(&mut self) -> Option<MediaState> {
            if self.mgr.is_none() {
                self.mgr = Manager::RequestAsync().ok().and_then(|op| op.get().ok());
            }
            let mgr = self.mgr.as_ref()?;
            let dt = self.last_poll.elapsed().as_millis() as i64;
            self.last_poll = Instant::now();

            let (mut state, mut timeline_ok) = match read_session(mgr) {
                Ok(Some(found)) => found,
                Ok(None) => {
                    self.clock_key.clear();
                    self.clock_ms = 0;
                    return None;
                }
                Err(_) => {
                    self.mgr = None; // re-create next poll
                    return None;
                }
            };

            if self.clock_key != state.key {
                self.clock_key = state.key.clone();
                self.clock_ms = if timeline_ok && state.position_ms > 0 && (state.duration_ms == 0 || state.position_ms < state.duration_ms) {
                    state.position_ms
                } else {
                    0
                };
            } else if state.playing {
                self.clock_ms += dt;
            }

            // Timeline sanity checks:
            // 1. If player reports position 0 continuously after 1.5s of playback, its timeline is inactive.
            if state.playing && state.position_ms == 0 && self.clock_ms > 1500 {
                timeline_ok = false;
            }
            // 2. If duration > 0 and position exceeds duration by > 3s, timeline is bogus.
            if state.duration_ms > 0 && state.position_ms > state.duration_ms + 3000 {
                timeline_ok = false;
            }
            // 3. If duration == 0 and position == 0, player has no timeline info.
            if state.duration_ms == 0 && state.position_ms == 0 {
                timeline_ok = false;
            }

            if !timeline_ok {
                state.position_ms = if state.duration_ms > 0 {
                    self.clock_ms.min(state.duration_ms)
                } else {
                    self.clock_ms
                };
                state.clock = "own".into();
            }
            Some(state)
        }
    }

    /// Returns the song plus whether the player's own timeline can be trusted.
    fn read_session(mgr: &Manager) -> windows::core::Result<Option<(MediaState, bool)>> {
        let session = match mgr.GetCurrentSession() {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let props = session.TryGetMediaPropertiesAsync()?.get()?;
        let title = props.Title()?.to_string();
        if title.trim().is_empty() {
            return Ok(None);
        }
        // Everything below is optional: a failing property must never hide the song.
        let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
        let source = session
            .SourceAppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let playing = session
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|s| s == Status::Playing)
            .unwrap_or(true);

        let (duration_ticks, pos_ticks, updated) = match session.GetTimelineProperties() {
            Ok(tl) => (
                tl.EndTime().map(|t| t.Duration).unwrap_or(0) - tl.StartTime().map(|t| t.Duration).unwrap_or(0),
                tl.Position().map(|t| t.Duration).unwrap_or(0),
                tl.LastUpdatedTime().map(|t| t.UniversalTime).unwrap_or(0),
            ),
            Err(_) => (0, 0, 0),
        };

        // SMTC reports the position at `updated`; extrapolate to "now" while playing.
        let now = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() / 100)
            .unwrap_or(0) as i64)
            + EPOCH_DIFF_TICKS;
        let mut elapsed = if playing && updated > 0 { (now - updated).max(0) } else { 0 };
        if elapsed > 12 * 3600 * 10_000_000 {
            elapsed = 0; // bogus timestamp from some players
        }
        let position_ms = ((pos_ticks + elapsed) / 10_000).max(0);
        let duration_ms = (duration_ticks / 10_000).max(0);

        // No timeline at all, or one that is clearly stale (position past the end of the song).
        let mut timeline_ok = updated > 0 || pos_ticks > 0;
        if duration_ms > 0 && position_ms > duration_ms + 3000 {
            timeline_ok = false;
        }
        let position_ms = if duration_ms > 0 { position_ms.min(duration_ms) } else { position_ms };

        Ok(Some((
            MediaState {
                key: make_key(&title, &artist),
                title,
                artist,
                album,
                source,
                playing,
                position_ms,
                duration_ms,
                clock: "player".into(),
            },
            timeline_ok,
        )))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub struct Reader;
    impl Reader {
        pub fn new() -> Self {
            Reader
        }
        pub fn read(&mut self) -> Option<MediaState> {
            None
        }
    }
}

/// Poll ~1.3x per second on a background thread and emit a `media` event.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut reader = platform::Reader::new();
        let mut last_was_none = false;
        loop {
            let state = reader.read();
            if state.is_none() {
                if !last_was_none {
                    let _ = app.emit("media", Option::<MediaState>::None);
                    last_was_none = true;
                }
            } else {
                last_was_none = false;
                let _ = app.emit("media", &state);
            }
            std::thread::sleep(Duration::from_millis(750));
        }
    });
}

#[cfg(windows)]
pub fn control(action: &str) {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
    if let Ok(op) = Manager::RequestAsync() {
        if let Ok(mgr) = op.get() {
            if let Ok(session) = mgr.GetCurrentSession() {
                match action {
                    "toggle" => { let _ = session.TryTogglePlayPauseAsync(); }
                    "next" => { let _ = session.TrySkipNextAsync(); }
                    "prev" => { let _ = session.TrySkipPreviousAsync(); }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn control(_action: &str) {}
