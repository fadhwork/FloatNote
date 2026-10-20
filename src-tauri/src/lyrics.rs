//! LRCLIB client plus an on-disk cache so each song is only fetched once.

use crate::lrc::{clean_artist, clean_title, parse_lrc, primary_artist, title_similarity, Line};
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

/// Choose the best candidate. A result is only accepted if its title resembles one of
/// `titles` (so a loose search can never hand back some unrelated song) and, when both
/// durations are known, it is within 30 s of the song that is playing.
fn pick_best(list: &[Remote], duration_s: f64, titles: &[String]) -> Option<LyricsResult> {
    let mut best: Option<(f64, LyricsResult)> = None;
    for r in list {
        let Some(res) = usable(r) else { continue };
        if !res.title.is_empty() {
            let sim = titles
                .iter()
                .map(|t| title_similarity(t, &res.title))
                .fold(0.0, f64::max);
            if sim < 0.6 {
                continue;
            }
        }
        let diff = if duration_s > 0.0 && res.duration > 0.0 {
            (res.duration - duration_s).abs()
        } else {
            0.0
        };
        if diff > 30.0 {
            continue;
        }
        if best.as_ref().map_or(true, |(d, _)| diff < *d) {
            best = Some((diff, res));
        }
    }
    best.map(|(_, r)| r)
}

/// Look a song up automatically: exact match first, then progressively looser searches
/// (the same ones you would otherwise run by hand in Settings -> Lyrics).
/// A failing step never stops the next one; an error is only returned if every step failed.
pub async fn fetch(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_s: f64,
) -> Result<Option<LyricsResult>, String> {
    let started = std::time::Instant::now();
    let cleaned = clean_title(title);
    let name = if cleaned.is_empty() { title.trim().to_string() } else { cleaned };
    let artist_clean = clean_artist(artist);
    let first_artist = primary_artist(&artist_clean);
    let mut last_err: Option<String> = None;

    // Titles a correct result may resemble.
    let mut titles: Vec<String> = vec![name.clone(), title.trim().to_string()];

    // 1. Exact signature match (needs an artist).
    if !first_artist.is_empty() {
        let mut q: Vec<(&str, String)> = vec![
            ("track_name", name.clone()),
            ("artist_name", artist_clean.clone()),
        ];
        if !album.is_empty() {
            q.push(("album_name", album.to_string()));
        }
        if duration_s > 0.0 {
            q.push(("duration", (duration_s.round() as i64).to_string()));
        }
        match client.get("https://lrclib.net/api/get").query(&q).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(r) = resp.json::<Remote>().await {
                    if let Some(res) = usable(&r) {
                        return Ok(Some(res));
                    }
                }
            }
            Ok(_) => {} // 404 etc: fall through to the searches
            Err(e) => last_err = Some(format!("network error: {e}")),
        }
    }

    // 2. Searches, from most to least specific.
    let mut by_fields: Vec<(&str, String)> = vec![("track_name", name.clone())];
    if !first_artist.is_empty() {
        by_fields.push(("artist_name", first_artist.clone()));
    }
    let mut attempts: Vec<Vec<(&str, String)>> = vec![
        by_fields,
        vec![("q", format!("{} {}", name, first_artist).trim().to_string())],
    ];
    // "Artist - Title" (common on YouTube) in either order.
    if let Some((left, right)) = title.split_once(" - ") {
        let (l_title, r_title) = (clean_title(left), clean_title(right));
        let (l_artist, r_artist) = (clean_artist(left), clean_artist(right));
        if !l_title.is_empty() && !r_title.is_empty() {
            titles.push(r_title.clone());
            titles.push(l_title.clone());
            attempts.push(vec![("track_name", r_title.clone()), ("artist_name", l_artist.clone())]);
            attempts.push(vec![("track_name", l_title.clone()), ("artist_name", r_artist.clone())]);
            attempts.push(vec![("q", format!("{} {}", r_title, l_artist))]);
        }
    }
    // The raw text, exactly as you would type it into the search box by hand.
    attempts.push(vec![("q", format!("{} {}", title.trim(), artist.trim()).trim().to_string())]);
    attempts.push(vec![("q", clean_title(&format!("{} {}", title, artist)))]);
    attempts.push(vec![("q", name.clone())]);
    attempts.push(vec![("q", title.trim().to_string())]);

    let mut any_ok = false;
    for params in attempts {
        if started.elapsed() > std::time::Duration::from_secs(15) {
            break; // do not keep the overlay on "Finding lyrics..." forever
        }
        match search_raw(client, &params).await {
            Ok(list) => {
                any_ok = true;
                if let Some(best) = pick_best(&list, duration_s, &titles) {
                    return Ok(Some(best));
                }
            }
            Err(e) => last_err = Some(e),
        }
    }
    if any_ok {
        Ok(None)
    } else {
        Err(last_err.unwrap_or_else(|| "lyrics lookup failed".to_string()))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str, secs: f64) -> Remote {
        Remote {
            track_name: Some(name.to_string()),
            artist_name: Some("X".to_string()),
            duration: Some(secs),
            instrumental: Some(false),
            synced_lyrics: Some("[00:01.00] hi".to_string()),
        }
    }

    #[test]
    fn rejects_unrelated_songs_and_far_durations() {
        let titles = vec!["Hati-Hati di Jalan".to_string()];
        let list = vec![remote("Totally Different", 200.0), remote("Hati Hati Di Jalan", 400.0)];
        assert!(pick_best(&list, 200.0, &titles).is_none());
    }

    #[test]
    fn prefers_closest_duration_among_matching_titles() {
        let titles = vec!["Hati-Hati di Jalan".to_string()];
        let list = vec![remote("Hati Hati di Jalan", 190.0), remote("Hati-Hati di Jalan", 201.0)];
        let best = pick_best(&list, 200.0, &titles).unwrap();
        assert_eq!(best.duration, 201.0);
    }
}
