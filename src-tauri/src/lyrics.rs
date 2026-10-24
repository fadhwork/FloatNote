//! LRCLIB and NetEase lyrics clients plus an on-disk cache so each song is only fetched once.

use crate::lrc::{clean_artist, clean_title, parse_lrc, primary_artist, title_similarity, Line};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
        return Some(LyricsResult {
            instrumental: true,
            ..base
        });
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
        .map(|(i, text)| Line {
            t: start + step * i as f64,
            text,
        })
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

#[derive(Deserialize, Debug)]
struct NetEaseSearchResponse {
    result: Option<NetEaseSearchResult>,
}

#[derive(Deserialize, Debug)]
struct NetEaseSearchResult {
    songs: Option<Vec<NetEaseSong>>,
}

#[derive(Deserialize, Debug)]
struct NetEaseSong {
    name: Option<String>,
    artists: Option<Vec<NetEaseArtist>>,
    duration: Option<u64>,
    id: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct NetEaseArtist {
    name: Option<String>,
}

fn netease_search_url(query: &str) -> String {
    reqwest::Url::parse_with_params(
        "https://music.163.com/api/search/get",
        &[
            ("s", query),
            ("type", "1"),
            ("limit", "10"),
            ("offset", "0"),
        ],
    )
    .expect("static NetEase URL and query parameters are valid")
    .to_string()
}

fn netease_lyrics_url(id: u64) -> String {
    reqwest::Url::parse_with_params(
        "https://music.163.com/api/song/lyric",
        &[("id", id.to_string()), ("lv", "-1"), ("tv", "-1")],
    )
    .expect("static NetEase URL and lyric id are valid")
    .to_string()
}

#[derive(Deserialize, Debug)]
struct NetEaseLyricResponse {
    lrc: Option<NetEaseLyricText>,
}

#[derive(Deserialize, Debug)]
struct NetEaseLyricText {
    lyric: Option<String>,
}

async fn netease_search(client: &reqwest::Client, query: &str) -> Result<Vec<NetEaseSong>, String> {
    let resp = client
        .get(netease_search_url(query))
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .send()
        .await
        .map_err(|e| format!("NetEase network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("NetEase returned {}", resp.status()));
    }
    let response: NetEaseSearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("bad NetEase search response: {e}"))?;
    Ok(response.result.and_then(|r| r.songs).unwrap_or_default())
}

async fn netease_lyrics(client: &reqwest::Client, id: u64) -> Result<Option<String>, String> {
    let resp = client
        .get(netease_lyrics_url(id))
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .send()
        .await
        .map_err(|e| format!("NetEase lyric network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("NetEase lyrics returned {}", resp.status()));
    }
    let response: NetEaseLyricResponse = resp
        .json()
        .await
        .map_err(|e| format!("bad NetEase lyric response: {e}"))?;
    let synced = response.lrc.and_then(|l| l.lyric).unwrap_or_default();
    Ok((!parse_lrc(&synced).is_empty()).then_some(synced))
}

fn netease_artist(song: &NetEaseSong) -> String {
    song.artists
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|artist| artist.name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn netease_match_score(song: &NetEaseSong, duration_s: f64, titles: &[String]) -> Option<f64> {
    let song_title = song.name.as_deref().unwrap_or_default();
    let similarity = titles
        .iter()
        .map(|candidate| title_similarity(candidate, song_title))
        .fold(0.0, f64::max);
    if song_title.trim().is_empty() || similarity < 0.6 {
        return None;
    }
    let song_duration = song.duration.unwrap_or_default() as f64 / 1000.0;
    let diff = if duration_s > 0.0 && song_duration > 0.0 {
        (song_duration - duration_s).abs()
    } else {
        0.0
    };
    (diff <= 30.0).then_some(diff)
}

fn netease_result(song: &NetEaseSong, lyrics: String) -> Option<LyricsResult> {
    let lines = parse_lrc(&lyrics);
    if lines.is_empty() {
        return None;
    }
    Some(LyricsResult {
        lines,
        instrumental: false,
        source: "netease".into(),
        title: song.name.clone().unwrap_or_default(),
        artist: netease_artist(song),
        duration: song.duration.unwrap_or_default() as f64 / 1000.0,
        approx: false,
    })
}

async fn fetch_netease(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    duration_s: f64,
    titles: &[String],
) -> Result<Option<LyricsResult>, String> {
    let queries = [
        format!("{} {}", title.trim(), artist.trim())
            .trim()
            .to_string(),
        title.trim().to_string(),
    ];
    let mut last_error = None;
    for query in queries.iter().filter(|query| !query.is_empty()) {
        let songs = match netease_search(client, query).await {
            Ok(songs) => songs,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        let mut candidates: Vec<(f64, NetEaseSong)> = songs
            .into_iter()
            .filter_map(|song| {
                netease_match_score(&song, duration_s, titles).map(|score| (score, song))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, song) in candidates {
            let Some(id) = song.id else { continue };
            match netease_lyrics(client, id).await {
                Ok(Some(text)) => {
                    if let Some(result) = netease_result(&song, text) {
                        return Ok(Some(result));
                    }
                }
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
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

/// Look up LRCLIB first, followed by NetEase when LRCLIB has no usable result.
/// A failing provider never blocks the next fallback.
pub async fn fetch(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_s: f64,
) -> Result<Option<LyricsResult>, String> {
    let cleaned = clean_title(title);
    let name = if cleaned.is_empty() {
        title.trim().to_string()
    } else {
        cleaned
    };
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
        match client
            .get("https://lrclib.net/api/get")
            .query(&q)
            .send()
            .await
        {
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
            attempts.push(vec![
                ("track_name", r_title.clone()),
                ("artist_name", l_artist.clone()),
            ]);
            attempts.push(vec![
                ("track_name", l_title.clone()),
                ("artist_name", r_artist.clone()),
            ]);
            attempts.push(vec![("q", format!("{} {}", r_title, l_artist))]);
        }
    }
    // The raw text, exactly as you would type it into the search box by hand.
    attempts.push(vec![(
        "q",
        format!("{} {}", title.trim(), artist.trim())
            .trim()
            .to_string(),
    )]);
    attempts.push(vec![("q", clean_title(&format!("{} {}", title, artist)))]);
    attempts.push(vec![("q", name.clone())]);
    attempts.push(vec![("q", title.trim().to_string())]);

    let mut any_ok = false;
    for params in attempts {
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

    // Keep LRCLIB first. NetEase is only consulted after the complete LRCLIB
    // lookup has no usable synced or approximate plain-lyrics result.
    match fetch_netease(client, title, artist, duration_s, &titles).await {
        Ok(Some(result)) => Ok(Some(result)),
        Ok(None) if any_ok => Ok(None),
        Ok(None) => Err(last_err.unwrap_or_else(|| "lyrics lookup failed".to_string())),
        Err(netease_error) => {
            if any_ok {
                Ok(None)
            } else {
                Err(last_err.unwrap_or(netease_error))
            }
        }
    }
}

/// Free-text LRCLIB search used by the manual "pick the right lyrics" box.
pub async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<LyricsResult>, String> {
    // "Official", "VEVO", "(Official Video)" ... only get in the way of a search.
    let cleaned = clean_title(query);
    let q = if cleaned.trim().is_empty() {
        query.trim().to_string()
    } else {
        cleaned
    };
    let list = search_raw(client, &[("q", q)]).await?;
    // Synced versions first, then plain-only ones (flagged `approx`).
    let mut out: Vec<LyricsResult> = list.iter().filter_map(usable).collect();
    out.extend(
        list.iter()
            .filter(|r| usable(r).is_none())
            .filter_map(usable_plain),
    );
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
    dir.join("lyrics-cache")
        .join(format!("{:016x}.json", fnv1a(key)))
}

pub fn load_cached(dir: &Path, key: &str) -> Option<LyricsResult> {
    load_cached_inner(dir, key, false)
}

/// Manual selections are durable overrides. Explicit provider refreshes may
/// bypass automatic results, but must never silently replace a manual choice.
pub fn load_manual_cached(dir: &Path, key: &str) -> Option<LyricsResult> {
    load_cached_inner(dir, key, true)
}

fn load_cached_inner(dir: &Path, key: &str, manual_only: bool) -> Option<LyricsResult> {
    let path = cache_file(dir, key);
    let text = fs::read_to_string(&path).ok()?;
    let mut r: LyricsResult = serde_json::from_str(&text).ok()?;
    if manual_only && r.source != "manual" {
        return None;
    }
    // Older versions could persist an empty `source: none` result. Treat those
    // entries as misses so a temporary failure or early no-match can recover.
    if !r.instrumental && r.lines.is_empty() {
        let _ = fs::remove_file(path);
        return None;
    }
    if !manual_only {
        r.source = "cache".into();
    }
    Some(r)
}

pub fn store_cached(dir: &Path, key: &str, r: &LyricsResult) {
    // Only durable provider results and intentional instrumentals belong in the
    // cache. Empty/no-match values are deliberately not persisted.
    if !r.instrumental && r.lines.is_empty() {
        return;
    }
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
        let list = vec![
            remote("Totally Different", 200.0),
            remote("Hati Hati Di Jalan", 400.0),
        ];
        assert!(pick_best(&list, 200.0, &titles, usable).is_none());
    }

    #[test]
    fn prefers_closest_duration_among_matching_titles() {
        let titles = vec!["Hati-Hati di Jalan".to_string()];
        let list = vec![
            remote("Hati Hati di Jalan", 190.0),
            remote("Hati-Hati di Jalan", 201.0),
        ];
        let best = pick_best(&list, 200.0, &titles, usable).unwrap();
        assert_eq!(best.duration, 201.0);
    }

    #[test]
    fn netease_urls_encode_query_and_id() {
        assert!(netease_search_url("Song & Artist").contains("Song+%26+Artist"));
        assert!(netease_lyrics_url(123).contains("id=123"));
    }

    #[test]
    fn parses_netease_search_and_timed_lyric_responses() {
        let search: NetEaseSearchResponse = serde_json::from_str(
            r#"{"result":{"songs":[{"id":123,"name":"Track","duration":200000,"artists":[{"name":"Artist"}]}]}}"#,
        ).unwrap();
        let song = search
            .result
            .unwrap()
            .songs
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(song.id, Some(123));
        assert_eq!(netease_artist(&song), "Artist");
        assert_eq!(
            netease_match_score(&song, 200.0, &["Track".into()]),
            Some(0.0)
        );

        let lyric: NetEaseLyricResponse =
            serde_json::from_str(r#"{"lrc":{"lyric":"[00:01.00]hello\n[00:02.00]world"}}"#)
                .unwrap();
        let text = lyric.lrc.unwrap().lyric.unwrap();
        let result = netease_result(&song, text).unwrap();
        assert_eq!(result.source, "netease");
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.duration, 200.0);
    }

    #[test]
    fn rejects_unrelated_or_duration_mismatched_netease_results() {
        let unrelated = NetEaseSong {
            name: Some("Completely Different Song".into()),
            artists: None,
            duration: Some(200_000),
            id: Some(1),
        };
        assert_eq!(
            netease_match_score(&unrelated, 200.0, &["Wanted Song".into()]),
            None
        );
        let mismatch = NetEaseSong {
            name: Some("Wanted Song".into()),
            duration: Some(300_000),
            ..unrelated
        };
        assert_eq!(
            netease_match_score(&mismatch, 200.0, &["Wanted Song".into()]),
            None
        );
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
    fn cache_ignores_empty_results_and_keeps_instrumentals() {
        let dir = std::env::temp_dir().join(format!("floatnote-lyrics-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let empty = LyricsResult { source: "none".into(), ..Default::default() };
        store_cached(&dir, "empty", &empty);
        assert!(load_cached(&dir, "empty").is_none());

        let instrumental = LyricsResult { instrumental: true, source: "lrclib".into(), ..Default::default() };
        store_cached(&dir, "instrumental", &instrumental);
        assert!(load_cached(&dir, "instrumental").is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn too_short_plain_text_is_ignored() {
        let r = Remote {
            track_name: None,
            artist_name: None,
            duration: None,
            instrumental: None,
            synced_lyrics: None,
            plain_lyrics: Some("la\nla".into()),
        };
        assert!(usable_plain(&r).is_none());
    }
}
