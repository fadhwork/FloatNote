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
}

fn make_key(title: &str, artist: &str) -> String {
    format!("{}|{}", title.trim().to_lowercase(), artist.trim().to_lowercase())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    /// 100 ns ticks between 1601-01-01 (Windows epoch) and 1970-01-01.
    const EPOCH_DIFF_TICKS: i64 = 116_444_736_000_000_000;

    pub struct Reader {
        mgr: Option<Manager>,
    }

    impl Reader {
        pub fn new() -> Self {
            Reader { mgr: None }
        }

        pub fn read(&mut self) -> Option<MediaState> {
            if self.mgr.is_none() {
                self.mgr = Manager::RequestAsync().ok().and_then(|op| op.get().ok());
            }
            let mgr = self.mgr.as_ref()?;
            match read_session(mgr) {
                Ok(state) => state,
                Err(_) => {
                    self.mgr = None; // re-create next poll
                    None
                }
            }
        }
    }

    fn read_session(mgr: &Manager) -> windows::core::Result<Option<MediaState>> {
        let session = match mgr.GetCurrentSession() {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let props = session.TryGetMediaPropertiesAsync()?.get()?;
        let title = props.Title()?.to_string();
        let artist = props.Artist()?.to_string();
        if title.trim().is_empty() {
            return Ok(None);
        }
        let album = props.AlbumTitle()?.to_string();
        let source = session
            .SourceAppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();

        let playing = session.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;
        let tl = session.GetTimelineProperties()?;
        let duration_ticks = tl.EndTime()?.Duration - tl.StartTime()?.Duration;
        let pos_ticks = tl.Position()?.Duration;
        let updated = tl.LastUpdatedTime()?.UniversalTime;

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
        let mut position_ms = (pos_ticks + elapsed) / 10_000;
        let duration_ms = (duration_ticks / 10_000).max(0);
        if duration_ms > 0 {
            position_ms = position_ms.min(duration_ms);
        }

        Ok(Some(MediaState {
            key: make_key(&title, &artist),
            title,
            artist,
            album,
            source,
            playing,
            position_ms: position_ms.max(0),
            duration_ms,
        }))
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
