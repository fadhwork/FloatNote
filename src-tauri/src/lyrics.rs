//! LRCLIB client plus an on-disk cache so each song is only fetched once.

use crate::lrc::{clean_title, parse_lrc, primary_artist, Line};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

pub const UA: &str = concat!(
    "FloatNote/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/fadhwork/FloatNote)"
);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsResult {
    pub lines: Vec<Line>,
    pub instrumental: bool,
    /// "lrclib", "cache", "manual" or "none".
    pub source: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
}

/// LRCLIB's JSON uses camelCase keys.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Remote {
    track_name: Option<String>,
    artist_name: Option<String>,
    duration: Option<f64>,
    instrumental: Option<bool>,
    synced_lyrics: Option<String>,
}

fn usable(r: &Remote) -> Option<LyricsResult> {
    let base = LyricsResult {
        lines: Vec::new(),
        instrumental: false,
        source: "lrclib".into(),
        title: r.track_name.clone().unwrap_or_default(),
        artist: r.artist_name.clone().unwrap_or_default(),
        duration: r.duration.unwrap_or(0.0),
    };
    if r.instrumental == Some(true) {
        return Some(LyricsResult { instrumental: true, ..base });
    }
    let lines = parse_lrc(r.synced_lyrics.as_deref()?);
    if lines.is_empty() {
        None
    } else {
        Some(LyricsResult { lines, ..base })
    }
}

async fn search_raw(
    client: &reqwest::Client,
    params: &[(&str, String)],
) -> Result<Vec<Remote>, String> {
    let resp = client
        .get("https://lrclib.net/api/search")
        .query(params)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("LRCLIB returned {}", resp.status()));
    }
    resp.json::<Vec<Remote>>()
        .await
        .map_err(|e| format!("bad LRCLIB response: {e}"))
}

fn pick_best(list: &[Remote], duration_s: f64) -> Option<LyricsResult> {
    let mut best: Option<(f64, LyricsResult)> = None;
    for r in list {
        let Some(res) = usable(r) else { continue };
        let diff = if duration_s > 0.0 && res.duration > 0.0 {
            (res.duration - duration_s).abs()
        } else {
            0.0
        };
        if diff > 15.0 {
            continue; // probably a different version of the song
        }
        if best.as_ref().map_or(true, |(d, _)| diff < *d) {
            best = Some((diff, res));
        }
    }
    best.map(|(_, r)| r)
}

/// Look a song up: exact match first, then fuzzy search.
pub async fn fetch(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_s: f64,
) -> Result<Option<LyricsResult>, String> {
    let cleaned = clean_title(title);
    let name = if cleaned.is_empty() { title.to_string() } else { cleaned };

    let mut q: Vec<(&str, String)> = vec![
        ("track_name", name.clone()),
        ("artist_name", artist.to_string()),
    ];
    if !album.is_empty() {
        q.push(("album_name", album.to_string()));
    }
    if duration_s > 0.0 {
        q.push(("duration", (duration_s.round() as i64).to_string()));
    }
    let resp = client
        .get("https://lrclib.net/api/get")
        .query(&q)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if resp.status().is_success() {
        if let Ok(r) = resp.json::<Remote>().await {
            if let Some(res) = usable(&r) {
                return Ok(Some(res));
            }
        }
    } else if resp.status().as_u16() != 404 {
        return Err(format!("LRCLIB returned {}", resp.status()));
    }

    let first_artist = primary_artist(artist);
    let attempts: Vec<Vec<(&str, String)>> = vec![
        vec![("track_name", name.clone()), ("artist_name", first_artist.clone())],
        vec![("q", format!("{} {}", name, first_artist))],
    ];
    for params in attempts {
        let list = search_raw(client, &params).await?;
        if let Some(best) = pick_best(&list, duration_s) {
            return Ok(Some(best));
        }
    }
    Ok(None)
}

/// Free-text search used by the manual "pick the right lyrics" box.
pub async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<LyricsResult>, String> {
    let list = search_raw(client, &[("q", query.to_string())]).await?;
    Ok(list.iter().filter_map(usable).take(8).collect())
}

// ---- cache -----------------------------------------------------------------

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn cache_file(dir: &Path, key: &str) -> PathBuf {
    dir.join("lyrics-cache").join(format!("{:016x}.json", fnv1a(key)))
}

pub fn load_cached(dir: &Path, key: &str) -> Option<LyricsResult> {
    let text = fs::read_to_string(cache_file(dir, key)).ok()?;
    let mut r: LyricsResult = serde_json::from_str(&text).ok()?;
    r.source = "cache".into();
    Some(r)
}

pub fn store_cached(dir: &Path, key: &str, r: &LyricsResult) {
    let path = cache_file(dir, key);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(r) {
        let _ = fs::write(path, text);
    }
}
