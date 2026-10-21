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
    /// true = no synced lyrics exist: the lines are spread evenly over the song (timing is a guess).
    pub approx: bool,
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
    plain_lyrics: Option<String>,
}

fn usable(r: &Remote) -> Option<LyricsResult> {
    let base = LyricsResult {
        lines: Vec::new(),
        instrumental: false,
        source: "lrclib".into(),
        title: r.track_name.clone().unwrap_or_default(),
        artist: r.artist_name.clone().unwrap_or_default(),
        duration: r.duration.unwrap_or(0.0),
        approx: false,
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

/// Only plain (unsynced) lyrics exist: spread the lines evenly over the song so something scrolls.
/// The first ~6% of the song is left for the intro. Timing is a guess, so the result is flagged `approx`.
fn usable_plain(r: &Remote) -> Option<LyricsResult> {
    if r.instrumental == Some(true) {
        return None;
    }
    let text = r.plain_lyrics.as_deref()?;
    let rows: Vec<String> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    if rows.len() < 4 {
        return None;
    }
    let total = r.duration.filter(|d| *d > 20.0).unwrap_or(180.0);
    let start = total * 0.06;
    let step = (total * 0.90) / rows.len() as f64;
    let lines = rows
        .into_iter()
        .enumerate()
        .map(|(i, text)| Line { t: start + step * i as f64, text })
        .collect();
    Some(LyricsResult {
        lines,
        instrumental: false,
        source: "lrclib".into(),
        title: r.track_name.clone().unwrap_or_default(),
        artist: r.artist_name.clone().unwrap_or_default(),
        duration: total,
        approx: true,
    })
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
fn pick_best(
    list: &[Remote],
    duration_s: f64,
    titles: &[String],
    convert: fn(&Remote) -> Option<LyricsResult>,
) -> Option<LyricsResult> {
    let mut best: Option<(f64, LyricsResult)> = None;
    for r in list {
        let Some(res) = convert(r) else { continue };
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
    // Remembered while searching: used only if no synced version turns up anywhere.
    let mut plain_fallback: Option<LyricsResult> = None;

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
                    plain_fallback = usable_plain(&r);
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
                if let Some(best) = pick_best(&list, duration_s, &titles, usable) {
                    return Ok(Some(best));
                }
                if plain_fallback.is_none() {
                    plain_fallback = pick_best(&list, duration_s, &titles, usable_plain);
                }
            }
            Err(e) => last_err = Some(e),
        }
    }
    if plain_fallback.is_some() {
        return Ok(plain_fallback);
    }
    if any_ok {
        Ok(None)
    } else {
        Err(last_err.unwrap_or_else(|| "lyrics lookup failed".to_string()))
    }
}

/// Free-text search used by the manual "pick the right lyrics" box.
pub async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<LyricsResult>, String> {
    // "Official", "VEVO", "(Official Video)" ... only get in the way of a search.
    let cleaned = clean_title(query);
    let q = if cleaned.trim().is_empty() { query.trim().to_string() } else { cleaned };
    let list = search_raw(client, &[("q", q)]).await?;
    // Synced versions first, then plain-only ones (flagged `approx`).
    let mut out: Vec<LyricsResult> = list.iter().filter_map(usable).collect();
    out.extend(list.iter().filter(|r| usable(r).is_none()).filter_map(usable_plain));
    out.truncate(8);
    Ok(out)
}

// ---- cache -----------------------------------------------------------------

pub(crate) fn fnv1a(s: &str) -> u64 {
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
            plain_lyrics: None,
        }
    }

    #[test]
    fn rejects_unrelated_songs_and_far_durations() {
        let titles = vec!["Hati-Hati di Jalan".to_string()];
        let list = vec![remote("Totally Different", 200.0), remote("Hati Hati Di Jalan", 400.0)];
        assert!(pick_best(&list, 200.0, &titles, usable).is_none());
    }

    #[test]
    fn prefers_closest_duration_among_matching_titles() {
        let titles = vec!["Hati-Hati di Jalan".to_string()];
        let list = vec![remote("Hati Hati di Jalan", 190.0), remote("Hati-Hati di Jalan", 201.0)];
        let best = pick_best(&list, 200.0, &titles, usable).unwrap();
        assert_eq!(best.duration, 201.0);
    }

    #[test]
    fn plain_lyrics_become_evenly_spaced_approx_lines() {
        let r = Remote {
            track_name: Some("T".into()),
            artist_name: Some("A".into()),
            duration: Some(200.0),
            instrumental: Some(false),
            synced_lyrics: None,
            plain_lyrics: Some("one\n\n two \nthree\nfour\nfive".into()),
        };
        assert!(usable(&r).is_none());
        let p = usable_plain(&r).unwrap();
        assert!(p.approx);
        assert_eq!(p.lines.len(), 5);
        assert_eq!(p.lines[1].text, "two");
        assert!((p.lines[0].t - 12.0).abs() < 1e-9);
        assert!(p.lines[4].t < 200.0 && p.lines[3].t < p.lines[4].t);
    }

    #[test]
    fn too_short_plain_text_is_ignored() {
        let r = Remote {
            track_name: None, artist_name: None, duration: None, instrumental: None,
            synced_lyrics: None, plain_lyrics: Some("la\nla".into()),
        };
        assert!(usable_plain(&r).is_none());
    }
}
