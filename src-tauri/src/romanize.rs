//! Romanization for singing along: a Latin-letter reading of lyrics written in kanji, kana,
//! hangul, hanzi, Cyrillic, Arabic, Thai, Devanagari, ... shown under the original line.
//!
//! Uses the same public Google web endpoint as the translation feature (`dt=rm` asks for the
//! transliteration of the source text), so it needs no API key and no big dictionary files.
//! Lines are sent in small batches, matched back safely, retried line by line when a batch does
//! not line up, and cached per song.

use crate::lyrics::fnv1a;
use serde_json::Value;
use std::{fs, path::Path};

const MAX_LINES: usize = 220;
const BATCH_LINES: usize = 12;
const BATCH_CHARS: usize = 900;

/// Is this a letter of a script that a reader of Latin letters cannot sound out?
fn is_foreign_letter(c: char) -> bool {
    matches!(c as u32,
        0x0370..=0x03FF            // Greek
        | 0x0400..=0x052F          // Cyrillic
        | 0x0530..=0x058F          // Armenian
        | 0x0590..=0x05FF          // Hebrew
        | 0x0600..=0x06FF          // Arabic
        | 0x0750..=0x077F          // Arabic supplement
        | 0x0900..=0x0DFF          // Devanagari .. Sinhala
        | 0x0E00..=0x0EFF          // Thai, Lao
        | 0x10A0..=0x10FF          // Georgian
        | 0x1100..=0x11FF          // Hangul jamo
        | 0x3040..=0x30FF          // Hiragana, Katakana
        | 0x3130..=0x318F          // Hangul compatibility jamo
        | 0x3400..=0x4DBF          // Han (extension A)
        | 0x4E00..=0x9FFF          // Han
        | 0xAC00..=0xD7AF          // Hangul syllables
        | 0xFF66..=0xFF9F)         // half-width Katakana
}

pub fn needs_romanization(line: &str) -> bool {
    line.chars().any(is_foreign_letter)
}

/// Source language for the whole song. Kanji alone could be Chinese or Japanese, so one kana
/// anywhere in the song decides "Japanese" for every line (a kanji-only line is read as Japanese).
pub fn source_hint(lines: &[String]) -> &'static str {
    let (mut kana, mut hangul, mut han) = (false, false, false);
    for c in lines.iter().flat_map(|l| l.chars()) {
        match c as u32 {
            0x3040..=0x30FF | 0xFF66..=0xFF9F => kana = true,
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => hangul = true,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF => han = true,
            _ => {}
        }
    }
    if kana {
        "ja"
    } else if hangul {
        "ko"
    } else if han {
        "zh-CN"
    } else {
        "auto"
    }
}

/// Collect the transliteration entries (`[null, null, "<romanized>"]`) of a gtx response that
/// was requested with `dt=rm`, joined with `sep`.
pub fn parse_translit(val: &Value, sep: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(segs) = val.get(0).and_then(|v| v.as_array()) {
        for seg in segs {
            let no_text = seg.get(0).map_or(true, |v| v.is_null()) && seg.get(1).map_or(true, |v| v.is_null());
            if !no_text {
                continue;
            }
            if let Some(t) = seg.get(2).and_then(|v| v.as_str()) {
                parts.push(t);
            }
        }
    }
    parts.join(sep)
}

/// Split a batch reading back into one entry per sent line, or None if the count is off.
pub fn split_batch(text: &str, expected: usize) -> Option<Vec<String>> {
    let lines: Vec<String> = text.split('\n').map(clean).collect();
    if lines.len() == expected {
        Some(lines)
    } else {
        None
    }
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn request(client: &reqwest::Client, text: &str, source: &str) -> Result<Value, String> {
    let resp = client
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[("client", "gtx"), ("sl", source), ("tl", "en"), ("dt", "t"), ("dt", "rm"), ("q", text)])
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("romanization service returned {}", resp.status()));
    }
    resp.json::<Value>().await.map_err(|e| format!("bad romanization response: {e}"))
}

fn cache_path(dir: &Path, key: &str, lines: &[String]) -> std::path::PathBuf {
    // The line text is part of the key: picking another lyrics version of the same song must not
    // reuse the readings of the old one.
    dir.join("romanizations")
        .join(format!("{:016x}.json", fnv1a(&format!("{key}|rm|{}", lines.join("\n")))))
}

/// One reading per input line. "" means "nothing to show" (already Latin letters, blank, or not
/// available). Only lines that contain non-Latin letters are sent.
pub async fn romanize_lines(
    client: &reqwest::Client,
    dir: &Path,
    key: &str,
    lines: &[String],
) -> Result<Vec<String>, String> {
    let path = cache_path(dir, key, lines);
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(hit) = serde_json::from_str::<Vec<String>>(&text) {
            if hit.len() == lines.len() {
                return Ok(hit);
            }
        }
    }

    let source = source_hint(lines);
    let mut out = vec![String::new(); lines.len()];
    let work: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| needs_romanization(l))
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

    for batch in batches {
        let text = batch.iter().map(|&i| lines[i].trim()).collect::<Vec<_>>().join("\n");
        let val = request(client, &text, source).await?;
        match split_batch(&parse_translit(&val, ""), batch.len()) {
            Some(reads) => {
                for (slot, r) in batch.iter().zip(reads) {
                    out[*slot] = r;
                }
            }
            None => {
                // The batch did not line up: ask line by line instead.
                for &i in &batch {
                    if let Ok(v) = request(client, lines[i].trim(), source).await {
                        out[i] = clean(&parse_translit(&v, " "));
                    }
                }
            }
        }
    }

    // Do not cache an empty result: it would hide the readings until the cache is deleted.
    if out.iter().any(|s| !s.is_empty()) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, serde_json::to_string(&out).unwrap_or_default());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_scripts_that_need_a_reading() {
        assert!(needs_romanization("月が綺麗ですね"));
        assert!(needs_romanization("사랑해 you"));
        assert!(needs_romanization("Привет"));
        assert!(!needs_romanization("Just plain English, café"));
        assert!(!needs_romanization("♪"));
    }

    #[test]
    fn kana_anywhere_makes_the_song_japanese() {
        let song = vec!["君の名は".to_string(), "ありがとう".to_string()];
        assert_eq!(source_hint(&song), "ja");
        assert_eq!(source_hint(&["你好".to_string()]), "zh-CN");
        assert_eq!(source_hint(&["안녕".to_string()]), "ko");
        assert_eq!(source_hint(&["Привет".to_string()]), "auto");
    }

    #[test]
    fn reads_the_translit_entry_of_a_batch() {
        let v = json!([[["Hello\n", "こんにちは\n", null, null, 3], ["Thanks", "ありがとう", null, null, 3], [null, null, "Konnichiwa\narigatō"]], null, "ja"]);
        let text = parse_translit(&v, "");
        assert_eq!(text, "Konnichiwa\narigatō");
        assert_eq!(split_batch(&text, 2).unwrap(), vec!["Konnichiwa", "arigatō"]);
        assert!(split_batch(&text, 3).is_none());
    }

    #[test]
    fn joins_several_translit_entries_for_one_line() {
        let v = json!([[["Hi. ", "あ。 ", null, null, 3], [null, null, "A."], [null, null, "Ii"]], null, "ja"]);
        assert_eq!(parse_translit(&v, " "), "A. Ii");
        assert_eq!(parse_translit(&json!({"x": 1}), ""), "");
    }
}
