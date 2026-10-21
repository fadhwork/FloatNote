//! Dual-subtitle translation through Google Translate's public web endpoint (no API key).
//!
//! The old version sent the whole song in one URL (too long, so it failed) and swallowed every
//! error. This one sends small batches, matches results to lines safely, falls back to
//! line-by-line when a batch does not line up, caches per song and language, and reports errors.

use crate::lyrics::fnv1a;
use serde_json::Value;
use std::{fs, path::Path};

const MAX_LINES: usize = 220;
const BATCH_LINES: usize = 20;
const BATCH_CHARS: usize = 1200;

/// Read a gtx response for a request that sent exactly `expected` non-empty lines joined by `\n`.
/// Returns (translated lines, detected source language), or None if the lines do not line up.
pub fn parse_batch(val: &Value, expected: usize) -> Option<(Vec<String>, String)> {
    let segs = val.get(0)?.as_array()?;
    let detected = val.get(2).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut open = false;
    for seg in segs {
        let trans = seg.get(0).and_then(|v| v.as_str()).unwrap_or("");
        let orig = seg.get(1).and_then(|v| v.as_str()).unwrap_or("");
        cur.push_str(trans.trim_end_matches('\n'));
        open = true;
        if orig.ends_with('\n') {
            lines.push(cur.trim().to_string());
            cur.clear();
            open = false;
        }
    }
    if open {
        lines.push(cur.trim().to_string());
    }
    if lines.len() == expected {
        Some((lines, detected))
    } else {
        None
    }
}

fn base_lang(code: &str) -> String {
    code.split('-').next().unwrap_or("").to_lowercase()
}

async fn request(client: &reqwest::Client, text: &str, target: &str) -> Result<Value, String> {
    let resp = client
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[("client", "gtx"), ("sl", "auto"), ("tl", target), ("dt", "t"), ("q", text)])
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("translation service returned {}", resp.status()));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("bad translation response: {e}"))
}

async fn translate_single(client: &reqwest::Client, text: &str, target: &str) -> Result<String, String> {
    let val = request(client, text, target).await?;
    let mut out = String::new();
    if let Some(segs) = val.get(0).and_then(|v| v.as_array()) {
        for seg in segs {
            if let Some(t) = seg.get(0).and_then(|v| v.as_str()) {
                out.push_str(t);
            }
        }
    }
    Ok(out.trim().to_string())
}

fn cache_path(dir: &Path, key: &str, target: &str) -> std::path::PathBuf {
    dir.join("translations")
        .join(format!("{:016x}.json", fnv1a(&format!("{key}|{target}"))))
}

/// Translate `lines` (blank lines allowed) into `target`. The result has one entry per input
/// line; an empty string means "no subtitle" (blank line, or the song is already in `target`).
pub async fn translate_lines(
    client: &reqwest::Client,
    dir: &Path,
    key: &str,
    lines: &[String],
    target: &str,
) -> Result<Vec<String>, String> {
    let path = cache_path(dir, key, target);
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(hit) = serde_json::from_str::<Vec<String>>(&text) {
            if hit.len() == lines.len() {
                return Ok(hit);
            }
        }
    }

    let mut out = vec![String::new(); lines.len()];
    let work: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.is_empty() && t != "\u{266a}"
        })
        .map(|(i, _)| i)
        .take(MAX_LINES)
        .collect();

    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut chars = 0usize;
    for i in work {
        let len = lines[i].len();
        if !cur.is_empty() && (cur.len() >= BATCH_LINES || chars + len > BATCH_CHARS) {
            batches.push(std::mem::take(&mut cur));
            chars = 0;
        }
        cur.push(i);
        chars += len;
    }
    if !cur.is_empty() {
        batches.push(cur);
    }

    let mut first = true;
    for batch in batches {
        let text = batch.iter().map(|&i| lines[i].trim()).collect::<Vec<_>>().join("\n");
        let val = request(client, &text, target).await?; // a failing request is an error, not silence
        match parse_batch(&val, batch.len()) {
            Some((translated, detected)) => {
                if first && base_lang(&detected) == base_lang(target) {
                    // Already in the target language: nothing to show, and remember that.
                    let _ = write_cache(&path, &out);
                    return Ok(out);
                }
                for (slot, t) in batch.iter().zip(translated) {
                    out[*slot] = t;
                }
            }
            None => {
                // The batch did not line up: translate it line by line instead.
                for &i in &batch {
                    if let Ok(t) = translate_single(client, lines[i].trim(), target).await {
                        out[i] = t;
                    }
                }
            }
        }
        first = false;
    }
    let _ = write_cache(&path, &out);
    Ok(out)
}

fn write_cache(path: &Path, out: &[String]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string(out).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_one_segment_per_line() {
        let v = json!([[["Hello\n", "Halo\n", null, null, 10], ["World", "Dunia", null, null, 10]], null, "id"]);
        let (lines, lang) = parse_batch(&v, 2).unwrap();
        assert_eq!(lines, vec!["Hello", "World"]);
        assert_eq!(lang, "id");
    }

    #[test]
    fn joins_sentence_segments_inside_one_line() {
        let v = json!([[["Hi. ", "Halo. "], ["How are you\n", "Apa kabar\n"], ["Bye", "Dah"]], null, "id"]);
        let (lines, _) = parse_batch(&v, 2).unwrap();
        assert_eq!(lines, vec!["Hi. How are you", "Bye"]);
    }

    #[test]
    fn rejects_batches_that_do_not_line_up() {
        let v = json!([[["One two\n", "a\n"]], null, "id"]);
        assert!(parse_batch(&v, 2).is_none());
        assert!(parse_batch(&json!({"x": 1}), 1).is_none());
    }

    #[test]
    fn language_base_codes() {
        assert_eq!(base_lang("zh-CN"), "zh");
        assert_eq!(base_lang("EN"), "en");
    }
}
